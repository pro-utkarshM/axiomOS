use super::exceptions::ExceptionContext;

/// Initialize syscall interface
pub fn init() {
    // ARM uses SVC instruction for syscalls
    // No additional setup needed beyond exception vector
}

/// Handle syscall from user mode
pub fn handle_syscall(ctx: &mut ExceptionContext) {
    // In ARM, syscall arguments are in x0-x5
    // Syscall number is in x8
    let n = ctx.x8 as usize;

    let arg1 = ctx.x0 as usize;
    let arg2 = ctx.x1 as usize;
    let arg3 = ctx.x2 as usize;
    let arg4 = ctx.x3 as usize;
    let arg5 = ctx.x4 as usize;
    let arg6 = ctx.x5 as usize;

    let result = crate::syscall::dispatch_syscall(n, arg1, arg2, arg3, arg4, arg5, arg6);

    // Return result in x0
    ctx.x0 = result as u64;

    // Advance ELR past SVC instruction (4 bytes)
    // The exception handler saves ELR to the stack context, so we modify it there.
    // When the handler returns, it restores ELR from this context.
}

// Legacy stub - kept for compatibility if referenced elsewhere
// SAFETY: This is the syscall handler entry point for AArch64.
// It is called from the vector table with valid arguments in registers x0-x8.
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
    // SAFETY: We are modifying ELR_EL1 to skip the SVC instruction.
    // This is necessary to resume execution at the next instruction.
    unsafe {
        let mut elr: u64;
        core::arch::asm!("mrs {}, elr_el1", out(reg) elr);
        elr += 4;
        core::arch::asm!("msr elr_el1, {}", in(reg) elr);
    }

    result as usize
}
