# rkBPF Task Tracking

## Implementation Status Overview

rkBPF currently exists as a **complete userspace library/framework** implementing all core algorithms and data structures. The next milestone is **kernel integration** to make it a working kernel instrumentation tool.

| Layer | Status | Description |
|-------|--------|-------------|
| Algorithms & Data Structures | ✅ Complete | Verifier, JIT, maps, loader |
| Userspace Tooling | ✅ Complete | rk-cli, rk-bridge, signing |
| Kernel Module | ❌ Not Started | Linux kernel integration |
| Example Programs | ❌ Not Started | .bpf.c programs |
| Platform Validation | ❌ Not Started | RPi4, Jetson testing |

---

## Phase 1: Core Runtime ✅

### Completed (Userspace Library)

- [x] **Profile System** - `kernel_bpf/src/profile/`
  - Dual-profile architecture (embedded vs cloud)
  - Compile-time feature selection
  - Profile-specific limits and capabilities

- [x] **Bytecode Module** - `kernel_bpf/src/bytecode/`
  - BPF instruction encoding/decoding
  - Opcode classes (ALU, JMP, MEM, etc.)
  - Register file abstraction

- [x] **Streaming Verifier** - `kernel_bpf/src/verifier/streaming.rs`
  - O(registers × basic_block_depth) memory complexity
  - Single-pass verification algorithm
  - CFG analysis and path tracking

- [x] **Interpreter** - `kernel_bpf/src/execution/interpreter.rs`
  - Full BPF instruction set execution
  - Bounded execution with instruction limits

- [x] **x86_64 JIT Compiler** - `kernel_bpf/src/execution/jit/`
  - Full instruction encoding
  - Register allocation (BPF R0-R10 → x86_64)
  - Jump patching, prologue/epilogue

- [x] **ARM64 JIT Compiler** - `kernel_bpf/src/execution/jit_aarch64.rs`
  - ARM64 instruction encoding
  - Register mapping for embedded targets

- [x] **Maps** - `kernel_bpf/src/maps/`
  - Array map with O(1) access
  - Hash map with linear probing
  - Ring buffer (lock-free)
  - Time-series map (circular buffer)
  - Static pool (embedded profile)

- [x] **ELF Loader** - `kernel_bpf/src/loader/`
  - Minimal ELF64 parser (~50KB, no libbpf dependency)
  - Section parsing, relocation handling
  - Helper resolution

- [x] **Helper Registry** - `kernel_bpf/src/verifier/helpers.rs`
  - Core helpers (map ops, ringbuf, time)
  - Robotics helpers (motor, gpio, pwm, iio, can)
  - Type-safe signature validation

---

## Phase 2: Robotics Integration ✅

### Completed (Framework/Stubs)

- [x] **Attach Point Abstraction** - `kernel_bpf/src/attach/`
  - Generic `AttachPoint` trait
  - Attach ID tracking, lifecycle management

- [x] **IIO Attach** - `kernel_bpf/src/attach/iio.rs`
  - IIO channel/device abstraction
  - Event filtering structures
  - ⚠️ **Stub only** - no actual kernel IIO hooks

- [x] **GPIO Attach** - `kernel_bpf/src/attach/gpio.rs`
  - Edge detection (rising/falling/both)
  - GPIO chip/line abstraction
  - ⚠️ **Stub only** - no actual kernel GPIO hooks

- [x] **PWM Attach** - `kernel_bpf/src/attach/pwm.rs`
  - PWM chip/channel abstraction
  - Duty cycle event structures
  - ⚠️ **Stub only** - no actual kernel PWM hooks

- [x] **Kprobe/Tracepoint** - `kernel_bpf/src/attach/kprobe.rs`, `tracepoint.rs`
  - Function entry/return probes
  - Tracepoint subsystem/event model
  - ⚠️ **Stub only** - no actual kernel registration

- [x] **ROS2 Bridge** - `userspace/rk_bridge/`
  - Ring buffer consumer via mmap
  - Event types (IMU, Motor, Safety, GPIO, TimeSeries)
  - Publisher backends (stdout, ROS2 placeholder)

---

## Phase 3: Production Hardening ✅

### Completed

