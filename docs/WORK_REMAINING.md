# Work Remaining for Industry Deployment

**Analysis Date:** 2026-01-28
**Overall Completion:** ~40-45%
**Remaining Work:** ~55-60%

---

## Executive Summary

Axiom has a solid foundation—the kernel boots on real hardware, **and BPF programs execute on timer/syscall hooks**. The RPi5 GPIO driver exists. The main work is connecting BPF to hardware attach points, hardening security, and building remaining drivers.

```
What's Done                          What's Left
───────────                          ──────────
✅ Bootable kernel (x86_64, ARM64)   🔴 IIO sensor attach wiring
✅ Memory management                 🔴 IIO hardware drivers
✅ BPF verifier + interpreter        🔴 Real-time guarantees
✅ x86_64 JIT + BPF maps             🔴 35 more syscalls
✅ Timer hooks (BPF executes!)       🔴 Production validation
✅ Syscall hooks (BPF executes!)     🔴 Kprobe/tracepoint infrastructure
✅ RPi5 GPIO driver (MMIO)
✅ GPIO attach wiring (Verified!)
✅ RPi5 PWM driver & wiring
✅ Security hardening (Syscalls)
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
| ARM64 JIT | ✅ Done | Full instruction set |
| Array maps | ✅ Done | |
| Hash maps | ✅ Done | |
| Ring buffer | ✅ Done | |
| TimeSeries maps | ✅ Done | |
| Static pool (embedded) | ✅ Done | 64KB fixed allocation |
| Program signing | ✅ Done | Ed25519 + SHA3-256 |
| BTF support | 🔴 Not done | Blocks rich debugging |

**Remaining Work:**
- BTF parsing for CO-RE support (~2-3 weeks)
- BPF-to-BPF calls (future)

---

### 3. BPF-Kernel Integration — 60% Complete ⚠️

The core BPF-kernel integration is **working**. Timer and syscall hooks execute BPF programs.

| Component | Status | Notes |
|-----------|--------|-------|
| BpfManager singleton | ✅ Done | Global program registry in `kernel/src/bpf/mod.rs` |
| bpf() syscall | ✅ Done | 6 operations: PROG_LOAD, PROG_ATTACH, MAP_CREATE/LOOKUP/UPDATE/DELETE |
| **Timer hooks** | ✅ Working | `execute_hooks(1, ctx)` in `idt.rs:169` and `interrupts.rs:63` |
| **Syscall hooks** | ✅ Working | `execute_hooks(5, ctx)` in `syscall/mod.rs` (Global Trace) |
| BPF helpers | ✅ Done | `bpf_ktime_get_ns`, `bpf_trace_printk`, `bpf_map_*`, `bpf_gpio_*`, `bpf_pwm_*` |
| **GPIO attach** | ✅ Working | Wired to RPi5 driver & verified with integration tests |
| **PWM attach** | ✅ Working | Wired to RPi5 driver & enabled via syscalls |
| **IIO sensor attach** | ⚠️ Simulated | Driver manager + attach integrated |
| **Kprobe** | 🔴 Abstraction only | No kernel infrastructure |
| **Tracepoint** | 🔴 Abstraction only | No kernel infrastructure |

**What's working today:**
```
Userspace → bpf(BPF_PROG_LOAD) → program stored
         → bpf(BPF_PROG_ATTACH, type=2) → attached to GPIO
         → Hardware Interrupt (RPi5 Pin 17) → BPF program executes!
