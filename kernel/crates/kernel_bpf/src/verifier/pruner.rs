//! State pruning for the path-sensitive verifier.
//!
//! The full path-sensitive verifier explores program states forward
//! through the CFG, re-exploring a basic block once per distinct entry
//! state. Without pruning, a program with N conditional branches has up
//! to 2^N reachable state shapes and verification time blows up
//! exponentially. State pruning collapses redundant exploration: if
//! we've already explored a state at this pc that is at least as
//! general as the one we're about to explore, skip it.
//!
//! This module provides the bookkeeping (`StatePruner`) and the
//! subsumption check (`StateSubsumes`) that decides whether one state
//! covers another. The verifier core consumes this through
//! `pruner.check_or_record(pc, &state)`. The decision is:
//!
//!   * `Prune` — a previously-explored state at this pc subsumes the
//!     current one; do not re-explore.
//!   * `Continue` — no prior state subsumes; record the current state
//!     and keep exploring.
//!
//! Reference: Linux `kernel/bpf/verifier.c` `is_state_visited` /
//! `regsafe`. The shape and semantics mirror that work; the
//! implementation is Rust-native and integrates with our existing
//! `RegState` / `VerifierState` types from `verifier::state`.
//!
//! ## Wiring status
//!
//! The pruner is implemented and tested but not yet consumed by
//! `core::Verifier::verify_safety`. The integration is intentionally a
//! separate change so we can validate the pruner in isolation first.
//! Tracked in #83.

use alloc::vec::Vec;

use super::state::{RegState, RegType, VerifierState};
use crate::bytecode::registers::Register;

/// Result of consulting the pruner before re-exploring a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruneDecision {
    /// No previously-recorded state at this pc subsumes the current
    /// one. The pruner has recorded the current state; the caller
    /// should continue exploration.
    Continue,

    /// A previously-explored state subsumes the current one. The
    /// caller should skip this branch.
    Prune,
}

/// Trait for "state `self` is at least as general as state `other`."
///
/// Lifted out of [`RegState`] / [`VerifierState`] so the verifier
/// core and tests can both consult it without going through the
/// pruner machinery. Implementations must be sound — if `a.subsumes(b)`
/// returns true then every concrete value that satisfies `b` must also
/// satisfy `a`. False negatives are permitted (lose precision); false
/// positives are not (unsoundly prune real bugs).
pub trait StateSubsumes<Rhs = Self> {
    fn subsumes(&self, other: &Rhs) -> bool;
}

impl StateSubsumes for RegState {
    fn subsumes(&self, other: &Self) -> bool {
        // Different types are not comparable. `NotInit` is special — if we
        // previously had it uninit, anything more specific is "more
        // initialised" and we should NOT prune (we'd be claiming a state
        // that observes a value is covered by a state that doesn't, which
        // is unsound).
        if self.reg_type != other.reg_type {
            return false;
        }

        match self.reg_type {
            RegType::NotInit => true,

            RegType::Scalar => {
                let lhs = self.scalar_value.as_ref();
                let rhs = other.scalar_value.as_ref();
                match (lhs, rhs) {
                    (None, _) => true, // We had "unknown scalar"; anything fits.
                    (Some(_), None) => false,
                    (Some(a), Some(b)) => {
                        // Interval check: our range must cover theirs.
                        let interval_ok = a.min <= b.min && a.max >= b.max;
                        // tnum check: our tnum must subsume theirs.
                        let tnum_ok = a.tnum.subsumes(&b.tnum);
                        interval_ok && tnum_ok
                    }
                }
            }

            // Pointer types: same type plus same offset is required for
            // subsumption. A real implementation would track range on
            // pointer arithmetic and allow our range to cover theirs; for
            // now we require exact offset, which is sound but conservative.
            _ => self.ptr_offset == other.ptr_offset && self.map_id == other.map_id,
        }
    }
}

impl StateSubsumes for VerifierState {
    fn subsumes(&self, other: &Self) -> bool {
        // Every register's state in `self` must subsume the corresponding
        // register in `other`. Stack subsumption deferred to a follow-up;
        // for now we require stack states to match exactly.
        for r in 0..Register::COUNT {
            if !self.regs[r].subsumes(&other.regs[r]) {
                return false;
            }
        }
        // Conservative stack equality — refine when we add tracked stack
        // ranges. Documented as a precision opportunity in the test below.
        if self.stack.max_depth() != other.stack.max_depth() {
            return false;
        }
        for offset in -(self.stack.max_depth() as i64)..0 {
            if self.stack.get(offset) != other.stack.get(offset) {
                return false;
            }
        }
        true
    }
}

/// Records explored states keyed by program counter so we can avoid
/// re-exploring redundant ones. Memory consumption is bounded by
/// `(distinct states) × (RegState cost)`; on a real program the verifier
/// is expected to converge to a small set of state shapes per pc.
///
/// For now this is keyed only by pc — multiple states at the same pc
/// are kept in a small per-pc list and the pruner walks them. Linux's
/// verifier uses a more elaborate hash + bucket structure; we'll move
/// to that if profile data shows the linear walk dominating.
#[derive(Debug, Default)]
pub struct StatePruner {
    /// For each pc, the set of states we've already explored.
    by_pc: alloc::collections::BTreeMap<usize, Vec<VerifierState>>,
}

