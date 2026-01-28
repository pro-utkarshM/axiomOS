# Work Remaining for Industry Deployment

**Analysis Date:** 2026-01-28
**Overall Completion:** ~35-40%
**Remaining Work:** ~60-65%

---

## Executive Summary

Axiom has a solid foundation—the kernel boots on real hardware, and the BPF subsystem works in isolation. However, the critical work of connecting BPF to hardware, hardening security, and building robotics-specific drivers remains.

```
What's Done                          What's Left
───────────                          ──────────
✅ Bootable kernel (x86_64, ARM64)   🔴 BPF wired to real hardware
✅ Memory management                 🔴 Security hardening
✅ BPF verifier + interpreter        🔴 Robotics drivers (GPIO/PWM/IIO)
✅ x86_64 JIT                        🔴 Real-time guarantees
✅ BPF maps                          🔴 33 more syscalls
✅ Basic VFS + Ext2                  🔴 Production validation
```

---

## Detailed Breakdown by Component

### 1. Kernel Core & Infrastructure — 85% Complete ✅

| Component | Status | Notes |
|-----------|--------|-------|
| Boot (x86_64) | ✅ Done | Full ACPI, APIC support |
| Boot (AArch64/RPi5) | ✅ Done | GIC, DTB parsing |
| Boot (RISC-V) | ⚠️ Partial | Boots but non-functional |
| Physical memory | ✅ Done | Sparse frame allocator |
| Virtual memory | ✅ Done | Paging works |
| Heap allocator | ✅ Done | linked_list_allocator |
| Process/Tasks | ✅ Done | Context switching works |
| VFS + Ext2 | ✅ Done | Mount, read, write |
| ELF loader | ✅ Done | Loads userspace binaries |

**Remaining Work:**
- RISC-V platform completion (~2-3 weeks)
- AArch64 demand paging (~1 week)
- VFS node reuse optimization

---

### 2. BPF Subsystem — 75% Complete ⚠️

| Component | Status | Notes |
|-----------|--------|-------|
| Streaming verifier | ✅ Done | O(n) memory, 50KB peak |
| Interpreter | ✅ Done | All instructions |
| x86_64 JIT | ✅ Done | Full instruction set |
| ARM64 JIT | ⚠️ Partial | Structure done, ~40% complete |
| Array maps | ✅ Done | |
| Hash maps | ✅ Done | |
| Ring buffer | ✅ Done | |
| TimeSeries maps | ✅ Done | |
| Static pool (embedded) | ✅ Done | 64KB fixed allocation |
| Program signing | ✅ Done | Ed25519 + SHA3-256 |
| BTF support | 🔴 Not done | Blocks rich debugging |

**Remaining Work:**
- Complete ARM64 JIT (~2 weeks)
- BTF parsing for CO-RE support (~2-3 weeks)
- BPF-to-BPF calls (future)

---

### 3. BPF-Kernel Integration — 30% Complete 🔴

This is the critical gap—the BPF subsystem exists but isn't fully wired into the running kernel.

| Component | Status | Notes |
|-----------|--------|-------|
| BpfManager singleton | ✅ Done | Global program registry |
| bpf() syscall | ✅ Done | PROG_LOAD, MAP_CREATE, etc. |
| Timer attach point | ⚠️ Partial | attach_type=1 |
| Syscall entry attach | ⚠️ Partial | attach_type=2 |
| **GPIO attach points** | 🔴 Abstraction only | Not connected to hardware |
| **PWM attach points** | 🔴 Abstraction only | Not connected to hardware |
| **IIO sensor attach** | 🔴 Abstraction only | Not connected to hardware |
| **Kprobe** | 🔴 Not implemented | |
| **Tracepoint** | 🔴 Not implemented | |
| **Scheduler hooks** | 🔴 Not implemented | |

**Remaining Work:**
- Wire timer interrupt → BPF execution (~1 week)
- Implement GPIO attach with real hardware (~2-3 weeks)
- Implement PWM observation (~2 weeks)
- Implement IIO sensor filtering (~2 weeks)
- Kprobe/tracepoint infrastructure (~3-4 weeks)

