# Axiom Task Tracking

## Implementation Status Overview

Axiom is a **complete operating system kernel** with BPF as a first-class primitive. The kernel boots on real hardware (x86_64, AArch64/RPi5, RISC-V). The BPF subsystem is fully implemented as a library. BPF integration is **in progress** with basic syscall support and manager structure in place.

| Layer | Status | Description |
|-------|--------|-------------|
| Kernel Core | ✅ Complete | Boot, memory, processes, VFS, syscalls |
| BPF Subsystem | ✅ Complete | Verifier, interpreter, JIT, maps, signing |
| BPF Integration | ⚠️ In Progress | Manager + syscall exist, needs hardening |
| Hardware Attach | ⚠️ In Progress | GPIO, PWM, IIO (Sim) integrated |
| Example Programs | ⚠️ Partial | BPF maps demo exists |

---

## Current Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        IMPLEMENTED                           │
│                                                              │
│  Kernel Core              BPF Subsystem (library)           │
│  ────────────             ───────────────────────           │
│  ✅ Limine boot           ✅ Streaming verifier             │
│  ✅ Physical memory       ✅ Interpreter                    │
│  ✅ Virtual memory        ✅ x86_64 JIT                     │
│  ✅ Process/tasks         ✅ ARM64 JIT                      │
│  ✅ Scheduler             ✅ Maps (array, hash, ring, ts)   │
│  ✅ VFS + Ext2            ✅ ELF loader                     │
│  ✅ DevFS                 ✅ Ed25519 signing                │
│  ✅ Syscalls (8/41)       ✅ Attach abstractions            │
│  ✅ ELF loader                                              │
│                                                              │
│  Architectures            Userspace                         │
│  ─────────────            ─────────                         │
│  ✅ x86_64 (full)         ✅ init (minimal)                 │
│  ✅ AArch64 (full)        ✅ minilib (syscalls)             │
│  ✅ RPi5 platform         ✅ rk-cli (tooling)               │
│  ⚠️ RISC-V (boot)         ✅ rk-bridge (events)             │
└──────────────────────────────────────────────────────────────┘
                              │
                              │ GAP: Integration
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                    INTEGRATION STATUS                        │
│                                                              │
│  ✅ bpf() syscall exists (kernel/src/syscall/bpf.rs)        │
│  ✅ BpfManager exists (kernel/src/bpf/mod.rs)               │
│  ✅ BPF maps demo working (kernel/demos/)                   │
│  ⚠️ Hardcoded map sizes (4-byte key, 8-byte value only)    │
│  ⚠️ Unsafe pointer casts without validation                 │
│  ❌ Attach points not hooked to hardware                    │
│  ❌ Timer/GPIO/syscall tracing hooks not implemented        │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Kernel Core ✅ COMPLETE

### Boot & Architecture
- [x] Limine bootloader integration
- [x] x86_64 boot sequence (GDT, IDT, ACPI, APIC, HPET)
- [x] AArch64 boot sequence (exception vectors, GIC, DTB)
- [x] AArch64 RPi5 platform support
- [x] RISC-V boot sequence (basic)

### Memory Management
- [x] Physical frame allocator (sparse regions, multi-size pages)
- [x] Virtual memory manager (address space tracking)
- [x] Kernel heap (linked list allocator)
- [x] Memory API traits (MemoryApi, Allocation)

### Process & Scheduling
- [x] Process abstraction (ProcessId, ProcessTree)
- [x] Task management (TaskId, state machine)
- [x] Context switching (CR3, register save/restore)
- [x] Round-robin scheduler
- [x] Per-CPU state

### Filesystem
- [x] VFS abstraction layer
- [x] Ext2 filesystem (read)
- [x] DevFS (/dev)
- [x] Path resolution

### Syscalls
- [x] Syscall dispatch (x86_64)
- [x] SYS_EXIT
- [x] SYS_READ
- [x] SYS_WRITE
- [x] SYS_OPEN
- [x] SYS_GETCWD
- [x] SYS_MMAP
- [x] SYS_FCNTL
- [x] SYS_STAT/FSTAT

### Drivers
- [x] Serial console (UART)
- [x] VirtIO block device
- [x] PCI enumeration

---

## Phase 2: BPF Subsystem ✅ COMPLETE (as library)

