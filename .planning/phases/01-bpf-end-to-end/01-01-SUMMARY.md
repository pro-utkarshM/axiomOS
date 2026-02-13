---
phase: 01-bpf-end-to-end
plan: 01
subsystem: bpf
tags: [bpf, trace_printk, timer, interpreter, userspace, qemu]

# Dependency graph
requires: []
provides:
  - BPF program loading from userspace via sys_bpf
  - Timer attach point executing BPF programs on every tick
  - bpf_loader userspace binary in disk image
  - Fixed interpreter stack offset calculation
affects: [01-02, 01-03, 02-rpi5-hardware-demos]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - BPF bytecode construction in userspace via BpfInsn arrays
    - sys_bpf(BPF_PROG_LOAD) + sys_bpf(BPF_PROG_ATTACH) pattern for load-then-attach
    - bpf_trace_printk helper ID = 2 in interpreter dispatch

key-files:
  created: []
  modified:
    - userspace/bpf_loader/src/main.rs
    - userspace/file_structure/src/lib.rs
    - userspace/init/src/main.rs
    - Cargo.toml
    - kernel/crates/kernel_bpf/src/execution/interpreter.rs

key-decisions:
  - "bpf_trace_printk helper ID is 2 (not 6 as initially assumed)"
  - "Stack offset fix: stack[stack.len() + offset] aligns with r10 + offset addressing"

patterns-established:
  - "BPF programs constructed as BpfInsn arrays in userspace, loaded via raw bytecode (cmd=5)"
  - "init spawns bpf_loader via execve(/bin/bpf_loader)"

issues-created: []

# Metrics
duration: 21min
completed: 2026-02-13
---

# Phase 1 Plan 1: BPF Trace via Timer Summary

**BPF program loaded from userspace fires bpf_trace_printk on every timer tick — 4073 traces in 30s of QEMU runtime, plus critical interpreter stack offset fix**

## Performance

- **Duration:** 21 min
- **Started:** 2026-02-13T17:13:49Z
- **Completed:** 2026-02-13T17:35:38Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Enhanced bpf_loader to construct 12-instruction BPF program with LD_DW_IMM, STX, MOV64, ADD64, CALL, EXIT
- BPF program calls bpf_trace_printk helper to output "BPF tick!" on every timer interrupt
- Added bpf_loader to disk image via file_structure and artifact dependencies
- Fixed critical interpreter stack offset calculation bug (data and pointers were at different locations)
- Full pipeline proven: userspace load → kernel verify → timer attach → interrupt execute → serial output

## Task Commits

Each task was committed atomically:

1. **Task 1: Enhance bpf_loader with trace_printk program** - `b0b8186` (feat)
2. **Task 2: Wire into disk image, fix interpreter, verify on QEMU** - `2048053` (feat)

## Files Created/Modified

- `userspace/bpf_loader/src/main.rs` - 12-instruction BPF program with trace_printk, load + attach logic
- `userspace/file_structure/src/lib.rs` - Added bpf_loader to disk image STRUCTURE
- `Cargo.toml` - Added bpf_loader artifact dependencies for x86 and aarch64
- `userspace/init/src/main.rs` - Changed init to spawn /bin/bpf_loader
- `kernel/crates/kernel_bpf/src/execution/interpreter.rs` - Fixed stack offset calculation

## Decisions Made

- bpf_trace_printk helper number is 2 (not 6 as plan context suggested) — verified from interpreter dispatch table
- Stack offset formula changed from `-(offset + size)` to `stack.len() + offset` to align with r10 pointer semantics

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed interpreter stack offset calculation**
- **Found during:** Task 2 (QEMU verification)
- **Issue:** BPF program stored string on stack but trace_printk printed empty — stack store at `-(offset + size)` didn't match helper pointer at `stack.len() + offset`
- **Fix:** Changed both execute_load and execute_store to use `stack.len() + offset`
- **Files modified:** `kernel/crates/kernel_bpf/src/execution/interpreter.rs`
- **Verification:** All 254 existing kernel_bpf tests pass, trace output now shows "BPF tick!"
- **Committed in:** `2048053`

### Deferred Enhancements

None.

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Fix was essential — without it, no BPF program passing stack pointers to helpers can work. This unblocks the entire BPF pipeline.

## Issues Encountered

None beyond the stack offset fix documented above.

## Next Phase Readiness

- BPF load/attach/execute pipeline fully proven on x86_64 QEMU
- Timer attach point fires on every tick with visible serial output
- Ready for Plan 01-02: Ringbuf userspace delivery
- Note: AArch64 verification not yet done (x86_64 only) — can verify in Phase 2

---
*Phase: 01-bpf-end-to-end*
*Completed: 2026-02-13*
