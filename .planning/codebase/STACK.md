# Technology Stack

**Analysis Date:** 2026-02-13

## Languages

**Primary:**
- Rust (nightly, no_std) - All kernel and userspace code — `Cargo.toml`, `rust-toolchain.toml`
  - Edition 2021 for kernel binary, 2024 for library crates
  - Bare-metal freestanding environment (no standard library)

**Secondary:**
- ARM64 Assembly - Boot and exception vectors — `kernel/src/arch/aarch64/boot.S`, `kernel/src/arch/aarch64/exception_vectors.S`
- RISC-V Assembly - Boot sequence — `kernel/src/arch/riscv64/boot.S`
- Bash - Build and deployment scripts — `scripts/build-rpi5.sh`, `scripts/deploy-rpi5.sh`, `scripts/run-virt.sh`

## Runtime

**Environment:**
- Bare-metal kernel (no_std, no OS runtime)
- Multi-architecture: x86_64, aarch64, riscv64gc
- Panic behavior: abort (no unwinding) — `Cargo.toml` `[profile.dev]` and `[profile.release]`

**Toolchain:**
- Rust nightly — `rust-toolchain.toml`
- Components: rustfmt, clippy, llvm-tools-preview, rust-src, miri
- Target triples: `x86_64-unknown-none`, `aarch64-unknown-none`, `riscv64gc-unknown-none-elf`

**Package Manager:**
- Cargo workspace with 14+ member crates — `Cargo.toml`
- Lockfile: `Cargo.lock` present
- Unstable feature: `bindeps` (artifact dependencies) — `.cargo/config.toml`

## Frameworks

**Core:**
- None (bare-metal kernel, no framework)

**Testing:**
- Standard Rust test framework (host-based, on testable crates)
- Criterion 0.5 - BPF benchmarks — `kernel/crates/kernel_bpf/Cargo.toml`
- Miri - Undefined behavior detection — `.github/workflows/build.yml`

**Build/Dev:**
- Cargo build system with custom `build.rs` scripts
- `cc` crate v1.0 - Assembly compilation — `kernel/build.rs`
- Custom linker scripts per architecture — `kernel/linker-x86_64.ld`, `kernel/linker-aarch64.ld`, `kernel/linker-riscv64.ld`

## Key Dependencies

**Hardware/ISA:**
- `x86_64 = "0.15"` - x86_64 CPU abstractions — `Cargo.toml`
- `aarch64-cpu = "9.4"` - ARM64 CPU abstractions — `Cargo.toml`
- `riscv = "0.11"` - RISC-V CPU abstractions — `Cargo.toml`
- `limine = "0.5"` - Limine bootloader protocol — `Cargo.toml`
- `acpi = "5.2"` - ACPI parsing (x86_64) — `Cargo.toml`
- `x2apic = "0.5"` - x2APIC interrupt controller — `Cargo.toml`
- `fdt = "0.1.5"` - Device Tree Blob parsing (AArch64) — `Cargo.toml`

**Device Drivers:**
- `virtio-drivers = "0.12"` - VirtIO block/network — `Cargo.toml`
- `uart_16550 = "0.4"` - UART serial driver — `Cargo.toml`

**Memory & Allocation:**
- `linked_list_allocator = "0.10"` - Kernel heap — `Cargo.toml`
- `volatile = "0.6"` - Volatile memory access (MMIO) — `Cargo.toml`

**Core Utilities:**
- `spin = "0.10"` - Spinlock synchronization — `Cargo.toml`
- `bitflags = "2.10"` - Bit manipulation — `Cargo.toml`
- `elf = "0.7"` - ELF binary parsing — `Cargo.toml`
- `sha3 = "0.11.0-rc.3"` - Cryptographic hash (BPF signing) — `Cargo.toml`
- `thiserror = "2.0"` - Error handling (no_std) — `Cargo.toml`
- `cordyceps = "0.3"` - Intrusive linked lists — `Cargo.toml`
- `linkme = "0.3"` - Linker section collection — `Cargo.toml`

**Build-Time:**
- `ovmf-prebuilt = "0.2.3"` - UEFI firmware images for QEMU — `Cargo.toml`
- `mkfs-ext2` (git) - ext2 filesystem creation — `Cargo.toml`

## Configuration

**Feature Flags (Cargo):**
- Architecture: `x86_64_arch`, `aarch64_arch`, `riscv64_arch` — `kernel/Cargo.toml`
- Platform: `rpi5`, `virt` — `kernel/Cargo.toml`
- BPF profiles: `cloud-profile`, `embedded-profile` (mutually exclusive) — `kernel/crates/kernel_bpf/Cargo.toml`
- Userspace: `x86_64_deps`, `aarch64_deps` — root `Cargo.toml`

**Build:**
- `.cargo/config.toml` - Target-specific rustflags (relocation-model=static, frame pointers, debug info)
- `kernel/linker-*.ld` - Custom linker scripts per architecture
- `limine.conf` - Bootloader configuration

**Build Profiles:**
- Both dev and release: `panic = "abort"` (kernel requirement)

## Platform Requirements

**Development:**
- Linux (build host)
- Rust nightly toolchain with cross-compilation targets
- QEMU for x86_64 and AArch64 emulation
- xorriso for ISO creation (x86_64)
- mke2fs for ext2 disk image creation

**Hardware Targets:**
- x86_64: QEMU with Limine bootloader (primary dev target)
- AArch64: QEMU virt machine + Raspberry Pi 5 (RPi5)
- RISC-V: QEMU (experimental/demo only)

---

*Stack analysis: 2026-02-13*
*Update after major dependency changes*
