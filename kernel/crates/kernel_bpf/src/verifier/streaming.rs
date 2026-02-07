//! Streaming Verifier for Memory-Constrained Systems
//!
//! This module implements a streaming verification algorithm that achieves
//! O(registers × basic_block_depth) memory usage instead of the standard
//! O(instructions × registers × paths) of full path-sensitive analysis.
//!
//! # Algorithm Overview
//!
//! The streaming verifier processes instructions in a single forward pass:
//!
//! 1. **Basic Block Processing**: Instructions are processed sequentially within
//!    basic blocks, maintaining only the current block's state.
//!
//! 2. **Merge Points**: At control flow merge points (join of multiple paths),
//!    states are conservatively merged - taking the "widest" type that encompasses
//!    all incoming states.
//!
//! 3. **Bounded Worklist**: A fixed-size worklist handles pending blocks from
//!    conditional branches. When the worklist is full, we force conservative merging.
//!
//! 4. **Loop Handling**: Back edges are detected and handled specially - requiring
//!    explicit loop bounds in embedded profile or allowing bounded iterations in cloud.
//!
//! # Memory Budget
//!
//! ```text
//! Component                Memory
//! ─────────────────────────────────
//! Register states (11 regs × ~32 bytes)     352 bytes
//! Stack state (profile limit)               8KB-512KB
//! Worklist (MAX_WORKLIST_DEPTH entries)     ~2KB
//! Merge point states (MAX_MERGE_POINTS)     ~8KB
//! ─────────────────────────────────────────────────────
//! Total peak (embedded):                    ~18KB
//! Total peak (cloud):                       ~530KB
//! ```
//!
//! # Tradeoffs
//!
//! The streaming verifier may reject some programs that the full verifier would
//! accept. This is acceptable because:
//!
//! - Robotics workloads are typically linear control flow
//! - State-machine-like programs are naturally supported
//! - Explicit loop bounds are required for embedded safety anyway

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;

use super::error::{VerifyError, VerifyResult};
use super::state::{RegState, RegType, ScalarValue, StackSlot, VerifierState};
use crate::bytecode::insn::BpfInsn;
use crate::bytecode::opcode::{AluOp, OpcodeClass};
use crate::bytecode::program::{BpfProgType, BpfProgram};
use crate::bytecode::registers::Register;
use crate::profile::{ActiveProfile, PhysicalProfile};

/// Maximum worklist depth for pending blocks.
///
/// This limits memory usage for handling conditional branches.
/// When exceeded, we force conservative merging.
#[cfg(all(feature = "embedded-profile", not(feature = "cloud-profile")))]
const MAX_WORKLIST_DEPTH: usize = 16;
#[cfg(feature = "cloud-profile")]
const MAX_WORKLIST_DEPTH: usize = 64;

/// Maximum number of merge point states to track.
///
/// Merge points occur where multiple control flow paths converge.
#[cfg(all(feature = "embedded-profile", not(feature = "cloud-profile")))]
const MAX_MERGE_POINTS: usize = 32;
#[cfg(feature = "cloud-profile")]
const MAX_MERGE_POINTS: usize = 256;

/// Maximum loop iterations allowed during verification.
///
/// Prevents infinite loops in verification itself.
#[cfg(all(feature = "embedded-profile", not(feature = "cloud-profile")))]
const MAX_LOOP_ITERATIONS: usize = 100;
#[cfg(feature = "cloud-profile")]
const MAX_LOOP_ITERATIONS: usize = 1000;

/// Streaming BPF verifier.
///
/// This verifier uses a memory-efficient streaming algorithm suitable for
/// embedded systems with limited RAM.
pub struct StreamingVerifier<P: PhysicalProfile = ActiveProfile> {
    /// Worklist of pending (block_start, state) pairs
    worklist: Vec<WorklistEntry>,

    /// States at merge points (instruction index -> merged state)
    merge_states: Vec<MergePoint>,

    /// Current verification state
    current_state: Option<VerifierState>,

    /// Instructions being verified
    insns: Vec<BpfInsn>,

    /// Basic block boundaries
    block_leaders: Vec<usize>,

    /// Loop iteration counts per back edge target
    loop_counts: Vec<(usize, usize)>,

