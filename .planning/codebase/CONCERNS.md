# Codebase Concerns

**Analysis Date:** 2026-01-21

## Tech Debt

**External tool dependency for disk image creation:**
- Issue: Uses external `mke2fs` command instead of Rust crate
- Files: `build.rs` line 44-60
- Why: Rapid prototyping, mkfs-ext2 crate not ready
- Impact: Build depends on e2fsprogs system package, not portable
- Fix approach: Use mkfs-ext2 crate once stable

**Incomplete memory deallocation:**
- Issue: Physical memory frames not deallocated on process exit
- Files: `kernel/src/mcore/mtask/process/mem.rs` lines 121, 148, 160 (three `todo!()` calls)
- Files: `kernel/src/mcore/mtask/process/mod.rs` line 235
- Why: Memory management implementation in progress
- Impact: Memory leak on process termination
- Fix approach: Implement proper frame deallocation in memory region cleanup

**Unnecessary Arc clone in syscall path:**
- Issue: Process cloned instead of borrowed in syscall access
- Files: `kernel/src/syscall/access.rs` line 26
- Why: Convenience during initial implementation
- Impact: Performance overhead on every syscall
- Fix approach: Refactor to use reference instead of clone

## Known Bugs

**Page fault handler infinite loop:**
- Symptoms: Kernel hangs on page fault instead of terminating process
- Trigger: Any page fault in userspace
- Files: `kernel/src/arch/idt.rs` lines 230, 271
- Workaround: None (kernel hangs)
- Root cause: Signal handling not implemented, FIXME comments present
- Fix: Implement SIGSEGV delivery or process termination

**Race condition in subscription updates:**
- Symptoms: Not observed yet (commented as potential issue)
- Trigger: Unknown
- Files: Not pinpointed
- Root cause: Multiple RwLocks in process structure could contend

## Security Considerations

**Minimal userspace pointer validation:**
- Risk: Syscalls accept userspace pointers with limited bounds checking
- Files: `kernel/src/syscall/mod.rs` lines 19-24, 94, 102, 110, 125
- Current mitigation: Basic null checks
- Recommendations: Implement comprehensive bounds checking, consider SMAP/SMEP equivalents

**Unsafe code without full documentation:**
- Risk: Some unsafe blocks lack comprehensive safety comments
- Files: `kernel/crates/kernel_vfs/src/path/mod.rs` line 17, `kernel/src/arch/idt.rs` lines 78-81
- Current mitigation: Code review
- Recommendations: Add safety comments to all unsafe blocks per CONTRIBUTING.md requirements

## Performance Bottlenecks

**VFS path lookup:**
- Problem: File systems stored in BTreeMap
- Files: `kernel/crates/kernel_vfs/src/vfs/mod.rs` line 23
- Measurement: Not profiled
- Cause: BTreeMap less efficient than trie for path lookups
- Improvement path: Consider trie data structure for mount points

**Memory mapping inefficiency:**
- Problem: Only 4KiB frames used for memory mapping
- Files: `kernel/src/syscall/access/mem.rs` line 49
- Measurement: Not profiled
- Cause: 2MiB and 1GiB frame support not implemented
- Improvement path: Add huge page support for large allocations

## Fragile Areas

**Interrupt descriptor table (IDT):**
- Why fragile: Complex exception handlers with multiple code paths
- Files: `kernel/src/arch/idt.rs` (419 lines)
- Common failures: Page fault handling incomplete, infinite loops
- Safe modification: Add comprehensive tests before changes
- Test coverage: Not unit testable (bare-metal)

**BPF verifier:**
- Why fragile: Complex static analysis with many edge cases
- Files: `kernel/crates/kernel_bpf/src/verifier/core.rs` (799 lines)
- Common failures: Missed edge cases in control flow analysis
- Safe modification: Add test cases for each verification rule
- Test coverage: Has tests but not comprehensive

