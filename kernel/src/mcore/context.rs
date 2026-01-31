use alloc::sync::Arc;
#[cfg(target_arch = "x86_64")]
use spin::Mutex;
use core::cell::UnsafeCell;

#[cfg(target_arch = "x86_64")]
use x86_64::registers::model_specific::KernelGsBase;
#[cfg(target_arch = "x86_64")]
use x86_64::structures::gdt::GlobalDescriptorTable;
#[cfg(target_arch = "x86_64")]
use x86_64::structures::idt::InterruptDescriptorTable;

#[cfg(target_arch = "x86_64")]
use crate::arch::gdt::Selectors;
#[cfg(target_arch = "x86_64")]
use crate::mcore::lapic::Lapic;
use crate::mcore::mtask::process::{Process, ProcessId};
use crate::mcore::mtask::scheduler::Scheduler;
use crate::mcore::mtask::task::Task;

#[derive(Debug)]
pub struct ExecutionContext {
    cpu_id: usize,
    #[cfg(target_arch = "x86_64")]
    lapic_id: usize,

    #[cfg(target_arch = "x86_64")]
    lapic: Mutex<Lapic>,

    #[cfg(target_arch = "x86_64")]
    _gdt: &'static GlobalDescriptorTable,
    #[cfg(target_arch = "x86_64")]
    sel: Selectors,
    #[cfg(target_arch = "x86_64")]
    _idt: &'static InterruptDescriptorTable,

    scheduler: UnsafeCell<Scheduler>,
}

impl ExecutionContext {
    #[cfg(target_arch = "x86_64")]
    pub fn new(
        cpu: &limine::mp::Cpu,
        gdt: &'static GlobalDescriptorTable,
        sel: Selectors,
        idt: &'static InterruptDescriptorTable,
        lapic: Lapic,
    ) -> Self {
        ExecutionContext {
            cpu_id: cpu.id as usize,
            lapic_id: cpu.lapic_id as usize,
            lapic: Mutex::new(lapic),
            _gdt: gdt,
            sel,
            _idt: idt,
            scheduler: UnsafeCell::new(Scheduler::new_cpu_local()),
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn new(cpu_id: usize) -> Self {
        ExecutionContext {
            cpu_id,
            scheduler: UnsafeCell::new(Scheduler::new_cpu_local()),
        }
    }

    #[must_use]
    pub fn try_load() -> Option<&'static Self> {
        #[cfg(target_arch = "x86_64")]
        {
            let ctx = KernelGsBase::read();
            if ctx.is_null() {
                None
            } else {
                // SAFETY: We checked that the pointer is not null.
                // The KernelGsBase register contains a pointer to the thread-local ExecutionContext.
                Some(unsafe { &*ctx.as_ptr() })
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
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
                // static ExecutionContext.
                Some(unsafe { &*(ctx_ptr as *const Self) })
            }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        None
    }

    /// # Panics
    /// This function panics if the execution context could not be loaded.
    /// This could happen if no execution context exists yet, or the pointer
    /// or its memory in `KernelGSBase` is invalid.
    #[must_use]
    pub fn load() -> &'static Self {
        Self::try_load().expect("could not load cpu context")
    }

    #[must_use]
    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    #[cfg(target_arch = "x86_64")]
    pub fn lapic_id(&self) -> usize {
        self.lapic_id
    }

    #[cfg(target_arch = "x86_64")]
    #[must_use]
    pub fn lapic(&self) -> &Mutex<Lapic> {
        &self.lapic
    }

    #[cfg(target_arch = "x86_64")]
    pub fn selectors(&self) -> &Selectors {
        &self.sel
    }

    /// Creates and returns a mutable reference to the scheduler.
    ///
    /// # Safety
    /// The caller must ensure that only one mutable reference
    /// to the scheduler exists at any time.
    #[allow(clippy::mut_from_ref)]
    // SAFETY: The caller must ensure exclusivity.
    pub unsafe fn scheduler_mut(&self) -> &mut Scheduler {
        // SAFETY: The UnsafeCell access is guarded by the caller's guarantee of exclusivity.
        unsafe { &mut *self.scheduler.get() }
    }

    pub fn scheduler(&self) -> &Scheduler {
        // SAFETY: We are accessing the scheduler immutably.
        // This is safe because everything in the context is cpu-local and we are not
        // concurrently modifying it from this thread unless via scheduler_mut which requires unsafe.
        unsafe {
            // SAFETY: this is safe because either:
            // * there is a mutable reference that is used for rescheduling, in which case we are
            //   not currently executing this
            // * there is no mutable reference, in which case we are safe because we're not modifying
            // * someone else has a mutable reference, in which case he violates the safety contract
            //   if this is executed
            //
            // The above is true because everything in the context is cpu-local.
            &*self.scheduler.get()
        }
    }

    pub fn pid(&self) -> ProcessId {
        self.scheduler().current_task().process().pid()
    }

    pub fn current_task(&self) -> &Task {
        self.scheduler().current_task()
    }

    pub fn current_process(&self) -> &Arc<Process> {
        self.current_task().process()
    }
}
