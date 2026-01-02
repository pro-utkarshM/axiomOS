pub mod boot;
pub mod context;
pub mod interrupts;
pub mod paging;
pub mod shutdown;
pub mod syscall;
pub mod trap;

use crate::arch::traits::Architecture;

pub struct Riscv64;

impl Architecture for Riscv64 {
    fn early_init() {
        // Setup trap vector early
        trap::init_trap_vector();
    }

    fn init() {
        // Initialize paging
        paging::init();

        // Initialize interrupt controller (PLIC/CLINT)
        interrupts::init();

        // Setup syscall interface
        syscall::init();
    }

    fn enable_interrupts() {
        unsafe {
            riscv::register::sstatus::set_sie();
        }
    }

    fn disable_interrupts() {
        unsafe {
            riscv::register::sstatus::clear_sie();
        }
    }

    fn are_interrupts_enabled() -> bool {
        riscv::register::sstatus::read().sie()
    }

    fn wait_for_interrupt() {
        unsafe {
            riscv::asm::wfi();
        }
    }

    fn shutdown() -> ! {
        shutdown::shutdown()
    }

    fn reboot() -> ! {
        shutdown::reboot()
    }
}
