# Architecture Support

axiom-ebpf now supports multiple architectures through an architecture abstraction layer.

## Supported Architectures

### x86-64 (Primary)
- **Status**: Fully supported
- **Bootloader**: Limine
- **Interrupt Controller**: APIC/LAPIC
- **Hardware Discovery**: ACPI
- **Paging**: 4-level page tables
- **Testing**: QEMU `qemu-system-x86_64`

### RISC-V 64-bit (RV64GC)
- **Status**: Initial implementation
- **Bootloader**: OpenSBI
- **Privilege Modes**: M-mode, S-mode, U-mode
- **Interrupt Controller**: PLIC + CLINT
- **Hardware Discovery**: Device Tree
- **Paging**: Sv39 (3-level page tables)
- **Testing**: QEMU `qemu-system-riscv64 -machine virt`

### ARM 64-bit (AArch64)
- **Status**: Initial implementation
- **Bootloader**: UEFI / Device Tree
- **Exception Levels**: EL0, EL1
- **Interrupt Controller**: GICv2/GICv3
- **Hardware Discovery**: Device Tree / ACPI
- **Paging**: 4KB granule, 48-bit VA, 4-level page tables
- **Testing**: QEMU `qemu-system-aarch64 -machine virt`

## Building for Different Architectures

### x86-64 (default)
```bash
cargo build --target x86_64-unknown-none
```

### RISC-V
```bash
rustup target add riscv64gc-unknown-none-elf
cargo build --target riscv64gc-unknown-none-elf
```

### ARM
```bash
rustup target add aarch64-unknown-none
cargo build --target aarch64-unknown-none
```

## Architecture Abstraction Layer

The architecture abstraction layer is defined in `kernel/src/arch/traits.rs` and provides:

- **Architecture trait**: Common interface for all architectures
- **TaskContext**: Architecture-specific task state
- **Interrupt management**: Enable/disable/wait for interrupts
- **Memory management**: Page table operations
- **System control**: Shutdown and reboot

Each architecture implements these traits in its respective module:
- `kernel/src/arch/x86_64/`
- `kernel/src/arch/riscv64/`
- `kernel/src/arch/aarch64/`

## Key Components by Architecture

### Interrupt Handling
- **x86-64**: IDT (Interrupt Descriptor Table) with 256 entries
- **RISC-V**: Trap vector with unified interrupt/exception handling
- **ARM**: Exception vector table with 16 entries (4 levels × 4 types)

### Context Switching
- **x86-64**: Save/restore callee-saved registers (rbx, rbp, r12-r15)
- **RISC-V**: Save/restore callee-saved registers (s0-s11, ra)
- **ARM**: Save/restore callee-saved registers (x19-x30)

### Syscall Interface
- **x86-64**: `int 0x80` instruction
- **RISC-V**: `ecall` instruction
- **ARM**: `svc` instruction

### Memory Management
- **x86-64**: 4-level paging (PML4, PDPT, PD, PT)
- **RISC-V**: Sv39 (3-level paging)
- **ARM**: 4-level paging with TTBR0/TTBR1 split

## Testing

### QEMU Testing

#### x86-64
```bash
qemu-system-x86_64 \
    -cdrom muffin.iso \
    -drive file=disk.img,format=raw \
    -serial stdio \
    -m 512M \
    -smp 4
```

#### RISC-V
```bash
qemu-system-riscv64 \
    -machine virt \
    -bios default \
    -kernel kernel \
    -drive file=disk.img,format=raw,if=virtio \
    -serial stdio \
    -m 512M \
    -smp 4
```

#### ARM
```bash
qemu-system-aarch64 \
    -machine virt \
    -cpu cortex-a57 \
    -bios QEMU_EFI.fd \
    -kernel kernel \
    -drive file=disk.img,format=raw,if=virtio \
    -serial stdio \
    -m 512M \
    -smp 4
```

## Implementation Status

### Completed
- [x] Architecture abstraction layer design
- [x] RISC-V trap handling
- [x] RISC-V interrupt controller (PLIC/CLINT)
- [x] RISC-V syscall interface
- [x] RISC-V paging (Sv39)
- [x] RISC-V context switching
- [x] ARM exception handling
- [x] ARM interrupt controller (GIC)
- [x] ARM syscall interface
- [x] ARM paging
- [x] ARM context switching
- [x] Linker scripts for RISC-V and ARM

### In Progress
- [ ] RISC-V bootloader integration
- [ ] ARM bootloader integration
- [ ] Device tree parsing
- [ ] Architecture-specific build system

### TODO
- [ ] RISC-V device drivers
- [ ] ARM device drivers
- [ ] Multi-core support for RISC-V
- [ ] Multi-core support for ARM
- [ ] Real hardware testing
- [ ] Performance optimization

## Contributing

When adding support for a new architecture:

1. Create a new directory under `kernel/src/arch/`
2. Implement the `Architecture` trait
3. Implement architecture-specific modules:
   - Boot initialization
   - Interrupt/exception handling
   - Context switching
   - Syscall interface
   - Memory management
   - System control (shutdown/reboot)
4. Create a linker script
5. Update the build system
6. Add documentation
7. Test on QEMU and real hardware

## References

- [RISC-V Privileged Specification](https://riscv.org/technical/specifications/)
- [ARM Architecture Reference Manual](https://developer.arm.com/documentation/)
- [OpenSBI Documentation](https://github.com/riscv-software-src/opensbi)
- [Limine Boot Protocol](https://github.com/limine-bootloader/limine)
- [QEMU Documentation](https://www.qemu.org/docs/master/)
