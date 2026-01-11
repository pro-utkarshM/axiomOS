//! Deadline-Oriented Scheduling Policy (Embedded Profile)
//!
//! This policy implements Earliest Deadline First (EDF) scheduling
//! with priority ceiling for BPF programs in embedded deployments.
//!
//! # Compile-Time Erasure
//!
//! This module is only available in embedded profile builds.

use super::policy::{BpfPolicy, SchedError, SchedResult};
use super::queue::{BpfQueue, QueuedProgram};
use crate::profile::EmbeddedProfile;

/// Deadline specification for real-time scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    /// Absolute deadline in nanoseconds since system boot
    pub absolute_ns: u64,
    /// Relative deadline from submission (for reference)
    pub relative_ns: u64,
}

impl Deadline {
    /// Create a new deadline.
    ///
    /// # Arguments
    ///
    /// * `absolute_ns` - Absolute deadline timestamp
    /// * `relative_ns` - Relative deadline from submission
    pub fn new(absolute_ns: u64, relative_ns: u64) -> Self {
        Self {
            absolute_ns,
            relative_ns,
        }
    }

    /// Create a deadline relative to a current time.
    ///
    /// # Arguments
    ///
    /// * `now_ns` - Current time in nanoseconds
    /// * `deadline_ns` - Deadline offset from now
    pub fn from_now(now_ns: u64, deadline_ns: u64) -> Self {
        Self {
            absolute_ns: now_ns.saturating_add(deadline_ns),
            relative_ns: deadline_ns,
        }
    }

    /// Check if the deadline has passed.
    pub fn is_expired(&self, now_ns: u64) -> bool {
        now_ns >= self.absolute_ns
    }

    /// Get time remaining until deadline (0 if expired).
    pub fn time_remaining(&self, now_ns: u64) -> u64 {
        self.absolute_ns.saturating_sub(now_ns)
    }
}

/// Energy budget for power-constrained execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct EnergyBudget {
    /// Maximum energy in microjoules
    pub max_uj: u64,
}

impl EnergyBudget {
    /// Create a new energy budget.
    #[allow(dead_code)]
    pub fn new(max_uj: u64) -> Self {
        Self { max_uj }
    }
}

/// Deadline-oriented scheduling policy.
///
/// Implements Earliest Deadline First (EDF) scheduling for hard
/// real-time BPF program execution. Programs with deadlines are
/// prioritized over those without.
pub struct DeadlinePolicy {
    /// Number of programs executed
    exec_count: u64,
    /// Number of deadline misses
    deadline_misses: u64,
    /// Current time provider (for testing, would use system time in production)
    current_time_ns: u64,
}

impl DeadlinePolicy {
    /// Create a new deadline policy.
    pub fn new() -> Self {
        Self {
            exec_count: 0,
            deadline_misses: 0,
            current_time_ns: 0,
        }
    }

    /// Get the number of programs executed.
    pub fn exec_count(&self) -> u64 {
        self.exec_count
    }

    /// Get the number of deadline misses.
    pub fn deadline_misses(&self) -> u64 {
        self.deadline_misses
    }

    /// Update the current time (for deadline checking).
    ///
    /// In production, this would be called from a timer interrupt
    /// or fetched from a monotonic clock.
    pub fn update_time(&mut self, now_ns: u64) {
        self.current_time_ns = now_ns;
    }

    /// Get the current time.
    pub fn current_time(&self) -> u64 {
        self.current_time_ns
    }

    /// Check if a program's deadline has already passed.
    fn check_deadline(&mut self, program: &QueuedProgram<EmbeddedProfile>) -> bool {
        if let Some(ref deadline) = program
            .deadline
            .as_ref()
            .filter(|d| d.is_expired(self.current_time_ns))
        {
            let _ = deadline; // Acknowledge we checked the deadline
            self.deadline_misses += 1;
            return true;
        }
        false
    }
}

impl Default for DeadlinePolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl BpfPolicy<EmbeddedProfile> for DeadlinePolicy {
    fn select(
        &mut self,
        queue: &mut BpfQueue<EmbeddedProfile>,
    ) -> Option<QueuedProgram<EmbeddedProfile>> {
        // Try to find program with earliest deadline
        let idx = queue.find_earliest_deadline()?;
        let program = queue.remove_at(idx)?;

        // Check for deadline miss
        self.check_deadline(&program);

        self.exec_count += 1;
        Some(program)
    }