```

**Remaining Work:**
- IIO sensor driver + BPF wiring (~2-3 weeks)
- Kprobe/tracepoint kernel infrastructure (~3-4 weeks)
- [x] Fix hardcoded key_size=4, value_size=8 in syscall handler (Done)

---

### 4. Syscall Interface — 17% Complete 🔴

**7 of 42 syscalls implemented** (x86_64 only, stubs on other archs)

| Implemented (8) | Missing (34) |
|-----------------|--------------|
| exit | fork, exec, wait, clone |
| read | dup, dup2, pipe |
| write, writev | lseek, stat, fstat |
| open, close | socket, bind, listen, accept |
| mmap | munmap, mprotect |
| getcwd | chdir, mkdir, rmdir |
| bpf | kill, signal, sigaction |
| | clock_gettime, nanosleep |
| | ioctl, fcntl, poll |

**Remaining Work:**
- Process lifecycle: fork, exec, wait (~2 weeks)
- File operations: lseek, stat, fstat (~1 week)
- Signal handling (~2 weeks)
- Remaining memory syscalls (~1 week)

---

### 5. Security & Safety — 40% Complete ⚠️

| Issue | Current State | Risk Level |
|-------|---------------|------------|
| Syscall pointer validation | ✅ Validated | **Low** |
| Address space verification | ✅ Validated | **Low** |
| Bounds checking | ✅ Validated | **Low** |
| Alignment validation | ✅ Validated | **Low** |
| Unsafe blocks | ✅ Audited | **Low** |
| Safety certification | Not started | **Blocking** |

**Specific Vulnerabilities:**
- [x] `kernel/src/syscall/bpf.rs`: User pointers cast directly without validation (Fixed with `validation.rs` wrappers)
- [x] No SAFETY comments on unsafe blocks (Audited and documented)

**Remaining Work:**
- [x] Document all unsafe blocks (Completed)
- [x] Internal Security audit (Completed)
- Define safety certification path (ongoing)

---

### 6. Hardware Drivers (Robotics) — 50% Complete ⚠️

| Driver | Abstraction | Hardware Driver | Wired to BPF | Priority |
|--------|-------------|-----------------|--------------|----------|
| GPIO | ✅ Done | ✅ RPi5 RP1 driver | ✅ Yes | **Critical** |
| PWM | ✅ Done | ✅ RPi5 driver | ✅ Yes | **Critical** |
| IIO/Sensors | ✅ Done | ⚠️ Simulated | ✅ Yes | High |
| Kprobe | ✅ Done | 🔴 No kernel infra | 🔴 No | High |
| Tracepoint | ✅ Done | 🔴 No kernel infra | 🔴 No | Medium |
| I2C | ⚠️ Type only | 🔴 Not implemented | 🔴 No | High |
| SPI | ⚠️ Type only | 🔴 Not implemented | 🔴 No | High |
| CAN bus | ⚠️ Type only | 🔴 Not implemented | 🔴 No | Medium |
| UART | N/A | ✅ RPi5 driver | N/A | Done |

**What exists:**
- `kernel/crates/kernel_bpf/src/attach/` - Full BPF attach abstractions (GPIO, PWM, IIO, Kprobe, Tracepoint)
- `kernel/src/arch/aarch64/platform/rpi5/gpio.rs` - Real RP1 GPIO driver with MMIO
- `kernel/src/arch/aarch64/platform/rpi5/pwm.rs` - Real RP1 PWM driver
- `kernel/src/arch/aarch64/platform/rpi5/uart.rs` - Real UART driver

**The gap:** The `attach()` methods for IIO/Kprobe are still stubs.

**Remaining Work:**
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
| **BPF-kernel wiring** | **60%** | **3-5 weeks** |
| **Syscalls** | **17%** | **6-8 weeks** |
| **Security hardening** | **40%** | **3-5 weeks** |
| **Hardware drivers** | **25%** | **6-10 weeks** |
| Testing | 25% | 6-8 weeks |
| Production readiness | 10% | 8-12 weeks |

**Key findings:**
- BPF-kernel wiring improved from 30% to 60% because timer and syscall hooks are WORKING
- GPIO abstraction + RPi5 hardware driver both exist (just need to connect them)
- PWM/IIO/Kprobe/Tracepoint abstractions complete (need HW drivers + wiring)

---

## Critical Path to MVP

```
Phase 1: BPF Integration (MOSTLY DONE ✅)
├── ✅ Wire timer interrupt to BPF execution (WORKING)
├── ✅ bpf() syscall with PROG_LOAD, ATTACH, MAP ops (WORKING)
├── ✅ BPF helpers: ktime, trace_printk, map_* (WORKING)
└── ✅ Fix hardcoded key/value sizes in syscall handler (Done)

Phase 2: Hardware Attach Points (Weeks 1-5)
├── Wire BPF GpioAttach → existing RPi5 GPIO driver (~1 week)
├── Button press → BPF program → LED toggle demo
├── PWM hardware driver + BPF wiring (~2-3 weeks)
└── Basic IIO sensor support (~2 weeks)

Phase 3: Security Hardening (Weeks 3-8, parallel)
├── Syscall pointer validation
├── Address space verification
├── Unsafe block documentation
└── Security audit

Phase 4: Real-World Validation (Weeks 6-10)
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
- **BPF timer and syscall hooks already working end-to-end**
- RPi5 GPIO hardware driver exists
- Clean Rust codebase with good architecture

**What blocks industry deployment:**
1. GPIO/PWM/IIO not wired to BPF (timer/syscall hooks work, hardware hooks don't)
2. Security vulnerabilities in syscall handling (hardcoded sizes, no pointer validation)
3. PWM/IIO hardware drivers not implemented
4. Unproven real-time guarantees
5. Insufficient testing and validation

**Estimated time to MVP (demo-able on RPi5):** 8-12 weeks
**Estimated time to production-ready:** 5-9 months

---

*Analysis based on codebase review: 2026-01-28*
*Update as milestones are completed*
