//! BPF Program Execution
//!
//! This module provides execution engines for BPF programs. The execution
//! strategy is determined at build time by the profile:
//!
//! | Mode        | Cloud Build | Embedded Build |
//! |-------------|-------------|----------------|
//! | JIT         | default     | **erased**     |
//! | Interpreter | fallback    | primary        |
//! | AOT         | rare        | encouraged     |
//!
//! # Compile-Time Erasure
//!
//! The JIT module is completely erased from embedded builds. This ensures
//! that embedded deployments cannot accidentally enable JIT compilation.

extern crate alloc;

mod interpreter;

// JIT is only available for cloud profile on x86_64
#[cfg(all(feature = "cloud-profile", target_arch = "x86_64"))]
pub mod jit;

pub use interpreter::Interpreter;

use crate::bytecode::program::BpfProgram;
use crate::profile::{ActiveProfile, PhysicalProfile};

/// Execution context passed to BPF programs.
///
/// This contains pointers to the program's input data and metadata.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BpfContext {
    /// Pointer to start of packet/data
    pub data: *const u8,
    /// Pointer to end of packet/data
    pub data_end: *const u8,
    /// Pointer to packet metadata
    pub data_meta: *const u8,
}

impl BpfContext {
    /// Create an empty context.
    pub const fn empty() -> Self {
        Self {
            data: core::ptr::null(),
            data_end: core::ptr::null(),
            data_meta: core::ptr::null(),
        }
    }

    /// Create a context from a data slice.
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            data: data.as_ptr(),
            data_end: unsafe { data.as_ptr().add(data.len()) },
            data_meta: core::ptr::null(),
        }
    }

    /// Get the data length.
    pub fn data_len(&self) -> usize {
        if self.data.is_null() || self.data_end.is_null() {
            0
        } else {
            unsafe { self.data_end.offset_from(self.data) as usize }
        }
    }
}

// Safety: BpfContext only contains raw pointers that are used read-only
unsafe impl Send for BpfContext {}
unsafe impl Sync for BpfContext {}

/// Result of BPF program execution.
pub type BpfResult = Result<u64, BpfError>;

/// Errors that can occur during BPF program execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpfError {
    /// Division by zero
    DivisionByZero,

    /// Out of bounds memory access
    OutOfBounds,

    /// Stack overflow
    StackOverflow,

    /// Invalid helper function
    InvalidHelper(i32),

    /// Execution timeout (instruction limit exceeded)
    Timeout,

    /// Invalid instruction
    InvalidInstruction,

    /// Program not loaded
    NotLoaded,
}

impl core::fmt::Display for BpfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::OutOfBounds => write!(f, "out of bounds memory access"),
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::InvalidHelper(id) => write!(f, "invalid helper function: {}", id),
            Self::Timeout => write!(f, "execution timeout"),
            Self::InvalidInstruction => write!(f, "invalid instruction"),
            Self::NotLoaded => write!(f, "program not loaded"),
        }
    }
}

/// Trait for BPF execution engines.
///
/// This trait defines the interface for executing BPF programs.
/// Different implementations (interpreter, JIT, AOT) provide
/// different performance characteristics.
pub trait BpfExecutor<P: PhysicalProfile = ActiveProfile> {
    /// Execute a BPF program with the given context.
    ///
    /// # Arguments
    ///
    /// * `program` - The verified BPF program to execute
    /// * `ctx` - The execution context (packet data, etc.)
    ///
    /// # Returns
    ///
    /// The return value from the BPF program (R0) on success,
    /// or a `BpfError` on failure.
    fn execute(&self, program: &BpfProgram<P>, ctx: &BpfContext) -> BpfResult;
}

/// Get the default executor for the active profile.
///
/// - Cloud: JIT (if available) or interpreter
/// - Embedded: interpreter only
pub fn default_executor<P: PhysicalProfile>() -> impl BpfExecutor<P> {
    // For now, always return interpreter
    // JIT would be selected based on profile in a full implementation
    Interpreter::<P>::new()
}

/// Helper function registry.
///
/// BPF programs can call helper functions by ID. This registry
/// maps helper IDs to function pointers.
pub type HelperFn = fn(u64, u64, u64, u64, u64) -> u64;

/// Built-in helper function IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum HelperFunc {
    /// Unspec / invalid
    Unspec = 0,

    /// Get current time in nanoseconds
    KtimeGetNs = 1,

    /// Print debug message (trace_printk)
    TracePrintk = 2,

    /// Get pseudo-random number
    GetPrandomU32 = 3,

    /// Get current SMP processor ID
    GetSmpProcessorId = 4,

    /// Map lookup element
    MapLookupElem = 5,

    /// Map update element
    MapUpdateElem = 6,

    /// Map delete element
    MapDeleteElem = 7,

    /// Probe read (safe memory read)
    ProbeRead = 8,

    /// Get current PID/TID
    GetCurrentPidTgid = 9,

    /// Get current UID/GID
    GetCurrentUidGid = 10,

    /// Get current comm (process name)
    GetCurrentComm = 11,
}

impl HelperFunc {
    /// Check if this helper is allowed for the given profile.
    pub fn is_allowed_for_profile<P: PhysicalProfile>(&self) -> bool {
        // Most helpers are allowed in both profiles
        match self {
            // All basic helpers are allowed
            Self::Unspec
            | Self::KtimeGetNs
            | Self::GetPrandomU32
            | Self::GetSmpProcessorId
            | Self::MapLookupElem
            | Self::MapUpdateElem
            | Self::MapDeleteElem
            | Self::GetCurrentPidTgid
            | Self::GetCurrentUidGid
            | Self::GetCurrentComm => true,

            // Trace/debug helpers may be restricted in embedded
            Self::TracePrintk | Self::ProbeRead => {
                // In a real implementation, check profile constraints
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_from_slice() {
        let data = [1u8, 2, 3, 4, 5];
        let ctx = BpfContext::from_slice(&data);
        assert_eq!(ctx.data_len(), 5);
    }

    #[test]
    fn empty_context() {
        let ctx = BpfContext::empty();
        assert_eq!(ctx.data_len(), 0);
    }
}