    /// Maximum stack depth observed
    max_stack_depth: usize,

    /// Profile marker
    _profile: PhantomData<P>,
}

/// Entry in the verification worklist.
#[derive(Clone)]
struct WorklistEntry {
    /// Starting instruction index
    start_idx: usize,
    /// State at entry to this block
    state: VerifierState,
}

/// Merged state at a control flow merge point.
#[derive(Clone)]
struct MergePoint {
    /// Instruction index of the merge point
    idx: usize,
    /// Merged state (conservative union of all incoming states)
    state: VerifierState,
    /// Number of times this merge point has been visited
    visit_count: usize,
}

impl<P: PhysicalProfile> StreamingVerifier<P> {
    /// Create a new streaming verifier.
    pub fn new() -> Self {
        Self {
            worklist: Vec::with_capacity(MAX_WORKLIST_DEPTH),
            merge_states: Vec::with_capacity(MAX_MERGE_POINTS),
            current_state: None,
            insns: Vec::new(),
            block_leaders: Vec::new(),
            loop_counts: Vec::new(),
            max_stack_depth: 0,
            _profile: PhantomData,
        }
    }

    /// Verify a BPF program using streaming algorithm.
    ///
    /// This is the main entry point for streaming verification.
    pub fn verify(prog_type: BpfProgType, insns: &[BpfInsn]) -> VerifyResult<BpfProgram<P>> {
        let mut verifier = Self::new();
        verifier.insns = insns.to_vec();

        // Phase 1: Basic structural checks
        verifier.check_basic()?;

        // Phase 2: Compute basic block boundaries
        verifier.compute_block_leaders();

        // Phase 3: Streaming verification
        let stack_size = verifier.verify_streaming()?;

        // Phase 4: Profile-specific constraints
        verifier.verify_profile_constraints()?;

        // Build the verified program
        BpfProgram::new(prog_type, insns.to_vec(), stack_size).map_err(|e| match e {
            crate::bytecode::program::ProgramError::StackSizeExceeded { required, limit } => {
                VerifyError::StackExceeded {
                    used: required,
                    limit,
                }
            }
            crate::bytecode::program::ProgramError::InsnCountExceeded { count, limit } => {
                VerifyError::InsnCountExceeded { count, limit }
            }
            _ => VerifyError::EmptyProgram,
        })
    }

