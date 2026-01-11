//! Memory Strategy Traits
//!
//! Defines how memory is allocated and managed based on the physical profile.
//!
//! # Strategies
//!
//! - **Elastic Memory** (Cloud): Dynamic allocation, resizing allowed
//! - **Static Memory** (Embedded): Fixed at init, no runtime allocation
//!
//! # Compile-Time Erasure
//!
//! Operations like `resize()` are completely erased from embedded builds
//! through feature-gated trait methods.

#![allow(dead_code)]

use super::sealed;

/// Memory allocation strategy.
///
/// This trait defines how BPF maps and program memory are allocated.
/// The strategy is determined at compile time by the profile.
pub trait MemoryStrategy: sealed::Sealed + 'static {
    /// Whether resize operations are allowed.
    ///
    /// - Cloud: true (maps can grow/shrink)
    /// - Embedded: false (resize methods are erased)
    const RESIZE_ALLOWED: bool;

    /// Whether dynamic allocation is allowed at runtime.
    ///
    /// - Cloud: true (allocator available)
    /// - Embedded: false (must use static pools)
    const DYNAMIC_ALLOC: bool;

    /// Whether memory can be shared between programs.
    ///
    /// - Cloud: true (shared maps supported)
    /// - Embedded: configurable (may be forbidden for isolation)
    const SHARING_ALLOWED: bool;

    /// Maximum total memory budget in bytes (0 = unlimited).
    ///
    /// - Cloud: 0 (unlimited, system constrained)
    /// - Embedded: Fixed budget set at build time
    const MEMORY_BUDGET: usize;
}

/// Elastic memory strategy for cloud profile.
///
/// Allows dynamic allocation, resizing, and growth as needed.
/// Memory is managed by the system allocator with no hard limits.
pub struct ElasticMemory;

impl sealed::Sealed for ElasticMemory {}

impl MemoryStrategy for ElasticMemory {
    /// Resize operations are allowed
    const RESIZE_ALLOWED: bool = true;

    /// Dynamic allocation via system allocator
    const DYNAMIC_ALLOC: bool = true;

    /// Cross-program sharing supported
    const SHARING_ALLOWED: bool = true;

    /// No hard budget (0 = unlimited)
    const MEMORY_BUDGET: usize = 0;
}

/// Static memory strategy for embedded profile.
///
/// All memory must be allocated from pre-defined static pools.
/// No runtime allocation or resizing is permitted.
pub struct StaticMemory;

impl sealed::Sealed for StaticMemory {}

impl MemoryStrategy for StaticMemory {
    /// Resize is forbidden - memory is fixed at init
    const RESIZE_ALLOWED: bool = false;

    /// No dynamic allocation - static pools only
    const DYNAMIC_ALLOC: bool = false;

    /// Sharing may be restricted for isolation
    const SHARING_ALLOWED: bool = false;

    /// 64KB default budget for embedded systems
    /// Can be overridden via build configuration
    const MEMORY_BUDGET: usize = 64 * 1024;
}

/// Trait for profile-aware memory allocation operations.
///
/// This trait provides methods that may be erased at compile time
/// based on the active profile's memory strategy.
pub trait ProfileMemoryOps<M: MemoryStrategy> {
    /// Attempt to resize a memory region.
    ///
    /// # Compile-Time Behavior
    ///
    /// - Cloud: Method exists and performs resize
    /// - Embedded: Method is erased (unreachable at compile time)
    ///
    /// # Errors
    ///
    /// Returns error if resize is not supported or fails.
    #[cfg(feature = "cloud-profile")]
    fn try_resize(&mut self, new_size: usize) -> Result<(), MemoryError>;

    /// Check remaining memory budget.
    ///
    /// # Returns
    ///
    /// - Cloud: Always returns `None` (unlimited)
    /// - Embedded: Returns remaining bytes in budget
    fn remaining_budget(&self) -> Option<usize>;
}

/// Memory operation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// Resize operation not supported by profile
    ResizeNotSupported,

    /// Out of memory or budget exhausted
    OutOfMemory,

    /// Invalid size requested
    InvalidSize,

    /// Memory region is in use and cannot be modified
    InUse,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elastic_allows_dynamic_ops() {
        assert!(ElasticMemory::RESIZE_ALLOWED);
        assert!(ElasticMemory::DYNAMIC_ALLOC);
        assert!(ElasticMemory::SHARING_ALLOWED);
        assert_eq!(ElasticMemory::MEMORY_BUDGET, 0);
    }

    #[test]
    fn static_forbids_dynamic_ops() {
        assert!(!StaticMemory::RESIZE_ALLOWED);
        assert!(!StaticMemory::DYNAMIC_ALLOC);
        assert!(!StaticMemory::SHARING_ALLOWED);
        assert!(StaticMemory::MEMORY_BUDGET > 0);
    }
}
