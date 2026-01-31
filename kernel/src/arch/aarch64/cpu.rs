//! ARM64 Per-CPU Execution Context
//!
//! Provides per-CPU state storage and access for the scheduler and other
//! CPU-local data. Uses TPIDR_EL1 to store a pointer to the CPU context.

use core::cell::UnsafeCell;

use spin::Once;

use super::context;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 4;

/// Per-CPU execution context
pub struct CpuContext {
    /// CPU ID (0-3 for RPi5)
    cpu_id: usize,

    /// Scheduler for this CPU
    scheduler: UnsafeCell<Scheduler>,
}

/// Simple scheduler state for a single CPU
pub struct Scheduler {
    /// Current task's stack pointer
    current_sp: usize,

    /// Idle task stack pointer (what we return to when no tasks)
    idle_sp: usize,

    /// Whether scheduler is initialized
    initialized: bool,

    /// Dummy location for old SP when switching away from terminated task
    dummy_sp: usize,
}

impl Scheduler {
    /// Create a new uninitialized scheduler
    const fn new() -> Self {
        Self {
            current_sp: 0,
            idle_sp: 0,
            initialized: false,
            dummy_sp: 0,
        }
    }

    /// Initialize the scheduler with the idle task
    pub fn init(&mut self, idle_sp: usize) {
        self.idle_sp = idle_sp;
        self.current_sp = idle_sp;
        self.initialized = true;
    }

    /// Check if scheduler is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get current task's stack pointer
    pub fn current_sp(&self) -> usize {
        self.current_sp
    }

    /// Schedule next task
    ///
    /// Returns (old_sp_ptr, new_sp, new_ttbr0)
    /// If no task switch needed, returns None
    pub fn schedule(&mut self, next_sp: Option<usize>) -> Option<(*mut usize, usize, usize)> {
        let next_sp = next_sp.unwrap_or(self.idle_sp);

        if next_sp == self.current_sp {
            return None;
        }

        let old_sp_ptr = &mut self.current_sp as *mut usize;
        self.current_sp = next_sp;

        // For now, don't switch page tables (kernel only)
        Some((old_sp_ptr, next_sp, 0))
    }

    /// Get a pointer to dummy SP location (for discarding old task)
    pub fn dummy_sp_ptr(&mut self) -> *mut usize {
        &mut self.dummy_sp as *mut usize
    }
}

// SAFETY: CpuContext is `Sync` because it is only accessed by the CPU that owns it
// (via TPIDR_EL1), or with external synchronization/interrupt disabling when accessed
// from other contexts. The internal `UnsafeCell` is guarded by these mechanisms.
unsafe impl Sync for CpuContext {}

// SAFETY: CpuContext is `Send` because it contains plain data (usize) and the
// `UnsafeCell` contents (Scheduler) are `Send`. Ownership transfer is valid,
// although instances are typically static per-CPU.
unsafe impl Send for CpuContext {}

impl CpuContext {
    /// Create a new CPU context
    const fn new(cpu_id: usize) -> Self {
        Self {
            cpu_id,
            scheduler: UnsafeCell::new(Scheduler::new()),
        }
    }

    /// Get the CPU ID
    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    /// Get mutable reference to scheduler
    ///
    /// # Safety
    /// Caller must ensure exclusive access (typically by disabling interrupts)
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn scheduler_mut(&self) -> &mut Scheduler {
        // SAFETY: The caller guarantees exclusive access to the scheduler.
        unsafe { &mut *self.scheduler.get() }
    }

    /// Get reference to scheduler
    pub fn scheduler(&self) -> &Scheduler {
        // SAFETY: Reading the scheduler is safe as long as no one is mutating it concurrently
        // without synchronization. In this UP (uni-processor) or per-CPU design, we should
        // be careful, but generally reading basic fields is okay.
        unsafe { &*self.scheduler.get() }
    }
}

// Per-CPU context storage (statically allocated for simplicity)
static CPU_CONTEXTS: [CpuContext; MAX_CPUS] = [
    CpuContext::new(0),
    CpuContext::new(1),
    CpuContext::new(2),
    CpuContext::new(3),
];

/// Current CPU context initialization flag
#[allow(dead_code)]
static CURRENT_CPU_INIT: Once = Once::new();

/// Initialize the current CPU's context
///
/// Must be called once per CPU during boot.
pub fn init_current_cpu(cpu_id: usize) {
    assert!(cpu_id < MAX_CPUS, "CPU ID out of range");

    let ctx = &CPU_CONTEXTS[cpu_id];
    let ctx_ptr = ctx as *const CpuContext as usize;

    // Store context pointer in TPIDR_EL1
    // SAFETY: Writing to TPIDR_EL1 is safe in EL1. We are storing the pointer to
    // the persistent static per-CPU context structure which remains valid for the
    // entire lifetime of the kernel.
    unsafe {
        core::arch::asm!(
            "msr tpidr_el1, {}",
            in(reg) ctx_ptr,
            options(nostack, preserves_flags)
        );
    }

    log::info!("CPU {} context initialized at {:#x}", cpu_id, ctx_ptr);
}

/// Get the current CPU's context
///
/// Returns None if not yet initialized.
pub fn try_current() -> Option<&'static CpuContext> {
    let ctx_ptr: usize;
    // SAFETY: Reading TPIDR_EL1 is safe.
    unsafe {
        core::arch::asm!(
            "mrs {}, tpidr_el1",
            out(reg) ctx_ptr,
            options(nostack, preserves_flags)
        );
    }

    if ctx_ptr == 0 {
        None
    } else {
        // SAFETY: If TPIDR_EL1 is non-zero, it must contain a valid pointer to a
        // static CpuContext set in init_current_cpu.
        Some(unsafe { &*(ctx_ptr as *const CpuContext) })
    }
}

/// Get the current CPU's context
///
/// # Panics
/// Panics if CPU context is not initialized.
pub fn current() -> &'static CpuContext {
    try_current().expect("CPU context not initialized")
}

/// Get the current CPU ID
pub fn cpu_id() -> usize {
    // Read MPIDR_EL1 to get CPU affinity
    let mpidr: usize;
    // SAFETY: Reading MPIDR_EL1 is safe.
    unsafe {
        core::arch::asm!(
            "mrs {}, mpidr_el1",
            out(reg) mpidr,
            options(nostack, preserves_flags)
        );
    }

    // Aff0 contains the CPU ID for Cortex-A76 (RPi5)
    mpidr & 0xFF
}

/// Reschedule on timer interrupt
///
/// Called from the timer interrupt handler.
pub fn timer_tick() {
    if let Some(ctx) = try_current() {
        // SAFETY: We are in an interrupt handler (timer tick), so we have exclusive access
        // to this CPU's scheduler state (assuming nested interrupts are handled correctly or disabled).
        let sched = unsafe { ctx.scheduler_mut() };

        if !sched.is_initialized() {
            return;
        }

        // For now, just return to idle - no real task queue yet
        // In the future, this would pull from a global task queue
        if let Some((old_sp_ptr, new_sp, new_ttbr0)) = sched.schedule(None) {
            // SAFETY: Context switch is unsafe as it modifies control flow and stack.
            // We trust the scheduler to provide valid stack pointers.
            unsafe {
                context::switch_impl(old_sp_ptr, new_sp, new_ttbr0);
            }
        }
    }
}

/// Synchronize I-cache and D-cache for JIT compilation
///
/// This function performs the necessary barriers and cache maintenance
/// to ensure that instructions written to memory are visible to the
/// instruction fetch unit.
#[no_mangle]
pub unsafe extern "C" fn aarch64_jit_sync_cache(start: usize, len: usize) {
    // Get cache line sizes
    let mut ctr_el0: u64;
    core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr_el0);

    // D-cache line size (in 4-byte words, log2)
    // Field DminLine is bits [19:16]
    // Cache line size in bytes = 4 << DminLine
    let dcache_line_shift = (ctr_el0 >> 16) & 0xF;
    let dcache_line_size = 4usize << dcache_line_shift;

    // I-cache line size (in 4-byte words, log2)
    // Field IminLine is bits [3:0]
    // Cache line size in bytes = 4 << IminLine
    let icache_line_shift = ctr_el0 & 0xF;
    let icache_line_size = 4usize << icache_line_shift;

    let end = start + len;

    // 1. Clean D-cache to PoU
    let mut addr = start & !(dcache_line_size - 1);
    while addr < end {
        core::arch::asm!("dc cvau, {}", in(reg) addr);
        addr += dcache_line_size;
    }

    // 2. DSB
    core::arch::asm!("dsb ish");

    // 3. Invalidate I-cache to PoU
    addr = start & !(icache_line_size - 1);
    while addr < end {
        core::arch::asm!("ic ivau, {}", in(reg) addr);
        addr += icache_line_size;
    }

    // 4. DSB
    core::arch::asm!("dsb ish");

    // 5. ISB
    core::arch::asm!("isb");
}
