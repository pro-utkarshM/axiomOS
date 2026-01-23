# rkBPF Task Tracking

## Phase 1: Core Runtime (Proposal Weeks 1-6)

### Completed

- [x] **Profile System** - `kernel_bpf/src/profile/`
- [x] **Bytecode Module** - `kernel_bpf/src/bytecode/`
- [x] **Interpreter** - `kernel_bpf/src/execution/interpreter.rs`
- [x] **Array Map** - `kernel_bpf/src/maps/array.rs`
- [x] **Scheduler** - `kernel_bpf/src/scheduler/`
- [x] **Streaming Verifier** - `kernel_bpf/src/verifier/streaming.rs` - O(registers × basic_block_depth) algorithm
- [x] **Ring Buffer Map** - `kernel_bpf/src/maps/ringbuf.rs` - Lock-free kernel-to-userspace streaming
- [x] **Hash Map** - `kernel_bpf/src/maps/hash.rs` - O(1) lookup with linear probing
- [x] **libbpf-free Loader** - `kernel_bpf/src/loader/` - Minimal ELF64 parser (~50KB)
- [x] **ARM64 JIT Compiler** - `kernel_bpf/src/execution/jit_aarch64.rs`
- [x] **x86_64 JIT Compiler** - `kernel_bpf/src/execution/jit/mod.rs` - Full instruction set support
- [x] **Verifier Helper Integration** - `kernel_bpf/src/verifier/helpers.rs` - Type-safe helper validation

**Phase 1 Progress: 100%** ✅

---

## Phase 2: Robotics Integration (Weeks 7-10)

### Completed

- [x] **Attach Point Abstraction** - `kernel_bpf/src/attach/mod.rs`
- [x] **IIO Subsystem Attach Points** - `kernel_bpf/src/attach/iio.rs`
- [x] **GPIO Event Hooks** - `kernel_bpf/src/attach/gpio.rs`
- [x] **PWM Observation Points** - `kernel_bpf/src/attach/pwm.rs`
- [x] **Kprobe/Tracepoint Attach** - `kernel_bpf/src/attach/kprobe.rs`, `tracepoint.rs`
- [x] **Time-Series Map Type** - `kernel_bpf/src/maps/timeseries.rs` - Circular buffer for sensor data
- [x] **ROS2 Bridge** - `userspace/rk_bridge/` - Event bridge with ring buffer consumer

**Phase 2 Progress: 100%** ✅

---

## Phase 3-4: Production & Ecosystem

### In Progress

- [ ] **Documentation** - partial

### Pending

- [ ] **Program Signing**
- [ ] **Deployment Tooling**
- [ ] **Benchmarks**

**Phase 3-4 Progress: ~10%**

---

## Priority Queue

1. ~~Streaming Verifier (core innovation)~~ DONE
2. ~~Ring Buffer Map~~ DONE
3. ~~Hash Map~~ DONE
4. ~~libbpf-free Loader~~ DONE
5. ~~ARM64 JIT Compiler~~ DONE
6. ~~Attach Point Abstraction~~ DONE
7. ~~IIO/GPIO/PWM Integration~~ DONE
8. ~~ROS2 Bridge~~ DONE
9. ~~Time-Series Map~~ DONE
10. ~~x86_64 JIT Compiler~~ DONE
11. ~~Helper function integration~~ DONE
12. Program Signing & Verification
13. Deployment Tooling (rk-cli)
14. Performance Benchmarks

---

## Recent Changes

### 2026-01-23

- **x86_64 JIT Compiler** - Complete implementation with:
  - BPF R0-R10 register mapping to x86_64 (RAX, RDI, RSI, RDX, RCX, R8, RBX, R13, R14, R15, RBP)
  - Full instruction encoding (MOV, ADD, SUB, MUL, DIV, AND, OR, XOR, shifts, jumps, loads/stores)
  - Byte swap operations (LE/BE 16/32/64-bit)
  - Jump patching for forward references
  - Proper prologue/epilogue with callee-saved registers

- **Helper Function Registry** - Type-safe helper validation with:
  - Core helpers (map operations, ringbuf, time, printing)
  - Robotics-specific helpers (motor_emergency_stop, timeseries_push, gpio, pwm, iio, can)
  - Profile-aware availability checking
  - Argument type validation

- **Time-Series Map** - Circular buffer for robotics sensor data with:
  - Automatic old entry eviction
  - Time-window queries
  - Statistics tracking (min/max/avg/count)
  - Profile-aware limits (4K embedded, 1M cloud)

- **ROS2 Bridge** - Userspace event bridge with:
  - Ring buffer consumer via mmap
  - Event types: IMU, Motor, Safety, GPIO, TimeSeries, Trace
  - Publisher backends (stdout, ROS2 placeholder)
  - CLI tool with demo mode
