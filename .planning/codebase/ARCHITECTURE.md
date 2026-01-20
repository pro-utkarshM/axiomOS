# Architecture

**Analysis Date:** 2026-01-21

## Pattern Overview

**Overall:** Modular Monolithic Kernel with Layered Subsystems

**Key Characteristics:**
- Single monolithic kernel binary (not microkernel)
- Modular crate-based organization (11 kernel subsystem crates)
- Architecture-agnostic core with architecture-specific implementations
- eBPF as a first-class kernel abstraction
- Profile-based build-time compilation (Cloud vs Embedded)

## Layers

**Hardware Abstraction Layer (Architecture-Specific):**
- Purpose: Platform-neutral interface for CPU, memory, interrupts
- Contains: Boot code, interrupt handlers, page tables, context switching
- Location: `kernel/src/arch/` with `traits.rs` defining common interface
- Depends on: Architecture-specific crates (x86_64, aarch64-cpu, riscv)
- Used by: Core kernel layer

**Core Kernel Layer:**
- Purpose: Memory management, process/task scheduling, timing
- Contains: Heap allocator, physical/virtual memory, process structures, scheduler
- Location: `kernel/src/mem/`, `kernel/src/mcore/`, `kernel/crates/kernel_*memory/`
- Depends on: Hardware abstraction layer
- Used by: Device management, filesystem, syscall layers

**Device Management & Drivers:**
- Purpose: Hardware device abstraction and drivers
- Contains: VirtIO drivers, PCI enumeration, block devices
- Location: `kernel/src/driver/`, `kernel/crates/kernel_device/`, `kernel/crates/kernel_pci/`
- Depends on: Core kernel (memory), HAL (interrupts)
- Used by: Filesystem layer, applications

**Filesystem Layer:**
- Purpose: File and directory abstractions, VFS
- Contains: VFS layer, ext2 implementation, devfs
- Location: `kernel/src/file/`, `kernel/crates/kernel_vfs/`, `kernel/crates/kernel_devfs/`
- Depends on: Device management (block devices)
- Used by: Syscall layer, applications

**Application/eBPF Execution Layer:**
- Purpose: BPF program execution and verification
- Contains: Bytecode interpreter, JIT compiler, verifier, maps, scheduler
- Location: `kernel/crates/kernel_bpf/`
- Depends on: Core kernel (memory allocation)
- Used by: Kernel subsystems for extensibility

## Data Flow

**Boot Sequence (x86-64):**

1. Limine bootloader loads kernel binary
2. `kernel_main()` in `kernel/src/main.rs` - checks bootloader revision
3. `kernel::init()` in `kernel/src/lib.rs`:
   - `init_boot_time()` - capture boot timestamp
   - `log::init()` - setup serial logging
   - `mem::init()` - heap and memory allocator
   - `acpi::init()` - ACPI table parsing
   - `apic::init()` - interrupt controller
   - `hpet::init()` - hardware timer
   - `mcore::init()` - multi-core and task scheduler
   - `pci::init()` - device enumeration
   - `file::init()` - VFS initialization
4. Mount root filesystem (ext2 on VirtIO block)
5. Load `/bin/init` via ELF loader
6. `Process::create_from_executable()` - spawn init process
7. Enter idle loop: `mcore::turn_idle()`

**Syscall Flow:**

1. User program executes syscall instruction
2. Architecture-specific handler (IDT on x86-64) - `kernel/src/arch/idt.rs`
3. Route to syscall dispatcher - `kernel/src/syscall/mod.rs`
4. Handler accesses: VFS (`kernel/src/file/vfs()`), process state, memory
5. Return to userspace with result

**State Management:**
- Per-process address space via page tables
- Global VFS instance with RwLock
- Global task queue for scheduling
- Process tree for parent-child relationships

## Key Abstractions

**Architecture Trait:**
- Purpose: Platform-neutral interface for architecture-specific operations
- Location: `kernel/src/arch/traits.rs`
- Examples: x86_64, AArch64, RISC-V implementations
- Pattern: Trait-based polymorphism

**Process:**
- Purpose: Core execution unit with address space, file descriptors
- Location: `kernel/src/mcore/mtask/process/mod.rs`
- Contains: PID, PPID, executable path, working directory, fd table, memory regions
- Pattern: Arc<Process> with interior mutability (RwLock)

**Task:**
- Purpose: Scheduling unit (threads within a process)
- Location: `kernel/src/mcore/mtask/task/`
- Contains: Task ID, stack, state (ready/blocked), execution context
- Pattern: Linked into global ready queue

**VfsNode:**
- Purpose: File/directory abstraction
- Location: `kernel/crates/kernel_vfs/src/vfs/node.rs`
- Pattern: Trait object for polymorphic file operations

**BpfProgram:**
- Purpose: Validated BPF program ready for execution
- Location: `kernel/crates/kernel_bpf/src/bytecode/program.rs`
- Pattern: Generic over PhysicalProfile for compile-time constraints

## Entry Points

**Build/Orchestration Entry:**
- Location: `src/main.rs`
- Triggers: `cargo run` or `cargo build`
- Responsibilities: Build kernel, create ISO, run QEMU

**Kernel Entry (x86-64):**
- Location: `kernel/src/main.rs:kernel_main()`
- Triggers: Limine bootloader
- Responsibilities: Initialize subsystems, mount root, spawn init, enter idle

**Kernel Entry (AArch64):**
- Location: `kernel/src/main.rs:kernel_main()` (cfg-gated)
- Triggers: Limine or platform-specific boot
- Responsibilities: Architecture init, scheduler setup, idle task

**Userspace Init:**
- Location: `userspace/init/src/main.rs:_start()`
- Triggers: Kernel loads `/bin/init`
- Responsibilities: First userspace process

## Error Handling

**Strategy:** Throw errors at boundaries, panic on unrecoverable failures

**Patterns:**
- `thiserror` for error types with `#[derive(Error)]`
- `Result<T, E>` propagation with `?` operator
- `expect()/unwrap()` in initialization (panic if boot fails)
- `todo!()` for unimplemented features (explicit crashes)

## Cross-Cutting Concerns

**Logging:**
- `log` crate facade with serial console backend
- Initialization: `kernel/src/log.rs`
- Output: Serial port (COM1 on x86-64)

**Validation:**
- BPF verifier for program safety - `kernel/crates/kernel_bpf/src/verifier/`
- Path validation in VFS - `kernel/crates/kernel_vfs/src/path/`
- Userspace pointer validation in syscalls (minimal)

**Synchronization:**
- `spin` crate for spinlocks and RwLocks (no_std compatible)
- `conquer_once::OnceCell` for one-time initialization
- `Arc<RwLock<T>>` for shared state

---

*Architecture analysis: 2026-01-21*
*Update when major patterns change*
