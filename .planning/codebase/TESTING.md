# Testing Patterns

**Analysis Date:** 2026-01-21

## Test Framework

**Runner:**
- Rust standard library `#[test]` with cargo test
- No external test framework

**Assertion Library:**
- Built-in `assert!`, `assert_eq!`, `assert_ne!`
- Pattern matching with `matches!` macro

**Run Commands:**
```bash
cargo test                                    # Run all tests
cargo test -p kernel_bpf                      # Single crate
cargo test -p kernel_bpf --no-default-features --features cloud-profile  # With features
cargo miri test -p kernel_abi                 # With Miri (UB detection)
```

## Test File Organization

**Location:**
- Integration tests in dedicated `tests/` directories
- No co-located unit tests (bare-metal constraint)
- Host-testable crates in `kernel/crates/kernel_*/`

**Naming:**
- `tests/*.rs` - Integration test files
- Descriptive names: `profile_contracts.rs`, `semantic_consistency.rs`

**Structure:**
```
kernel/crates/kernel_bpf/
├── src/
│   └── lib.rs
└── tests/
    ├── profile_contracts.rs
    └── semantic_consistency.rs
```

## Test Structure

**Suite Organization:**
```rust
#[test]
fn test_name() {
    // arrange
    let input = create_test_input();

    // act
    let result = function_under_test(input);

    // assert
    assert_eq!(result, expected);
}
```

**Patterns:**
- One assertion focus per test
- Feature-gated tests with `#[cfg(feature = "...")]`
- No setup/teardown (tests are independent)

**Feature-Gated Test Example:**
```rust
#[test]
#[cfg(feature = "cloud-profile")]
fn cloud_profile_has_high_limits() {
    use kernel_bpf::profile::CloudProfile;

    assert!(CloudProfile::MAX_STACK_SIZE >= 512 * 1024);
    assert!(CloudProfile::MAX_INSN_COUNT >= 1_000_000);
    assert!(CloudProfile::JIT_ALLOWED);
}
```

## Mocking

**Framework:**
- No mocking framework (tests use real implementations)
- Test doubles created manually when needed

**Patterns:**
- BPF programs built with `ProgramBuilder` for testing
- Context created with `BpfContext::empty()` or `BpfContext::from_slice()`

**What to Mock:**
- Not applicable (kernel code tests real implementations)

**What NOT to Mock:**
- Core algorithms (verifier, interpreter)
- Data structures (instructions, programs)

## Fixtures and Factories

**Test Data:**
```rust
// Factory pattern for test programs
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 42))
    .insn(BpfInsn::exit())
    .build()
    .expect("valid program");
```

**Location:**
- Inline in test files (no separate fixtures directory)
- Helper functions for complex setup

## Coverage

**Requirements:**
- No enforced coverage target
- Focus on critical paths (BPF verifier, interpreter)

**Configuration:**
- Not configured (no coverage tool in CI)

**Strategy:**
- Test profile contracts explicitly
- Test semantic consistency across profiles
- Test error conditions

## Test Types

**Unit Tests:**
- Scope: Individual crate functionality
- Location: `kernel/crates/kernel_*/tests/`
- Run: `cargo test -p <crate_name>`

**Integration Tests:**
- Scope: Cross-crate functionality
- Not currently implemented (kernel is bare-metal)

**E2E Tests:**
- Scope: Full kernel boot and execution
- Method: Manual testing via QEMU
- Run: `cargo run` (builds and boots in QEMU)

**Bare-Metal Limitation:**
- Main kernel (`kernel/src/`) cannot run standard tests
- Uses custom linker script incompatible with test harness
- Testable code extracted to `kernel/crates/kernel_*/`

## Common Patterns

**Semantic Consistency Testing:**
```rust
#[test]
fn semantic_return_constant() {
    let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
        .insn(BpfInsn::mov64_imm(0, 42))
        .insn(BpfInsn::exit())
        .build()
        .expect("valid program");

    let interp = Interpreter::<ActiveProfile>::new();
    let result = interp.execute(&program, &BpfContext::empty());

    assert_eq!(result, Ok(42));
}
```

**Profile Contract Testing:**
```rust
#[test]
#[cfg(feature = "embedded-profile")]
fn embedded_profile_has_strict_limits() {
    use kernel_bpf::profile::EmbeddedProfile;

    assert!(EmbeddedProfile::MAX_STACK_SIZE <= 8 * 1024);
    assert!(EmbeddedProfile::MAX_INSN_COUNT <= 100_000);
    assert!(!EmbeddedProfile::JIT_ALLOWED);
}
```

**Type Identity Testing:**
```rust
#[test]
#[cfg(feature = "embedded-profile")]
fn embedded_profile_is_active() {
    use core::any::TypeId;
    use kernel_bpf::profile::EmbeddedProfile;

    assert_eq!(
        TypeId::of::<ActiveProfile>(),
        TypeId::of::<EmbeddedProfile>()
    );
}
```

## CI/CD Testing Pipeline

**GitHub Actions Workflow (`.github/workflows/build.yml`):**

1. **Lint Job:**
   ```bash
   cargo fmt -- --check
   cargo clippy --workspace --lib -- -D clippy::all
   ```

2. **Test Job (Matrix: debug, release):**
   ```bash
   cargo test [--release]
   ```

3. **Miri Job (Per-Crate):**
   ```bash
   cargo miri setup
   cargo miri test -p <package>
   ```
   - Tests all crates except kernel_bpf (special handling)

4. **Miri kernel_bpf Job (Profile Matrix):**
   ```bash
   cargo miri test -p kernel_bpf --no-default-features --features cloud-profile
   cargo miri test -p kernel_bpf --no-default-features --features embedded-profile
   ```

5. **Build Job:**
   - Depends on: test, miri, miri-kernel-bpf
   - Full release build: `cargo build --release`

**Schedule:** Every push + twice daily (cron: `0 5,17 * * *`)

**BPF Profile CI (`.github/workflows/bpf-profiles.yml`):**
- Dedicated workflow for BPF profile testing
- Tests both profiles separately
- Semantic consistency verification
- Format check

## Testing Best Practices

**Pre-Submission Checklist (from CONTRIBUTING.md):**
```bash
cargo fmt -- --check              # Format check
cargo clippy --workspace --lib -- -D clippy::all  # Lint
cargo build --workspace --lib     # Build check
cargo test                        # Run tests
cargo miri setup && cargo miri test -p <crate>  # UB detection
```

**Command Restrictions:**
- Don't run `cargo test` on bare-metal targets
- Use `--workspace --lib` to avoid linker errors
- Test individual kernel crates: `cargo test -p kernel_abi`

**Miri for Unsafe Code:**
- Required for crates with unsafe code
- Detects undefined behavior
- Run: `cargo miri test -p <crate>`

---

*Testing analysis: 2026-01-21*
*Update when test patterns change*
