use crate::arch::traits::TaskContext;

/// Switch from one task context to another
pub unsafe fn switch_context(from: &mut TaskContext, to: &TaskContext) {
    // Save current context to 'from'
    // Load new context from 'to'

    core::arch::asm!(
        // Save callee-saved registers
        "sd ra, 0(a0)",
        "sd sp, 8(a0)",
        "sd s0, 16(a0)",
        "sd s1, 24(a0)",
        "sd s2, 32(a0)",
        "sd s3, 40(a0)",
        "sd s4, 48(a0)",
        "sd s5, 56(a0)",
        "sd s6, 64(a0)",
        "sd s7, 72(a0)",
        "sd s8, 80(a0)",
        "sd s9, 88(a0)",
        "sd s10, 96(a0)",
        "sd s11, 104(a0)",

        // Load new context from 'to'
        "ld ra, 0(a1)",
        "ld sp, 8(a1)",
        "ld s0, 16(a1)",
        "ld s1, 24(a1)",
        "ld s2, 32(a1)",
        "ld s3, 40(a1)",
        "ld s4, 48(a1)",
        "ld s5, 56(a1)",
        "ld s6, 64(a1)",
        "ld s7, 72(a1)",
        "ld s8, 80(a1)",
        "ld s9, 88(a1)",
        "ld s10, 96(a1)",
        "ld s11, 104(a1)",

        // Switch page table if different
        "ld t0, 112(a1)",  // Load new satp
        "csrr t1, satp",    // Read current satp
        "beq t0, t1, 1f",   // Skip if same
        "csrw satp, t0",    // Write new satp
        "sfence.vma",       // Flush TLB
        "1:",

        in("a0") from,
        in("a1") to,
        options(noreturn)
    );
}

/// Initialize a new task context
pub fn init_task_context(stack_top: usize, entry_point: usize, arg: usize) -> TaskContext {
    let mut context = TaskContext::default();

    context.stack_pointer = stack_top;
    context.instruction_pointer = entry_point;

    // Set up initial register state
    context.arch_state.ra = entry_point;
    // a0 will contain the argument

    context
}
