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
        // SAFETY: daifclr is the interrupt mask clear register. Writing #2
        // clears the IRQ mask bit, enabling IRQ interrupts. This is safe as
        // it only affects interrupt delivery, and we're in kernel mode.
        unsafe {
            core::arch::asm!("msr daifclr, #2");
        }
    }

    fn disable_interrupts() {
        // SAFETY: daifset is the interrupt mask set register. Writing #2
        // sets the IRQ mask bit, disabling IRQ interrupts. This is safe as
        // it only affects interrupt delivery, and we're in kernel mode.
        unsafe {
            core::arch::asm!("msr daifset, #2");
        }
    }

    fn are_interrupts_enabled() -> bool {
        let daif: u64;
        // SAFETY: Reading the DAIF register is always safe. It contains the
        // current interrupt mask state. Bit 7 (I bit) indicates IRQ masking.
        unsafe {
            core::arch::asm!("mrs {}, daif", out(reg) daif);
        }
        (daif & 0x80) == 0
    }

    fn wait_for_interrupt() {
        // SAFETY: wfi (wait for interrupt) halts the CPU until an interrupt
        // occurs. This is safe as long as interrupts are properly configured.
        // The kernel calls this from idle loops when there's no work to do.
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