**Process creation:**
- Why fragile: Multiple subsystem interactions (memory, VFS, ELF loader)
- Files: `kernel/src/mcore/mtask/process/mod.rs` (395 lines)
- Common failures: Resource cleanup on failure
- Safe modification: Add error handling for each step
- Test coverage: Not unit testable (bare-metal)

## Scaling Limits

**Physical memory manager:**
- Current capacity: Depends on available RAM
- Limit: Linear search for free frames across regions
- Files: `kernel/crates/kernel_physical_memory/src/lib.rs` line 130
- Symptoms at limit: Slow allocation with many regions
- Scaling path: Implement region boundary crossing for better utilization

## Dependencies at Risk

**mkfs-ext2 (git dependency):**
- Risk: External git dependency from `https://github.com/tsatke/mkfs`
- Files: `Cargo.toml` lines 75-76
- Impact: Build breaks if repository unavailable
- Migration plan: Wait for crates.io release or fork

## Missing Critical Features

**ext2 write support:**
- Problem: Filesystem is read-only
- Files: `kernel/src/file/ext2.rs` line 102 (`todo!()`)
- Current workaround: Pre-populate disk image at build time
- Blocks: Any userspace file modification
- Implementation complexity: Medium (ext2 write support well-documented)

**Signal handling:**
- Problem: No POSIX signal delivery mechanism
- Files: `kernel/src/arch/idt.rs` lines 230, 271 (FIXME comments)
- Current workaround: Kernel hangs on fatal errors
- Blocks: Proper process termination, job control
- Implementation complexity: High (requires userspace stack manipulation)

**Symlink support:**
- Problem: Symbolic links not implemented in ext2 or VFS
- Files: `kernel/src/file/ext2.rs` line 148 (`todo!()`)
- Current workaround: None
- Blocks: Standard Unix filesystem operations
- Implementation complexity: Low-Medium

**Demand paging:**
- Problem: No lazy page allocation
- Files: `kernel/src/arch/aarch64/exceptions.rs` line 178, `kernel/src/arch/riscv64/trap.rs` line 109
- Current workaround: Allocate all pages upfront
- Blocks: Efficient memory usage
- Implementation complexity: Medium

**Copy-on-Write (COW):**
- Problem: Fork would require full memory copy
- Files: `kernel/src/arch/aarch64/exceptions.rs` line 199, `kernel/src/arch/riscv64/trap.rs` line 109
- Current workaround: Not applicable (no fork yet)
- Blocks: Efficient process forking
- Implementation complexity: Medium-High

## Test Coverage Gaps

**Kernel core (bare-metal):**
- What's not tested: Main kernel code (`kernel/src/`)
- Risk: Regressions in boot, interrupt handling, syscalls
- Priority: High
- Difficulty to test: Requires custom test harness or QEMU-based testing

**Platform-specific code:**
- What's not tested: AArch64 and RISC-V implementations
- Files: `kernel/src/arch/aarch64/`, `kernel/src/arch/riscv64/`
- Risk: Broken builds on non-x86 platforms
- Priority: Medium
- Difficulty to test: Requires cross-compilation and emulation

**Syscall handlers:**
- What's not tested: Syscall implementations
- Files: `kernel/src/syscall/`
- Risk: ABI breakage, security vulnerabilities
- Priority: High
- Difficulty to test: Requires userspace test programs

## TODO/FIXME Summary

**High Priority (Critical Path):**
- Memory deallocation: 3+ `todo!()` in process/mem.rs
- Signal handling: 2 FIXME in idt.rs
- ext2 write: `todo!()` in ext2.rs

**Medium Priority (Functionality):**
- Symlinks, absolute paths, parent traversal in ext2.rs
- Huge page support in memory mapping
- Device tree parsing improvements

**Low Priority (Optimization):**
- BTreeMap to trie for VFS
- Remove unnecessary Arc clones
- Cross-region memory allocation

---

*Concerns audit: 2026-01-21*
*Update as issues are fixed or new ones discovered*
