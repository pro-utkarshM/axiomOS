# Codebase Structure

**Analysis Date:** 2026-01-21

## Directory Layout

```
axiom-ebpf/
├── .cargo/              # Cargo configuration
├── .github/             # GitHub Actions workflows
│   └── workflows/       # CI/CD definitions
├── docs/                # Design documentation
├── kernel/              # Core OS kernel
│   ├── crates/          # Modular kernel subsystems
│   │   ├── kernel_abi/
│   │   ├── kernel_bpf/
│   │   ├── kernel_devfs/
│   │   ├── kernel_device/
│   │   ├── kernel_elfloader/
│   │   ├── kernel_memapi/
│   │   ├── kernel_pci/
│   │   ├── kernel_physical_memory/
│   │   ├── kernel_syscall/
│   │   ├── kernel_vfs/
│   │   └── kernel_virtual_memory/
│   ├── demos/           # Architecture demos (RISC-V)
│   └── src/             # Main kernel source
│       ├── arch/        # Architecture-specific code
│       ├── driver/      # Device drivers
│       ├── file/        # Filesystem layer
│       ├── mcore/       # Multi-core & scheduling
│       ├── mem/         # Memory management
│       └── syscall/     # Syscall handlers
├── scripts/             # Build and deploy scripts
├── src/                 # Build orchestration (QEMU launcher)
├── userspace/           # User-mode programs
│   ├── file_structure/  # Filesystem image builder
│   ├── init/            # Init process (PID 1)
│   └── minilib/         # Userspace syscall library
├── Cargo.toml           # Workspace definition
├── Cargo.lock           # Dependency lock
├── build.rs             # ISO/disk image builder
├── limine.conf          # Bootloader configuration
└── rust-toolchain.toml  # Nightly toolchain
```

## Directory Purposes

