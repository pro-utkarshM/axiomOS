# Axiom: Runtime-Programmable Kernel for Robotics

## What This Is

A bare-metal operating system kernel where runtime programmability is the foundation. Built from scratch in Rust with BPF-style verified programs as first-class kernel primitives. Targets Raspberry Pi 5 as primary hardware platform, with x86_64 QEMU as the development environment.

## Core Value

**A button press on RPi5 triggers a verified BPF program in the kernel that controls hardware — and you can change that program without rebuilding or reflashing the kernel.**

## Requirements

### Validated

- ✓ Bootable kernel on x86_64 and AArch64 — existing
- ✓ Physical and virtual memory management — existing
- ✓ Process model with fork/exec/waitpid — existing
- ✓ VFS with ext2 and devfs — existing
- ✓ Syscall dispatch (8+ syscalls implemented) — existing
- ✓ ELF loader for userspace binaries — existing
- ✓ BPF subsystem: streaming verifier, interpreter, ARM64 JIT — existing
- ✓ BPF maps: array, hash, ringbuf, timeseries — existing
- ✓ BPF ELF loader and signing (Ed25519 + SHA3) — existing
- ✓ BPF cloud/embedded compile-time profiles — existing
- ✓ BPF Manager with load/attach/execute/hooks — existing
- ✓ sys_bpf syscall (map create, prog load, attach, detach) — existing
- ✓ Timer interrupt BPF attach point — existing
- ✓ RPi5 platform support (GPIO, PWM, UART drivers) — existing
- ✓ Multi-architecture (x86_64, AArch64) — existing
- ✓ Work-stealing scheduler — existing
- ✓ Limine bootloader (x86_64), raw binary boot (AArch64/RPi5) — existing
- ✓ CI/CD with GitHub Actions (lint, test, Miri, build) — existing

### Active

- [ ] End-to-end BPF from userspace: load BPF ELF via bpf_loader, execute on event, output visible
- [ ] GPIO BPF attach point wired to real RP1 hardware (button → BPF → LED)
- [ ] PWM BPF observation (motor commands traced via BPF with nanosecond timestamps)
- [ ] Safety interlock demo (limit switch → BPF → motor emergency stop, cannot bypass from userspace)
- [ ] BPF helper functions for GPIO control (bpf_gpio_set), motor stop (bpf_motor_emergency_stop)
- [ ] BPF ringbuf output from kernel to userspace (event stream)
- [ ] IIO sensor filtering at kernel level via BPF
- [ ] Performance benchmarks: boot time, memory footprint, BPF load time, interrupt latency
- [ ] Comparison benchmarks vs minimal Linux
- [ ] Example BPF programs library (10+ programs)
- [ ] User-facing documentation and getting-started guide
- [ ] Academic positioning for AgenticOS2026 / ASPLOS

### Out of Scope

- RISC-V completion — experimental/demo only, not production target for this milestone
- Demand paging / CoW / signals — not required for RPi5 BPF demos, harden later
- Real-time guarantees — future work, not claiming RT in v1
- Network boot / OTA updates — scripted SD card deploy is sufficient for now
- Formal verification of BPF verifier — research contribution, not implementation target
- Multi-node / distributed deployment — single device focus
- Full POSIX compliance — minimal syscall set for demos

## Context

**Brownfield project.** ~25K lines of Rust kernel code already built. The kernel boots on real hardware (RPi5) and in QEMU. The BPF subsystem is complete as a library but the integration gap remains: BPF programs need to be loadable from userspace, attached to real hardware events, and executed with visible output.

The immediate path is:
1. Wire BPF into running kernel events (timer already done, GPIO/PWM next)
2. Build progressive demos on RPi5: GPIO→LED, motor tracing, safety interlock
3. Benchmark and document for academic/investor audiences

Hardware available: Raspberry Pi 5 with basic I/O (buttons, LEDs, breadboard). Motor controller and sensors to be sourced for advanced demos.

Deployment: `scripts/deploy-rpi5.sh` builds kernel8.img and flashes to SD card boot partition. Network boot planned later for faster iteration.

## Constraints

- **Hardware**: RPi5 (8GB) as primary target — all demos must run on this
- **Toolchain**: Rust nightly required (no_std, bare-metal targets)
- **BPF Profiles**: Must select cloud-profile or embedded-profile at build time (mutually exclusive)
- **Testing**: Main kernel binary not unit-testable (bare-metal linker); logic must be in workspace crates
- **Boot**: Limine for x86_64, raw kernel8.img for RPi5 — different boot paths per architecture

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| RPi5 as primary demo target | Real hardware proves thesis better than QEMU | — Pending |
| Skip demand paging/CoW/signals for now | Not required for BPF demo path; avoid scope creep | — Pending |
| RISC-V out of scope | Focus resources on x86_64 (dev) + AArch64 (demo) | — Pending |
| BPF wiring before kernel hardening | Demo value > robustness for current milestone | — Pending |
| Progressive demo approach (GPIO→PWM→safety) | Each demo builds on previous, de-risks incrementally | — Pending |
| SD card deploy for now | Simple and reliable; network boot is optimization | — Pending |

---
*Last updated: 2026-02-13 after initialization*
