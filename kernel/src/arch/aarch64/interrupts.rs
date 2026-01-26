//! ARM64 Interrupt Handling
//!
//! This module handles interrupt initialization and dispatching for ARM64.
//! It uses the GIC (Generic Interrupt Controller) for interrupt management
//! and the ARM generic timer for scheduling.

use super::gic;

/// Physical timer IRQ number
const TIMER_IRQ: u32 = gic::irq::TIMER_PHYS;

/// Initialize interrupt controller and timer
pub fn init() {
    // Initialize the GIC
    gic::init();

    // Enable timer interrupt
    gic::enable_irq(TIMER_IRQ);

    // Set timer priority (high priority)
    gic::set_priority(TIMER_IRQ, 0x80);

    // Initialize and start the timer
    init_timer();

    log::info!("ARM interrupts initialized");
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
