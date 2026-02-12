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

    // SP_EL0 is saved in the exception frame by save_context
    let sp = ctx.sp_el0;

    let mut user_ctx = crate::arch::UserContext {
        inner: *ctx,
        sp,
    };

    let result = crate::syscall::dispatch_syscall(&mut user_ctx, n, arg1, arg2, arg3, arg4, arg5, arg6);

    // Copy back potentially modified context (important for execve)
    *ctx = user_ctx.inner;

    // If sp changed (execve), update sp_el0 in the frame so restore_context restores it
    if user_ctx.sp != sp {
        ctx.sp_el0 = user_ctx.sp;
    }

    // Return result in x0
    ctx.x0 = result as u64;

    // Advance ELR past SVC instruction (4 bytes)
    // The exception handler saves ELR to the stack context, so we modify it there.
    // When the handler returns, it restores ELR from this context.
}
