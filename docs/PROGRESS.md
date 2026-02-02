# Axiom Development Progress Tracker

**Last Updated:** 2026-02-03
**Overall Progress:** Phase 0 - Critical Blockers
**Next Milestone:** Boot to userspace on x86_64

---

## Overview

This document tracks progress toward the vision outlined in [proposal.md](proposal.md). Each phase builds on the previous, with clear success criteria from the proposal.

**Legend:**
- 🔴 Blocked / Critical
- 🔧 In Progress
- ⏳ Next Up
- ✅ Complete
- ⏸️ Deferred

---

## Phase 0: Critical Blockers 🔴

**Goal:** Get kernel booting to userspace on both architectures.

**Blocking:** Everything. Cannot demonstrate runtime programmability without a working kernel.

| # | Task | Status | Priority | Owner | Notes |
|---|------|--------|----------|-------|-------|
| 0.1 | Fix build system (artifact dependencies) | ✅ Complete | Critical | - | Completed 2026-02-03 |
| 0.2 | Fix x86_64 boot crash (page table remapping) | ✅ Complete | **CRITICAL** | - | FIXED 2026-02-03: Use bootloader page tables instead of creating new ones |
| 0.3 | Fix AArch64 context switching in interrupt handlers | ⏳ Next | **CRITICAL** | - | Documented in `WORK_REMAINING.md`, interrupts disabled as workaround |
| 0.4 | Get kernel booting to userspace (x86_64) | 🔧 In Progress | Critical | - | Was blocked by 0.2, now unblocked |
| 0.5 | Get kernel booting to userspace (AArch64/RPi5) | ⏳ Blocked | Critical | - | Blocked by 0.3 |

**Exit Criteria:**
- [x] Build system works for both architectures
- [ ] x86_64 kernel boots to /bin/init
- [ ] AArch64 kernel boots to /bin/init on RPi5
- [ ] Serial console shows init output

---

## Phase 1: BPF Integration (Weeks 1-4)

**Goal:** Demonstrate runtime programmability - load BPF program from userspace, see it execute.

**Proposal Success Criteria (from proposal.md line 466-470):**
- Boot Axiom ✅ (when blockers fixed)
- Load BPF program from userspace
- Program executes on timer interrupt ✅ (kernel-only, works)
- Output visible in serial console ✅

| # | Task | Status | Dependencies | Notes |
|---|------|--------|--------------|-------|
| 1.1 | Wire BPF manager into kernel initialization | ⏳ Ready | 0.2, 0.3 | BpfManager struct exists in `kernel/src/bpf/mod.rs` |
| 1.2 | Complete bpf() syscall userspace wrapper in minilib | ⏳ Ready | 0.4, 0.5 | Kernel-side syscall exists at `kernel/src/syscall/bpf.rs` |
| 1.3 | Write simple userspace BPF loader | ⏳ Ready | 1.2 | Load program, attach to timer |
| 1.4 | End-to-end test: userspace → bpf() → execute | ⏳ Ready | 1.1, 1.2, 1.3 | **MILESTONE** |
| 1.5 | Create example: counter program using BPF maps | ⏳ Ready | 1.4 | Demonstrates state persistence |

**Exit Criteria:**
- [ ] Userspace program calls bpf(BPF_PROG_LOAD)
- [ ] Program loads successfully
- [ ] Program executes on timer interrupt
- [ ] Can read/write BPF maps from userspace
- [ ] Serial console shows BPF output

**Deliverable:** Video demo of loading BPF program at runtime

---

## Phase 2: Hardware Attach Points (Weeks 5-10)

**Goal:** Connect BPF to real hardware on Raspberry Pi 5.

**Proposal Success Criteria (from proposal.md line 482-485):**
- Button press triggers BPF program
- BPF program controls LED
- Motor commands traced with nanosecond precision

| # | Task | Status | Dependencies | Notes |
|---|------|--------|--------------|-------|
| 2.1 | Wire GPIO attach point to RPi5 GPIO driver | ⏳ Ready | 1.4 | Driver exists: `kernel/src/arch/aarch64/platform/rpi5/gpio.rs` |
| 2.2 | Demo: Button press → BPF → LED toggle | ⏳ Ready | 2.1 | **MILESTONE** - First hardware demo |
| 2.3 | Wire PWM attach point to RPi5 PWM driver | ⏳ Ready | 2.1 | Driver exists: `kernel/src/arch/aarch64/platform/rpi5/pwm.rs` |
| 2.4 | Demo: Motor command observation via PWM | ⏳ Ready | 2.3 | Trace timing with nanosecond precision |
| 2.5 | Implement IIO sensor hardware drivers | ⏸️ Deferred | - | Currently simulated |
| 2.6 | Wire IIO attach point to hardware drivers | ⏸️ Deferred | 2.5 | Attach abstraction exists |
| 2.7 | Full RPi5 hardware integration test | ⏳ Ready | 2.2, 2.4 | All attach points working |

**Exit Criteria:**
- [ ] GPIO button triggers BPF program on RPi5
- [ ] BPF program controls LED
- [ ] PWM motor commands traced in real-time
- [ ] All demos run on physical hardware

