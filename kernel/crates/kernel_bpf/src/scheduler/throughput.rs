//! Throughput-Oriented Scheduling Policy (Cloud Profile)
//!
//! This policy prioritizes throughput and fairness, allowing BPF
//! programs to run with cooperative preemption and soft latency bounds.
//!
//! # Compile-Time Erasure
//!
//! This module is only available in cloud profile builds.

use super::policy::{BpfPolicy, SchedResult};
use super::queue::{BpfQueue, QueuedProgram};
use crate::profile::CloudProfile;

/// Throughput-oriented scheduling policy.
///
/// Programs are selected based on priority, with FIFO ordering
/// within the same priority level. This provides fair scheduling
/// while respecting priority hints.
pub struct ThroughputPolicy {
    /// Number of programs executed
    exec_count: u64,
}

impl ThroughputPolicy {
    /// Create a new throughput policy.
    pub fn new() -> Self {
        Self { exec_count: 0 }
    }

    /// Get the number of programs executed.
    pub fn exec_count(&self) -> u64 {
        self.exec_count
    }
}

impl Default for ThroughputPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl BpfPolicy<CloudProfile> for ThroughputPolicy {
    fn select(
        &mut self,
        queue: &mut BpfQueue<CloudProfile>,
    ) -> Option<QueuedProgram<CloudProfile>> {
        let idx = queue.find_highest_priority()?;
        let program = queue.remove_at(idx)?;
        self.exec_count += 1;
        Some(program)
    }

    fn admit(
        &self,
        queue: &BpfQueue<CloudProfile>,
        _program: &QueuedProgram<CloudProfile>,
    ) -> SchedResult<()> {
        if queue.is_full() {
            return Err(super::policy::SchedError::QueueFull);
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
    use crate::scheduler::{BpfExecRequest, ExecPriority, ProgId};

    fn create_test_program() -> Arc<crate::bytecode::program::BpfProgram<CloudProfile>> {
        let program = ProgramBuilder::<CloudProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 0))
            .insn(BpfInsn::exit())
            .build()
            .expect("valid program");
        Arc::new(program)
    }

    #[test]
    fn throughput_policy_selects_highest_priority() {
        let mut policy = ThroughputPolicy::new();
        let mut queue = BpfQueue::<CloudProfile>::new();

        // Add programs with different priorities
        let req1 = BpfExecRequest::new(ProgId(1), create_test_program(), BpfContext::empty())
            .with_priority(ExecPriority::Low);
        queue.enqueue(QueuedProgram::from_request(req1)).unwrap();

        let req2 = BpfExecRequest::new(ProgId(2), create_test_program(), BpfContext::empty())
            .with_priority(ExecPriority::High);
        queue.enqueue(QueuedProgram::from_request(req2)).unwrap();

        let req3 = BpfExecRequest::new(ProgId(3), create_test_program(), BpfContext::empty())
            .with_priority(ExecPriority::Normal);
        queue.enqueue(QueuedProgram::from_request(req3)).unwrap();

        // Should select highest priority first
        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(2));
        assert_eq!(policy.exec_count(), 1);

        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(3));

        let prog = policy.select(&mut queue).expect("select");
        assert_eq!(prog.id, ProgId(1));

        assert_eq!(policy.exec_count(), 3);
    }

    #[test]
    fn throughput_policy_fifo_same_priority() {
        let mut policy = ThroughputPolicy::new();
        let mut queue = BpfQueue::<CloudProfile>::new();

        // Add programs with same priority
        for i in 1..=3 {
            let req = BpfExecRequest::new(ProgId(i), create_test_program(), BpfContext::empty())
                .with_priority(ExecPriority::Normal);
            queue.enqueue(QueuedProgram::from_request(req)).unwrap();
        }

        // Should be FIFO
        assert_eq!(policy.select(&mut queue).unwrap().id, ProgId(1));
        assert_eq!(policy.select(&mut queue).unwrap().id, ProgId(2));
        assert_eq!(policy.select(&mut queue).unwrap().id, ProgId(3));
    }
}