### Bytecode
- [x] Instruction encoding/decoding (`kernel_bpf/src/bytecode/`)
- [x] Opcode classes (ALU64, ALU32, JMP, LDX, STX, etc.)
- [x] Register file (R0-R10)
- [x] Program representation with profile constraints

### Verifier
- [x] Streaming verifier (`kernel_bpf/src/verifier/streaming.rs`)
- [x] O(registers × basic_block_depth) memory complexity
- [x] CFG analysis and reachability
- [x] Register type tracking (11 types)
- [x] Helper function validation
- [x] Profile-aware constraints

### Execution
- [x] Interpreter (`kernel_bpf/src/execution/interpreter.rs`)
  - All ALU operations
  - All jump conditions
  - Memory load/store
  - Helper dispatch
- [x] x86_64 JIT (`kernel_bpf/src/execution/jit/`)
  - Full instruction encoding
  - Register allocation
  - Prologue/epilogue
- [x] ARM64 JIT (`kernel_bpf/src/execution/jit_aarch64.rs`)
  - Structure complete
  - Register mapping
  - ✅ Instruction emission complete

### Maps
- [x] Array map - O(1) lookup
- [x] Hash map - linear probing
- [x] Ring buffer - lock-free SPMC
- [x] Time-series map - circular buffer
- [x] Static pool - embedded profile allocator

### Loader
- [x] ELF64 parser (no libbpf)
- [x] Section extraction
- [x] Relocation handling
- [x] Map definition parsing

### Signing
- [x] SHA3-256 hashing
- [x] Ed25519 signatures
- [x] Signed program format
- [x] TrustedKey management

### Attach Abstractions
- [x] AttachPoint trait
- [x] Kprobe abstraction
- [x] Tracepoint abstraction
- [x] GPIO abstraction
- [x] PWM abstraction
- [x] IIO abstraction
- ⚠️ All are framework only - not connected to kernel

### Scheduler
- [x] ThroughputPolicy (cloud)
- [x] DeadlinePolicy (embedded, EDF)
- [x] Program queue management

---

## Phase 3: BPF Integration ⚠️ IN PROGRESS

### BPF Manager (kernel component)
- [x] `BpfManager` struct in kernel (`kernel/src/bpf/mod.rs`)
  - Holds loaded programs
  - Manages program lifecycle
  - Tracks attached programs
- [x] Integration with kernel initialization

### bpf() Syscall
- [x] Add SYS_BPF to kernel_abi
- [x] Syscall handler (`kernel/src/syscall/bpf.rs`)
- [x] Commands implemented:
  - [x] BPF_PROG_LOAD
  - [x] BPF_MAP_CREATE
  - [x] BPF_PROG_ATTACH
  - [x] BPF_PROG_DETACH
  - [x] BPF_MAP_LOOKUP
  - [x] BPF_MAP_UPDATE

### ⚠️ Known Issues (from codebase analysis)
- [x] **Hardcoded BPF Map Sizes** - All maps assume 4-byte keys, 8-byte values
  - File: `kernel/src/syscall/bpf.rs` (lines 67, 101-103)
  - Fix: Extract key/value sizes from BpfAttr structure
- [x] **Unsafe Pointer Casts** - User pointers cast without validation
  - File: `kernel/src/syscall/bpf.rs` (lines 22, 54, 88, 123, 150, 177)
  - Fix: Validated using `UserspacePtr` and `copy_from_userspace` in `validation.rs`
- [x] **Missing Safety Comments** - Unsafe blocks lack SAFETY documentation
  - Fix: Added safety comments to critical low-level code (boot, mm, drivers)

### Attach Point Implementation
- [x] Attach point abstractions exist (`kernel_bpf/src/attach/`)
  - [x] Kprobe abstraction
  - [x] Tracepoint abstraction
  - [x] GPIO abstraction
  - [x] PWM abstraction
  - [x] IIO abstraction
- [ ] **Wire to actual kernel events** (NOT DONE)
  - [ ] Timer interrupt hook (HPET/ARM timer)
  - [x] Syscall entry hooks (Global)
  - [ ] Function tracing instrumentation

### Helper Implementation
- [x] bpf_gpio_read/write/toggle/set_output
- [x] bpf_pwm_write
- [ ] bpf_ktime_get_ns() - read kernel time
- [x] bpf_map_lookup_elem() - map lookup
- [x] bpf_map_update_elem() - map update
- [x] bpf_map_delete_elem() - map delete
- [x] bpf_ringbuf_output() - event output
- [x] bpf_trace_printk() - debug output to serial