**Deliverable:** Live RPi5 hardware demo at conference

---

## Phase 3: Real-World Validation (Weeks 11-16)

**Goal:** Production-ready validation and benchmarks.

**Proposal Success Criteria (from proposal.md line 497-500):**
- IMU data filtered at kernel level
- Safety interlock enforced by kernel (cannot be bypassed)
- Published comparison vs Linux

| # | Task | Status | Dependencies | Notes |
|---|------|--------|--------------|-------|
| 3.1 | Safety interlock demo (limit switch → e-stop) | ⏳ Ready | 2.2 | **KEY DEMO** - Kernel-enforced safety |
| 3.2 | IMU sensor integration and kernel filtering | ⏳ Ready | 2.6 | Drop invalid readings before userspace |
| 3.3 | Implement remaining syscalls (33 more) | ⏸️ Partial | - | Currently 8/41 implemented |
| 3.4 | Performance benchmarks vs Linux | ⏳ Ready | 2.7 | Boot time, memory, latency, interrupt overhead |
| 3.5 | Create 10 example BPF programs | ⏳ Ready | 2.7 | Proposal target: 10 examples minimum |
| 3.6 | Field testing on actual robot hardware | ⏸️ Future | 3.1, 3.2 | Requires robot partner |

**Exit Criteria:**
- [ ] Safety interlock demo working and documented
- [ ] Performance comparison published
- [ ] 10+ example programs available
- [ ] Field validation complete

**Deliverable:** Technical report comparing Axiom vs Linux for robotics

---

## Phase 4: Academic Publication

**Goal:** Submit to AgenticOS2026 Workshop (ASPLOS).

| # | Task | Status | Dependencies | Notes |
|---|------|--------|--------------|-------|
| 4.1 | Write paper draft | ⏸️ Future | 3.4, 3.5 | Workshop explicitly wants eBPF for real-time systems |
| 4.2 | Create evaluation section | ⏸️ Future | 3.4 | Benchmarks and comparisons |
| 4.3 | Submit to AgenticOS2026 | ⏸️ Future | 4.1, 4.2 | Deadline TBD |

**Exit Criteria:**
- [ ] Paper submitted
- [ ] Evaluation complete
- [ ] Artifacts available

---

## Current Status Summary

**What's Working:**
- ✅ Build system (x86_64, AArch64)
- ✅ x86_64 kernel boots successfully (FIXED!)
- ✅ Memory initialization and heap setup
- ✅ BPF subsystem complete (verifier, JIT, maps, signing)
- ✅ Timer interrupt hooks execute BPF programs
- ✅ Syscall hooks execute BPF programs
- ✅ RPi5 GPIO/PWM hardware drivers exist
- ✅ BPF attach abstractions implemented

**What's Broken:**
- 🔴 AArch64: Context switching in interrupt handlers crashes kernel
- 🔴 Cannot boot to userspace on either architecture yet (next step)

**Next Immediate Steps:**
1. ✅ ~~Fix x86_64 page table crash~~ (COMPLETED!)
2. Continue x86_64 boot and reach userspace
3. Fix AArch64 context switching crash
4. Get init running in userspace on both architectures
5. Wire BPF manager into kernel
6. Complete Phase 1 demo

---

## Metrics & Goals

From proposal.md (line 543-548):

| Metric | Target | Stretch | Current |
|--------|--------|---------|---------|
| Kernel memory footprint | <10MB | <5MB | Unknown (can't boot) |
| Boot to init | <1s | <500ms | N/A (crashes) |
| BPF load time | <10ms | <1ms | N/A |
| Interrupt latency | <10μs | <1μs | N/A |
| Programs shipped | 10 examples | 50 examples | 2 demos exist |

---

## Known Issues & Blockers

See also: [WORK_REMAINING.md](WORK_REMAINING.md)

### Critical
1. **~~x86_64 Page Table Crash~~** - ✅ FIXED 2026-02-03
   - Solution: Use bootloader's page tables directly instead of creating new ones
   - Avoids chicken-and-egg problem with HHDM mapping during page table setup
   - Kernel now boots successfully past memory initialization

2. **AArch64 Context Switch Crash** - Documented in WORK_REMAINING.md
   - Context switching inside interrupt handlers corrupts stack
   - Interrupts disabled as workaround
   - Requires deferred scheduling or proper exception context handling

### High Priority
3. **RISC-V Incomplete** - Boot works but system non-functional
4. **Syscall Coverage** - Only 8/41 implemented (need fork, exec, signals, etc.)

### Medium Priority
5. **BTF Support Missing** - Blocks rich debugging and CO-RE
6. **Test Coverage Gaps** - BPF syscall handler has no unit tests

---

## Resources

- **Proposal:** [docs/proposal.md](proposal.md)
- **Architecture:** [.planning/codebase/ARCHITECTURE.md](../.planning/codebase/ARCHITECTURE.md)
- **Work Remaining:** [docs/WORK_REMAINING.md](WORK_REMAINING.md)
- **Task Details:** [docs/tasks.md](tasks.md)

---

**Next Update:** When Phase 0 critical blockers are resolved
