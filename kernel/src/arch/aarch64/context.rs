use crate::arch::traits::TaskContext;

/// Switch from one task context to another
pub unsafe fn switch_context(from: &mut TaskContext, to: &TaskContext) {
    // Save current context to 'from'
    // Load new context from 'to'

    core::arch::asm!(
        // Save callee-saved registers
        "stp x19, x20, [x0, #0]",
        "stp x21, x22, [x0, #16]",
        "stp x23, x24, [x0, #32]",
        "stp x25, x26, [x0, #48]",
        "stp x27, x28, [x0, #64]",
        "stp x29, x30, [x0, #80]",
        "mov x9, sp",
        "str x9, [x0, #96]",

        // Load new context from 'to'
        "ldp x19, x20, [x1, #0]",
        "ldp x21, x22, [x1, #16]",
        "ldp x23, x24, [x1, #32]",
        "ldp x25, x26, [x1, #48]",
        "ldp x27, x28, [x1, #64]",
        "ldp x29, x30, [x1, #80]",
        "ldr x9, [x1, #96]",
        "mov sp, x9",

        // Switch page tables if different
        "ldr x9, [x1, #104]",  // Load new TTBR0
        "mrs x10, ttbr0_el1",
        "cmp x9, x10",
        "b.eq 1f",
        "msr ttbr0_el1, x9",
        "dsb ishst",
        "tlbi vmalle1is",
        "dsb ish",
        "isb",
        "1:",

        in("x0") from,
        in("x1") to,
        options(noreturn)
    );
}

/// Initialize a new task context
pub fn init_task_context(stack_top: usize, entry_point: usize, arg: usize) -> TaskContext {
    let mut context = TaskContext::default();

    context.stack_pointer = stack_top;
    context.instruction_pointer = entry_point;

    // Set up initial register state
    context.arch_state.x30 = entry_point as u64; // Link register
    // x0 will contain the argument

    context
}
