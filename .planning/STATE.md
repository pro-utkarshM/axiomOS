# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-13)

**Core value:** A button press on RPi5 triggers a verified BPF program in the kernel that controls hardware — and you can change that program without rebuilding or reflashing the kernel.
**Current focus:** Phase 1 complete — ready for Phase 2

## Current Position

Phase: 2 of 4 (RPi5 Hardware Demos)
Plan: 1 of 3 in current phase
Status: In progress
Last activity: 2026-02-13 — Completed 02-01-PLAN.md

Progress: ████░░░░░░ 40%

## Performance Metrics

**Velocity:**
- Total plans completed: 4
- Average duration: 12 min
- Total execution time: 0.8 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. BPF End-to-End | 3/3 | 41 min | 14 min |
| 2. RPi5 Hardware | 1/3 | 8 min | 8 min |

**Recent Trend:**
- Last 5 plans: 21min, 9min, 11min, 8min
- Trend: Stable/improving

## Accumulated Context

### Decisions

- Helper IDs: trace_printk=2, ktime_get_ns=1, ringbuf_output=6, map_lookup=3, map_update=4
- Stack offset: stack[stack.len() + offset] for r10 pointer semantics
- BPF_RINGBUF_POLL = 37
- Lock-free execution: clone programs, drop lock, execute (avoids helper deadlock)
- Generic pointer load/store in interpreter for map value access

### Deferred Issues

- ~~GPIO/PWM/IIO/syscall hook handlers deadlock~~ — RESOLVED in 02-01
- Process exit panic (address space mapper assertion) — not BPF-related, investigate later
- Pre-existing kernel_bpf scheduler test failures (52 errors)

### Blockers/Concerns

None blocking Phase 2.

## Session Continuity

Last session: 2026-02-13
Stopped at: Completed 02-01-PLAN.md (deadlock fix + GPIO demo)
Resume file: None
