# axiom-ebpf

**A multi-architecture, bare-metal operating system kernel in Rust, exploring eBPF as a first-class kernel execution model.**

---

## Overview

`axiom-ebpf` is a research-oriented operating system kernel written in Rust, designed to run **directly on hardware** across multiple architectures (x86_64, AArch64, RISC-V).

The project investigates a central question:

> *What does an operating system look like if safe, verified programs (eBPF) are treated as a core kernel abstraction rather than a bolt-on feature?*

Unlike Linux-based eBPF work, `axiom-ebpf` is **not built on top of an existing kernel**. Instead, it explores eBPF integration at the **OS design level**, alongside memory management, filesystems, syscalls, and userspace.

---

## Goals

* Build a **clean, modular, Rust-based kernel**
* Support **multiple CPU architectures** from a single codebase
* Provide a **stable kernel ↔ userspace ABI**
* Integrate **eBPF as a kernel-native execution mechanism**
* Move toward **POSIX.1-2024 compatibility**
* Remain suitable for **research, experimentation, and formal reasoning**

This is **not** a production OS. Correctness, clarity, and extensibility are prioritized over feature completeness.

---

## Key Features

### Kernel Architecture

* Bare-metal kernel (no host OS)
* Boots via the **Limine** bootloader
* Modular subsystem design using Rust crates
* Architecture-specific code cleanly isolated

### Memory Management

* Physical memory allocator
* Virtual memory with paging and address spaces
* Clear separation between physical and virtual layers

### Process & Execution Model

* Kernel-managed processes and threads
* Syscall interface designed for POSIX alignment
* ELF loader for userspace binaries

### Filesystems

* Virtual File System (VFS) abstraction
* `devfs` support
* Extensible filesystem interface

### eBPF Integration (Core Research Area)

* Kernel-resident eBPF subsystem
* Safe, verifier-backed execution model
* Designed to evolve toward:

  * Runtime kernel instrumentation
  * Policy enforcement
  * Observability and safety logic

### Multi-Architecture Support

* x86_64
* AArch64
* RISC-V (actively developed)
* Architecture-neutral core where possible

---

## Repository Structure

```
.
├── Cargo.toml          # Workspace definition
├── build.rs            # Workspace build logic
├── limine.conf         # Bootloader configuration
├── docs/               # Design and architecture documentation
├── kernel/             # The operating system kernel
├── userspace/          # Userspace programs and libraries
├── scripts/            # Build and deployment helpers
└── src/main.rs         # Build orchestration entry
```

---

## Documentation (`docs/`)

Architecture and design decisions are documented explicitly:

```
docs/
├── ARCHITECTURE_SUPPORT.md   # Supported CPUs and constraints
├── MULTI_ARCH_STRATEGY.md    # Cross-architecture design approach
├── PORTING_DESIGN.md         # How to port to new platforms
├── RISCV.md                  # RISC-V–specific notes
├── RPI5_PROPOSAL.md          # Raspberry Pi 5 bring-up plan
└── RPI5_TODO.md              # Pi 5 implementation checklist
```

Reading these is strongly recommended before contributing.

---

## Kernel Layout (`kernel/`)

The kernel is organized as a **collection of internal crates**, each representing a subsystem.

### Kernel Subsystems

```
kernel/crates/
├── kernel_abi              # Kernel ↔ userspace ABI definitions
├── kernel_bpf              # eBPF core subsystem
├── kernel_devfs            # Device filesystem
├── kernel_device           # Device abstraction layer
├── kernel_elfloader        # ELF binary loader
├── kernel_memapi           # Memory management APIs
├── kernel_pci              # PCI enumeration and drivers
├── kernel_physical_memory  # Physical memory allocator
├── kernel_virtual_memory   # Paging and address spaces
├── kernel_syscall          # Syscall dispatch
└── kernel_vfs              # Virtual filesystem layer
```

Each crate is designed to be:

* Logically isolated
* Testable where possible
* Replaceable without destabilizing the kernel

---

### Core Kernel Source

```
kernel/src/
├── arch/              # Architecture-specific implementations
├── driver/            # Hardware drivers
├── mem/               # Memory glue code
├── syscall/           # Syscall handlers
├── file/              # File abstractions
├── mcore/             # Multi-core support
├── acpi.rs
├── apic.rs
├── hpet.rs
├── time.rs
├── backtrace.rs
├── limine.rs
├── log.rs
├── serial.rs
├── sse.rs
└── main.rs            # Kernel entry point
```

Separate entry points exist for different architectures, including minimal RISC-V bring-up paths.

---

## Userspace (`userspace/`)

Userspace is intentionally minimal and tightly controlled.

```
userspace/
├── init/            # Initial userspace process
├── minilib/         # Minimal userspace support library
└── file_structure/  # Filesystem layout definitions
```

Design goals:

* No dependency on glibc
* Explicit ABI boundary
* Gradual POSIX feature adoption

---

## Building & Running

### Prerequisites

* Rust (nightly, configured via `rust-toolchain.toml`)
* `xorriso` (ISO creation)
* `e2fsprogs` (filesystem utilities)
* QEMU (optional, for emulation)

```bash
sudo apt install xorriso e2fsprogs qemu-system
```

---

### Quick Start (QEMU)

```bash
# Build and run
cargo run

# Headless mode
cargo run -- --headless

# With GDB debugging (localhost:1234)
cargo run -- --debug

# Custom resources
cargo run -- --smp 4 --mem 512M
```

---

### Architecture-Specific Builds

```bash
# RISC-V
./scripts/build-riscv.sh
./scripts/run-riscv.sh

# Raspberry Pi 5
./scripts/build-rpi5.sh
./scripts/deploy-rpi5.sh
```

---

## Testing

Subsystems extracted into standalone crates can be tested on the host:

```bash
# cloud-profile <kernel-bpf>
axiom-ebpf main ❯ cargo test --features cloud-profile

# embedded-profile <kernel-bpf>
axiom-ebpf main ✗ cargo test --features embedded-profile
```

The kernel binary itself cannot run standard unit tests due to bare-metal constraints.

---

## Current Status

* Active research and experimentation
* Core kernel subsystems functional
* RISC-V and Raspberry Pi 5 bring-up in progress
* eBPF integration under active development

Expect breaking changes.

---

## Who This Is For

This project is intended for:

* OS / kernel engineers
* Systems researchers
* Embedded and architecture researchers
* Students exploring kernel design beyond Linux
* Anyone interested in **eBPF as an OS primitive**

---

## Contributing

Contributions are welcome, especially in:

* Architecture bring-up
* Memory management
* eBPF verifier and runtime design
* Syscall and ABI design
* Documentation and correctness proofs

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for details.

---

## License

Dual-licensed under:

* **Apache License 2.0**
* **MIT License**