**kernel/crates/kernel_abi/**
- Purpose: Kernel ↔ userspace ABI definitions
- Contains: errno, fcntl, mman, limits, syscall numbers
- Key files: `src/errno.rs`, `src/syscall.rs`

**kernel/crates/kernel_bpf/**
- Purpose: eBPF subsystem (core research area)
- Contains: Bytecode, verifier, execution engines, maps, scheduler
- Key files: `src/lib.rs`, `src/bytecode/insn.rs`, `src/verifier/core.rs`, `src/execution/interpreter.rs`
- Subdirectories: `bytecode/`, `verifier/`, `execution/`, `maps/`, `scheduler/`, `profile/`

**kernel/crates/kernel_vfs/**
- Purpose: Virtual filesystem abstraction
- Contains: Path handling, mount management, node abstractions
- Key files: `src/lib.rs`, `src/vfs/mod.rs`, `src/path/mod.rs`

**kernel/crates/kernel_physical_memory/**
- Purpose: Physical memory frame allocator
- Contains: Sparse region-based frame tracking
- Key files: `src/lib.rs`, `src/region.rs`

**kernel/src/arch/**
- Purpose: Architecture-specific implementations
- Contains: x86_64, AArch64, RISC-V code
- Key files: `traits.rs` (interface), `x86_64.rs`, `idt.rs`, `gdt.rs`
- Subdirectories: `aarch64/` (full impl), `riscv64/` (experimental)

**kernel/src/mcore/**
- Purpose: Multi-core support and task management
- Contains: Process structures, scheduler, context switching
- Key files: `mod.rs`, `context.rs`, `lapic.rs`
- Subdirectories: `mtask/` (process/, task/, scheduler/)

**kernel/src/driver/**
- Purpose: Device drivers
- Contains: PCI, VirtIO, block device abstractions
- Key files: `mod.rs`, `pci.rs`, `block.rs`, `raw.rs`
- Subdirectories: `virtio/` (block, gpu, hal)

**kernel/src/file/**
- Purpose: Filesystem layer
- Contains: VFS wrapper, ext2, devfs
- Key files: `mod.rs`, `ext2.rs`, `devfs.rs`

**userspace/init/**
- Purpose: Init process (first userspace program)
- Contains: Minimal init that prints greeting
- Key files: `src/main.rs`

**userspace/minilib/**
- Purpose: Userspace standard library
- Contains: Syscall wrappers
- Key files: `src/lib.rs`

## Key File Locations

**Entry Points:**
- `src/main.rs` - Build orchestration, QEMU launcher
- `kernel/src/main.rs` - Kernel entry point (`kernel_main`)
- `kernel/src/lib.rs` - Kernel initialization (`kernel::init()`)
- `userspace/init/src/main.rs` - Userspace init (`_start`)

**Configuration:**
- `Cargo.toml` - Workspace definition
- `.cargo/config.toml` - Build targets and rustflags
- `rust-toolchain.toml` - Nightly toolchain
- `rustfmt.toml` - Code formatting
- `limine.conf` - Bootloader configuration

**Core Logic:**
- `kernel/src/mcore/mtask/process/mod.rs` - Process management
- `kernel/src/mcore/mtask/scheduler/global.rs` - Task scheduling
- `kernel/src/mem/heap.rs` - Heap allocator
- `kernel/src/syscall/mod.rs` - Syscall dispatch
- `kernel/crates/kernel_bpf/src/execution/interpreter.rs` - BPF interpreter

**Testing:**
- `kernel/crates/kernel_bpf/tests/` - BPF profile and semantic tests
- `.github/workflows/build.yml` - CI workflow
- `.github/workflows/bpf-profiles.yml` - BPF-specific CI

**Documentation:**
- `README.md` - User-facing documentation
- `CONTRIBUTING.md` - Contributor guidelines
- `docs/` - Design proposals and architecture docs

## Naming Conventions

**Files:**
- snake_case.rs - Rust source files
- UPPERCASE.md - Important project files (README, CONTRIBUTING)
- kebab-case.sh - Shell scripts
- *.toml - Configuration files

**Directories:**
- snake_case - Rust module directories
- kernel_ prefix - Kernel crate names
- Plural for collections (crates/, scripts/, docs/)

**Special Patterns:**
- mod.rs - Module container
- lib.rs - Crate root
- main.rs - Binary entry point
- tests/ - Integration tests directory
- linker-*.ld - Architecture-specific linker scripts

## Where to Add New Code

**New Kernel Subsystem Crate:**
- Implementation: `kernel/crates/kernel_<name>/src/`
- Add to workspace: `Cargo.toml` members array
- Add to kernel deps: `kernel/Cargo.toml`

**New Architecture Support:**
- Implementation: `kernel/src/arch/<arch>/`
- Add trait impl in: `kernel/src/arch/traits.rs`
- Linker script: `kernel/src/linker-<arch>.ld`

**New Device Driver:**
- Implementation: `kernel/src/driver/<driver>.rs`
- VirtIO devices: `kernel/src/driver/virtio/<device>.rs`
- Register in: `kernel/src/driver/mod.rs`

**New Syscall:**
- Definition: `kernel/crates/kernel_abi/src/syscall.rs`
- Handler: `kernel/src/syscall/`
- Access impl: `kernel/src/syscall/access/`

**New BPF Feature:**
- Implementation: `kernel/crates/kernel_bpf/src/<module>/`
- Tests: `kernel/crates/kernel_bpf/tests/`
- Profile constraints: `kernel/crates/kernel_bpf/src/profile/`

**New Userspace Program:**
- Implementation: `userspace/<program>/src/main.rs`
- Add Cargo.toml: `userspace/<program>/Cargo.toml`
- Add to workspace: Root `Cargo.toml`

## Special Directories

**kernel/crates/**
- Purpose: Modular kernel subsystems (testable on host)
- Source: Manually created, part of workspace
- Committed: Yes

**target/**
- Purpose: Build artifacts
- Source: Generated by cargo build
- Committed: No (in .gitignore)

**.planning/**
- Purpose: Project planning and codebase documentation
- Source: Generated by GSD tooling
- Committed: Yes

---

*Structure analysis: 2026-01-21*
*Update when directory structure changes*
