# Roadmap: Axiom

## Overview

Take a kernel with a complete BPF subsystem and wire it into real hardware. Start with proving BPF end-to-end in QEMU, then bring it to RPi5 with progressive hardware demos (GPIO→LED, motor tracing, safety interlock). Benchmark against Linux and package for academic/investor audiences.

## Domain Expertise

None

## Phases

- [ ] **Phase 1: BPF End-to-End** - Prove the full userspace→kernel→execute→output pipeline in QEMU
- [ ] **Phase 2: RPi5 Hardware Demos** - Wire BPF to real GPIO/PWM on RPi5 with progressive demos
- [ ] **Phase 3: Benchmarks & Validation** - Performance benchmarks and comparison vs Linux
- [ ] **Phase 4: Docs & Ecosystem** - Example programs, documentation, academic positioning

## Phase Details

### Phase 1: BPF End-to-End
**Goal**: Load a BPF ELF from userspace via bpf_loader, attach to a kernel event, execute, and see output on serial console. Proves the full pipeline works before touching hardware.
**Depends on**: Nothing (first phase)
**Research**: Unlikely (all components exist, this is internal wiring)
**Plans**: 3 plans

**Key risks:**
- AArch64 userspace boot may need fixing
- Ringbuf kernel→userspace delivery not yet plumbed

Plans:
- [x] 01-01: BPF trace via timer — bpf_loader loads program, trace_printk fires on every tick
- [x] 01-02: Ringbuf userspace delivery — BPF_RINGBUF_POLL syscall + deadlock fix + demo
- [x] 01-03: End-to-end demo — array map counter + ringbuf events + 3 helpers, proven on QEMU

### Phase 2: RPi5 Hardware Demos
**Goal**: Three progressive demos on real RPi5 hardware: GPIO→BPF→LED, PWM motor tracing, safety interlock (interrupt→BPF→hardware, zero userspace dependency).
**Depends on**: Phase 1
**Research**: Unlikely (GPIO/PWM drivers exist in kernel, connecting existing pieces to BPF attach points)
**Plans**: 3 plans

Plans:
- [x] 02-01: Fix deadlock in all hook handlers + GPIO demo built for RPi5
- [ ] 02-02: Wire PWM BPF attach point — motor commands traced with nanosecond timestamps via ringbuf
- [ ] 02-03: Safety interlock demo — limit switch interrupt → BPF → motor emergency stop (no userspace in loop)

### Phase 3: Benchmarks & Validation
**Goal**: Quantitative validation — boot time, memory footprint, BPF load time, interrupt latency. Comparison against minimal Linux. IIO sensor filtering demo.
**Depends on**: Phase 2
**Research**: Likely (need fair comparison methodology against Linux)
**Research topics**: Minimal Linux benchmark methodology for embedded, interrupt latency measurement techniques, fair kernel-to-kernel comparison criteria, IIO subsystem patterns
**Plans**: 2 plans

Plans:
- [ ] 03-01: IIO sensor BPF attach point and filtering demo
- [ ] 03-02: Performance benchmarks suite and Linux comparison

### Phase 4: Docs & Ecosystem
**Goal**: Package everything for external consumption — example BPF programs, getting-started guide, academic positioning for AgenticOS2026/ASPLOS.
**Depends on**: Phase 3
**Research**: Unlikely (documenting existing work)
**Plans**: 2 plans

Plans:
- [ ] 04-01: Example BPF programs library (10+ programs covering all attach types)
- [ ] 04-02: Documentation, getting-started guide, and academic paper outline

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|---------------|--------|-----------|
| 1. BPF End-to-End | 3/3 | Complete | 2026-02-13 |
| 2. RPi5 Hardware Demos | 1/3 | In progress | - |
| 3. Benchmarks & Validation | 0/2 | Not started | - |
| 4. Docs & Ecosystem | 0/2 | Not started | - |
