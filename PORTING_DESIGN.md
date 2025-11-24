# Muffin OS Architecture Porting Design

## Overview

This document outlines the design and implementation strategy for porting Muffin OS from x86-64 to RISC-V and ARM architectures.

## Current Architecture Dependencies

### x86-64 Specific Components

1. **Bootloader**: Limine (x86-64 only)
2. **Interrupt Handling**: IDT (Interrupt Descriptor Table)
3. **Segmentation**: GDT (Global Descriptor Table), TSS (Task State Segment)
4. **Interrupt Controller**: APIC/LAPIC
5. **Hardware Discovery**: ACPI
6. **Context Switching**: x86-64 specific assembly and registers
7. **Syscall Interface**: `int 0x80` instruction
8. **Memory Management**: x86-64 page tables (4-level paging)

## Architecture Abstraction Layer Design

### Module Structure

```
kernel/src/arch/
├── mod.rs                    # Architecture trait definitions and exports
├── x86_64/
│   ├── mod.rs
│   ├── boot.rs              # Limine bootloader integration
│   ├── gdt.rs               # GDT and TSS setup
│   ├── idt.rs               # IDT and interrupt handlers
│   ├── interrupts.rs        # Interrupt controller (APIC/LAPIC)
│   ├── context.rs           # Context switching
│   ├── syscall.rs           # Syscall entry
│   ├── paging.rs            # Page table management
│   └── shutdown.rs          # Platform shutdown
├── riscv64/
│   ├── mod.rs
│   ├── boot.rs              # SBI or OpenSBI bootloader integration
│   ├── trap.rs              # Trap handling (interrupts + exceptions)
│   ├── interrupts.rs        # PLIC/CLINT interrupt controller
│   ├── context.rs           # Context switching
│   ├── syscall.rs           # Syscall entry (ecall)
│   ├── paging.rs            # Sv39/Sv48 page table management
│   └── shutdown.rs          # SBI shutdown
└── aarch64/
    ├── mod.rs
    ├── boot.rs              # Device tree or UEFI boot
    ├── exceptions.rs        # Exception vector table
    ├── interrupts.rs        # GIC (Generic Interrupt Controller)
    ├── context.rs           # Context switching
    ├── syscall.rs           # Syscall entry (svc)
    ├── paging.rs            # ARM page table management
    └── shutdown.rs          # PSCI shutdown
```

### Core Architecture Traits

```rust
pub trait Architecture {
    // Initialization
    fn early_init();
    fn init();
    
    // Interrupt management
    fn enable_interrupts();
    fn disable_interrupts();
    fn are_interrupts_enabled() -> bool;
    fn wait_for_interrupt();
    
    // Context switching
    fn switch_context(from: &mut TaskContext, to: &TaskContext);
    
    // Syscall handling
    fn setup_syscall_interface();
    
    // Memory management
    fn setup_paging();
    fn flush_tlb();
    fn flush_tlb_page(addr: VirtAddr);
    
    // Platform control
    fn shutdown() -> !;
    fn reboot() -> !;
}

pub struct TaskContext {
    // Architecture-specific register state
    registers: ArchRegisters,
    stack_pointer: usize,
    instruction_pointer: usize,
}
```

## RISC-V Implementation Plan

### Target Specifications
- **ISA**: RV64GC (64-bit, General + Compressed)
- **Privilege Levels**: M-mode (machine), S-mode (supervisor), U-mode (user)
- **Paging**: Sv39 (39-bit virtual addressing)
- **Bootloader**: OpenSBI (SBI v1.0)
- **Interrupt Controller**: PLIC (Platform-Level Interrupt Controller) + CLINT (Core-Local Interruptor)

### Key Components

#### 1. Boot Process
- Use OpenSBI as firmware/bootloader
- Kernel runs in S-mode
- Device tree for hardware discovery
- Setup trap vector (`stvec` register)

#### 2. Trap Handling
- Unified trap handler for interrupts and exceptions
- CSR registers: `sstatus`, `sepc`, `scause`, `stval`
- Timer interrupts via CLINT
- External interrupts via PLIC

#### 3. Context Switching
- Save/restore general-purpose registers (x0-x31)
- Save/restore floating-point registers (f0-f31)
- Switch `satp` register for address space
- `sfence.vma` for TLB flush

#### 4. Syscall Interface
- Use `ecall` instruction
- Trap to S-mode handler
- Arguments in registers a0-a7
- Return value in a0

#### 5. Memory Management
- Sv39: 3-level page tables
- Page size: 4 KiB, 2 MiB (megapages), 1 GiB (gigapages)
- PTE format: 64-bit entries
- `satp` register for page table base

### Dependencies
- `riscv` crate for CSR access and low-level operations
- OpenSBI for firmware services
- QEMU `virt` machine for testing

## ARM (AArch64) Implementation Plan

