/* Tell the linker that we want an AArch64 ELF output file */
OUTPUT_FORMAT(elf64-littleaarch64)
OUTPUT_ARCH(aarch64)

/* We want the symbol _start_asm to be our entry point (assembly) */
ENTRY(_start_asm)

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
    /* Raspberry Pi 5 loads kernel at 0x80000 physical address */
    /* We start without MMU enabled, so use physical addresses */
    /* QEMU virt loads kernel at 0x40080000 */
    . = 0x40080000;

    /* Boot code must come first */
    .text : {
        __text_start = .;
        KEEP(*(.text.boot))
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
    } :data

    /* BSS section */
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        *(.bss.stack)
        __bss_end = .;
    } :data

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}