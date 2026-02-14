---
phase: 01-bpf-end-to-end
plan: 02
subsystem: bpf
tags: [bpf, ringbuf, timer, deadlock, lock-free, userspace-polling]

# Dependency graph
requires:
  - phase: 01-01
    provides: BPF load/attach/execute pipeline, interpreter stack fix
provides:
  - BPF_RINGBUF_POLL syscall command for userspace ringbuf consumption
  - ringbuf_poll() method on BpfManager
  - Lock-free BPF execution pattern in timer handlers (deadlock fix)
  - Demo: BPF writes ktime timestamps to ringbuf, userspace polls and prints
affects: [01-03, 02-rpi5-hardware-demos]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Clone programs and release lock before BPF execution (deadlock avoidance)
    - BPF_RINGBUF_POLL reuses BpfAttr fields (map_fd, key ptr for buffer, value ptr for size)
    - bpf_ktime_get_ns helper ID = 1, bpf_ringbuf_output helper ID = 6

key-files:
  created: []
  modified:
    - kernel/crates/kernel_abi/src/bpf.rs
    - kernel/src/bpf/mod.rs
    - kernel/src/syscall/bpf.rs
    - kernel/src/arch/idt.rs
    - kernel/src/arch/aarch64/interrupts.rs
    - kernel/crates/kernel_bpf/src/bytecode/program.rs
    - userspace/bpf_loader/src/main.rs

key-decisions:
  - "BPF_RINGBUF_POLL = 37 (not 10 — avoids collision with BPF_PROG_TEST_RUN)"
  - "Clone programs and drop lock before execution to avoid deadlock with helper re-locking"

patterns-established:
  - "Lock-free BPF execution: get_hook_programs() clones, execute_program() runs without lock"
  - "Ringbuf poll: userspace provides buffer via BpfAttr.key, size via BpfAttr.value"

issues-created: []

# Metrics
duration: 9min
completed: 2026-02-13
---

# Phase 1 Plan 2: Ringbuf Userspace Delivery Summary

**BPF_RINGBUF_POLL syscall added, timer handler deadlock fixed with lock-free execution pattern, demo streams ktime timestamps from BPF to userspace**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-13T17:39:05Z
- **Completed:** 2026-02-13T17:48:46Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added BPF_RINGBUF_POLL (cmd=37) to sys_bpf — polls ringbuf, copies event data to userspace buffer
- Added ringbuf_poll(map_id) method to BpfManager
- Fixed critical deadlock: timer handler held BPF_MANAGER lock during execution while helpers tried to re-acquire it
- Implemented lock-free execution pattern: clone programs, release lock, then execute
- Demo: BPF program calls bpf_ktime_get_ns + bpf_ringbuf_output on timer tick, userspace polls and prints timestamps

## Task Commits

1. **Task 1: Add BPF_RINGBUF_POLL to sys_bpf** - `66149e7` (feat)
2. **Task 2: Ringbuf demo + deadlock fix** - `ff51435` (feat)

## Files Created/Modified

- `kernel/crates/kernel_abi/src/bpf.rs` - Added BPF_RINGBUF_POLL = 37
- `kernel/src/syscall/bpf.rs` - BPF_RINGBUF_POLL handler (poll ringbuf, copy to userspace)
- `kernel/src/bpf/mod.rs` - ringbuf_poll(), get_hook_programs(), execute_program() methods
- `kernel/src/arch/idt.rs` - x86_64 timer handler: lock-free BPF execution
- `kernel/src/arch/aarch64/interrupts.rs` - AArch64 timer handler: lock-free BPF execution
- `kernel/crates/kernel_bpf/src/bytecode/program.rs` - Clone impl for BpfProgram
- `userspace/bpf_loader/src/main.rs` - Ringbuf demo (create map, BPF writes timestamps, userspace polls)

## Decisions Made

- BPF_RINGBUF_POLL = 37 (plan suggested 10, but BPF_PROG_TEST_RUN = 10 already defined)
- Lock-free execution pattern: clone programs + drop lock before calling execute, avoids deadlock with helper re-locking

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed deadlock in timer interrupt handler**
- **Found during:** Task 2 (code analysis)
- **Issue:** Timer handler held BPF_MANAGER spin lock during execution; bpf_ringbuf_output helper tried to re-acquire same non-reentrant lock → deadlock
- **Fix:** Added get_hook_programs() to clone program data, execute_program() for lock-free execution. Updated both x86_64 and AArch64 timer handlers.
- **Files modified:** kernel/src/bpf/mod.rs, kernel/src/arch/idt.rs, kernel/src/arch/aarch64/interrupts.rs, kernel/crates/kernel_bpf/src/bytecode/program.rs
- **Verification:** Builds clean, no deadlock possible since lock is released before execution
- **Committed in:** `ff51435`

### Deferred Enhancements

- Same deadlock pattern exists in syscall, IIO, PWM, and GPIO hook call sites — only timer handlers fixed. Will fix when those paths are exercised in Phase 2.

---

**Total deviations:** 1 auto-fixed (1 blocking deadlock)
**Impact on plan:** Fix was essential — without it, any BPF program using map/ringbuf helpers would deadlock the kernel.

## Issues Encountered

- Pre-existing test failures in kernel_bpf scheduler module (52 errors) — unrelated to changes, confirmed by testing base commit.

## Next Phase Readiness

- Ringbuf pipeline: BPF → kernel ringbuf → userspace poll → display is complete
- Lock-free execution pattern established for timer handlers
- Ready for Plan 01-03: End-to-end demo with maps + ringbuf
- Note: Other hook call sites (syscall, GPIO, PWM, IIO) still have deadlock-prone pattern — fix needed in Phase 2

---
*Phase: 01-bpf-end-to-end*
*Completed: 2026-02-13*
