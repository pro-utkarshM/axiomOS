/* Tell the linker that we want a RISC-V 64-bit ELF output file */
OUTPUT_FORMAT(elf64-littleriscv)
OUTPUT_ARCH(riscv)

/* We want the symbol _start to be our entry point */
ENTRY(_start)

/* Define the program headers we want so the bootloader gives us the right */
/* MMU permissions */
PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    /* Kernel starts at standard RISC-V kernel load address */
    /* OpenSBI loads kernel at 0x80200000 by default */
    . = 0x80200000;

    .text : {
        *(.text.boot)
        *(.text .text.*)
    } :text

    /* Move to the next memory page for .rodata */
    . = ALIGN(4096);

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Move to the next memory page for .data */
    . = ALIGN(4096);

    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    } :data

    /* BSS section */
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(4096);
        *(.bss.stack)
        __bss_end = .;
    } :data

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
