//! ARM64 Per-CPU Execution Context
//!
//! Provides per-CPU state storage and access for the scheduler and other
//! CPU-local data. Uses TPIDR_EL1 to store a pointer to the CPU context.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

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

// Safety: CpuContext is only accessed from the CPU that owns it
// (via TPIDR_EL1), and we ensure exclusive access via interrupt disabling.
unsafe impl Sync for CpuContext {}
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
        unsafe { &mut *self.scheduler.get() }
    }

    /// Get reference to scheduler
    pub fn scheduler(&self) -> &Scheduler {
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
static CURRENT_CPU_INIT: Once = Once::new();

/// Initialize the current CPU's context
///
/// Must be called once per CPU during boot.
pub fn init_current_cpu(cpu_id: usize) {
    assert!(cpu_id < MAX_CPUS, "CPU ID out of range");

    let ctx = &CPU_CONTEXTS[cpu_id];
    let ctx_ptr = ctx as *const CpuContext as usize;

    // Store context pointer in TPIDR_EL1
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
        let sched = unsafe { ctx.scheduler_mut() };

        if !sched.is_initialized() {
            return;
        }

        // For now, just return to idle - no real task queue yet
        // In the future, this would pull from a global task queue
        if let Some((old_sp_ptr, new_sp, new_ttbr0)) = sched.schedule(None) {
            unsafe {
                context::switch_impl(old_sp_ptr, new_sp, new_ttbr0);
            }
        }
    }
}
