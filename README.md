# Axiom

**A runtime-programmable kernel for embedded systems and robotics.**

---

## The Problem

Every robot runs Linux. Every team freezes their kernel because one bad change bricks the device.

```
Want better scheduling?     → Rebuild, reflash, pray.
Need to debug production?   → Guesswork.
Fix a driver bug?           → Cross-compile, create image, flash, hope.
```

**Axiom** is a kernel where behavior is defined by verified programs that can be loaded, updated, and replaced at runtime—without reflashing.

---

## What Is This?

Axiom is a **bare-metal operating system kernel** written in Rust, designed from the ground up with runtime programmability as a first-class primitive.

```
┌─────────────────────────────────────────────────────────────┐
│                       AXIOM KERNEL                          │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              BPF Program Layer                        │  │
│  │                                                       │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │  │
│  │  │ Drivers │ │ Filters │ │ Safety  │ │ Sched   │    │  │
│  │  │ (prog)  │ │ (prog)  │ │ (prog)  │ │ (prog)  │    │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘    │  │
│  │                                                       │  │
│  │  All programs: verified, bounded, safe               │  │
│  │  Can be loaded/unloaded/updated at runtime           │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   Kernel Core                         │  │
│  │  Memory │ Processes │ VFS │ Syscalls │ BPF Verifier  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │               Architecture Layer                      │  │
│  │       x86_64    │    AArch64 (RPi5)    │   RISC-V    │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**This is not eBPF bolted onto Linux.** This is a kernel designed from day one around safe, verified, runtime-loadable programs.

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| **Kernel Core** | ✅ Complete | Boots on real hardware |
| Physical Memory | ✅ Complete | Sparse frame allocator |
| Virtual Memory | ✅ Complete | Paging, address spaces |
| Process/Tasks | ✅ Complete | Scheduling, context switch |
| VFS | ✅ Complete | Ext2, DevFS |
| Syscalls | ⚠️ Partial | 8 of 41 implemented |
| **BPF Subsystem** | ✅ Complete | Full implementation |
| Streaming Verifier | ✅ Complete | O(n) memory, 50KB peak |
| Interpreter | ✅ Complete | All instructions |
| x86_64 JIT | ✅ Complete | Full instruction set |
| ARM64 JIT | ✅ Complete | Full instruction set |
| Maps | ✅ Complete | Array, Hash, RingBuf, TimeSeries |
| **Architecture** | | |
| x86_64 | ✅ Complete | ACPI, APIC, full boot |
| AArch64 | ✅ Complete | GIC, DTB, RPi5 platform |
| RISC-V | ⚠️ Partial | Boot sequence works |
| **BPF Integration** | ✅ Complete | Syscall, attach points, helpers |

**BPF is now wired into the kernel!** Load programs via `sys_bpf`, attach to Timer (type=1) or Syscall (type=2) events.

---

## Quick Start

### Prerequisites

```bash
# Ubuntu/Debian
sudo apt install xorriso e2fsprogs qemu-system

# Rust nightly (configured via rust-toolchain.toml)
rustup update
```

### Build & Run

```bash
# Build and run in QEMU (x86_64)
cargo run

# Headless mode
cargo run -- --headless

# With GDB debugging
cargo run -- --debug

# Custom resources
cargo run -- --smp 4 --mem 512M
```

### Architecture-Specific

```bash
# RISC-V
./scripts/build-riscv.sh
./scripts/run-riscv.sh