### Userspace Integration
- [ ] Update minilib with bpf() syscall wrapper
- [ ] Simple BPF loader program
- [ ] Test loading program from userspace

---

## Technical Debt & Concerns

### Security Issues (Priority: High)
- [x] **Syscall Pointer Validation** - User pointers passed to unsafe blocks
  - Validated: Address space verification, alignment validation, bounds checking implemented in `validation.rs`
  - Files: `kernel/src/syscall/bpf.rs` uses secure validation wrappers

### Code Quality (Priority: Medium)
- [x] **Missing SAFETY Comments** - Addressed in critical paths
- [x] **Build & Lint Cleanliness** - `cargo fmt`, `clippy`, and `cargo test` passing for workspace
- [ ] **Edition 2024 in Cargo.toml** - Doesn't exist, should be "2021"
  - Files: `Cargo.toml`, `kernel/Cargo.toml`

### Performance (Priority: Low)
- [ ] **Linear Physical Memory Search** - O(n) per allocation
  - File: `kernel/crates/kernel_physical_memory/src/lib.rs` (lines 73-80)
  - Fix: Add buddy allocator or bitmap-based tracking
- [ ] **BTreeMap for VFS Paths** - O(log n) on every file operation
  - File: `kernel/crates/kernel_vfs/src/vfs/mod.rs` (line 23)
  - Fix: Trie-based mount point tracking

### Missing Features (Priority: Medium)
- [ ] **BTF Parsing** - Binary Type Format not implemented
  - File: `kernel/crates/kernel_bpf/src/loader/mod.rs` (line 152)
  - Blocks: Rich debugging, CO-RE (Compile Once Run Everywhere)
- [ ] **VFS Node Reuse** - Repeated file opens create new VfsNodes
  - File: `kernel/crates/kernel_vfs/src/vfs/mod.rs` (line 89)
- [ ] **Mount Point Validation** - Can mount at non-directory paths
  - File: `kernel/crates/kernel_vfs/src/vfs/mod.rs` (line 57)

### Platform Gaps
- [ ] **RISC-V Incomplete**
  - `kernel/src/main_riscv.rs` - Only prints TODO messages
  - `kernel/src/arch/riscv64/interrupts.rs` - PLIC not implemented
  - `kernel/src/arch/riscv64/paging.rs` - Kernel page tables not set up
- [ ] **AArch64 Demand Paging** - Not implemented
  - `kernel/src/arch/aarch64/exceptions.rs` (line 178)
  - Impact: All memory must be pre-allocated

### Test Coverage Gaps (Priority: High)
- [ ] BPF syscall handler - No unit tests
- [ ] Unsafe pointer operations - Limited testing
- [ ] RISC-V platform - No automated tests
- [ ] JIT compiler correctness - Limited coverage

### Dependencies at Risk
- [ ] `zerocopy = "0.9.0-alpha.0"` - Alpha version
- [ ] `sha3 = "0.11.0-rc.3"` - Release candidate
- [ ] Git dependencies (`mkfs-ext2`, `mkfs-filesystem`) - Not versioned

---

## Phase 4: Hardware Attach (RPi5) ⚠️ IN PROGRESS

### GPIO
- [x] RPi5 GPIO driver in kernel
- [x] Edge detection interrupt handling
- [x] GPIO attach point implementation
- [x] BPF execution on GPIO event

### PWM
- [x] RPi5 PWM driver in kernel
- [x] PWM sysfs-like syscalls
- [x] PWM attach point implementation
- [x] BPF helpers for PWM control

### IIO (Simulated)
- [x] Driver manager structure
- [x] Simulated device
- [x] Attach point integrated (ATTACH_TYPE_IIO)

### Timer (high-resolution)
- [ ] ARM timer configuration
- [ ] Configurable tick rate
- [ ] BPF execution with timing data

### Demo: GPIO → BPF → LED
- [ ] Button press detected by GPIO interrupt
- [ ] BPF program executes
- [ ] BPF program toggles LED via helper
- [ ] End-to-end demo on real RPi5

---

## Phase 5: Validation & Demos ⚠️ PARTIAL

