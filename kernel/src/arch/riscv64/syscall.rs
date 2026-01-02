use riscv::register::sepc;

/// Initialize syscall interface
pub fn init() {
    // RISC-V uses ecall instruction for syscalls
    // No additional setup needed beyond trap handler
}

/// Handle syscall from user mode
pub fn handle_syscall() {
    // Get registers from trap frame
    // In RISC-V, syscall arguments are in a0-a7
    // Syscall number is in a7
    // We need to extract these from the trap context

    // For now, this is a placeholder
    // The actual implementation would need to:
    // 1. Extract registers from trap frame
    // 2. Call dispatch_syscall with arguments
    // 3. Store return value in a0
    // 4. Advance sepc past the ecall instruction

    log::debug!("Syscall handler called");

    // Advance past ecall instruction (4 bytes)
    unsafe {
        let pc = sepc::read();
        sepc::write(pc + 4);
    }
}

/// Syscall entry point with full context
#[no_mangle]
pub extern "C" fn syscall_handler_with_context(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
) -> usize {
    // a7 contains syscall number
    // a0-a6 contain arguments
    let result = crate::syscall::dispatch_syscall(a7, a0, a1, a2, a3, a4, a5);

    // Advance past ecall instruction
    unsafe {
        let pc = sepc::read();
        sepc::write(pc + 4);
    }

    result as usize
}