- [x] **Program Signing** - `kernel_bpf/src/signing/`
  - SHA3-256 (Keccak) pure Rust implementation
  - Ed25519 signature verification
  - Signed program format (magic, version, hash, signature)
  - TrustedKey management with profile limits

- [x] **Deployment CLI** - `userspace/rk_cli/`
  - Key management (generate, export, import, list)
  - Program signing and verification
  - Build integration (clang wrapper)
  - Deploy commands (local/remote)
  - Project scaffolding (`rk init`)

- [x] **Benchmarks** - `kernel_bpf/benches/`
  - Interpreter benchmarks (arithmetic, loops, conditionals)
  - Verifier benchmarks (scaling, control flow)
  - Map benchmarks (array, hash, ringbuf)

---

## Phase 4: Ecosystem ❌

### Not Started

- [ ] **Example BPF Programs**
  - Sensor filtering example (IIO)
  - Safety interlock example (GPIO)
  - Motor tracing example (PWM)
  - End-to-end demo programs

- [ ] **Kernel Module**
  - Linux kernel module for program loading
  - Syscall/ioctl interface
  - Actual kprobe/tracepoint registration
  - IIO/GPIO/PWM subsystem hooks

- [ ] **Platform Validation**
  - Raspberry Pi 4 testing
  - Jetson Nano testing
  - Memory footprint measurement
  - Control loop overhead benchmarks

- [ ] **Demo Scenarios** (from proposal)
  - Live Safety Patching demo
  - Production Debugging demo
  - Unified Timeline demo

- [ ] **Academic Publication**
  - AgenticOS2026 Workshop paper
  - Benchmark results documentation

---

## Architecture Notes

### What Works Today

```
┌──────────────────────────────────────────────────────────┐
│                    Userspace                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │  rk-cli     │  │  rk-bridge  │  │  Your Tool      │   │
│  │  (sign,     │  │  (events →  │  │  (uses          │   │
│  │   deploy)   │  │   ROS2)     │  │   kernel_bpf)   │   │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘   │
│         │                │                   │            │
│         └────────────────┼───────────────────┘            │
│                          │                                │
│              ┌───────────▼───────────┐                    │
│              │     kernel_bpf        │  ← You are here    │
│              │  (Rust library)       │                    │
│              │                       │                    │
│              │  • Verifier           │                    │
│              │  • JIT (x86/ARM64)    │                    │
│              │  • Maps               │                    │
│              │  • Loader             │                    │
│              │  • Signing            │                    │
│              └───────────────────────┘                    │
└──────────────────────────────────────────────────────────┘
                          │
                          │ NOT YET CONNECTED
                          ▼
┌──────────────────────────────────────────────────────────┐
│                     Linux Kernel                          │
│                                                          │
│   kprobes │ tracepoints │ IIO │ GPIO │ PWM               │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### Next Steps for Full Functionality

1. **Kernel Module** - Write a loadable kernel module that:
   - Exposes ioctl interface for loading verified programs
   - Registers programs with kprobe/tracepoint infrastructure
   - Hooks into IIO/GPIO/PWM subsystems for robotics events

2. **Example Programs** - Create `.bpf.c` files demonstrating:
   - Sensor data filtering
   - Safety interlocks
   - Motor control tracing

3. **Integration Testing** - Validate on target platforms:
   - Memory footprint (<10MB target)
   - Verification time (<100ms for 1K instructions)
   - Runtime overhead (<1% on control loops)

---

## File Index

| Path | Description |
|------|-------------|
| `kernel/crates/kernel_bpf/` | Core library |
| `kernel/crates/kernel_bpf/src/verifier/` | Streaming verifier |
| `kernel/crates/kernel_bpf/src/execution/` | Interpreter + JIT |
| `kernel/crates/kernel_bpf/src/maps/` | Map implementations |
| `kernel/crates/kernel_bpf/src/loader/` | ELF loader |
| `kernel/crates/kernel_bpf/src/attach/` | Attach point stubs |
| `kernel/crates/kernel_bpf/src/signing/` | Cryptographic signing |
| `userspace/rk_cli/` | Deployment CLI |
| `userspace/rk_bridge/` | ROS2 event bridge |
| `docs/proposal.md` | Full project proposal |
| `docs/howto.md` | Usage guide |
