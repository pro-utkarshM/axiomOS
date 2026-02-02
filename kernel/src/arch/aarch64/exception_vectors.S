// exception_vectors.S - AArch64 Exception Vector Table
//
// ARM64 exception vectors are organized as:
//   - 4 exception types: Synchronous, IRQ, FIQ, SError
//   - 4 execution contexts: current EL SP0, current EL SPx, lower EL 64-bit, lower EL 32-bit
//   - Total: 16 entries, each 128 bytes (0x80)
//   - Table must be 2KB aligned
//
// This provides trampolines to Rust exception handlers.

.section .text
.global exception_vector_base
.balign 2048

// Macro to handle invalid exceptions
.macro invalid_exception kind, source
    .balign 0x80
    // Save caller-saved registers
    stp     x0, x1, [sp, #-16]!
    stp     x2, x3, [sp, #-16]!
    stp     x4, x5, [sp, #-16]!
    stp     x6, x7, [sp, #-16]!
    stp     x8, x9, [sp, #-16]!
    stp     x10, x11, [sp, #-16]!
    stp     x12, x13, [sp, #-16]!
    stp     x14, x15, [sp, #-16]!
    stp     x16, x17, [sp, #-16]!
    stp     x18, x29, [sp, #-16]!
    stp     x30, xzr, [sp, #-16]!

    // Call Rust handler with kind and source
    mov     x0, #\kind
    mov     x1, #\source
    bl      handle_invalid_exception

    // Should never return, but if it does, hang
1:  wfi
    b       1b
.endm

exception_vector_base:

// =============================================================================
// Current EL with SP0 (not typically used in kernel)
// =============================================================================
invalid_exception 0, 0    // Synchronous
invalid_exception 1, 0    // IRQ
invalid_exception 2, 0    // FIQ
invalid_exception 3, 0    // SError

// Macro to save context
// Allocates stack space and saves x0-x30, ELR_EL1, SPSR_EL1
.macro save_context
    sub     sp, sp, #272

    stp     x0, x1, [sp, #0]
    stp     x2, x3, [sp, #16]
    stp     x4, x5, [sp, #32]
    stp     x6, x7, [sp, #48]
    stp     x8, x9, [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    str     x30, [sp, #240]

    mrs     x9, elr_el1
    mrs     x10, spsr_el1
    stp     x9, x10, [sp, #248]
.endm

// Macro to restore context
// Restores x0-x30, ELR_EL1, SPSR_EL1 and frees stack space
.macro restore_context
    ldp     x9, x10, [sp, #248]
    msr     elr_el1, x9
    msr     spsr_el1, x10

    ldr     x30, [sp, #240]
    ldp     x28, x29, [sp, #224]
    ldp     x26, x27, [sp, #208]
    ldp     x24, x25, [sp, #192]
    ldp     x22, x23, [sp, #176]
    ldp     x20, x21, [sp, #160]
    ldp     x18, x19, [sp, #144]
    ldp     x16, x17, [sp, #128]
    ldp     x14, x15, [sp, #112]
    ldp     x12, x13, [sp, #96]
    ldp     x10, x11, [sp, #80]
    ldp     x8, x9, [sp, #64]
    ldp     x6, x7, [sp, #48]
    ldp     x4, x5, [sp, #32]
    ldp     x2, x3, [sp, #16]
    ldp     x0, x1, [sp, #0]

    add     sp, sp, #272
.endm

// =============================================================================
// Current EL with SPx (kernel mode exceptions)
// =============================================================================
.balign 0x80
curr_el_spx_sync:
    save_context
    mov     x0, sp  // Pass context pointer
    bl      handle_sync_exception
    restore_context
    eret

.balign 0x80
curr_el_spx_irq:
    save_context
    mov     x0, sp  // Pass context pointer
    bl      handle_irq
    restore_context
    eret

invalid_exception 2, 1    // FIQ
invalid_exception 3, 1    // SError

// =============================================================================
// Lower EL using AArch64 (user space exceptions)
// =============================================================================
.balign 0x80
lower_el_aarch64_sync:
    save_context
    mov     x0, sp  // Pass context pointer
    bl      handle_sync_exception
    restore_context
    eret

.balign 0x80
lower_el_aarch64_irq:
    save_context
    mov     x0, sp  // Pass context pointer
    bl      handle_irq
    restore_context
    eret

invalid_exception 2, 2    // FIQ
invalid_exception 3, 2    // SError

// =============================================================================
// Lower EL using AArch32 (not used - we don't support 32-bit user space)
// =============================================================================
invalid_exception 0, 3    // Synchronous
invalid_exception 1, 3    // IRQ
invalid_exception 2, 3    // FIQ
invalid_exception 3, 3    // SError
