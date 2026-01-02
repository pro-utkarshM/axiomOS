use riscv::register::{sie, sip, time};

/// Initialize interrupt controller (PLIC and CLINT)
pub fn init() {
    unsafe {
        // Enable supervisor timer, software, and external interrupts
        sie::set_stimer();
        sie::set_ssoft();
        sie::set_sext();
    }

    // Initialize timer
    init_timer();
}

/// Initialize timer interrupt
fn init_timer() {
    // Set next timer interrupt (10ms from now)
    let timebase = 10_000_000; // 10 MHz timebase
    let interval = timebase / 100; // 10ms

    unsafe {
        let next = time::read() + interval;
        set_timer(next);
    }
}

/// Set timer compare value via SBI
unsafe fn set_timer(time: usize) {
    // SBI call to set timer
    sbi_set_timer(time as u64);
}

/// SBI set timer call
#[inline(always)]
unsafe fn sbi_set_timer(stime_value: u64) {
    sbi_call(0x54494D45, 0, stime_value, 0, 0);
}

/// Generic SBI call
#[inline(always)]
unsafe fn sbi_call(
    extension: usize,
    function: usize,
    arg0: u64,
    arg1: usize,
    arg2: usize,
) -> usize {
    let error: usize;
    core::arch::asm!(
        "ecall",
        in("a0") arg0,
        in("a1") arg1,
        in("a2") arg2,
        in("a6") function,
        in("a7") extension,
        lateout("a0") error,
    );
    error
}

/// Handle software interrupt (IPI)
pub fn handle_soft_interrupt() {
    // Clear software interrupt pending bit
    unsafe {
        sip::clear_ssoft();
    }

    log::debug!("Software interrupt");
}

/// Handle timer interrupt
pub fn handle_timer_interrupt() {
    // Clear timer interrupt by setting next timer
    let timebase = 10_000_000;
    let interval = timebase / 100; // 10ms

    unsafe {
        let next = time::read() + interval;
        set_timer(next);
    }

    // Notify scheduler
    if let Some(ctx) = crate::mcore::context::ExecutionContext::try_load() {
        unsafe {
            ctx.scheduler_mut().reschedule();
        }
    }
}

/// Handle external interrupt (PLIC)
pub fn handle_external_interrupt() {
    // TODO: Read PLIC claim register to get interrupt source
    // TODO: Handle device-specific interrupts
    // TODO: Write to PLIC complete register

    log::debug!("External interrupt");
}

/// End of interrupt
pub fn end_of_interrupt() {
    // RISC-V doesn't require explicit EOI for most interrupts
    // PLIC interrupts are completed by writing to the complete register
}
