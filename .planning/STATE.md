# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-13)

**Core value:** A button press on RPi5 triggers a verified BPF program in the kernel that controls hardware — and you can change that program without rebuilding or reflashing the kernel.
**Current focus:** Phase 1 — BPF End-to-End

## Current Position

Phase: 1 of 4 (BPF End-to-End)
Plan: 2 of 3 in current phase
Status: In progress
Last activity: 2026-02-13 — Completed 01-02-PLAN.md

Progress: ██░░░░░░░░ 20%

## Performance Metrics

**Velocity:**
- Total plans completed: 2
- Average duration: 15 min
- Total execution time: 0.5 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. BPF End-to-End | 2/3 | 30 min | 15 min |

**Recent Trend:**
- Last 5 plans: 21min, 9min
- Trend: Improving

## Accumulated Context

### Decisions

- Plan 01-01: bpf_trace_printk helper ID is 2, bpf_ktime_get_ns is 1, bpf_ringbuf_output is 6
- Plan 01-01: Stack offset fix — stack[stack.len() + offset] aligns with r10 pointer semantics
- Plan 01-02: BPF_RINGBUF_POLL = 37 (avoids collision with existing cmd 10)
- Plan 01-02: Lock-free execution: clone programs, drop lock, then execute (deadlock avoidance)

### Deferred Issues

- Other hook call sites (syscall, GPIO, PWM, IIO) still use deadlock-prone execute_hooks() pattern — fix in Phase 2

### Blockers/Concerns

- ~~AArch64 userspace boot~~ — RESOLVED
- ~~Ringbuf kernel→userspace delivery~~ — RESOLVED (01-02)
- Pre-existing kernel_bpf scheduler test failures (52 errors) — unrelated to our changes

## Session Continuity

Last session: 2026-02-13
Stopped at: Completed 01-02-PLAN.md (ringbuf userspace delivery)
Resume file: None