### Target Specifications
- **Architecture**: ARMv8-A (AArch64)
- **Exception Levels**: EL0 (user), EL1 (kernel), EL2 (hypervisor), EL3 (secure monitor)
- **Paging**: 4KB granule, 48-bit VA
- **Bootloader**: UEFI or device tree
- **Interrupt Controller**: GICv2/GICv3 (Generic Interrupt Controller)

### Key Components

#### 1. Boot Process
- Boot via UEFI or device tree
- Kernel runs in EL1
- Setup exception vector table
- Initialize GIC

#### 2. Exception Handling
- 4 exception levels (EL0-EL3)
- Exception vector table with 16 entries
- System registers: `ELR_EL1`, `ESR_EL1`, `FAR_EL1`, `SPSR_EL1`
- Synchronous and asynchronous exceptions

#### 3. Context Switching
- Save/restore general-purpose registers (x0-x30)
- Save/restore NEON/FP registers (v0-v31)
- Switch `TTBR0_EL1` and `TTBR1_EL1` for address space
- `tlbi` instruction for TLB invalidation

#### 4. Syscall Interface
- Use `svc` (supervisor call) instruction
- Arguments in registers x0-x7
- Return value in x0
- Syscall number in x8

#### 5. Memory Management
- 4-level page tables (48-bit VA)
- Page sizes: 4 KiB, 2 MiB, 1 GiB
- Separate page tables for kernel (TTBR1_EL1) and user (TTBR0_EL1)
- Translation table base registers

### Dependencies
- `aarch64` or `cortex-a` crate for low-level operations
- ARM Trusted Firmware for secure boot (optional)
- QEMU `virt` machine for testing

## Build System Changes

### Cargo Configuration

1. **Target Specifications**
   - `riscv64gc-unknown-none-elf` for RISC-V
   - `aarch64-unknown-none` for ARM

2. **Linker Scripts**
   - `kernel/linker-x86_64.ld` (existing)
   - `kernel/linker-riscv64.ld` (new)
   - `kernel/linker-aarch64.ld` (new)

3. **Feature Flags**
   ```toml
   [features]
   default = ["x86_64"]
   x86_64 = []
   riscv64 = []
   aarch64 = []
   ```

4. **Conditional Compilation**
   ```rust
   #[cfg(target_arch = "x86_64")]
   mod x86_64;
   
   #[cfg(target_arch = "riscv64")]
   mod riscv64;
   
   #[cfg(target_arch = "aarch64")]
   mod aarch64;
   ```

### Build Script Updates

- Detect target architecture
- Select appropriate bootloader
- Use correct linker script
- Build architecture-specific ISO/image

## Testing Strategy

1. **QEMU Emulation**
   - x86_64: `qemu-system-x86_64`
   - RISC-V: `qemu-system-riscv64 -machine virt`
   - ARM: `qemu-system-aarch64 -machine virt`

2. **Unit Tests**
   - Architecture-agnostic code in separate crates
   - Mock architecture traits for testing

3. **Integration Tests**
   - Boot test for each architecture
   - Basic syscall tests
   - Context switching tests

## Implementation Phases

### Phase 1: Architecture Abstraction Layer
- Define core traits
- Refactor x86_64 code to implement traits
- Update kernel initialization to use traits

### Phase 2: RISC-V Support
- Implement RISC-V architecture module
- Create RISC-V linker script
- Integrate OpenSBI bootloader
- Test on QEMU

### Phase 3: ARM Support
- Implement ARM architecture module
- Create ARM linker script
- Integrate bootloader (UEFI/device tree)
- Test on QEMU

### Phase 4: Build System Integration
- Update Cargo.toml for multi-arch
- Update build.rs for architecture detection
- Create architecture-specific build commands

## Challenges and Considerations

1. **Bootloader Differences**
   - Limine (x86-64) vs OpenSBI (RISC-V) vs UEFI (ARM)
   - Different boot protocols and memory layouts

2. **Interrupt Handling**
   - IDT (x86-64) vs trap vectors (RISC-V) vs exception vectors (ARM)
   - Different interrupt controller interfaces

3. **Memory Management**
   - Different page table formats
   - Different TLB management instructions

4. **Device Drivers**
   - VirtIO drivers should be mostly portable
   - Platform-specific device discovery (ACPI vs device tree)

5. **Inline Assembly**
   - Architecture-specific assembly needs rewriting
   - Use architecture-specific intrinsics where possible

## Future Enhancements

1. **Additional Architectures**
   - 32-bit variants (riscv32, armv7)
   - Other architectures (PowerPC, MIPS)

2. **Performance Optimizations**
   - Architecture-specific optimizations
   - SIMD/vector instructions

3. **Hardware Support**
   - Real hardware testing
   - Board-specific configurations

## References

- [RISC-V Privileged Specification](https://riscv.org/technical/specifications/)
- [ARM Architecture Reference Manual](https://developer.arm.com/documentation/)
- [OpenSBI Documentation](https://github.com/riscv-software-src/opensbi)
- [Limine Boot Protocol](https://github.com/limine-bootloader/limine)