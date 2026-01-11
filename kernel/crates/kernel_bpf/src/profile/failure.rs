//! Failure Semantic Traits
//!
//! Defines how failures are handled based on the physical profile.
//!
//! # Semantics
//!
//! - **Restart Acceptable** (Cloud): System restart is normal recovery
//! - **Recovery Required** (Embedded): Restart forbidden, must use recovery partition
//!
//! # Compile-Time Erasure
//!
//! Recovery partition handling is erased from cloud builds.
//! Restart paths are erased from embedded builds.

#![allow(dead_code)]

use super::sealed;

/// Failure handling semantics.
///
/// This trait defines how the kernel handles failures in BPF programs
/// and the BPF subsystem itself. The semantics are determined at compile
/// time by the profile.
pub trait FailureSemantic: sealed::Sealed + 'static {
    /// Whether restart is an acceptable recovery mechanism.
    ///
    /// - Cloud: true (restart is normal)
    /// - Embedded: false (restart may be catastrophic)
    const RESTART_OK: bool;

    /// Whether a recovery partition is required.
    ///
    /// - Cloud: false (restart suffices)
    /// - Embedded: true (must have recovery path)
    const RECOVERY_REQUIRED: bool;

    /// Whether BPF program failures should be isolated.
    ///
    /// - Cloud: true (kill program, continue system)
    /// - Embedded: true (strict isolation mandatory)
    const ISOLATION_REQUIRED: bool;

    /// Whether to log failures for post-mortem analysis.
    ///
    /// - Cloud: true (logging to persistent storage)
    /// - Embedded: configurable (may have limited storage)
    const LOGGING_ENABLED: bool;
}

/// Restart-acceptable failure semantics for cloud profile.
///
/// In cloud environments, system restart is a normal recovery mechanism.
/// The system is designed to survive and recover from restarts gracefully.
pub struct RestartAcceptable;

impl sealed::Sealed for RestartAcceptable {}

impl FailureSemantic for RestartAcceptable {
    /// Restart is normal recovery
    const RESTART_OK: bool = true;

    /// No special recovery partition needed
    const RECOVERY_REQUIRED: bool = false;

    /// Isolate failures to prevent cascade
    const ISOLATION_REQUIRED: bool = true;

    /// Full logging available
    const LOGGING_ENABLED: bool = true;
}

/// Recovery-required failure semantics for embedded profile.
///
/// In embedded environments, system restart may be impossible or catastrophic.
/// A recovery partition must exist to handle failures gracefully.
pub struct RecoveryRequired;

impl sealed::Sealed for RecoveryRequired {}

impl FailureSemantic for RecoveryRequired {
    /// Restart is forbidden
    const RESTART_OK: bool = false;

    /// Must have recovery partition
    const RECOVERY_REQUIRED: bool = true;

    /// Strict isolation mandatory
    const ISOLATION_REQUIRED: bool = true;

    /// Logging may be limited
    const LOGGING_ENABLED: bool = true;
}

/// Failure severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FailureSeverity {
    /// Informational - not an error
    Info = 0,

    /// Warning - potential issue, operation continues
    Warning = 1,

    /// Error - operation failed, program terminated
    Error = 2,

    /// Critical - subsystem compromised
    Critical = 3,

    /// Fatal - system must recover or halt
    Fatal = 4,
}

/// Failure information for BPF program execution.
#[derive(Debug, Clone)]
pub struct FailureInfo {
    /// Severity of the failure
    pub severity: FailureSeverity,

    /// Error code
    pub code: FailureCode,

    /// Program ID that failed (if applicable)
    pub program_id: Option<u64>,

    /// Instruction pointer at failure
    pub instruction_ptr: Option<usize>,

    /// Human-readable description
    pub description: &'static str,
}

