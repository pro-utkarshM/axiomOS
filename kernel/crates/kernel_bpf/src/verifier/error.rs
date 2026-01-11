//! Verification Errors
//!
//! Error types returned when BPF program verification fails.

use core::fmt;

use crate::bytecode::registers::Register;

/// Errors that can occur during BPF program verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    // ========================================
    // Core safety violations (both profiles)
    // ========================================
    /// Invalid opcode in bytecode
    InvalidOpcode {
        /// Instruction index
        insn_idx: usize,
        /// Invalid opcode value
        opcode: u8,
    },

    /// Invalid register number
    InvalidRegister {
        /// Instruction index
        insn_idx: usize,
        /// Invalid register value
        reg: u8,
    },

    /// Use of uninitialized register
    UninitializedRegister {
        /// Instruction index
        insn_idx: usize,
        /// Uninitialized register
        reg: Register,
    },

    /// Out of bounds memory access
    OutOfBoundsAccess {
        /// Instruction index
        insn_idx: usize,
        /// Attempted access address/offset
        offset: i64,
        /// Access size
        size: usize,
    },

    /// Invalid memory access (wrong pointer type)
    InvalidMemoryAccess {
        /// Instruction index
        insn_idx: usize,
        /// Description of the issue
        reason: &'static str,
    },

    /// Unreachable instruction detected
    UnreachableInstruction {
        /// Index of unreachable instruction
        insn_idx: usize,
    },

    /// Infinite loop detected
    InfiniteLoop {
        /// Instruction index where loop starts
        insn_idx: usize,
    },

    /// Invalid jump target
    InvalidJump {
        /// Instruction index of jump
        insn_idx: usize,
        /// Invalid target offset
        target: i32,
    },

    /// Program does not terminate (no exit instruction)
    NoExit,

    /// Program is empty
    EmptyProgram,

    /// Invalid helper function call
    InvalidHelper {
        /// Instruction index
        insn_idx: usize,
        /// Helper ID
        helper_id: i32,
    },

    /// Division by zero possible
    DivisionByZero {
        /// Instruction index
        insn_idx: usize,
    },

    // ========================================
    // Profile-specific violations
    // ========================================
    /// Stack size exceeds profile limit
    StackExceeded {
        /// Used stack size
        used: usize,
        /// Profile limit
        limit: usize,
    },

    /// Instruction count exceeds profile limit
    InsnCountExceeded {
        /// Instruction count
        count: usize,
        /// Profile limit
        limit: usize,
    },

    /// Write to read-only register (R10)
    WriteToReadOnly {
        /// Instruction index
        insn_idx: usize,
    },

    /// Misaligned memory access
    MisalignedAccess {
        /// Instruction index
        insn_idx: usize,
        /// Access offset
        offset: i64,
        /// Required alignment
        alignment: usize,
    },

    // ========================================
    // Embedded profile specific
    // ========================================
    /// Worst-case execution time exceeded (embedded only)
    #[cfg(feature = "embedded-profile")]
    WcetExceeded {
        /// Estimated cycles
        cycles: u64,
        /// Budget
        budget: u64,
    },

    /// Program is not interrupt-safe (embedded only)
    #[cfg(feature = "embedded-profile")]
    InterruptUnsafe {
        /// Instruction index
        insn_idx: usize,
        /// Reason
        reason: &'static str,
    },

    /// Dynamic allocation attempted (embedded only)
    #[cfg(feature = "embedded-profile")]
    DynamicAllocationAttempted {
        /// Instruction index
        insn_idx: usize,
    },

    /// Unbounded loop detected (embedded only)
    #[cfg(feature = "embedded-profile")]
    UnboundedLoop {
        /// Instruction index where loop starts
        insn_idx: usize,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOpcode { insn_idx, opcode } => {
                write!(
                    f,
                    "invalid opcode {:#04x} at instruction {}",
                    opcode, insn_idx
                )
            }
            Self::InvalidRegister { insn_idx, reg } => {
                write!(f, "invalid register {} at instruction {}", reg, insn_idx)
            }
            Self::UninitializedRegister { insn_idx, reg } => {
                write!(
                    f,
                    "use of uninitialized register {} at instruction {}",
                    reg, insn_idx
                )
            }
            Self::OutOfBoundsAccess {
                insn_idx,
                offset,
                size,
            } => {
                write!(
                    f,
                    "out of bounds access at offset {} size {} at instruction {}",
                    offset, size, insn_idx
                )
            }
            Self::InvalidMemoryAccess { insn_idx, reason } => {
                write!(
                    f,
                    "invalid memory access at instruction {}: {}",
                    insn_idx, reason
                )
            }
            Self::UnreachableInstruction { insn_idx } => {
                write!(f, "unreachable instruction at {}", insn_idx)
            }
            Self::InfiniteLoop { insn_idx } => {
                write!(f, "infinite loop detected at instruction {}", insn_idx)
            }
            Self::InvalidJump { insn_idx, target } => {
                write!(
                    f,
                    "invalid jump target {} at instruction {}",
                    target, insn_idx
                )
            }
            Self::NoExit => write!(f, "program does not exit"),
            Self::EmptyProgram => write!(f, "program is empty"),
            Self::InvalidHelper {
                insn_idx,
                helper_id,
            } => {
                write!(
                    f,
                    "invalid helper function {} at instruction {}",
                    helper_id, insn_idx
                )
            }
            Self::DivisionByZero { insn_idx } => {
                write!(f, "possible division by zero at instruction {}", insn_idx)
            }
            Self::StackExceeded { used, limit } => {
                write!(f, "stack size {} exceeds limit {}", used, limit)
            }
            Self::InsnCountExceeded { count, limit } => {
                write!(f, "instruction count {} exceeds limit {}", count, limit)
            }
            Self::WriteToReadOnly { insn_idx } => {
                write!(
                    f,
                    "write to read-only register R10 at instruction {}",
                    insn_idx
                )
            }
            Self::MisalignedAccess {
                insn_idx,
                offset,
                alignment,
            } => {
                write!(
                    f,
                    "misaligned access at offset {} (alignment {}) at instruction {}",
                    offset, alignment, insn_idx
                )
            }
            #[cfg(feature = "embedded-profile")]
            Self::WcetExceeded { cycles, budget } => {
                write!(f, "WCET {} cycles exceeds budget {}", cycles, budget)
            }
            #[cfg(feature = "embedded-profile")]
            Self::InterruptUnsafe { insn_idx, reason } => {
                write!(
                    f,
                    "interrupt-unsafe operation at instruction {}: {}",
                    insn_idx, reason
                )
            }
            #[cfg(feature = "embedded-profile")]
            Self::DynamicAllocationAttempted { insn_idx } => {
                write!(
                    f,
                    "dynamic allocation attempted at instruction {}",
                    insn_idx
                )
            }
            #[cfg(feature = "embedded-profile")]
            Self::UnboundedLoop { insn_idx } => {
                write!(f, "unbounded loop detected at instruction {}", insn_idx)
            }
        }
    }
}

/// Result type for verification operations.
pub type VerifyResult<T> = Result<T, VerifyError>;
