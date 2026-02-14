# Architecture

**Analysis Date:** 2026-02-13

## Pattern Overview

**Overall:** Layered monolithic kernel with modular workspace crates and eBPF extensibility

**Key Characteristics:**
- No_std Rust kernel with bare-metal boot on x86_64, AArch64, RISC-V
- Multi-core capable with per-CPU context and work-stealing scheduler
- Trait-based hardware abstraction layer (HAL)
- eBPF subsystem with compile-time cloud/embedded profile selection
- Process model with fork/exec/waitpid and file descriptors

## Layers

```
┌─────────────────────────────────────────────────────────────┐
│ USERSPACE                                                    │
│ - init, demos, bpf_loader                                   │
│ - minilib syscall wrappers                                  │
└──────────────────────────────────────────────────────────────┘
                    ↑ Syscalls (int 0x80 / SVC)
┌─────────────────────────────────────────────────────────────┐
│ PROCESS/TASK LAYER                                           │
│ - Process & Task management    kernel/src/mcore/mtask/      │
│ - Work-stealing scheduler      kernel/src/mcore/mtask/scheduler/ │
│ - File descriptors & memory    kernel/src/mcore/mtask/process/   │
└──────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│ SUBSYSTEM LAYER                                              │
│ - BPF Runtime                  kernel/src/bpf/              │
│ - BPF Engine (crate)           kernel/crates/kernel_bpf/    │
│ - Syscall Dispatch             kernel/src/syscall/           │
│ - Device Drivers               kernel/src/driver/            │
│ - Virtual File System          kernel/src/file/              │
└──────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│ MEMORY & INTERRUPT LAYER                                     │
│ - Physical/Virtual Allocators  kernel/src/mem/               │
│ - Page Tables & Address Space  kernel/src/mem/address_space/ │
│ - Exception/Interrupt Handlers kernel/src/arch/*/            │
└──────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│ HARDWARE ABSTRACTION LAYER                                   │
│ - Architecture trait           kernel/src/arch/traits.rs     │
│ - x86_64 implementation       kernel/src/arch/x86_64.rs     │
│ - AArch64 implementation      kernel/src/arch/aarch64/      │
│ - RISC-V implementation       kernel/src/arch/riscv64/      │
└──────────────────────────────────────────────────────────────┘
```

**Userspace Layer:**
- Purpose: User applications communicating with kernel via syscalls
- Contains: init process, demo programs, minilib (libc replacement)
- Depends on: kernel_abi for syscall numbers and ABI definitions

**Process/Task Layer:**
- Purpose: Process lifecycle, scheduling, context switching
- Contains: Process struct (pid, address space, fd table), Task struct (execution context)
- Depends on: Memory layer for address spaces, HAL for context switch

**Subsystem Layer:**
- Purpose: Kernel services and extensibility
- Contains: BPF manager, syscall dispatch, VFS (ext2, devfs, pipes), device drivers
- Depends on: Process layer for current task, Memory layer for allocations

**Memory & Interrupt Layer:**
- Purpose: Physical/virtual memory management, exception handling
- Contains: Frame allocator, page table management, interrupt dispatch
- Depends on: HAL for architecture-specific paging and interrupt setup

**HAL Layer:**
- Purpose: Abstract architecture differences behind common trait
- Contains: `Architecture` trait with early_init, init, interrupt control, shutdown
- Depends on: CPU/firmware directly

## Data Flow

**Boot Sequence (x86_64):**
1. Limine bootloader loads kernel ELF at high memory
2. `kernel_main` entry — `kernel/src/main.rs:53`
3. `kernel::init()` coordination — `kernel/src/lib.rs:66-131`
   - Logging, HHDM, heap, page tables
   - ACPI, APIC, HPET initialization
   - BPF Manager initialization
   - VFS init (devfs at /dev)
   - Multi-core/scheduler init
   - PCI enumeration, VirtIO driver init
   - Root filesystem mount (ext2)
4. Spawn init process from `/bin/init`
5. Enter scheduler loop

**Syscall Flow:**
1. User process executes syscall instruction (int 0x80 / SVC)
2. Exception handler saves context — `kernel/src/arch/idt.rs` or `kernel/src/arch/aarch64/exceptions.rs`
3. `dispatch_syscall()` routes by syscall number — `kernel/src/syscall/mod.rs`
4. BPF hooks execute if attached (ATTACH_TYPE_SYSCALL) — `kernel/src/bpf/mod.rs:119-140`
5. Syscall handler executes (sys_read, sys_write, sys_bpf, etc.)
6. Result returned to userspace as isize (negative = errno)