    fn admit(
        &self,
        queue: &BpfQueue<EmbeddedProfile>,
        program: &QueuedProgram<EmbeddedProfile>,
    ) -> SchedResult<()> {
        if queue.is_full() {
            return Err(SchedError::QueueFull);
        }

        // Reject programs with already-expired deadlines
        if program
            .deadline
            .as_ref()
            .is_some_and(|d| d.is_expired(self.current_time_ns))
        {
            return Err(SchedError::InvalidDeadline);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use super::*;
    use crate::bytecode::insn::BpfInsn;
    use crate::bytecode::program::{BpfProgType, ProgramBuilder};
    use crate::execution::BpfContext;
    use crate::scheduler::policy::ExecPriority;
    use crate::scheduler::{BpfExecRequest, ProgId};

    fn create_test_program() -> Arc<crate::bytecode::program::BpfProgram<EmbeddedProfile>> {
        let program = ProgramBuilder::<EmbeddedProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 0))
            .insn(BpfInsn::exit())
            .build()
            .expect("valid program");
        Arc::new(program)
    }

    #[test]
    fn deadline_creation() {
        let deadline = Deadline::new(1000, 500);
        assert_eq!(deadline.absolute_ns, 1000);
        assert_eq!(deadline.relative_ns, 500);
    }

    #[test]
    fn deadline_from_now() {
        let deadline = Deadline::from_now(500, 300);
        assert_eq!(deadline.absolute_ns, 800);
        assert_eq!(deadline.relative_ns, 300);
    }

    #[test]
    fn deadline_expiration() {
        let deadline = Deadline::new(1000, 500);

        assert!(!deadline.is_expired(500));
        assert!(!deadline.is_expired(999));
        assert!(deadline.is_expired(1000));
        assert!(deadline.is_expired(2000));
    }

    #[test]
    fn deadline_time_remaining() {
        let deadline = Deadline::new(1000, 500);

        assert_eq!(deadline.time_remaining(0), 1000);
        assert_eq!(deadline.time_remaining(500), 500);
        assert_eq!(deadline.time_remaining(1000), 0);
        assert_eq!(deadline.time_remaining(2000), 0);
    }

    #[test]
    fn deadline_policy_selects_earliest() {
        let mut policy = DeadlinePolicy::new();
        let mut queue = BpfQueue::<EmbeddedProfile>::new();

        // Add programs with different deadlines
        let mut req1 = BpfExecRequest::new(ProgId(1), create_test_program(), BpfContext::empty());
        req1.deadline = Some(Deadline::new(2000, 1000));
        queue.enqueue(QueuedProgram::from_request(req1)).unwrap();

        let mut req2 = BpfExecRequest::new(ProgId(2), create_test_program(), BpfContext::empty());
        req2.deadline = Some(Deadline::new(500, 500)); // Earlier deadline
        queue.enqueue(QueuedProgram::from_request(req2)).unwrap();

        let mut req3 = BpfExecRequest::new(ProgId(3), create_test_program(), BpfContext::empty());
        req3.deadline = Some(Deadline::new(1000, 500));
        queue.enqueue(QueuedProgram::from_request(req3)).unwrap();

        // Should select earliest deadline first
        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(2)); // deadline 500

        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(3)); // deadline 1000

        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(1)); // deadline 2000
    }

    #[test]
    fn deadline_miss_tracking() {
        let mut policy = DeadlinePolicy::new();
        policy.update_time(600); // Time is at 600

        let mut queue = BpfQueue::<EmbeddedProfile>::new();

        // Add program with expired deadline
        let mut req = BpfExecRequest::new(ProgId(1), create_test_program(), BpfContext::empty());
        req.deadline = Some(Deadline::new(500, 500)); // Already expired
        queue.enqueue(QueuedProgram::from_request(req)).unwrap();

        let _prog = policy.select(&mut queue).expect("select");

        // Should have recorded a deadline miss
        assert_eq!(policy.deadline_misses(), 1);
    }

    #[test]
    fn fallback_to_priority_without_deadline() {
        let mut policy = DeadlinePolicy::new();
        let mut queue = BpfQueue::<EmbeddedProfile>::new();

        // Add programs without deadlines
        let req1 = BpfExecRequest::new(ProgId(1), create_test_program(), BpfContext::empty())
            .with_priority(ExecPriority::Low);
        queue.enqueue(QueuedProgram::from_request(req1)).unwrap();

        let req2 = BpfExecRequest::new(ProgId(2), create_test_program(), BpfContext::empty())
            .with_priority(ExecPriority::High);
        queue.enqueue(QueuedProgram::from_request(req2)).unwrap();

        // Should fall back to priority ordering
        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(2)); // Higher priority
    }
}