---

### 4. Syscall Interface — 20% Complete 🔴

| Implemented (8) | Missing (33+) |
|-----------------|---------------|
| read | fork, exec, wait |
| write | mmap, munmap, mprotect |
| open | socket, bind, listen, accept |
| close | pipe, dup, dup2 |
| exit | kill, signal handling |
| bpf | clock_gettime, nanosleep |
| mmap (basic) | ioctl |
| getpid | stat, fstat, lstat |

**Remaining Work:**
- Process lifecycle syscalls (~2 weeks)
- Memory management syscalls (~1 week)
- File system syscalls (~1 week)
- Signal handling (~2 weeks)
- Networking syscalls (if needed) (~3-4 weeks)

---

### 5. Security & Safety — 15% Complete 🔴 CRITICAL

| Issue | Current State | Risk Level |
|-------|---------------|------------|
| Syscall pointer validation | Hardcoded casts, no validation | **Critical** |
| Address space verification | Missing (user vs kernel) | **Critical** |
| Bounds checking | Missing on data lengths | **High** |
| Alignment validation | Missing | **Medium** |
| Unsafe blocks | 70+ undocumented | **High** |
| Safety certification | Not started | **Blocking** |

**Specific Vulnerabilities:**
- `kernel/src/syscall/bpf.rs`: User pointers cast directly without validation
- Hardcoded 4-byte key / 8-byte value assumption for all maps
- No SAFETY comments on unsafe blocks

**Remaining Work:**
- Add pointer validation layer (~2 weeks)
- Document all unsafe blocks (~1 week)
- Security audit (~2-4 weeks)
- Define safety certification path (ongoing)

---

### 6. Hardware Drivers (Robotics) — 25% Complete ⚠️

| Driver | Abstraction | Hardware Driver | Wired to BPF | Priority |
|--------|-------------|-----------------|--------------|----------|
| GPIO | ✅ Done | ✅ RPi5 RP1 driver | 🔴 No | **Critical** |
| PWM | ✅ Done | 🔴 Not implemented | 🔴 No | **Critical** |
| IIO/Sensors | ✅ Done | 🔴 Not implemented | 🔴 No | High |
| Kprobe | ✅ Done | 🔴 No kernel infra | 🔴 No | High |
| Tracepoint | ✅ Done | 🔴 No kernel infra | 🔴 No | Medium |
| I2C | ⚠️ Type only | 🔴 Not implemented | 🔴 No | High |
| SPI | ⚠️ Type only | 🔴 Not implemented | 🔴 No | High |
| CAN bus | ⚠️ Type only | 🔴 Not implemented | 🔴 No | Medium |
| UART | N/A | ✅ RPi5 driver | N/A | Done |

**What exists:**
- `kernel/crates/kernel_bpf/src/attach/` - Full BPF attach abstractions (GPIO, PWM, IIO, Kprobe, Tracepoint)
- `kernel/src/arch/aarch64/platform/rpi5/gpio.rs` - Real RP1 GPIO driver with MMIO
- `kernel/src/arch/aarch64/platform/rpi5/uart.rs` - Real UART driver

**The gap:** The `attach()` methods are stubs. Example from `gpio.rs`:
```rust
// In a real implementation:
// 1. Open the GPIO chip via /dev/gpiochipN
// 2. Request the line with edge detection
// 3. Register a callback that invokes the BPF program
```

**Remaining Work:**
- Wire GPIO attach → RPi5 GPIO driver (~1 week)
- PWM hardware driver + BPF wiring (~2-3 weeks)
- I2C/SPI bus drivers (~2-3 weeks)
- IIO subsystem for sensors (~3-4 weeks)
- Kprobe kernel infrastructure (~2-3 weeks)

---

### 7. Testing & Validation — 25% Complete 🔴