**BPF Program Lifecycle:**
1. Userspace calls `sys_bpf(BPF_PROG_LOAD, elf_bytes)` — `kernel/src/syscall/bpf.rs`
2. BPF Manager loads ELF, verifies bytecode — `kernel/src/bpf/mod.rs`
3. Program stored with ID in manager
4. `sys_bpf(BPF_PROG_ATTACH, attach_type, prog_id)` hooks program to event
5. On event (timer tick, GPIO edge, syscall), `execute_hooks()` runs all attached programs
6. Execution via interpreter (x86_64) or JIT (AArch64)

**State Management:**
- Per-process: Address space, file descriptors, memory regions
- Global: BPF Manager (Mutex-protected singleton), VFS, process tree
- Per-CPU: Scheduler, execution context

## Key Abstractions

**Architecture Trait (HAL):**
- Purpose: Abstract hardware differences
- Location: `kernel/src/arch/traits.rs`
- Methods: early_init, init, enable/disable_interrupts, wait_for_interrupt, shutdown, reboot
- Implementations: Aarch64, X86_64, Riscv64

**Process:**
- Purpose: Resource container (address space, file descriptors, children)
- Location: `kernel/src/mcore/mtask/process/mod.rs`
- Pattern: Arc-wrapped with RwLock for concurrent access
- Key: Process::root() as initial process, Process::create_from_executable() for new programs

**Task:**
- Purpose: Execution unit (kernel/user stacks, registers, state)
- Location: `kernel/src/mcore/mtask/task/mod.rs`
- States: Ready, Running, Blocked, Zombie
- Pattern: Intrusive linked list for scheduler queues

**BPF Profile (Sealed Trait):**
- Purpose: Compile-time selection between cloud and embedded behavior
- Location: `kernel/crates/kernel_bpf/src/profile/mod.rs`
- Pattern: Sealed trait with const generics (MAX_STACK_SIZE, MAX_INSN_COUNT, JIT_ALLOWED)
- Associated types: MemoryStrategy, SchedulerPolicy, FailureSemantic

**BPF Manager:**
- Purpose: Runtime BPF program management
- Location: `kernel/src/bpf/mod.rs`
- Operations: load_program, attach, detach, execute, execute_hooks
- Attach types: Timer(1), GPIO(2), PWM(3), IIO(4), Syscall(5)

## Entry Points

**Kernel Main (x86_64):**
- Location: `kernel/src/main.rs:53`
- Triggers: Limine bootloader
- Responsibilities: Call kernel::init(), spawn init, enter scheduler

**Kernel Main (AArch64):**
- Location: `kernel/src/main.rs:89`
- Triggers: _start in boot.S → boot.rs
- Responsibilities: Same as x86_64 after arch-specific init

**Initialization Coordinator:**
- Location: `kernel/src/lib.rs:66` (`init()`)
- Triggers: kernel_main after arch early_init
- Responsibilities: Initialize all subsystems in order

**Syscall Dispatch:**
- Location: `kernel/src/syscall/mod.rs`
- Triggers: int 0x80 (x86_64) or SVC (AArch64)
- Responsibilities: Route syscall number to handler, return result

## Error Handling

**Strategy:** Panic on unrecoverable errors, Result for recoverable operations

**Kernel Panics:**
- Boot failures (missing init, no block device)
- Double faults, unhandled exceptions
- Memory exhaustion during critical allocation

**Syscall Errors:**
- Negative return values = errno (EINVAL, ENOENT, ENOMEM, etc.)
- Defined in `kernel/crates/kernel_abi/src/errno.rs`

**BPF Errors:**
- Enum-based: LoadError, VerifyError, AttachError — `kernel/crates/kernel_bpf/src/*/error.rs`
- Custom Display implementations with context

## Cross-Cutting Concerns

**Logging:**
- `log` crate facade with serial backend — `kernel/src/log.rs`, `kernel/src/serial.rs`
- Kernel-level logging via `log::info!`, `log::error!`, etc.

**Synchronization:**
- Spinlocks (`spin` crate) for kernel-level mutual exclusion
- RwLock for read-heavy data (VFS, process tree)
- Mutex for BPF Manager, allocators
- Interrupts disabled during critical sections

**Memory Safety:**
- Rust ownership model for kernel code
- BPF verifier for loaded programs (bounded loops, memory bounds)
- Address space isolation between processes

---

*Architecture analysis: 2026-02-13*
*Update when major patterns change*