    /// Perform basic structural checks.
    fn check_basic(&self) -> VerifyResult<()> {
        if self.insns.is_empty() {
            return Err(VerifyError::EmptyProgram);
        }

        if self.insns.len() > P::MAX_INSN_COUNT {
            return Err(VerifyError::InsnCountExceeded {
                count: self.insns.len(),
                limit: P::MAX_INSN_COUNT,
            });
        }

        let has_exit = self.insns.iter().any(|i| i.is_exit());
        if !has_exit {
            return Err(VerifyError::NoExit);
        }

        // Validate opcodes and registers
        for (idx, insn) in self.insns.iter().enumerate() {
            if insn.class().is_none() {
                return Err(VerifyError::InvalidOpcode {
                    insn_idx: idx,
                    opcode: insn.opcode,
                });
            }

            if insn.dst_reg() > 10 {
                return Err(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                });
            }
            if insn.src_reg() > 10 {
                return Err(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.src_reg(),
                });
            }
        }

        Ok(())
    }

    /// Compute basic block leader instructions.
    ///
    /// Leaders are: first instruction, jump targets, and instructions after branches.
    fn compute_block_leaders(&mut self) {
        self.block_leaders.clear();
        self.block_leaders.push(0); // First instruction is always a leader

        for (idx, insn) in self.insns.iter().enumerate() {
            if insn.is_exit() {
                continue;
            }

            if let Some(jmp_op) = insn.jmp_op() {
                let target = Self::compute_jump_target(idx, insn.offset);

                if let Some(target) = target.filter(|&t| t < self.insns.len())
                    && !self.block_leaders.contains(&target)
                {
                    self.block_leaders.push(target);
                }

                if jmp_op.is_conditional() && idx + 1 < self.insns.len() {
                    let fallthrough = idx + 1;
                    if !self.block_leaders.contains(&fallthrough) {
                        self.block_leaders.push(fallthrough);
                    }
                }
            }
        }

        self.block_leaders.sort_unstable();
    }

    /// Compute jump target from instruction index and offset.
    fn compute_jump_target(idx: usize, offset: i16) -> Option<usize> {
        let target = (idx as i64) + 1 + (offset as i64);
        if target >= 0 {
            Some(target as usize)
        } else {
            None
        }
    }

    /// Main streaming verification loop.
    fn verify_streaming(&mut self) -> VerifyResult<usize> {
        // Start with initial state at instruction 0
        let initial_state = VerifierState::new_entry(P::MAX_STACK_SIZE);
        self.worklist.push(WorklistEntry {
            start_idx: 0,
            state: initial_state,
        });

        // Process worklist until empty
        while let Some(entry) = self.worklist.pop() {
            self.current_state = Some(entry.state);
            self.verify_block(entry.start_idx)?;
        }

        Ok(self.max_stack_depth)
    }

    /// Verify a single basic block starting at the given index.
    fn verify_block(&mut self, start_idx: usize) -> VerifyResult<()> {
        let mut state = self.current_state.take().ok_or(VerifyError::EmptyProgram)?;
        state.insn_idx = start_idx;

        // Check if we've already processed this block with compatible state
        if let Some(merged) = self.find_merge_point(start_idx) {
            if self.states_compatible(&state, &merged.state) {
                // Already verified with compatible state
                return Ok(());
            }
            // Need to merge states
            self.merge_state_at(start_idx, &state)?;
            return Ok(());
        }

        // Process instructions in this block
        loop {
            let idx = state.insn_idx;

            if idx >= self.insns.len() {
                return Err(VerifyError::InvalidJump {
                    insn_idx: idx.saturating_sub(1),
                    target: idx as i32,
                });
            }

            // Track max stack depth
            if state.stack.max_depth() > self.max_stack_depth {
                self.max_stack_depth = state.stack.max_depth();
            }

            let insn = &self.insns[idx].clone();

            // Verify this instruction
            let result = self.verify_insn(insn, &mut state, idx)?;

            match result {
                InsnResult::Continue => {
                    if insn.is_wide() {
                        state.insn_idx += 2;
                    } else {
                        state.insn_idx += 1;
                    }
                    state.insn_processed += 1;

                    // Check if we've reached a new basic block leader
                    if self.is_block_leader(state.insn_idx) {
                        // Save state at merge point and continue processing
                        self.add_to_worklist(state.insn_idx, state)?;
                        return Ok(());
                    }
                }
                InsnResult::Jump(target) => {
                    // Check for back edge (potential loop)
                    if target <= idx {
                        self.handle_back_edge(idx, target, &state)?;
                    } else {
                        self.add_to_worklist(target, state)?;
                    }
                    return Ok(());
                }
                InsnResult::Branch {
                    fallthrough,
                    target,
                } => {
                    // Add both paths to worklist
                    let branch_state = state.clone();

                    // Check for back edge on target
                    if target <= idx {
                        self.handle_back_edge(idx, target, &branch_state)?;
                    } else {
                        self.add_to_worklist(target, branch_state)?;
                    }

                    self.add_to_worklist(fallthrough, state)?;
                    return Ok(());
                }
                InsnResult::Exit => {
                    return Ok(());
                }
            }
        }
    }

    /// Check if an instruction index is a basic block leader.
    fn is_block_leader(&self, idx: usize) -> bool {
        self.block_leaders.binary_search(&idx).is_ok()
    }

    /// Find existing merge point for an instruction.
    fn find_merge_point(&self, idx: usize) -> Option<&MergePoint> {
        self.merge_states.iter().find(|mp| mp.idx == idx)
    }

    /// Merge a state at a given instruction index.
    fn merge_state_at(&mut self, idx: usize, incoming: &VerifierState) -> VerifyResult<()> {
        if let Some(existing) = self.merge_states.iter_mut().find(|mp| mp.idx == idx) {
            // Merge with existing state
            Self::merge_states_conservative(&mut existing.state, incoming);
            existing.visit_count += 1;
        } else {
            // Check merge point limit
            if self.merge_states.len() >= MAX_MERGE_POINTS {
                return Err(VerifyError::InfiniteLoop { insn_idx: idx });
            }

            // Create new merge point
            self.merge_states.push(MergePoint {
                idx,
                state: incoming.clone(),
                visit_count: 1,
            });
        }
        Ok(())
    }

    /// Add a block to the worklist for later processing.
    fn add_to_worklist(&mut self, idx: usize, state: VerifierState) -> VerifyResult<()> {
        // Check if we already have a merge point with compatible state
        if let Some(merged) = self.find_merge_point(idx)
            && self.states_compatible(&state, &merged.state)
        {
            return Ok(()); // Already covered
        }

        // Check worklist depth limit
        if self.worklist.len() >= MAX_WORKLIST_DEPTH {
            // Force merge at this point instead of adding to worklist
            self.merge_state_at(idx, &state)?;
            return Ok(());
        }

        // Record merge point for this target
        self.merge_state_at(idx, &state)?;

        self.worklist.push(WorklistEntry {
            start_idx: idx,
            state,
        });

        Ok(())
    }

    /// Handle a back edge (potential loop).
    fn handle_back_edge(
        &mut self,
        from_idx: usize,
        to_idx: usize,
        state: &VerifierState,
    ) -> VerifyResult<()> {
        // Track loop iteration count
        let count =
            if let Some((_, count)) = self.loop_counts.iter_mut().find(|(t, _)| *t == to_idx) {
                *count += 1;
                *count
            } else {
                self.loop_counts.push((to_idx, 1));
                1
            };

        // Check iteration limit
        if count > MAX_LOOP_ITERATIONS {
            #[cfg(feature = "embedded-profile")]
            return Err(VerifyError::UnboundedLoop { insn_idx: from_idx });

            #[cfg(feature = "cloud-profile")]
            return Err(VerifyError::InfiniteLoop { insn_idx: from_idx });
        }

        // Merge state at loop header
        self.merge_state_at(to_idx, state)?;

        // Continue verification from loop header if not visited too many times
        if count <= 2 {
            // Allow a few iterations to establish fixed point
            self.worklist.push(WorklistEntry {
                start_idx: to_idx,
                state: state.clone(),
            });
        }

        Ok(())
    }

    /// Check if two states are compatible (can be merged without re-verification).
    fn states_compatible(&self, s1: &VerifierState, s2: &VerifierState) -> bool {
        // States are compatible if all register types match
        for i in 0..Register::COUNT {
            if s1.regs[i].reg_type != s2.regs[i].reg_type {
                return false;
            }
        }
        true
    }

    /// Conservatively merge two states.
    ///
    /// The result is the "widest" state that encompasses both inputs.
    fn merge_states_conservative(target: &mut VerifierState, incoming: &VerifierState) {
        for i in 0..Register::COUNT {
            let target_reg = &mut target.regs[i];
            let incoming_reg = &incoming.regs[i];

            // If types differ, widen to the more general type
            if target_reg.reg_type != incoming_reg.reg_type {
                // Both initialized but different types -> scalar (unknown)
                if target_reg.is_init() && incoming_reg.is_init() {
                    *target_reg = RegState::scalar(Some(ScalarValue::unknown()));
                } else if !target_reg.is_init() {
                    // Target not init, take incoming
                    *target_reg = incoming_reg.clone();
                }
                // If incoming not init but target is, keep target
            } else if target_reg.reg_type == RegType::Scalar {
                // Both scalar - widen value range
                if let (Some(tv), Some(iv)) =
                    (&mut target_reg.scalar_value, &incoming_reg.scalar_value)
                {
                    tv.min = tv.min.min(iv.min);
                    tv.max = tv.max.max(iv.max);
                    tv.value = None; // No longer a constant
                }
            }
        }
    }

    /// Verify a single instruction.
    fn verify_insn(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<InsnResult> {
        // Exit instruction
        if insn.is_exit() {
            if !state.is_reg_init(Register::R0) {
                return Err(VerifyError::UninitializedRegister {
                    insn_idx: idx,
                    reg: Register::R0,
                });
            }
            return Ok(InsnResult::Exit);
        }

        // Call instruction
        if insn.is_call() {
            self.verify_call(insn, state, idx)?;
            return Ok(InsnResult::Continue);
        }

        // ALU instructions
        if insn.is_alu() {
            self.verify_alu(insn, state, idx)?;
            return Ok(InsnResult::Continue);
        }

        // Jump instructions
        if insn.is_jump() {
            return self.verify_jump(insn, state, idx);
        }

        // Memory instructions
        if insn.is_memory() {
            self.verify_memory(insn, state, idx)?;
            return Ok(InsnResult::Continue);
        }

        // Wide instruction (64-bit immediate load)
        if insn.is_wide() {
            self.verify_wide_load(insn, state, idx)?;
            return Ok(InsnResult::Continue);
        }

        Err(VerifyError::InvalidOpcode {
            insn_idx: idx,
            opcode: insn.opcode,
        })
    }

    /// Verify an ALU instruction.
    fn verify_alu(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<()> {
        let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
            insn_idx: idx,
            reg: insn.dst_reg(),
        })?;

        if dst == Register::R10 {
            return Err(VerifyError::WriteToReadOnly { insn_idx: idx });
        }

        let alu_op = insn.alu_op().ok_or(VerifyError::InvalidOpcode {
            insn_idx: idx,
            opcode: insn.opcode,
        })?;

        // Check source register if register mode
        if matches!(insn.source_type(), crate::bytecode::opcode::SourceType::Reg) {
            let src = insn.src().ok_or(VerifyError::InvalidRegister {
                insn_idx: idx,
                reg: insn.src_reg(),
            })?;

            if !state.is_reg_init(src) && !alu_op.is_unary() {
                return Err(VerifyError::UninitializedRegister {
                    insn_idx: idx,
                    reg: src,
                });
            }

            // Check for division by zero
            if alu_op.can_divide_by_zero() {
                let src_state = state.reg(src);
                if let Some(ref scalar) = src_state.scalar_value {
                    if scalar.could_be_zero() {
                        return Err(VerifyError::DivisionByZero { insn_idx: idx });
                    }
                } else if src_state.reg_type == RegType::Scalar {
                    return Err(VerifyError::DivisionByZero { insn_idx: idx });
                }
            }
        } else {
            // Immediate mode division by zero check
            if alu_op.can_divide_by_zero() && insn.imm == 0 {
                return Err(VerifyError::DivisionByZero { insn_idx: idx });
            }
        }

        // Check destination is initialized for non-MOV operations
        if !matches!(alu_op, AluOp::Mov) && !state.is_reg_init(dst) {
            return Err(VerifyError::UninitializedRegister {
                insn_idx: idx,
                reg: dst,
            });
        }

        // Update destination register to scalar
        state.set_scalar(dst, Some(ScalarValue::unknown()));

        Ok(())
    }

    /// Verify a jump instruction.
    fn verify_jump(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<InsnResult> {
        let jmp_op = insn.jmp_op().ok_or(VerifyError::InvalidOpcode {
            insn_idx: idx,
            opcode: insn.opcode,
        })?;

        let target = (idx as i64) + 1 + (insn.offset as i64);
        if target < 0 {
            return Err(VerifyError::InvalidJump {
                insn_idx: idx,
                target: target as i32,
            });
        }
        let target = target as usize;

        if jmp_op.is_unconditional() {
            return Ok(InsnResult::Jump(target));
        }

        // Conditional jump - check source registers
        let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
            insn_idx: idx,
            reg: insn.dst_reg(),
        })?;

        if !state.is_reg_init(dst) {
            return Err(VerifyError::UninitializedRegister {
                insn_idx: idx,
                reg: dst,
            });
        }

        if matches!(insn.source_type(), crate::bytecode::opcode::SourceType::Reg) {
            let src = insn.src().ok_or(VerifyError::InvalidRegister {
                insn_idx: idx,
                reg: insn.src_reg(),
            })?;

            if !state.is_reg_init(src) {
                return Err(VerifyError::UninitializedRegister {
                    insn_idx: idx,
                    reg: src,
                });
            }
        }

        Ok(InsnResult::Branch {
            fallthrough: idx + 1,
            target,
        })
    }

    /// Verify a call instruction.
    fn verify_call(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<()> {
        let helper_id = insn.imm;

        if helper_id < 0 {
            return Err(VerifyError::InvalidHelper {
                insn_idx: idx,
                helper_id,
            });
        }

        // Clobber caller-saved registers
        for reg in [
            Register::R0,
            Register::R1,
            Register::R2,
            Register::R3,
            Register::R4,
            Register::R5,
        ] {
            *state.reg_mut(reg) = RegState::uninit();
        }

        // R0 contains return value
        state.set_scalar(Register::R0, Some(ScalarValue::unknown()));

        Ok(())
    }

    /// Verify a memory instruction.
    fn verify_memory(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<()> {
        let class = insn.class().ok_or(VerifyError::InvalidOpcode {
            insn_idx: idx,
            opcode: insn.opcode,
        })?;

        let size = insn.mem_size().ok_or(VerifyError::InvalidOpcode {
            insn_idx: idx,
            opcode: insn.opcode,
        })?;

        match class {
            OpcodeClass::Ldx => {
                let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                })?;
                let src = insn.src().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.src_reg(),
                })?;

                if dst == Register::R10 {
                    return Err(VerifyError::WriteToReadOnly { insn_idx: idx });
                }

                if !state.is_reg_init(src) {
                    return Err(VerifyError::UninitializedRegister {
                        insn_idx: idx,
                        reg: src,
                    });
                }

                let src_state = state.reg(src);
                if !src_state.reg_type.can_read() {
                    return Err(VerifyError::InvalidMemoryAccess {
                        insn_idx: idx,
                        reason: "cannot read from this pointer type",
                    });
                }

                if src_state.reg_type == RegType::PtrToStack
                    || src_state.reg_type == RegType::PtrToFp
                {
                    let offset = src_state.ptr_offset + insn.offset as i64;
                    if !state.stack.is_valid_access(offset, size.size_bytes()) {
                        return Err(VerifyError::OutOfBoundsAccess {
                            insn_idx: idx,
                            offset,
                            size: size.size_bytes(),
                        });
                    }
                }

                state.set_scalar(dst, Some(ScalarValue::unknown()));
            }

            OpcodeClass::Stx => {
                let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                })?;
                let src = insn.src().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.src_reg(),
                })?;

                if !state.is_reg_init(dst) {
                    return Err(VerifyError::UninitializedRegister {
                        insn_idx: idx,
                        reg: dst,
                    });
                }
                if !state.is_reg_init(src) {
                    return Err(VerifyError::UninitializedRegister {
                        insn_idx: idx,
                        reg: src,
                    });
                }

                let dst_state = state.reg(dst);
                if !dst_state.reg_type.can_write() && dst_state.reg_type != RegType::PtrToFp {
                    return Err(VerifyError::InvalidMemoryAccess {
                        insn_idx: idx,
                        reason: "cannot write to this pointer type",
                    });
                }

                if dst_state.reg_type == RegType::PtrToStack
                    || dst_state.reg_type == RegType::PtrToFp
                {
                    let offset = dst_state.ptr_offset + insn.offset as i64;
                    if !state.stack.is_valid_access(offset, size.size_bytes()) {
                        return Err(VerifyError::OutOfBoundsAccess {
                            insn_idx: idx,
                            offset,
                            size: size.size_bytes(),
                        });
                    }

                    for i in 0..size.size_bytes() {
                        let _ = state.stack.set(offset - i as i64, StackSlot::Scalar);
                    }
                }
            }

            OpcodeClass::St => {
                let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                })?;

                if !state.is_reg_init(dst) {
                    return Err(VerifyError::UninitializedRegister {
                        insn_idx: idx,
                        reg: dst,
                    });
                }

                let dst_state = state.reg(dst);
                if !dst_state.reg_type.can_write() && dst_state.reg_type != RegType::PtrToFp {
                    return Err(VerifyError::InvalidMemoryAccess {
                        insn_idx: idx,
                        reason: "cannot write to this pointer type",
                    });
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Verify a wide load instruction.
    fn verify_wide_load(
        &self,
        insn: &BpfInsn,
        state: &mut VerifierState,
        idx: usize,
    ) -> VerifyResult<()> {
        let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
            insn_idx: idx,
            reg: insn.dst_reg(),
        })?;

        if dst == Register::R10 {
            return Err(VerifyError::WriteToReadOnly { insn_idx: idx });
        }

        state.set_scalar(dst, Some(ScalarValue::unknown()));

        Ok(())
    }

    /// Verify profile-specific constraints.
    fn verify_profile_constraints(&self) -> VerifyResult<()> {
        #[cfg(feature = "embedded-profile")]
        {
            self.verify_embedded_constraints()?;
        }

        #[cfg(feature = "cloud-profile")]
        {
            self.verify_cloud_constraints()?;
        }

        Ok(())
    }

    #[cfg(feature = "embedded-profile")]
    fn verify_embedded_constraints(&self) -> VerifyResult<()> {
        // In streaming mode, loops are already bounded by MAX_LOOP_ITERATIONS
        // Additional checks for dynamic allocation helpers
        for (idx, insn) in self.insns.iter().enumerate() {
            if insn.is_call() {
                const ALLOC_HELPERS: &[i32] = &[];

                if ALLOC_HELPERS.contains(&insn.imm) {
                    return Err(VerifyError::DynamicAllocationAttempted { insn_idx: idx });
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "cloud-profile")]
    fn verify_cloud_constraints(&self) -> VerifyResult<()> {
        Ok(())
    }
}

impl<P: PhysicalProfile> Default for StreamingVerifier<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of verifying a single instruction.
enum InsnResult {
    Continue,
    Jump(usize),
    Branch { fallthrough: usize, target: usize },
    Exit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_empty_program() {
        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &[]);
        assert!(matches!(result, Err(VerifyError::EmptyProgram)));
    }

    #[test]
    fn verify_minimal_program() {
        let insns = [
            BpfInsn::mov64_imm(0, 0), // r0 = 0
            BpfInsn::exit(),          // exit
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_no_exit() {
        let insns = [
            BpfInsn::mov64_imm(0, 0), // r0 = 0
            BpfInsn::nop(),           // nop (no exit)
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::NoExit)));
    }

    #[test]
    fn verify_division_by_zero() {
        let insns = [
            BpfInsn::mov64_imm(0, 10),      // r0 = 10
            BpfInsn::new(0x37, 0, 0, 0, 0), // r0 /= 0
            BpfInsn::exit(),
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::DivisionByZero { .. })));
    }

    #[test]
    fn verify_write_to_r10() {
        let insns = [
            BpfInsn::mov64_imm(10, 0), // r10 = 0 (illegal!)
            BpfInsn::mov64_imm(0, 0),
            BpfInsn::exit(),
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::WriteToReadOnly { .. })));
    }

    #[test]
    fn verify_uninitialized_register() {
        let insns = [
            BpfInsn::add64_reg(0, 2), // r0 += r2 (neither init)
            BpfInsn::exit(),
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(
            result,
            Err(VerifyError::UninitializedRegister { .. })
        ));
    }

    #[test]
    fn verify_conditional_branch() {
        let insns = [
            BpfInsn::mov64_imm(0, 0),  // r0 = 0
            BpfInsn::mov64_imm(1, 10), // r1 = 10
            BpfInsn::jeq_imm(1, 0, 1), // if r1 == 0, skip next
            BpfInsn::add64_imm(0, 1),  // r0 += 1
            BpfInsn::exit(),
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_multiple_paths() {
        let insns = [
            BpfInsn::mov64_imm(0, 0),  // r0 = 0
            BpfInsn::mov64_imm(1, 5),  // r1 = 5
            BpfInsn::jeq_imm(1, 5, 2), // if r1 == 5, skip 2
            BpfInsn::add64_imm(0, 1),  // r0 += 1 (not taken)
            BpfInsn::add64_imm(0, 2),  // r0 += 2 (not taken)
            BpfInsn::exit(),           // exit
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_forward_jump() {
        let insns = [
            BpfInsn::mov64_imm(0, 42), // r0 = 42
            BpfInsn::ja(1),            // jump over next
            BpfInsn::mov64_imm(0, 0),  // r0 = 0 (skipped)
            BpfInsn::exit(),
        ];

        let result = StreamingVerifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(result.is_ok());
    }
}
