# Axiom

## What This Is

Axiom is a runtime-programmable operating system kernel for robotics where kernel behavior is defined by verified, hot-loadable BPF programs. It replaces the frozen, monolithic Linux kernel with a minimal trusted core that allows safe, instant behavior changes without rebuilding or reflashing.

## Core Value

Runtime programmability — the ability to change kernel behavior instantly and safely in the field without rebuilding or reflashing.

## Requirements

### Validated

- ✓ Kernel Core (x86_64, AArch64) — boots on real hardware
- ✓ Physical & Virtual Memory Management — sparse frame allocator, paging
- ✓ Process Scheduling — multi-core task switching
- ✓ VFS + Ext2 Support — basic filesystem operations
- ✓ BPF Subsystem — streaming verifier (O(n)), interpreter, JITs (x86/ARM64), maps
- ✓ Userspace Foundation — ELF loader, init process, minilib

### Active

- [ ] Wire BPF subsystem into running kernel (currently library-only)
- [ ] Implement `bpf()` syscall for userspace interaction
- [ ] Create timer interrupt attach point for BPF programs
- [ ] Demonstrate end-to-end BPF program execution (userspace load -> kernel execute -> output)

### Out of Scope

- Full POSIX compliance (aiming for "POSIX-ish" subset)
- GUI or Desktop Environment support
- Complex BPF features: Tail calls, BPF-to-BPF calls, spin locks (explicitly unsupported)
- Legacy hardware support (focus on modern robotics targets like RPi5)

## Context

**Technical Environment:**
- Built from scratch in Rust
- Target hardware: Raspberry Pi 5 (AArch64), x86_64 (QEMU/PC)
- BPF programs used for drivers, schedulers, and safety interlocks

**Problem:**
- Linux kernels are "frozen" in robotics; changing them risks bricking devices.
- Debugging requires rebuild/reflash cycles.
- Axiom solves this by making the kernel programmable at runtime via verified bytecode.

## Constraints

- **Language**: Rust — for memory safety in the core.
- **Footprint**: <10MB kernel image — to fit embedded constraints.
- **Verification**: All loaded programs MUST pass the streaming verifier.
- **Safety**: No arbitrary memory access; bounded loops only.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Streaming Verifier | O(n) memory usage required for embedded (vs O(nodes) in Linux) | — Pending |
| Programs as Primitives | Everything (drivers, schedulers) should be BPF programs | — Pending |
| No Libbpf | Custom ELF loader/relocator to keep footprint small | — Pending |

---
*Last updated: 2026-02-07 after initialization*
