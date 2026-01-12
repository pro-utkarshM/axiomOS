//! ARM64 Context Switching
//!
//! Implements task context switching for ARM64. Uses a stack-based approach
//! where callee-saved registers are saved to the current stack, then SP is
//! switched to the new task's stack and registers are restored.
//!
//! Callee-saved registers on ARM64 (AAPCS64):
//! - x19-x28: General purpose callee-saved
//! - x29 (FP): Frame pointer
//! - x30 (LR): Link register (return address)
//! - SP: Stack pointer
//! - NZCV flags (in PSTATE, but we don't need to save these explicitly)

use core::arch::asm;

/// Saved register frame on the stack during context switch
///
/// This structure is pushed/popped during switch_context.
/// Must match the assembly in switch_impl exactly.
#[repr(C)]
pub struct SwitchFrame {
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64, // Frame pointer
    pub x30: u64, // Link register (return address)
}

impl SwitchFrame {
    /// Size of the switch frame in bytes
    pub const SIZE: usize = core::mem::size_of::<Self>();
}

/// Perform a context switch from one task to another
///
/// # Arguments
/// * `old_sp` - Pointer to where the old stack pointer should be saved
/// * `new_sp` - The new stack pointer to load
/// * `new_ttbr0` - The new TTBR0 value (user page table), or 0 to keep current
///
#[unsafe(naked)]
pub unsafe extern "C" fn switch_impl(
    _old_sp: *mut usize,
    _new_sp: usize,
    _new_ttbr0: usize,
) {
    // x0 = old_sp (pointer to save current SP)
    // x1 = new_sp (new stack pointer value)
    // x2 = new_ttbr0 (new page table, 0 = don't switch)
    core::arch::naked_asm!(
        // Save callee-saved registers to current stack
        // We save 12 registers (96 bytes), must be 16-byte aligned
        "stp x29, x30, [sp, #-16]!",
        "stp x27, x28, [sp, #-16]!",
        "stp x25, x26, [sp, #-16]!",
        "stp x23, x24, [sp, #-16]!",
        "stp x21, x22, [sp, #-16]!",
        "stp x19, x20, [sp, #-16]!",

        // Save current SP to *old_sp
        "mov x9, sp",
        "str x9, [x0]",

        // Load new SP
        "mov sp, x1",

        // Switch page tables if new_ttbr0 != 0
        "cbz x2, 1f",
        "msr ttbr0_el1, x2",
        "isb",
        "tlbi vmalle1is",
        "dsb ish",
        "isb",
        "1:",

        // Restore callee-saved registers from new stack
        "ldp x19, x20, [sp], #16",
        "ldp x21, x22, [sp], #16",
        "ldp x23, x24, [sp], #16",
        "ldp x25, x26, [sp], #16",
        "ldp x27, x28, [sp], #16",
        "ldp x29, x30, [sp], #16",

        // Return to new task (x30/LR has the return address)
        "ret",
    );
}

/// Initialize a stack for a new task
///
/// Sets up the initial stack frame so that when switch_impl switches to this
/// task, it will "return" to the entry point.
///
/// # Arguments
/// * `stack_top` - Top of the allocated stack (highest address)
/// * `entry_point` - Function to execute when task starts
/// * `arg` - Argument to pass to the entry function (in x0)
///
/// # Returns
/// The initial stack pointer value to use for this task
pub fn init_task_stack(stack_top: usize, entry_point: usize, arg: usize) -> usize {
    // Stack must be 16-byte aligned
    let stack_top = stack_top & !0xF;

    // We need space for:
    // 1. SwitchFrame (96 bytes) - what switch_impl expects
    // 2. The entry trampoline sets up x0 with arg

    let frame_ptr = (stack_top - SwitchFrame::SIZE) as *mut SwitchFrame;

    unsafe {
        let frame = &mut *frame_ptr;

        // Zero all callee-saved registers
        frame.x19 = 0;
        frame.x20 = 0;
        frame.x21 = 0;
        frame.x22 = 0;
        frame.x23 = 0;
        frame.x24 = 0;
        frame.x25 = 0;
        frame.x26 = 0;
        frame.x27 = 0;
        frame.x28 = 0;
        frame.x29 = 0; // Frame pointer

        // x30 (LR) = entry point - this is where switch_impl will "return" to
        frame.x30 = entry_point as u64;

        // Store arg in x19 - the trampoline will move it to x0
        frame.x19 = arg as u64;
    }

    frame_ptr as usize
}

/// Task entry trampoline
///
/// This is the first code executed by a new task after context switch.
/// It moves the argument from x19 to x0 and jumps to the actual entry point.
///
/// Note: The actual entry point is in x30 (LR) when we get here, but we
/// already returned to it. So we need a different approach - we'll use
/// a wrapper that's set as the entry point.
#[unsafe(naked)]
pub unsafe extern "C" fn task_entry_trampoline() {
    core::arch::naked_asm!(
        // x19 contains the argument
        // x20 contains the actual entry point
        "mov x0, x19",
        "br x20",
    );
}

/// Initialize a stack for a new task with trampoline
///
/// Like init_task_stack but uses a trampoline to properly pass the argument.
pub fn init_task_stack_with_arg(
    stack_top: usize,
    entry_point: usize,
    arg: usize,
) -> usize {
    let stack_top = stack_top & !0xF;
    let frame_ptr = (stack_top - SwitchFrame::SIZE) as *mut SwitchFrame;

    unsafe {
        let frame = &mut *frame_ptr;

        // x19 = argument (will be moved to x0 by trampoline)
        frame.x19 = arg as u64;
        // x20 = actual entry point (trampoline will branch to this)
        frame.x20 = entry_point as u64;

        // Zero other registers
        frame.x21 = 0;
        frame.x22 = 0;
        frame.x23 = 0;
        frame.x24 = 0;
        frame.x25 = 0;
        frame.x26 = 0;
        frame.x27 = 0;
        frame.x28 = 0;
        frame.x29 = 0;

        // x30 (LR) = trampoline - switch_impl returns here
        frame.x30 = task_entry_trampoline as usize as u64;
    }

    frame_ptr as usize
}

/// Get the current stack pointer
#[inline]
pub fn current_sp() -> usize {
    let sp: usize;
    unsafe {
        asm!("mov {}, sp", out(reg) sp, options(nomem, nostack));
    }
    sp
}

/// Get the current frame pointer
#[inline]
pub fn current_fp() -> usize {
    let fp: usize;
    unsafe {
        asm!("mov {}, x29", out(reg) fp, options(nomem, nostack));
    }
    fp
}

/// Get the current link register (return address)
#[inline]
pub fn current_lr() -> usize {
    let lr: usize;
    unsafe {
        asm!("mov {}, x30", out(reg) lr, options(nomem, nostack));
    }
    lr
}