impl StatePruner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consult the pruner with the current `state` at program counter
    /// `pc`. If any previously-recorded state at this pc subsumes the
    /// current one, return [`PruneDecision::Prune`]. Otherwise record
    /// the current state and return [`PruneDecision::Continue`].
    pub fn check_or_record(&mut self, pc: usize, state: &VerifierState) -> PruneDecision {
        let entries = self.by_pc.entry(pc).or_default();
        for prior in entries.iter() {
            if prior.subsumes(state) {
                return PruneDecision::Prune;
            }
        }
        entries.push(state.clone());
        PruneDecision::Continue
    }

    /// Drop all recorded state. Useful between independent program
    /// verifications when the pruner is held in a long-lived context.
    pub fn clear(&mut self) {
        self.by_pc.clear();
    }

    /// Total number of recorded states across all pcs. Diagnostic only.
    pub fn recorded(&self) -> usize {
        self.by_pc.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::registers::Register;
    use crate::verifier::state::{ScalarValue, TnumValue};

    fn entry_state() -> VerifierState {
        VerifierState::new_entry(512)
    }

    #[test]
    fn empty_pruner_records_first_state() {
        let mut pruner = StatePruner::new();
        let s = entry_state();
        assert_eq!(pruner.check_or_record(0, &s), PruneDecision::Continue);
        assert_eq!(pruner.recorded(), 1);
    }

    #[test]
    fn identical_state_is_pruned() {
        let mut pruner = StatePruner::new();
        let s = entry_state();
        pruner.check_or_record(0, &s);
        assert_eq!(pruner.check_or_record(0, &s), PruneDecision::Prune);
    }

    #[test]
    fn different_pc_is_not_pruned() {
        let mut pruner = StatePruner::new();
        let s = entry_state();
        pruner.check_or_record(0, &s);
        // Same state at a different pc — explore separately.
        assert_eq!(pruner.check_or_record(1, &s), PruneDecision::Continue);
    }

    #[test]
    fn unknown_scalar_subsumes_constant_at_same_pc() {
        let mut pruner = StatePruner::new();

        // First exploration arrives with r0 = unknown scalar
        let mut s_general = entry_state();
        s_general.regs[Register::R0 as usize] = RegState::scalar(Some(ScalarValue::unknown()));
        pruner.check_or_record(0, &s_general);

        // Second exploration arrives at the same pc with r0 = constant 7.
        // The previously-seen unknown state is more general; prune.
        let mut s_specific = entry_state();
        s_specific.regs[Register::R0 as usize] = RegState::scalar(Some(ScalarValue::constant(7)));
        assert_eq!(pruner.check_or_record(0, &s_specific), PruneDecision::Prune);
    }

    #[test]
    fn constant_does_not_subsume_unknown() {
        let mut pruner = StatePruner::new();

        let mut s_specific = entry_state();
        s_specific.regs[Register::R0 as usize] = RegState::scalar(Some(ScalarValue::constant(7)));
        pruner.check_or_record(0, &s_specific);

        let mut s_general = entry_state();
        s_general.regs[Register::R0 as usize] = RegState::scalar(Some(ScalarValue::unknown()));
        assert_eq!(
            pruner.check_or_record(0, &s_general),
            PruneDecision::Continue
        );
    }

    #[test]
    fn different_reg_types_do_not_subsume() {
        let r_scalar = RegState::scalar(Some(ScalarValue::unknown()));
        let r_stack = RegState::stack_ptr(-8);
        assert!(!r_scalar.subsumes(&r_stack));
        assert!(!r_stack.subsumes(&r_scalar));
    }

    #[test]
    fn tnum_subsumption_lifts_to_regstate() {
        // r0_a: low 8 bits unknown, rest known zero, range [0, 255]
        let mut sv_a = ScalarValue::unknown();
        sv_a.min = 0;
        sv_a.max = 255;
        sv_a.tnum = TnumValue {
            value: 0,
            mask: 0xff,
        };

        // r0_b: known constant 42, which fits in r0_a's tnum + interval
        let sv_b = ScalarValue::constant(42);

        let a = RegState::scalar(Some(sv_a));
        let b = RegState::scalar(Some(sv_b));

        assert!(
            a.subsumes(&b),
            "wider tnum + interval should subsume a contained constant"
        );
        assert!(!b.subsumes(&a), "constant cannot subsume wider tnum");
    }

    #[test]
    fn recorded_count_grows_only_on_continue() {
        let mut pruner = StatePruner::new();
        let s = entry_state();

        pruner.check_or_record(0, &s); // Continue
        pruner.check_or_record(0, &s); // Prune
        pruner.check_or_record(1, &s); // Continue (different pc)
        pruner.check_or_record(1, &s); // Prune

        assert_eq!(pruner.recorded(), 2);
    }
}