# Raspberry Pi 5
./scripts/build-rpi5.sh
./scripts/deploy-rpi5.sh
```

---

## Repository Structure

```
axiom-ebpf/
├── kernel/
│   ├── src/                      # Core kernel
│   │   ├── main.rs               # Entry points
│   │   ├── arch/                 # x86_64, aarch64, riscv64
│   │   ├── mcore/                # Processes, tasks, scheduler
│   │   ├── mem/                  # Memory management
│   │   ├── file/                 # VFS layer
│   │   ├── syscall/              # Syscall handlers
│   │   └── driver/               # VirtIO, PCI, block
│   │
│   └── crates/                   # Kernel subsystems
│       ├── kernel_bpf/           # BPF subsystem (verifier, JIT, maps)
│       ├── kernel_abi/           # Syscall numbers, errno
│       ├── kernel_physical_memory/
│       ├── kernel_virtual_memory/
│       ├── kernel_vfs/
│       ├── kernel_syscall/
│       ├── kernel_elfloader/
│       ├── kernel_device/
│       ├── kernel_devfs/
│       ├── kernel_pci/
│       └── kernel_memapi/
│
├── userspace/
│   ├── init/                     # Root process
│   ├── minilib/                  # Syscall wrappers
│   ├── rk_cli/                   # Deployment CLI
│   └── rk_bridge/                # Event consumer
│
├── docs/
│   ├── proposal.md               # Full vision and roadmap
│   ├── tasks.md                  # Implementation status
│   └── implementation.md         # Technical details
│
├── build.rs                      # ISO/disk image creation
└── limine.conf                   # Bootloader config
```

---

## The BPF Subsystem

The heart of Axiom's runtime programmability:

### Streaming Verifier

Standard BPF verifiers hold entire program state in memory (50-100MB). Ours processes in a single forward pass:

```
Standard:  O(instructions × registers × paths) = ~100MB
Axiom:     O(registers × basic_block_depth)    = ~50KB
```

### Profile System

Compile-time selection between deployment targets:

| Profile | Stack | Instructions | JIT | Memory |
|---------|-------|--------------|-----|--------|
| Cloud | 512KB | 1M (soft) | Yes | Heap |
| Embedded | 8KB | 100K (hard) | Optional | 64KB static pool |

### Testing

```bash
# Test with embedded profile (default)
cargo test -p kernel_bpf

# Test with cloud profile
cargo test -p kernel_bpf --no-default-features --features cloud-profile

# Run benchmarks
cargo bench -p kernel_bpf --features cloud-profile
```

---

## Robotics-Specific Features

Attach points designed for embedded systems:

```c
// GPIO - safety interlocks
SEC("gpio/chip0/line17/rising")
int limit_switch(struct gpio_event *evt) {
    bpf_motor_emergency_stop(MOTOR_ALL);
    return 0;
}

// PWM - motor observation
SEC("pwm/chip0/channel0")
int trace_motor(struct pwm_state *state) {
    bpf_ringbuf_output(&events, &state, sizeof(*state), 0);
    return 0;
}

// IIO - sensor filtering
SEC("iio/device0/accel_x")
int filter_accel(struct iio_event *evt) {
    return (evt->value >= MIN && evt->value <= MAX) ? 1 : 0;
}
```

---

## Comparison

| | Linux + eBPF | Zephyr | FreeRTOS | Axiom |
|---|---|---|---|---|
| Runtime programmable | Partial | No | No | **Yes** |
| Verified programs | Yes | No | No | **Yes** |
| Memory footprint | 100MB+ | <1MB | <100KB | **<10MB** |
| POSIX-ish userspace | Yes | Partial | No | **Yes** |
| Multi-core | Yes | Limited | Limited | **Yes** |
| Robotics focus | No | No | No | **Yes** |

---

## Documentation

| Document | Description |
|----------|-------------|
| [proposal.md](docs/proposal.md) | Full vision, architecture, business model |
| [tasks.md](docs/tasks.md) | Implementation status and roadmap |
| [implementation.md](docs/implementation.md) | Technical details, code examples |

---

## Contributing

Contributions welcome, especially in:

- BPF integration (wiring subsystem into kernel)
- Architecture bring-up (RISC-V, ARM platforms)
- Robotics attach points (GPIO, PWM, IIO drivers)
- Documentation and examples

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

---

## License

Dual-licensed under:

- **Apache License 2.0**
- **MIT License**

---

## Contact

**Author:** Utkarsh

**Target venues:** AgenticOS2026 Workshop (ASPLOS), Robotics conferences

**Status:** Seeking collaborators, funding, and early adopters
