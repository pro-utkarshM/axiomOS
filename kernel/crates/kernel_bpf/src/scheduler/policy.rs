//! Scheduling Policy Traits
//!
//! This module defines the abstract policy interface for BPF program
//! scheduling. Concrete policies are implemented per-profile.

use super::queue::{BpfQueue, QueuedProgram};
use crate::profile::PhysicalProfile;

/// Execution priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ExecPriority {
    /// Lowest priority - background execution
    Low = 0,
    /// Default priority
    #[default]
    Normal = 1,
    /// Elevated priority
    High = 2,
    /// Highest priority - critical execution
    Critical = 3,
}

/// Scheduling errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedError {
    /// Queue is full
    QueueFull,
    /// Program not found
    NotFound,
    /// Invalid deadline (embedded only)
    #[cfg(feature = "embedded-profile")]
    InvalidDeadline,
    /// Deadline miss detected (embedded only)
    #[cfg(feature = "embedded-profile")]
    DeadlineMiss,
}

impl core::fmt::Display for SchedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "scheduler queue is full"),
            Self::NotFound => write!(f, "program not found"),
            #[cfg(feature = "embedded-profile")]
            Self::InvalidDeadline => write!(f, "invalid deadline"),
            #[cfg(feature = "embedded-profile")]
            Self::DeadlineMiss => write!(f, "deadline miss detected"),
        }
    }
}

/// Result type for scheduling operations.
pub type SchedResult<T> = Result<T, SchedError>;

/// Scheduling policy trait.
///
/// Policies determine how programs are selected from the ready queue.
pub trait BpfPolicy<P: PhysicalProfile> {
    /// Select the next program to execute from the queue.
    ///
    /// Returns `None` if the queue is empty or no program should run.
    fn select(&mut self, queue: &mut BpfQueue<P>) -> Option<QueuedProgram<P>>;

    /// Check if a program can be admitted to the queue.
    ///
    /// Policies can reject programs that would violate constraints.
    fn admit(&self, queue: &BpfQueue<P>, program: &QueuedProgram<P>) -> SchedResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(ExecPriority::Low < ExecPriority::Normal);
        assert!(ExecPriority::Normal < ExecPriority::High);
        assert!(ExecPriority::High < ExecPriority::Critical);
    }

    #[test]
    fn default_priority() {
        assert_eq!(ExecPriority::default(), ExecPriority::Normal);
    }
}
