//! Scheduler Policy Traits
//!
//! Defines how BPF programs are scheduled based on the physical profile.
//!
//! # Policies
//!
//! - **Throughput Optimized** (Cloud): Best-effort fairness, maximize throughput
//! - **Deadline Aware** (Embedded): EDF scheduling, priority ceilings, energy budgets
//!
//! # Compile-Time Erasure
//!
//! Deadline and energy tracking structures are erased from cloud builds.
//! Cloud-specific optimizations are erased from embedded builds.

#![allow(dead_code)]

use super::sealed;

/// Scheduler policy for BPF program execution.
///
/// This trait defines how the kernel schedules BPF programs.
/// The policy is determined at compile time by the profile.
pub trait SchedulerPolicy: sealed::Sealed + 'static {
    /// Whether hard deadline enforcement is required.
    ///
    /// - Cloud: false (best-effort latency)
    /// - Embedded: true (real-time deadlines)
    const DEADLINE_ENFORCED: bool;

    /// Whether energy-aware scheduling is required.
    ///
    /// - Cloud: false (power assumed infinite)
    /// - Embedded: true (energy budgets tracked)
    const ENERGY_AWARE: bool;

    /// Whether priority ceiling protocol is enforced.
    ///
    /// - Cloud: false (simple priority)
    /// - Embedded: true (prevents priority inversion)
    const PRIORITY_CEILING: bool;

    /// Whether preemption is allowed during BPF execution.
    ///
    /// - Cloud: true (preemptive multitasking)
    /// - Embedded: configurable (may need atomic execution)
    const PREEMPTION_ALLOWED: bool;

    /// Default scheduling quantum in microseconds.
    ///
    /// - Cloud: Larger quantum for throughput
    /// - Embedded: Smaller quantum for responsiveness
    const DEFAULT_QUANTUM_US: u64;
}

/// Throughput-optimized scheduling for cloud profile.
///
/// Focuses on maximizing overall throughput with fair sharing.
/// Uses best-effort latency bounds without hard guarantees.
pub struct ThroughputOptimized;

impl sealed::Sealed for ThroughputOptimized {}

impl SchedulerPolicy for ThroughputOptimized {
    /// No hard deadlines - best effort
    const DEADLINE_ENFORCED: bool = false;

    /// No energy tracking - power assumed infinite
    const ENERGY_AWARE: bool = false;

    /// Simple priority without ceiling protocol
    const PRIORITY_CEILING: bool = false;

    /// Preemption enabled for fairness
    const PREEMPTION_ALLOWED: bool = true;

    /// 10ms quantum for throughput
    const DEFAULT_QUANTUM_US: u64 = 10_000;
}

/// Deadline-aware scheduling for embedded profile.
///
/// Implements Earliest Deadline First (EDF) scheduling with:
/// - Hard deadline enforcement
/// - Priority ceiling protocol
/// - Energy budget tracking
pub struct DeadlineAware;

impl sealed::Sealed for DeadlineAware {}

impl SchedulerPolicy for DeadlineAware {
    /// Hard deadlines enforced
    const DEADLINE_ENFORCED: bool = true;

    /// Energy budgets tracked
    const ENERGY_AWARE: bool = true;

    /// Priority ceiling prevents inversion
    const PRIORITY_CEILING: bool = true;

    /// Preemption with careful ceiling management
    const PREEMPTION_ALLOWED: bool = true;

    /// 1ms quantum for responsiveness
    const DEFAULT_QUANTUM_US: u64 = 1_000;
}

/// Priority level for BPF programs.
///
/// Lower values indicate higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Priority(pub u8);

impl Priority {
    /// Highest priority (real-time critical)
    pub const HIGHEST: Self = Self(0);

    /// High priority
    pub const HIGH: Self = Self(64);

    /// Default/normal priority
    pub const DEFAULT: Self = Self(128);

    /// Low priority (background)
    pub const LOW: Self = Self(192);

    /// Lowest priority (idle)
    pub const LOWEST: Self = Self(255);
}

impl Default for Priority {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Deadline specification for embedded profile.
///
/// This struct is only available in embedded profile builds.
/// It is completely erased from cloud builds.
#[cfg(feature = "embedded-profile")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    /// Absolute deadline in nanoseconds from boot
    pub absolute_ns: u64,

