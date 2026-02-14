# Codebase Concerns

**Analysis Date:** 2026-02-13

## Tech Debt

**Nanosleep Busy-Waits:**
- Issue: `sys_nanosleep` uses CPU-burning busy loop instead of proper sleep queues
- Files: `kernel/src/syscall/mod.rs` lines 574-587
- Why: Wait queues not yet implemented
- Impact: Poor power efficiency, wasted CPU cycles for sleeping processes
- Fix approach: Implement wait queue subsystem, block task until timer fires

**BPF Manager Single Mutex:**
- Issue: All BPF operations (map access, loading, attachment) serialized behind one Mutex
- Files: `kernel/src/lib.rs` line 57 (`BPF_MANAGER: OnceCell<Mutex<BpfManager>>`)
- Why: Simple initial implementation
- Impact: Multi-core systems serialize all BPF operations
- Fix approach: Fine-grained locking per program/map, or lock-free data structures

**BPF Syscall Error Context Lost:**
- Issue: All BPF syscall errors mapped to `-1` regardless of error type
- Files: `kernel/src/syscall/bpf.rs` lines 22, 32, 64, 72, 82, 87, 93
- Why: Quick implementation without error mapping
- Impact: Userspace cannot distinguish bad argument vs out of memory vs permission denied
- Fix approach: Map BPF error variants to specific errno values

**Memory Deallocation Incomplete on Process Exit:**
- Issue: AddressSpace doesn't strictly track ownership of frames for Drop
- Files: `kernel/src/mem/address_space/mod.rs` line 624 (TODO comment)
- Why: Not yet implemented
- Impact: Long-running systems with process churn may leak physical memory
- Fix approach: Implement frame ownership tracking in AddressSpace Drop

## Known Bugs

**Stack Isolation FIXME:**
- Symptoms: Task stacks in higher half may be writable by unrelated tasks
- Files: `kernel/src/mcore/mtask/task/stack.rs` line 232
- Workaround: None documented
- Root cause: Stack allocated with WRITABLE permissions without full isolation validation
- Blocked by: Requires lower-half stack mapping or additional page table isolation

**sp_el0 Context Switch Ambiguity (AArch64):**
- Symptoms: User stack pointer may not be properly saved/restored during all task switches
- Files: `kernel/src/arch/aarch64/context.rs` lines 88-103
- Workaround: Works correctly for exception entry/return paths
- Root cause: sp_el0 handling during kernel-to-kernel context switches not fully documented/verified

## Security Considerations

**BPF Signature Verification Not Wired:**
- Risk: Unsigned/malicious BPF programs can be loaded without verification
- Files: `kernel/crates/kernel_bpf/src/signing/` exists but `kernel/src/syscall/bpf.rs` doesn't call it
- Current mitigation: None (any valid BPF bytecode accepted)
- Recommendations: Call signature verify() before program loading in sys_bpf

**BPF Attribute Field Validation Minimal:**
- Risk: Only checks `size >= sizeof(BpfAttr)`; no field-specific validation
- Files: `kernel/src/syscall/bpf.rs` lines 14-23
- Current mitigation: Size check only
- Recommendations: Add per-field bounds checking and validation

**Userspace Pointer Validation Not Enforced:**
- Risk: Callers must remember to validate userspace pointers; no compile-time enforcement
- Files: `kernel/src/syscall/validation.rs` (helpers exist but usage is opt-in)
- Current mitigation: Validation helpers available
- Recommendations: Newtype wrapper for userspace pointers requiring validation before dereference

## Performance Bottlenecks

**BPF Manager Lock Contention:**
- Problem: Single Mutex for all BPF operations
- Files: `kernel/src/lib.rs` line 57
- Cause: Coarse-grained locking
- Improvement: Per-program/per-map locks or RwLock for read-heavy operations

## Fragile Areas

**AArch64 Exception Context Offsets:**
- Why fragile: Hard-coded offsets (272, 248, 256, 232) in naked assembly for restore_user_context
- Files: `kernel/src/arch/aarch64/context.rs` lines 308-334
- Common failures: Any modification to ExceptionContext struct silently corrupts restore
- Safe modification: Add compile-time offset assertions, or use offset_of!() macro
- Test coverage: No automated verification of offset correctness