| Area | Status | Coverage |
|------|--------|----------|
| BPF verifier tests | ✅ Good | High |
| BPF interpreter tests | ✅ Good | High |
| BPF map tests | ✅ Good | High |
| Syscall handler tests | 🔴 None | **0%** |
| Integration tests | 🔴 Manual only | Low |
| Hardware-in-loop tests | 🔴 None | **0%** |
| Performance benchmarks | ⚠️ Partial | Medium |
| Miri (undefined behavior) | ✅ CI enabled | Good |

**Remaining Work:**
- Syscall handler test suite (~2 weeks)
- End-to-end BPF lifecycle tests (~1-2 weeks)
- Hardware-in-loop test framework (~3-4 weeks)
- Performance benchmark suite (~1-2 weeks)

---

### 8. Production Readiness — 10% Complete 🔴

| Item | Status |
|------|--------|
| Real-time latency guarantees | 🔴 Not proven |
| Memory footprint validation | 🔴 Not measured |
| Boot time benchmarks | 🔴 Not measured |
| Field testing | 🔴 Not started |
| Example programs library | 🔴 1-2 only |
| User documentation | 🔴 Minimal |
| API documentation | 🔴 Minimal |
| Tooling ecosystem | ⚠️ Basic CLI |

**Remaining Work:**
- Latency measurement framework (~1-2 weeks)
- Memory profiling (~1 week)
- 10+ example BPF programs (~2-3 weeks)
- Documentation (~2-4 weeks)
- Field testing with real robot (~4+ weeks)

---

## Effort Estimates by Category

| Category | % Done | Effort to Complete |
|----------|--------|-------------------|
| Kernel infrastructure | 85% | 2-3 weeks |
| BPF engine | 75% | 4-5 weeks |
| **BPF-kernel wiring** | **30%** | **4-6 weeks** |
| **Syscalls** | **20%** | **6-8 weeks** |
| **Security hardening** | **15%** | **4-6 weeks** |
| **Hardware drivers** | **25%** | **6-10 weeks** |
| Testing | 25% | 6-8 weeks |
| Production readiness | 10% | 8-12 weeks |

**Note:** Hardware drivers improved from 5% to 25% because:
- GPIO abstraction complete + RPi5 hardware driver exists
- PWM/IIO/Kprobe/Tracepoint abstractions complete (just need wiring)
- Only actual hardware drivers + wiring remain

---

## Critical Path to MVP

```
Phase 1: BPF Integration (Weeks 1-3)
├── Wire timer interrupt to BPF execution
├── End-to-end demo: load program → executes on tick
└── Serial output visible

Phase 2: Hardware Attach Points (Weeks 4-8)
├── Wire BPF GpioAttach → existing RPi5 GPIO driver (driver exists!)
├── Button press → BPF program → LED toggle
├── PWM hardware driver + BPF wiring
└── Basic IIO sensor support

Phase 3: Security Hardening (Weeks 5-10, parallel)
├── Syscall pointer validation
├── Address space verification
├── Unsafe block documentation
└── Security audit

Phase 4: Real-World Validation (Weeks 9-14)
├── IMU sensor integration
├── Safety interlock demo
├── Performance benchmarks
└── Field testing on robot hardware
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Security vulnerabilities in syscalls | High | Critical | Prioritize validation layer |
| ARM64 JIT bugs | Medium | High | More testing, fallback to interpreter |
| Real-time guarantees not achievable | Medium | High | Measure early, adjust architecture |
| Hardware driver complexity | High | Medium | Start with GPIO only |
| BTF complexity blocking adoption | Medium | Medium | Defer, use manual definitions |

---

## Summary

**What makes Axiom promising:**
- Solid kernel foundation that boots on real hardware
- Complete BPF verification and execution engine
- Clean Rust codebase with good architecture

**What blocks industry deployment:**
1. BPF not connected to real hardware (GPIO, PWM, sensors)
2. Security vulnerabilities in syscall handling
3. Missing robotics-specific drivers
4. Unproven real-time guarantees
5. Insufficient testing and validation

**Estimated time to MVP (demo-able on RPi5):** 12-16 weeks
**Estimated time to production-ready:** 6-12 months

---

*Analysis based on codebase review: 2026-01-28*
*Update as milestones are completed*
