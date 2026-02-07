# Roadmap: Axiom

## Overview

Axiom is building a runtime-programmable kernel for robotics where behavior is defined by verified BPF programs. This roadmap moves from the current state (a library of BPF components and a basic kernel) to a fully integrated system running on Raspberry Pi 5 hardware, demonstrating real-world robotics use cases like safety interlocks and motor control observation.

## Domain Expertise

None

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: BPF Integration** - Wire BPF subsystem, syscalls, and timer attach point
- [ ] **Phase 2: Hardware Attach** - GPIO and PWM attach points for RPi5
- [ ] **Phase 3: Validation** - IMU integration and safety interlock demo
- [ ] **Phase 4: Ecosystem** - Documentation and example library

## Phase Details

### Phase 1: BPF Integration
**Goal**: Wire BPF subsystem into running kernel (currently library-only), implement `bpf()` syscall, and demonstrate end-to-end execution.
**Depends on**: Nothing (Foundation)
**Research**: Unlikely (components exist, integration work)
**Plans**: TBD

### Phase 2: Hardware Attach
**Goal**: Implement GPIO and PWM attach points for Raspberry Pi 5 to enable hardware interaction.
**Depends on**: Phase 1
**Research**: Likely (RPi5 hardware registers)
**Research topics**: RPi5 GPIO/PWM register maps, interrupt routing for BPF
**Plans**: TBD

### Phase 3: Validation
**Goal**: Demonstrate real-world value with IMU integration and kernel-level safety interlocks.
**Depends on**: Phase 2
**Research**: Likely (IMU driver specifics)
**Research topics**: IMU sensor communication protocols (I2C/SPI), safety logic patterns
**Plans**: TBD

### Phase 4: Ecosystem
**Goal**: Make the system usable by others with documentation and a library of example programs.
**Depends on**: Phase 3
**Research**: Unlikely
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. BPF Integration | 0/0 | Not started | - |
| 2. Hardware Attach | 0/0 | Not started | - |
| 3. Validation | 0/0 | Not started | - |
| 4. Ecosystem | 0/0 | Not started | - |
