# Coding Conventions

**Analysis Date:** 2026-01-21

## Naming Patterns

**Files:**
- snake_case.rs for all Rust source files
- mod.rs for module containers
- lib.rs for crate roots, main.rs for binaries
- Tests: `tests/*.rs` in dedicated tests directory (not co-located)

**Functions:**
- snake_case for all functions
- No special prefix for async functions
- `new()` for constructors, `from_*()` for conversions
- `try_*()` for fallible operations returning Option/Result

**Variables:**
- snake_case for variables
- UPPER_SNAKE_CASE for constants
- No underscore prefix for private fields

**Types:**
- PascalCase for structs, enums, traits
- No I prefix for interfaces/traits (use descriptive names)
- Error suffix for error types (e.g., `MountError`, `VerifyError`)

**Crates:**
- snake_case with `kernel_` prefix for kernel subsystems
- Examples: `kernel_vfs`, `kernel_bpf`, `kernel_physical_memory`

## Code Style

**Formatting:**
- Tool: rustfmt with `rustfmt.toml`
- Settings: `imports_granularity = "Module"`, `group_imports = "StdExternalCrate"`
- Enforced via CI: `cargo fmt -- --check`

**Linting:**
- Tool: clippy
- Severity: All warnings as errors (`-D clippy::all`)
- Scope: Workspace libraries (`--workspace --lib`)
- Run: `cargo clippy --workspace --lib -- -D clippy::all`

**Attributes:**
- `#[must_use]` on functions returning important values
- `#[inline]` and `#[inline(always)]` for performance-critical code
- `#[repr(C)]` or `#[repr(u8/u32)]` for FFI and bytecode structs
- `#[cfg(...)]` for feature-gated and profile-specific code

## Import Organization

**Order:**
1. `core::` and `alloc::` imports (no_std environment)
2. External crates (spin, thiserror, etc.)
3. Workspace crates (kernel_*, etc.)
4. Crate-internal imports (`crate::`, `super::`)

**Grouping:**
- Blank line between groups
- Configured via rustfmt: `group_imports = "StdExternalCrate"`

**Path Aliases:**
- None defined (use full paths)

## Error Handling

**Patterns:**
- Use `thiserror` for error type definitions
- Propagate errors with `?` operator
- `expect()` only in initialization code where failure should panic
- `todo!()` for unimplemented features (explicit, documented crashes)

**Error Types:**
```rust
#[derive(Debug, Copy, Clone, Eq, PartialEq, Error)]
pub enum MountError {
    #[error("the mount point is already used")]
    AlreadyMounted,
}
```

**When to panic:**
- Boot-time initialization failures
- Invariant violations (should never happen)
- Use `expect("descriptive message")` not bare `unwrap()`

## Logging

**Framework:**
- `log` crate facade (no_std compatible)
- Backend: Serial console via `uart_16550`

**Patterns:**
- `info!()` for significant state changes
- `debug!()` for detailed execution flow
- `error!()` for failures (with context)
- No `println!()` in kernel code

**Location:**
- Setup: `kernel/src/log.rs`
- Usage: Throughout kernel with `use log::{info, debug, error}`

## Comments

**When to Comment:**
- Explain "why" not "what"
- Document safety invariants for `unsafe` blocks (required)
- Module-level docs explaining purpose and architecture
- ASCII diagrams for complex data structures

**Doc Comments:**
- `//!` for module-level documentation
- `///` for item-level documentation
- Use markdown with code blocks and tables

**Example (from `kernel_bpf/src/lib.rs`):**
```rust
//! Single-Source eBPF Kernel with Build-Time Physical Profiles
//!
//! | Property | Cloud | Embedded |
//! |----------|-------|----------|
//! | Memory   | Elastic | Static |
```

**TODO Comments:**
- Format: `// TODO: description`
- Include context for incomplete features
- FIXME for known bugs needing attention

## Function Design

**Size:**
- Keep functions focused (single responsibility)
- Large files (>400 lines) are acceptable for complex subsystems
- Extract helpers when logic is reusable

**Parameters:**
- Use generics with trait bounds for flexibility
- Example: `path: impl AsRef<AbsolutePath>`
- Destructure in function body, not parameter list

**Return Values:**
- Return `Result<T, E>` for fallible operations
- Use `Option<T>` for optional values
- Explicit return types (no inference for public APIs)

## Module Design

**Exports:**
- Private modules with selective re-exports in lib.rs
- Pattern: `mod foo; pub use foo::*;` or `pub use foo::SpecificType;`

**Example (from `kernel_abi/src/lib.rs`):**
```rust
mod errno;
mod fcntl;
mod syscall;

pub use errno::*;
pub use fcntl::*;
pub use syscall::*;
```

**Barrel Files:**
- lib.rs re-exports public API
- Internal helpers stay private
- Avoid circular dependencies

## Derives & Traits

**Standard Derives:**
- `Clone, Copy` for value types
- `Debug` for all public types
- `PartialEq, Eq` for comparable types
- `Default` when sensible defaults exist
- `Hash` for hashable types

**Common Pattern:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Register {
    R0 = 0,
    // ...
}
```

## Unsafe Code

**Requirements:**
- Safety comment required for all `unsafe` blocks (per CONTRIBUTING.md)
- Explain why the operation is safe
- Minimize unsafe scope

**Example:**
```rust
// SAFETY: data_end is always >= data (validated in from_slice)
unsafe { self.data_end.offset_from(self.data) as usize }
```

## Feature Flags

**Profile Features (kernel_bpf):**
- `cloud-profile` - Servers, VMs, elastic resources
- `embedded-profile` - IoT, RPi5, static resources
- Mutually exclusive at compile time

**Pattern:**
```rust
#[cfg(all(feature = "cloud-profile", feature = "embedded-profile"))]
compile_error!("Cannot enable both profiles");
```

---

*Convention analysis: 2026-01-21*
*Update when patterns change*
