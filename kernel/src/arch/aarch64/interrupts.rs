//! ARM64 Interrupt Handling
//!
//! This module handles interrupt initialization and dispatching for ARM64.
//! It uses the GIC (Generic Interrupt Controller) for interrupt management
//! and the ARM generic timer for scheduling.
//!
//! # RP1 GPIO Interrupt Routing
//!
//! On Raspberry Pi 5, the RP1 southbridge connects via PCIe2. The RP1 has
//! its own internal interrupt controller that aggregates all peripheral
//! interrupts (GPIO, UART, SPI, etc.) and routes them to the main GIC
//! via PCIe MSI or legacy interrupts.
//!
//! According to the BCM2712 device tree:
//! - PCIe2 INTA -> GIC SPI 229 (IRQ 261)
//! - PCIe2 INTB -> GIC SPI 230 (IRQ 262)
//! - PCIe2 INTC -> GIC SPI 231 (IRQ 263)
//! - PCIe2 INTD -> GIC SPI 232 (IRQ 264)
//!
//! The RP1's GPIO Bank 0 generates internal IRQ 0, which routes through
//! the RP1's interrupt controller to one of these PCIe lines.

use super::gic;

/// Physical timer IRQ number (PPI 14 = IRQ 30)
const TIMER_IRQ: u32 = gic::irq::TIMER_PHYS;

/// RP1 GPIO IRQ number
///
/// The RP1 connects via PCIe2, which uses GIC SPI 229-232 for INTA-D.
/// GIC SPI numbers map to IRQ IDs as: SPI N = IRQ (32 + N).
/// So PCIe2 INTA (SPI 229) = IRQ 261.
///
/// Note: The RP1 has its own internal interrupt controller. GPIO Bank 0
/// is RP1 internal IRQ 0. A full implementation would need to also read
/// the RP1's interrupt status registers to determine which peripheral
/// (GPIO, UART, etc.) raised the interrupt.
const RP1_GPIO_IRQ: u32 = 261; // GIC SPI 229 = 32 + 229

/// Initialize interrupt controller and timer
pub fn init() {
    // Initialize the GIC
    gic::init();

    // Enable timer interrupt (PPI 14)
    gic::enable_irq(TIMER_IRQ);
    gic::set_priority(TIMER_IRQ, 0x80);

    // Enable RP1 GPIO interrupt (routed via PCIe2)
    gic::enable_irq(RP1_GPIO_IRQ);
    gic::set_priority(RP1_GPIO_IRQ, 0x80);

    // Initialize and start the timer
    init_timer();

    log::info!(
        "ARM interrupts initialized (timer={}, gpio={})",
        TIMER_IRQ,
        RP1_GPIO_IRQ
    );
}

/// Handle IRQ interrupt (called from exception vector)
#[unsafe(no_mangle)]
pub extern "C" fn handle_irq() {
    // Acknowledge the interrupt and get its ID
    let irq = gic::acknowledge();

    // Check for spurious interrupt
    if irq == gic::irq::SPURIOUS {
        return;
    }

    // Dispatch based on IRQ number
    match irq {
        TIMER_IRQ => handle_timer_interrupt(),
        RP1_GPIO_IRQ => {
            #[cfg(feature = "rpi5")]
            crate::arch::aarch64::platform::rpi5::gpio::handle_interrupt();
            #[cfg(not(feature = "rpi5"))]
            log::warn!("RP1 GPIO IRQ on non-rpi5 build");
        }
        _ => {
            log::warn!("Unhandled IRQ: {}", irq);
        }
    }

    // Signal end of interrupt
    if irq != gic::irq::SPURIOUS {
        gic::end_of_interrupt(irq);
    }
}

/// Handle timer interrupt
fn handle_timer_interrupt() {
    // Clear and reset timer for next interrupt
    clear_timer_interrupt();
    set_next_timer();

    // Run BPF hooks (AttachType::Timer = 1)
    if let Some(manager) = crate::BPF_MANAGER.get() {
        let ctx = kernel_bpf::execution::BpfContext::empty();
        manager.lock().execute_hooks(1, &ctx);
    }

    // Trigger scheduler tick (may cause context switch)
    super::cpu::timer_tick();
}

/// Clear timer interrupt
fn clear_timer_interrupt() {
    unsafe {
        // Disable timer to clear interrupt
        core::arch::asm!("msr cntp_ctl_el0, {}", in(reg) 0u64);
    }
}

/// Set next timer interrupt
fn set_next_timer() {
    unsafe {
        // Read timer frequency
        let cntfrq: u64;
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) cntfrq);

        // Read current counter value
        let cntvct: u64;
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) cntvct);

        // Set timer to fire in 10ms (100 Hz)
        let interval = cntfrq / 100;
        let next = cntvct + interval;

        // Write compare value
        core::arch::asm!("msr cntp_cval_el0, {}", in(reg) next);

        // Enable timer (bit 0 = enable, bit 1 = mask output)
        core::arch::asm!("msr cntp_ctl_el0, {}", in(reg) 1u64);
    }
}

/// Initialize timer
pub fn init_timer() {
    set_next_timer();
    log::debug!("ARM generic timer initialized (100 Hz)");
}

/// End of interrupt (public wrapper)
pub fn end_of_interrupt(irq_id: u32) {
    gic::end_of_interrupt(irq_id);
}