    /// Relative deadline from submission (for logging)
    pub relative_ns: u64,
}

#[cfg(feature = "embedded-profile")]
impl Deadline {
    /// Create a deadline relative to now.
    pub fn from_relative(relative_ns: u64, now_ns: u64) -> Self {
        Self {
            absolute_ns: now_ns.saturating_add(relative_ns),
            relative_ns,
        }
    }

    /// Check if deadline has passed.
    pub fn is_missed(&self, now_ns: u64) -> bool {
        now_ns > self.absolute_ns
    }

    /// Time remaining until deadline (0 if passed).
    pub fn time_remaining(&self, now_ns: u64) -> u64 {
        self.absolute_ns.saturating_sub(now_ns)
    }
}

/// Energy budget for embedded profile.
///
/// This struct is only available in embedded profile builds.
/// It is completely erased from cloud builds.
#[cfg(feature = "embedded-profile")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnergyBudget {
    /// Maximum energy in microjoules
    pub max_uj: u64,

    /// Energy consumed so far
    pub consumed_uj: u64,
}

#[cfg(feature = "embedded-profile")]
impl EnergyBudget {
    /// Create a new energy budget.
    pub fn new(max_uj: u64) -> Self {
        Self {
            max_uj,
            consumed_uj: 0,
        }
    }

    /// Check if budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.consumed_uj >= self.max_uj
    }

    /// Remaining energy in microjoules.
    pub fn remaining(&self) -> u64 {
        self.max_uj.saturating_sub(self.consumed_uj)
    }

    /// Consume energy from budget.
    ///
    /// Returns true if consumption was within budget.
    pub fn consume(&mut self, amount_uj: u64) -> bool {
        self.consumed_uj = self.consumed_uj.saturating_add(amount_uj);
        self.consumed_uj <= self.max_uj
    }
}

/// Deadline miss information for embedded profile.
#[cfg(feature = "embedded-profile")]
#[derive(Debug, Clone, Copy)]
pub struct DeadlineMiss {
    /// Program that missed deadline
    pub program_id: u64,

    /// The missed deadline
    pub deadline: Deadline,

    /// Actual completion time
    pub actual_ns: u64,

    /// How much the deadline was exceeded by
    pub overrun_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_policy_constants() {
        assert!(!ThroughputOptimized::DEADLINE_ENFORCED);
        assert!(!ThroughputOptimized::ENERGY_AWARE);
        assert!(!ThroughputOptimized::PRIORITY_CEILING);
        assert!(ThroughputOptimized::PREEMPTION_ALLOWED);
    }

    #[test]
    fn deadline_policy_constants() {
        assert!(DeadlineAware::DEADLINE_ENFORCED);
        assert!(DeadlineAware::ENERGY_AWARE);
        assert!(DeadlineAware::PRIORITY_CEILING);
        assert!(DeadlineAware::PREEMPTION_ALLOWED);
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::HIGHEST < Priority::HIGH);
        assert!(Priority::HIGH < Priority::DEFAULT);
        assert!(Priority::DEFAULT < Priority::LOW);
        assert!(Priority::LOW < Priority::LOWEST);
    }

    #[cfg(feature = "embedded-profile")]
    #[test]
    fn deadline_time_remaining() {
        let deadline = Deadline::from_relative(1000, 5000);
        assert_eq!(deadline.absolute_ns, 6000);
        assert_eq!(deadline.time_remaining(5000), 1000);
        assert_eq!(deadline.time_remaining(6000), 0);
        assert!(!deadline.is_missed(5000));
        assert!(deadline.is_missed(6001));
    }

    #[cfg(feature = "embedded-profile")]
    #[test]
    fn energy_budget_consumption() {
        let mut budget = EnergyBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        assert!(!budget.is_exhausted());

        assert!(budget.consume(500));
        assert_eq!(budget.remaining(), 500);

        assert!(budget.consume(500));
        assert_eq!(budget.remaining(), 0);
        assert!(budget.is_exhausted());

        // Consuming more than remaining
        assert!(!budget.consume(1));
    }
}
