# Technology Stack

**Analysis Date:** 2026-01-21

## Languages

**Primary:**
- Rust (Edition 2024) - All kernel and userspace code - `Cargo.toml`, `kernel/Cargo.toml`

**Secondary:**
- Assembly (x86_64, AArch64, RISC-V) - Architecture-specific bootup - `kernel/src/arch/aarch64/boot.S`

## Runtime

**Environment:**
- Bare-metal x86_64 - Primary target: `x86_64-unknown-none` - `.cargo/config.toml`
- Bare-metal AArch64 - `aarch64-unknown-none` - `.cargo/config.toml`
- Bare-metal RISC-V 64-bit - Configured in `kernel/demos/riscv/`
- Rust Nightly toolchain required - `rust-toolchain.toml`

**Package Manager:**
- Cargo (workspace-based)
- Lockfile: `Cargo.lock` present
- Workspace members defined in `Cargo.toml` lines 16-36

## Frameworks

**Core:**
- None (vanilla bare-metal Rust kernel)

**Bootloader:**
- Limine v9.x - Multi-protocol bootloader (BIOS + UEFI) - cloned during build from `https://github.com/limine-bootloader/limine.git --branch=v9.x-binary`

**Testing:**
- Rust standard `#[test]` with cargo test
- Miri - Undefined behavior detection - `rust-toolchain.toml`
- Clippy - Static analysis - `rust-toolchain.toml`

**Build/Dev:**
- Custom build.rs - ISO/disk image builder - `build.rs`
- QEMU - Emulation for testing - `README.md`
- OVMF - UEFI firmware for x86_64 emulation - `build.rs`

## Key Dependencies

**Critical:**
- `limine` (0.5) - Bootloader protocol - `Cargo.toml`
- `x86_64` (0.15) - x86-64 CPU abstractions - `Cargo.toml`
- `virtio-drivers` (0.12) - VirtIO device drivers - `Cargo.toml`
- `spin` (0.10) - Spinlock synchronization (no_std) - `Cargo.toml`
- `thiserror` (2.0) - Error handling - `Cargo.toml`

**Infrastructure:**
- `linked_list_allocator` (0.10) - Heap allocator - `Cargo.toml`
- `acpi` (5.2) - ACPI table parsing - `Cargo.toml`
- `x2apic` (0.5) - x2APIC controller (x86_64) - `kernel/Cargo.toml`
- `uart_16550` (0.4) - UART driver - `Cargo.toml`
- `elf` (0.7) - ELF binary parsing - `Cargo.toml`
- `fdt` (0.1.5) - Flattened Device Tree (ARM) - `Cargo.toml`

**Architecture Support:**
- `riscv` (0.11) - RISC-V CPU support - `Cargo.toml`
- `aarch64-cpu` (9.4) - ARM64 CPU support - `Cargo.toml`
- `raw-cpuid` (11) - CPUID instruction wrapper - `Cargo.toml`

**Git Dependencies:**
- `mkfs-ext2` - Custom ext2 filesystem builder - `https://github.com/tsatke/mkfs`
- `mkfs-filesystem` - Generic filesystem builder - `https://github.com/tsatke/mkfs`

## Configuration

**Environment:**
- No runtime environment variables required
- Build-time env vars: `CARGO_BIN_FILE_KERNEL_kernel`, `OUT_DIR`, `CARGO_MANIFEST_DIR` - `build.rs`

**Build:**
- `.cargo/config.toml` - Cargo build settings with architecture-specific rustflags
- `rust-toolchain.toml` - Nightly toolchain with components: rustfmt, clippy, llvm-tools-preview, rust-src, miri
- `rustfmt.toml` - Code formatting configuration
- `limine.conf` - Bootloader configuration

**Feature-Gated Configuration:**
- BPF profiles selected at compile-time - `kernel/crates/kernel_bpf/Cargo.toml`
  - `--features cloud-profile` - Servers, VMs, containers
  - `--features embedded-profile` - RPi5, IoT, real-time systems
- Profiles are mutually exclusive via compile-time checks - `kernel/crates/kernel_bpf/src/lib.rs`

## Platform Requirements

**Development:**
- Linux (any distro with toolchain support)
- Required tools: `xorriso`, `qemu-system-x86_64`, `e2fsprogs` (mke2fs)
- Rust nightly toolchain

**Production:**
- Bare-metal x86_64 with UEFI or BIOS
- Bare-metal AArch64 (Raspberry Pi 5 supported)
- Bare-metal RISC-V 64-bit (experimental)
- Boots from ISO (BIOS/UEFI) or disk image

---

*Stack analysis: 2026-01-21*
*Update after major dependency changes*