### Example BPF Programs
- [x] BPF maps demo (`kernel/demos/`) - Timer tick counter with maps
- [ ] `hello.bpf.c` - minimal program, prints to serial
- [ ] `counter.bpf.c` - counts events using map
- [x] `userspace/syscall_demo` - trace syscalls and log args (Rust)
- [x] `userspace/iio_demo` - read sensor data (Rust)
- [x] `userspace/gpio_demo` - toggles LED on button press (Rust)
- [ ] `safety_interlock.bpf.c` - emergency stop demo

### Performance Benchmarks
- [ ] Kernel memory footprint
- [ ] Boot time
- [ ] BPF verification time
- [ ] BPF execution overhead
- [ ] Interrupt latency

### Demo Scenarios
- [ ] **Runtime Behavior Change**: Load new scheduling policy live
- [ ] **Production Debugging**: Attach trace to running kernel
- [ ] **Safety Interlock**: Kernel-enforced emergency stop

### Documentation
- [ ] Getting started guide
- [ ] BPF program writing guide
- [ ] Architecture documentation
- [ ] API reference

---

## File Index

### Kernel Core
| Path | Description |
|------|-------------|
| `kernel/src/main.rs` | Entry points (x86_64, aarch64, riscv64) |
| `kernel/src/lib.rs` | Kernel initialization |
| `kernel/src/arch/` | Architecture-specific code |
| `kernel/src/mcore/` | Process, task, scheduler |
| `kernel/src/mem/` | Memory management glue |
| `kernel/src/file/` | VFS, Ext2, DevFS |
| `kernel/src/syscall/` | Syscall handlers |
| `kernel/src/syscall/bpf.rs` | **BPF syscall handler** |
| `kernel/src/bpf/` | **BPF manager (kernel-side)** |
| `kernel/src/driver/` | VirtIO, PCI |
| `kernel/demos/` | **Demo programs (BPF maps demo)** |

### Kernel Crates
| Path | Description |
|------|-------------|
| `kernel/crates/kernel_bpf/` | BPF subsystem |
| `kernel/crates/kernel_abi/` | Syscall numbers, errno |
| `kernel/crates/kernel_physical_memory/` | Frame allocator |
| `kernel/crates/kernel_virtual_memory/` | Address space |
| `kernel/crates/kernel_vfs/` | VFS abstraction |
| `kernel/crates/kernel_syscall/` | Syscall utilities |
| `kernel/crates/kernel_elfloader/` | ELF loading |
| `kernel/crates/kernel_device/` | Device abstraction |
| `kernel/crates/kernel_devfs/` | DevFS implementation |
| `kernel/crates/kernel_pci/` | PCI enumeration |
| `kernel/crates/kernel_memapi/` | Memory API traits |

### BPF Subsystem
| Path | Description |
|------|-------------|
| `kernel/crates/kernel_bpf/src/verifier/` | Streaming verifier |
| `kernel/crates/kernel_bpf/src/execution/` | Interpreter + JIT |
| `kernel/crates/kernel_bpf/src/maps/` | Map implementations |
| `kernel/crates/kernel_bpf/src/loader/` | ELF loader |
| `kernel/crates/kernel_bpf/src/attach/` | Attach abstractions |
| `kernel/crates/kernel_bpf/src/signing/` | Cryptographic signing |
| `kernel/crates/kernel_bpf/src/profile/` | Cloud/embedded profiles |
| `kernel/crates/kernel_bpf/src/scheduler/` | BPF scheduler |

### Userspace
| Path | Description |
|------|-------------|
| `userspace/init/` | Root process |
| `userspace/minilib/` | Syscall wrappers |
| `userspace/rk_cli/` | Deployment CLI |
| `userspace/rk_bridge/` | Event consumer |

### Documentation
| Path | Description |
|------|-------------|
| `docs/proposal.md` | Full project proposal |
| `docs/tasks.md` | This file |
| `docs/implementation.md` | Implementation details |
| `docs/howto.md` | Usage guide |

### Codebase Analysis (`.planning/codebase/`)
| Path | Description |
|------|-------------|
| `ARCHITECTURE.md` | System architecture and data flow |
| `STRUCTURE.md` | Directory layout and key files |
| `STACK.md` | Technology stack and dependencies |
| `CONVENTIONS.md` | Coding conventions and patterns |
| `TESTING.md` | Test framework and CI pipeline |
| `INTEGRATIONS.md` | External integrations |
| `CONCERNS.md` | Technical debt and known issues |
