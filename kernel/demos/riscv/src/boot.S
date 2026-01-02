# RISC-V Boot Assembly Entry Point
# This is the first code that runs when OpenSBI jumps to the kernel
# Entry state:
#   a0 (x10) = hart_id
#   a1 (x11) = dtb physical address
#   Mode: S-mode (supervisor)
#   Paging: disabled
#   Interrupts: disabled

.section .text.boot
.global _start

_start:
    # Save boot arguments (a0=hart_id, a1=dtb_addr)
    # We'll pass these to Rust code
    mv      s0, a0          # Save hart_id in s0
    mv      s1, a1          # Save dtb_addr in s1
    
    # Only hart 0 continues, others wait
    bnez    a0, .Lwait_for_interrupt
    
    # Clear BSS section
    la      t0, __bss_start
    la      t1, __bss_end
.Lclear_bss:
    bgeu    t0, t1, .Lbss_done
    sd      zero, 0(t0)
    addi    t0, t0, 8
    j       .Lclear_bss
.Lbss_done:
    
    # Setup stack pointer
    # Stack grows downward, so set sp to top of stack
    la      sp, __stack_top
    
    # Restore boot arguments for Rust function call
    mv      a0, s0          # hart_id
    mv      a1, s1          # dtb_addr
    
    # Jump to Rust entry point
    call    _start_rust
    
    # Should never return, but just in case
    j       .Lhalt

.Lwait_for_interrupt:
    # Secondary harts (non-zero hart_id) wait here
    wfi
    j       .Lwait_for_interrupt

.Lhalt:
    wfi
    j       .Lhalt

# Define stack section
.section .bss.stack
.align 12  # 4KB alignment
__stack_bottom:
    .space 0x10000  # 64KB stack
__stack_top:
