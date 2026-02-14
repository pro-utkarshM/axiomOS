# Testing Patterns

**Analysis Date:** 2026-02-13

## Test Framework

**Runner:**
- Standard Rust test framework (host-based)
- Kernel binary cannot be unit-tested (bare-metal linker); logic extracted to testable crates

**Assertion Library:**
- Built-in `assert!`, `assert_eq!`, `assert!(matches!())`

**Benchmarks:**
- Criterion 0.5 — `kernel/crates/kernel_bpf/Cargo.toml`

**UB Detection:**
- Miri (undefined behavior detection) — `.github/workflows/build.yml`

**Run Commands:**
```bash
cargo test                              # All tests across workspace
cargo test -p kernel_bpf                # Single crate
cargo test --test bpf_integration       # Single integration test
cargo test --release                    # Release mode
cargo bench -p kernel_bpf              # All BPF benchmarks
cargo bench --bench interpreter         # Single benchmark
cargo miri test -p kernel_bpf          # Miri UB detection
```

## Test File Organization

**Location:**
- Integration tests: `kernel/crates/kernel_bpf/tests/*.rs`
- Unit tests: `#[cfg(test)] mod tests` within source files
- Benchmarks: `kernel/crates/kernel_bpf/benches/*.rs`
- VFS test helpers: `kernel/crates/kernel_vfs/src/vfs/testing.rs`

**Testable Crates:**
- `kernel_abi`, `kernel_bpf`, `kernel_vfs`, `kernel_syscall`
- `kernel_devfs`, `kernel_elfloader`, `kernel_physical_memory`, `kernel_virtual_memory`

**Non-Testable:**
- `kernel` (main binary) — bare-metal linker script incompatible with test harness

## Test Structure

**Integration Test Organization** (`kernel/crates/kernel_bpf/tests/bpf_integration.rs`):
```rust
// Setup helpers at module level
fn interpreter() -> Interpreter<ActiveProfile> {
    Interpreter::new()
}

// Test stubs for external C functions (~90 lines)
#[unsafe(no_mangle)]
pub extern "C" fn bpf_ktime_get_ns() -> u64 { 0 }

// Organized in submodules by feature
mod program_lifecycle { ... }
mod arithmetic_operations { ... }
mod control_flow { ... }
mod memory_operations { ... }
mod map_operations { ... }
```

**Integration Test Files:**
- `bpf_integration.rs` - Core BPF (program lifecycle, arithmetic, control flow, memory, maps)
- `gpio_integration.rs` - GPIO hardware attachment
- `pwm_integration.rs` - PWM control and observation
- `profile_contracts.rs` - Profile-specific constraints
- `semantic_consistency.rs` - Cross-profile semantic validation

**Unit Test Pattern:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_type_availability() { ... }

    #[test]
    fn attach_config_creation() { ... }
}
```

**Patterns:**
- Builder pattern for test program creation (ProgramBuilder)
- Test stubs for FFI functions (bpf_ktime_get_ns, etc.)
- Submodule organization by feature area
- `assert!(matches!())` for enum variant checking

## Benchmarks

**Criterion Configuration** (`kernel/crates/kernel_bpf/benches/`):

1. **interpreter.rs** - BPF execution performance
   - Arithmetic operations (simple_math)
   - Loop execution (10, 100, 1000 iterations)
   - Conditional jumps

2. **verifier.rs** - Verification performance

3. **maps.rs** - Map operation performance

**Pattern:**
```rust
fn bench_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter/arithmetic");
    let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
        .insn(BpfInsn::mov64_imm(0, 0))
        .insn(BpfInsn::exit())
        .build()
        .expect("valid program");
    group.bench_function("simple_math", |b| {
        b.iter(|| interp.execute(black_box(&program), black_box(&ctx)))
    });
}
```

## Test Data

**Factory Pattern:**
```rust
// In test file
fn interpreter() -> Interpreter<ActiveProfile> {
    Interpreter::new()
}

// Program construction via builder
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 42))
    .insn(BpfInsn::exit())
    .build()
    .expect("valid program");
```

**VFS Test Helper:**
- `TestFs` struct in `kernel/crates/kernel_vfs/src/vfs/testing.rs` for mocking filesystem

## Coverage

**Requirements:**
- No enforced coverage target
- Focus on BPF subsystem (highest test density)
- Miri runs per-crate in CI for undefined behavior detection

**Gaps:**
- Fork/exec/waitpid only smoke-tested via `userspace/fork_test/`
- Page fault handling untested
- BPF kernel integration (attachment lifecycle) not integration-tested
- No stress tests for concurrent scenarios

## CI/CD Integration

**GitHub Actions Pipeline** (`.github/workflows/build.yml`):
1. **Lint** - rustfmt check + clippy with `-D clippy::all`
2. **Test** - Debug + Release modes
3. **Miri** - UB detection on kernel crates
4. **Build** - Full bootable ISO creation

**BPF Profile CI** (`.github/workflows/bpf-profiles.yml`):
- Cloud and embedded profile separate testing
- Mutual exclusion verification
- Semantic consistency validation

**Pre-Commit Workflow:**
```bash
cargo fmt -- --check          # Format check
cargo clippy -- -D clippy::all # Lint
cargo build                    # Build
cargo test                     # Tests
cargo miri test -p <crate>    # Optional: Miri
```

---

*Testing analysis: 2026-02-13*
*Update when test patterns change*
