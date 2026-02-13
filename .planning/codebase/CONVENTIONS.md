# Coding Conventions

**Analysis Date:** 2026-02-13

## Naming Patterns

**Files:**
- `snake_case.rs` for all Rust source files
- `mod.rs` for module directories
- `error.rs` for per-module error types alongside `mod.rs`
- `*.S` (uppercase) for assembly files

**Functions:**
- `snake_case` for all functions
- `init()` for subsystem initialization
- `dispatch_*` for routing functions
- `sys_*` for syscall handlers — `kernel/src/syscall/`

**Variables:**
- `snake_case` for variables
- `UPPER_SNAKE_CASE` for constants and statics
- `ATTACH_TYPE_*` for BPF attach type constants — `kernel/src/bpf/mod.rs`

**Types:**
- `PascalCase` for structs, enums, traits
- No prefix conventions (no `I` for interfaces)
- Error enums: `{Module}Error` (e.g., `LoadError`, `VerifyError`, `AttachError`)
- Result aliases: `{Module}Result<T>` pairing with error type

**Crates:**
- `kernel_` prefix for all kernel subsystem crates
- Underscore-separated: `kernel_bpf`, `kernel_vfs`, `kernel_syscall`

## Code Style

**Formatting:**
- rustfmt with `rustfmt.toml` configuration
- `imports_granularity = "Module"` — groups imports by module
- `group_imports = "StdExternalCrate"` — separates std, external, crate imports
- Standard 4-space indentation

**Linting:**
- Clippy with all warnings as errors: `-D clippy::all`
- Run: `cargo clippy -- -D clippy::all`
- Enforced in CI — `.github/workflows/build.yml`

**Rust Edition:**
- 2021 for kernel binary — `kernel/Cargo.toml`
- 2024 for workspace member crates — `Cargo.toml` workspace section

## Import Organization

**Order (enforced by rustfmt):**
1. Standard library / core / alloc (`use core::`, `use alloc::`)
2. External crates (`use spin::`, `use bitflags::`)
3. Workspace crates (`use kernel_bpf::`, `use kernel_abi::`)
4. Crate-internal (`use crate::`, `use super::`)

**Grouping:**
- Blank line between groups (rustfmt enforced)
- Module-level granularity (not item-level)

## Error Handling

**Patterns:**
- Custom enum-based errors with structured variants
- `Display` impl for human-readable messages
- Result<T, ErrorType> aliases per module
- `thiserror` crate used where available — `Cargo.toml`

**Error Types (BPF example):**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    ElfTooSmall,
    InvalidMagic,
    UnsupportedMapType(u32),  // context in variant payload
}
```

**Panic Behavior:**
- `panic = "abort"` in both dev and release profiles
- `expect()` with descriptive messages for boot-critical operations
- Panics acceptable only for unrecoverable kernel errors

## Unsafe Code

**Safety Documentation:**
- All unsafe blocks MUST have `// SAFETY:` comments explaining soundness
- Pattern: Comment immediately preceding the unsafe block

**Example:**
```rust
// SAFETY: We export "kernel_main" as the symbol name for the bootloader to find.
// This symbol name is unique and required by the Limine protocol.
#[unsafe(export_name = "kernel_main")]
unsafe extern "C" fn main() -> ! {
```

**Common Unsafe Patterns:**
- FFI bindings to C-convention functions (BPF helpers)
- Export of Rust functions with C calling convention
- Direct pointer manipulation with bounds checking
- Inline assembly for CPU-specific operations (wfi, hlt)
- Volatile MMIO access

**Test Stubs:**
- External C functions stubbed in tests with `#[unsafe(no_mangle)]`
- Safety comments on test stubs too

## Documentation

**Module-Level (//!):**
- Full module overview with purpose and design rationale
- Markdown tables for comparisons (cloud vs embedded profiles)
- ASCII diagrams for memory layouts
- Example from `kernel/crates/kernel_bpf/src/lib.rs`

**Item-Level (///):**
- Brief one-liner summary
- Detailed explanation with context
- `# Profile Differences` sections where applicable

**Inline Comments:**
- Minimal; code is self-documenting
- Phase markers for complex algorithms: `// Phase 1: Basic checks`
- `// SAFETY:` for unsafe blocks
- `// TODO:` / `// FIXME:` for known gaps

## Compile-Time Profiles

**BPF Profile System:**
- Mutually exclusive features: `cloud-profile` vs `embedded-profile`
- Enforced via `compile_error!` macro — `kernel/crates/kernel_bpf/src/lib.rs`
- Sealed trait pattern prevents external implementations
- `ActiveProfile` type alias resolves to selected profile
- Code erasure: `#[cfg(feature = "cloud-profile")]` removes embedded-only code and vice versa

## Module Design

**Exports:**
- `pub mod` declarations in `lib.rs` for crate-level modules
- `mod.rs` files for re-exporting submodule contents
- Public API through crate root

**Patterns:**
- Large modules use `mod.rs` + companion files (`error.rs`, individual types)
- Related error types colocated in `error.rs`
- Trait definitions in module root, implementations in separate files

---

*Convention analysis: 2026-02-13*
*Update when patterns change*