**Boot Sequence Panics:**
- Why fragile: Multiple unwrap()/expect() on boot-critical operations
- Files: `kernel/src/main.rs` lines 63, 70, 78-80, 112, 119, 127-129
- Common failures: Missing /bin/init or block device causes kernel panic
- Safe modification: Add graceful fallback or diagnostic panic screen
- Test coverage: Not testable (bare-metal)

**BPF Profile Mutual Exclusion:**
- Why fragile: Must select exactly one of cloud-profile/embedded-profile at build time
- Files: `kernel/crates/kernel_bpf/src/lib.rs` lines 90-100
- Common failures: Build fails if both/neither selected
- Safe modification: CI enforces correctness — `.github/workflows/bpf-profiles.yml`

## Missing Critical Features

**Demand Paging:**
- Problem: User page faults panic instead of allocating pages
- Files: `kernel/src/arch/aarch64/exceptions.rs` line 213, `kernel/src/arch/idt.rs` lines 369, 381
- Current workaround: All pages pre-allocated at process creation
- Blocks: Dynamic heap growth, stack growth, memory-mapped files
- Implementation: Medium-high complexity

**Copy-on-Write (CoW):**
- Problem: fork() cannot share pages with CoW semantics
- Files: `kernel/src/arch/aarch64/exceptions.rs` line 234
- Current workaround: Full page copy on fork (expensive)
- Blocks: Efficient multi-process workloads
- Implementation: Medium complexity (requires page fault handler integration)

**Signals:**
- Problem: No POSIX signal delivery mechanism
- Files: `kernel/src/arch/idt.rs` lines 342, 383 ("FIXME: once we have signals, trigger SIGSEGV")
- Current workaround: Page faults panic the kernel
- Blocks: Graceful process termination, error recovery
- Implementation: High complexity (signal stacks, handlers, delivery)

**BPF Stack Usage Calculation:**
- Problem: Programs loaded with 0 stack usage assumption
- Files: `kernel/src/bpf/mod.rs` line 52 ("TODO: Calculate stack usage via Verifier")
- Current workaround: None (stack overflow possible)
- Blocks: Safe BPF execution on embedded profile (8KB static pool)

**Process Group Support:**
- Problem: waitpid() doesn't support POSIX process groups
- Files: `kernel/src/syscall/process.rs` lines 112-113
- Current workaround: Wait for specific PID only
- Blocks: Shell job control

**Fork argc/argv Passing:**
- Problem: execve doesn't pass argc/argv to new process
- Files: `kernel/src/syscall/process.rs` line 56 ("TODO: pass argc/argv")
- Current workaround: Programs don't use command-line arguments

## Incomplete Platform Support

**RISC-V:**
- Status: Boot works, most subsystems non-functional
- Files: `kernel/src/arch/riscv64/` — interrupts, paging, trap handling all have TODOs
- Impact: Cannot run userspace applications on RISC-V
- Specific gaps: PLIC (interrupt controller), page tables, lazy memory, syscall dispatch

**RPi5 Hardware Drivers:**
- Status: GPIO and PWM drivers exist but only enabled with `feature = "rpi5"`
- Files: `kernel/src/arch/aarch64/platform/rpi5/`
- Impact: Hardware features gated behind build-time feature flag

## Test Coverage Gaps

**Kernel Integration (BPF attachment lifecycle):**
- What's not tested: BPF program loading via syscall, attachment to kernel events, execution in context
- Risk: BPF wiring into kernel could break without detection
- Priority: High
- Difficulty: Requires kernel-level test harness or QEMU-based testing

**Fork/Exec/Waitpid:**
- What's not tested: Only basic smoke test via `userspace/fork_test/`
- Risk: Edge cases in process creation, memory copying, zombie reaping
- Priority: Medium

**Page Fault Handling:**
- What's not tested: No automated tests for page fault paths
- Risk: Demand paging and CoW changes could introduce regressions
- Priority: Medium (currently panics, so limited blast radius)

**Concurrent/Stress Scenarios:**
- What's not tested: High-frequency BPF execution, large process trees, memory exhaustion
- Risk: Race conditions and deadlocks under load
- Priority: Medium
- Difficulty: Requires multi-threaded test framework or QEMU scripting

---

*Concerns audit: 2026-02-13*
*Update as issues are fixed or new ones discovered*