/// Failure codes for BPF subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum FailureCode {
    /// No error
    None = 0,

    // Verification failures (100-199)
    /// Invalid opcode in bytecode
    InvalidOpcode = 100,
    /// Uninitialized register access
    UninitializedRegister = 101,
    /// Out of bounds memory access
    OutOfBounds = 102,
    /// Infinite loop detected
    InfiniteLoop = 103,
    /// Stack limit exceeded
    StackOverflow = 104,
    /// Instruction limit exceeded
    InsnLimitExceeded = 105,

    // Execution failures (200-299)
    /// Division by zero
    DivisionByZero = 200,
    /// Invalid helper function call
    InvalidHelper = 201,
    /// Map operation failed
    MapError = 202,
    /// Timeout during execution
    Timeout = 203,

    // Resource failures (300-399)
    /// Out of memory
    OutOfMemory = 300,
    /// Resource limit exceeded
    ResourceLimit = 301,

    // Profile-specific failures (400-499)
    /// WCET budget exceeded (embedded only)
    #[cfg(feature = "embedded-profile")]
    WcetExceeded = 400,
    /// Deadline missed (embedded only)
    #[cfg(feature = "embedded-profile")]
    DeadlineMissed = 401,
    /// Energy budget exhausted (embedded only)
    #[cfg(feature = "embedded-profile")]
    EnergyExhausted = 402,
    /// Interrupt safety violation (embedded only)
    #[cfg(feature = "embedded-profile")]
    InterruptUnsafe = 403,

    // System failures (500-599)
    /// Internal kernel error
    InternalError = 500,
    /// Recovery required
    RecoveryNeeded = 501,
}

/// Recovery action to take after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Continue execution (warning only)
    Continue,

    /// Terminate the BPF program
    TerminateProgram,

    /// Restart the BPF subsystem
    RestartSubsystem,

    /// Invoke recovery partition (embedded only)
    #[cfg(feature = "embedded-profile")]
    InvokeRecovery,

    /// System halt (last resort)
    Halt,
}

impl FailureInfo {
    /// Determine the appropriate recovery action for this failure.
    pub fn recovery_action<F: FailureSemantic>(&self) -> RecoveryAction {
        match self.severity {
            FailureSeverity::Info | FailureSeverity::Warning => RecoveryAction::Continue,
            FailureSeverity::Error => RecoveryAction::TerminateProgram,
            FailureSeverity::Critical => {
                if F::RESTART_OK {
                    RecoveryAction::RestartSubsystem
                } else {
                    #[cfg(feature = "embedded-profile")]
                    {
                        RecoveryAction::InvokeRecovery
                    }
                    #[cfg(not(feature = "embedded-profile"))]
                    {
                        RecoveryAction::RestartSubsystem
                    }
                }
            }
            FailureSeverity::Fatal => {
                #[cfg(feature = "embedded-profile")]
                if F::RECOVERY_REQUIRED {
                    return RecoveryAction::InvokeRecovery;
                }
                RecoveryAction::Halt
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_acceptable_constants() {
        assert!(RestartAcceptable::RESTART_OK);
        assert!(!RestartAcceptable::RECOVERY_REQUIRED);
        assert!(RestartAcceptable::ISOLATION_REQUIRED);
    }

    #[test]
    fn recovery_required_constants() {
        assert!(!RecoveryRequired::RESTART_OK);
        assert!(RecoveryRequired::RECOVERY_REQUIRED);
        assert!(RecoveryRequired::ISOLATION_REQUIRED);
    }

    #[test]
    fn severity_ordering() {
        assert!(FailureSeverity::Info < FailureSeverity::Warning);
        assert!(FailureSeverity::Warning < FailureSeverity::Error);
        assert!(FailureSeverity::Error < FailureSeverity::Critical);
        assert!(FailureSeverity::Critical < FailureSeverity::Fatal);
    }

    #[test]
    fn recovery_action_for_error() {
        let failure = FailureInfo {
            severity: FailureSeverity::Error,
            code: FailureCode::DivisionByZero,
            program_id: Some(42),
            instruction_ptr: Some(100),
            description: "Division by zero",
        };

        assert_eq!(
            failure.recovery_action::<RestartAcceptable>(),
            RecoveryAction::TerminateProgram
        );
    }
}
