/// Initialize syscall interface
pub fn init() {
    // ARM uses SVC instruction for syscalls
    // No additional setup needed beyond exception vector
}

/// Handle syscall from user mode
pub fn handle_syscall() {
    // Get registers from exception context
    // In ARM, syscall arguments are in x0-x7
    // Syscall number is in x8

    // For now, this is a placeholder
    log::debug!("Syscall handler called");

    // Advance ELR past SVC instruction (4 bytes)
    unsafe {
        let mut elr: u64;
        core::arch::asm!("mrs {}, elr_el1", out(reg) elr);
        elr += 4;
        core::arch::asm!("msr elr_el1, {}", in(reg) elr);
    }
}

/// Syscall entry point with full context
#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler_with_context(
    x0: usize,
    x1: usize,
    x2: usize,
    x3: usize,
    x4: usize,
    x5: usize,
    _x6: usize,
    _x7: usize,
    x8: usize,
) -> usize {
    // x8 contains syscall number
    // x0-x6 contain arguments
    let result = crate::syscall::dispatch_syscall(x8, x0, x1, x2, x3, x4, x5);

    // Advance past SVC instruction
    unsafe {
        let mut elr: u64;
        core::arch::asm!("mrs {}, elr_el1", out(reg) elr);
        elr += 4;
        core::arch::asm!("msr elr_el1, {}", in(reg) elr);
    }

    result as usize
}
