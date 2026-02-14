# Codebase Structure

**Analysis Date:** 2026-02-13

## Directory Layout

```
axiom-ebpf/
├── .cargo/                 # Cargo configuration (rustflags, unstable features)
├── .github/                # CI/CD workflows and dependabot
├── kernel/                 # Main kernel binary and subsystem crates
│   ├── src/               # Kernel source code
│   │   ├── arch/          # Architecture-specific code (HAL)
│   │   ├── bpf/           # BPF Manager (kernel integration)
│   │   ├── mcore/         # Multi-core, processes, scheduler
│   │   ├── mem/           # Memory management
│   │   ├── syscall/       # Syscall dispatch and handlers
│   │   ├── file/          # VFS, ext2, devfs, pipes
│   │   └── driver/        # Device drivers (VirtIO, PCI, IIO)
│   ├── crates/            # Kernel subsystem crates (11 crates)
│   └── linker-*.ld        # Linker scripts per architecture
├── userspace/              # Userspace programs
│   ├── init/              # Root process
│   ├── minilib/           # Minimal libc (syscall wrappers)
│   ├── bpf_loader/        # BPF program loader
│   └── *_demo/            # Hardware and feature demos
├── scripts/                # Build and deployment scripts
├── docs/                   # Documentation
├── Cargo.toml              # Workspace root
├── build.rs                # ISO/disk image creation
├── limine.conf             # Bootloader configuration
└── rust-toolchain.toml     # Nightly toolchain specification
```

## Directory Purposes

**kernel/src/arch/:**
- Purpose: Architecture-specific code behind HAL trait
- Contains: Boot sequences, exception vectors, paging, interrupt controllers
- Key files: `traits.rs` (Architecture trait), `x86_64.rs`, `idt.rs`, `gdt.rs`
- Subdirectories:
  - `aarch64/` - ARM64 with platform variants (`platform/rpi5/`, `platform/virt/`)
  - `riscv64/` - RISC-V (minimal/experimental)

**kernel/src/bpf/:**
- Purpose: BPF Manager wiring BPF engine into kernel
- Contains: Program loading, attachment, execution hooks, JIT memory
- Key files: `mod.rs` (BpfManager), `helpers.rs` (BPF helper implementations), `jit_memory.rs`

**kernel/src/mcore/:**
- Purpose: Multi-core support, process/task model, scheduling
- Contains: Per-CPU context, task lifecycle, scheduler
- Subdirectories:
  - `mtask/task/` - Task struct, states, stacks, queues
  - `mtask/process/` - Process struct, file descriptors, memory regions, process tree
  - `mtask/scheduler/` - Work-stealing scheduler, context switch, cleanup

**kernel/src/mem/:**
- Purpose: Memory management coordination
- Contains: Physical/virtual allocators, heap, address space management
- Key files: `mod.rs` (init), `address_space.rs`, `heap.rs`, `phys.rs`, `virt.rs`

**kernel/src/syscall/:**
- Purpose: Syscall dispatch and handler implementations
- Key files: `mod.rs` (dispatch_syscall), `bpf.rs` (sys_bpf), `process.rs` (fork/exec/waitpid), `validation.rs`

**kernel/src/file/:**
- Purpose: Filesystem abstractions
- Key files: `mod.rs` (VFS init), `ext2.rs`, `devfs.rs`, `pipe.rs`

**kernel/src/driver/:**
- Purpose: Device driver implementations
- Key files: `block.rs`, `iio.rs` (with BPF hooks), `pci.rs`
- Subdirectories: `virtio/` (block, gpu, mmio, hal)

**kernel/crates/:**
- Purpose: Independently testable kernel subsystem crates
- Contains: 11 crates extracted from kernel for modularity and testing

**userspace/:**
- Purpose: Userspace binaries compiled into disk image
- Key: `minilib/` provides syscall wrappers; `init/` is the root process

## Key File Locations

**Entry Points:**
- `kernel/src/main.rs` - Kernel entry (kernel_main for x86_64 and AArch64)
- `kernel/src/lib.rs` - Initialization coordinator (init())
- `kernel/src/arch/aarch64/boot.rs` - AArch64 _start entry
- `kernel/src/arch/aarch64/boot.S` - AArch64 assembly entry
- `src/main.rs` - QEMU runner (cargo run launches QEMU)

**Configuration:**
- `Cargo.toml` - Workspace root, dependencies, feature flags
- `kernel/Cargo.toml` - Kernel binary config, arch features
- `.cargo/config.toml` - Rustflags, target config
- `rust-toolchain.toml` - Nightly toolchain specification
- `limine.conf` - Bootloader config
- `kernel/linker-x86_64.ld` - x86_64 linker script
- `kernel/linker-aarch64.ld` - AArch64 linker script

**Core Kernel Logic:**
- `kernel/src/bpf/mod.rs` - BPF Manager (load, attach, execute)
- `kernel/src/syscall/mod.rs` - Syscall dispatch
- `kernel/src/mcore/mtask/process/mod.rs` - Process model
- `kernel/src/mcore/mtask/scheduler/mod.rs` - Scheduler
- `kernel/src/mem/mod.rs` - Memory management init
- `kernel/src/file/mod.rs` - VFS initialization

**BPF Engine (Crate):**
- `kernel/crates/kernel_bpf/src/lib.rs` - BPF crate root, profile selection
- `kernel/crates/kernel_bpf/src/verifier/` - Streaming verifier
- `kernel/crates/kernel_bpf/src/execution/interpreter.rs` - BPF interpreter
- `kernel/crates/kernel_bpf/src/execution/jit_aarch64.rs` - ARM64 JIT
- `kernel/crates/kernel_bpf/src/maps/` - Map implementations (array, hash, ringbuf, timeseries)
- `kernel/crates/kernel_bpf/src/loader/` - ELF loader for BPF objects
- `kernel/crates/kernel_bpf/src/profile/` - Cloud/embedded profile system
- `kernel/crates/kernel_bpf/src/signing/` - Ed25519 + SHA3 signing

**Testing:**
- `kernel/crates/kernel_bpf/tests/` - BPF integration tests
- `kernel/crates/kernel_bpf/benches/` - Criterion benchmarks
- `kernel/crates/kernel_vfs/src/vfs/testing.rs` - VFS test helpers

**Build System:**
- `build.rs` - ISO creation, OVMF download, disk image generation
- `kernel/build.rs` - Linker script selection, assembly compilation
- `scripts/build-rpi5.sh` - RPi5 kernel build
- `scripts/deploy-rpi5.sh` - RPi5 SD card deployment

## Naming Conventions

**Files:**
- `snake_case.rs` for all Rust source files
- `mod.rs` for module aggregation directories
- `error.rs` for per-module error types
- `*.S` for assembly files (uppercase extension)

**Directories:**
- `snake_case` for all directories
- Crate names: `kernel_` prefix for kernel subsystem crates
- Platform variants: `platform/{name}/` under arch directories

**Crates:**
- `kernel_abi` - ABI definitions shared with userspace
- `kernel_bpf` - BPF subsystem
- `kernel_vfs` - Virtual file system
- `kernel_syscall` - Syscall implementations
- `kernel_*` - Other kernel subsystems

## Where to Add New Code

**New Syscall:**
- Handler: `kernel/src/syscall/{name}.rs` or add to existing module
- Dispatch: Add case in `kernel/src/syscall/mod.rs` dispatch_syscall()
- ABI: Add syscall number in `kernel/crates/kernel_abi/src/syscall.rs`
- Userspace wrapper: `userspace/minilib/src/lib.rs`

**New Device Driver:**
- Implementation: `kernel/src/driver/{name}.rs`
- If VirtIO: `kernel/src/driver/virtio/{name}.rs`
- If platform-specific: `kernel/src/arch/aarch64/platform/{platform}/{name}.rs`

**New BPF Attach Point:**
- Type definition: Add to `kernel/crates/kernel_bpf/src/attach/`
- Constant: Add ATTACH_TYPE_* in `kernel/src/bpf/mod.rs`
- Hook site: Call `execute_hooks()` at appropriate kernel location

**New BPF Map Type:**
- Implementation: `kernel/crates/kernel_bpf/src/maps/{name}.rs`
- Registration: Add to MapType enum in `kernel/crates/kernel_bpf/src/maps/mod.rs`

**New Architecture:**
- Directory: `kernel/src/arch/{name}/`
- Implement: Architecture trait from `kernel/src/arch/traits.rs`
- Linker script: `kernel/linker-{name}.ld`
- Build config: Feature flag in `kernel/Cargo.toml`

**New Userspace Program:**
- Directory: `userspace/{name}/`
- Cargo.toml: Depend on `minilib` and `kernel_abi`
- Add to workspace members in root `Cargo.toml`
- Add to disk image in `build.rs`

## Special Directories

**kernel/crates/:**
- Purpose: Independently compilable and testable kernel subsystems
- Source: Part of Cargo workspace, linked into kernel binary
- Committed: Yes
- Note: Main kernel binary cannot be unit-tested (bare-metal linker); these crates enable host testing

**target/:**
- Purpose: Build artifacts (ISO, disk images, kernel binaries)
- Source: Generated by cargo build
- Committed: No (.gitignore)

**.planning/:**
- Purpose: Project planning documents
- Committed: Yes (planning artifacts)

---

*Structure analysis: 2026-02-13*
*Update when directory structure changes*
