pub mod boot;
pub mod context;
pub mod cpu;
pub mod dtb;
pub mod exceptions;
pub mod gic;
pub mod interrupts;
pub mod mem;
pub mod mm;
pub mod paging;
pub mod phys;
pub mod platform;
pub mod shutdown;
pub mod syscall;

use aarch64_cpu::asm::barrier;
use aarch64_cpu::registers::*;

use crate::arch::traits::Architecture;

pub struct Aarch64;

impl Architecture for Aarch64 {
    fn early_init() {
        // Setup exception vector table
        exceptions::init_exception_vector();
    }

    fn init() {
        // Initialize memory management (physical allocator + page tables)
        mm::init();

        // Initialize interrupt controller (GIC)
        interrupts::init();

        // Setup syscall interface
        syscall::init();
    }

    fn enable_interrupts() {
        unsafe {
            core::arch::asm!("msr daifclr, #2");
        }
    }

    fn disable_interrupts() {
        unsafe {
            core::arch::asm!("msr daifset, #2");
        }
    }

    fn are_interrupts_enabled() -> bool {
        let daif: u64;
        unsafe {
            core::arch::asm!("mrs {}, daif", out(reg) daif);
        }
        (daif & 0x80) == 0
    }

    fn wait_for_interrupt() {
        unsafe {
            core::arch::asm!("wfi");
        }
    }

    fn shutdown() -> ! {
        shutdown::shutdown()
    }

    fn reboot() -> ! {
        shutdown::reboot()
    }
}
