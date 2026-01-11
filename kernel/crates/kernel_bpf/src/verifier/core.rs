//! Core Verifier Implementation
//!
//! The verifier performs static analysis of BPF programs to ensure safety.
//! It tracks register types, validates memory accesses, and enforces
//! profile-specific constraints.

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;

use super::cfg::ControlFlowGraph;
use super::error::{VerifyError, VerifyResult};
use super::state::{RegState, RegType, ScalarValue, StackSlot, VerifierState};
use crate::bytecode::insn::BpfInsn;
use crate::bytecode::opcode::{AluOp, OpcodeClass};
use crate::bytecode::program::{BpfProgType, BpfProgram};
use crate::bytecode::registers::Register;
use crate::profile::{ActiveProfile, PhysicalProfile};

/// BPF program verifier.
///
/// The verifier ensures that BPF programs are safe to execute by performing
/// static analysis. It is parameterized by the physical profile, which
/// determines the constraints to enforce.
pub struct Verifier<P: PhysicalProfile = ActiveProfile> {
    /// Control flow graph
    cfg: Option<ControlFlowGraph>,

    /// Verifier states at each instruction (for path-sensitive analysis)
    states: Vec<Option<VerifierState>>,

    /// Profile marker
    _profile: PhantomData<P>,
}

impl<P: PhysicalProfile> Verifier<P> {
    /// Create a new verifier.
    pub fn new() -> Self {
        Self {
            cfg: None,
            states: Vec::new(),
            _profile: PhantomData,
        }
    }

    /// Verify a BPF program.
    ///
    /// This is the main entry point for verification. It performs:
    /// 1. Basic structural checks
    /// 2. CFG construction
    /// 3. Core safety verification
    /// 4. Profile-specific constraint checks
    ///
    /// # Returns
    ///
    /// On success, returns a validated `BpfProgram`.
    /// On failure, returns a `VerifyError` describing the issue.
    pub fn verify(prog_type: BpfProgType, insns: &[BpfInsn]) -> VerifyResult<BpfProgram<P>> {
        let mut verifier = Self::new();

        // Phase 1: Basic checks
        verifier.check_basic(insns)?;

        // Phase 2: Build CFG
        let cfg = ControlFlowGraph::build(insns);
        verifier.cfg = Some(cfg);

        // Phase 3: Core safety verification
        let stack_size = verifier.verify_safety(insns)?;

        // Phase 4: Profile-specific constraints
        verifier.verify_profile_constraints(insns)?;

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
    fn check_basic(&self, insns: &[BpfInsn]) -> VerifyResult<()> {
        // Empty program check
        if insns.is_empty() {
            return Err(VerifyError::EmptyProgram);
        }

        // Instruction count check
        if insns.len() > P::MAX_INSN_COUNT {
            return Err(VerifyError::InsnCountExceeded {
                count: insns.len(),
                limit: P::MAX_INSN_COUNT,
            });
        }

        // Check for exit instruction
        let has_exit = insns.iter().any(|i| i.is_exit());
        if !has_exit {
            return Err(VerifyError::NoExit);
        }

        // Validate all opcodes
        for (idx, insn) in insns.iter().enumerate() {
            if insn.class().is_none() {
                return Err(VerifyError::InvalidOpcode {
                    insn_idx: idx,
                    opcode: insn.opcode,
                });
            }

            // Check register validity
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

    /// Verify program safety (core checks for all profiles).
    fn verify_safety(&mut self, insns: &[BpfInsn]) -> VerifyResult<usize> {
        let cfg = self.cfg.as_ref().unwrap();

        // Check reachability
        let reachable = cfg.reachable_instructions();
        for idx in 0..insns.len() {
            // Skip wide instruction continuations
            if idx > 0 && insns[idx - 1].is_wide() {
                continue;
            }

            if !reachable.contains(&idx) && !insns[idx].is_exit() {
                return Err(VerifyError::UnreachableInstruction { insn_idx: idx });
            }
        }

        // Initialize states
        self.states = alloc::vec![None; insns.len()];

        // Start verification from entry
        let initial_state = VerifierState::new_entry(P::MAX_STACK_SIZE);
        self.verify_path(insns, 0, initial_state)?;

        // Return computed stack size
        let max_stack = self
            .states
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.stack.max_depth())
            .max()
            .unwrap_or(0);

        Ok(max_stack)
    }

    /// Verify a single execution path.
    fn verify_path(
        &mut self,
        insns: &[BpfInsn],
        start_idx: usize,
        mut state: VerifierState,
    ) -> VerifyResult<()> {
        state.insn_idx = start_idx;

        loop {
            let idx = state.insn_idx;

            // Bounds check
            if idx >= insns.len() {
                return Err(VerifyError::InvalidJump {
                    insn_idx: idx.saturating_sub(1),
                    target: idx as i32,
                });
            }

            // Check for infinite loops (visited same instruction too many times)
            if state.insn_processed > P::MAX_INSN_COUNT {
                return Err(VerifyError::InfiniteLoop { insn_idx: idx });
            }

            // Merge or store state
            if let Some(existing) = &self.states[idx] {
                // Already verified this path with compatible state
                if self.states_compatible(&state, existing) {
                    return Ok(());
                }
                // Different state - would need full state merging for production
                // For now, just continue
            }
            self.states[idx] = Some(state.clone());

            let insn = &insns[idx];

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
                }
                InsnResult::Jump(target) => {
                    state.insn_idx = target;
                    state.insn_processed += 1;
                }
                InsnResult::Branch {
                    fallthrough,
                    target,
                } => {
                    // Verify both paths
                    let mut branch_state = state.clone();
                    branch_state.insn_idx = target;
                    branch_state.insn_processed += 1;
                    self.verify_path(insns, target, branch_state)?;

                    state.insn_idx = fallthrough;
                    state.insn_processed += 1;
                }
                InsnResult::Exit => {
                    return Ok(());
                }
            }
        }
    }

    /// Check if two states are compatible (for path merging).
    fn states_compatible(&self, s1: &VerifierState, s2: &VerifierState) -> bool {
        // Simple compatibility check: same register types
        for i in 0..Register::COUNT {
            if s1.regs[i].reg_type != s2.regs[i].reg_type {
                return false;
            }
        }
        true
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
            // R0 should be initialized (return value)
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

        // Unknown instruction class
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

        // Check write to R10
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
                    // Unknown scalar, could be zero
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

        // Calculate target
        let target = (idx as i64) + 1 + (insn.offset as i64);
        if target < 0 {
            return Err(VerifyError::InvalidJump {
                insn_idx: idx,
                target: target as i32,
            });
        }
        let target = target as usize;

        // Unconditional jump
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

        // Check if helper is valid (simplified - would need helper registry)
        if helper_id < 0 {
            return Err(VerifyError::InvalidHelper {
                insn_idx: idx,
                helper_id,
            });
        }

        // Caller-saved registers are clobbered
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

        // R0 contains return value (scalar)
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
                // Load: dst = *(src + offset)
                let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                })?;
                let src = insn.src().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.src_reg(),
                })?;

                // Check R10 write
                if dst == Register::R10 {
                    return Err(VerifyError::WriteToReadOnly { insn_idx: idx });
                }

                // Source must be initialized pointer
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

                // Check stack bounds if stack pointer
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

                // Result is scalar
                state.set_scalar(dst, Some(ScalarValue::unknown()));
            }

            OpcodeClass::Stx => {
                // Store: *(dst + offset) = src
                let dst = insn.dst().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.dst_reg(),
                })?;
                let src = insn.src().ok_or(VerifyError::InvalidRegister {
                    insn_idx: idx,
                    reg: insn.src_reg(),
                })?;

                // Both must be initialized
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

                // Update stack state if writing to stack
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

                    // Mark stack slots as written
                    for i in 0..size.size_bytes() {
                        let _ = state.stack.set(offset - i as i64, StackSlot::Scalar);
                    }
                }
            }

            OpcodeClass::St => {
                // Store immediate: *(dst + offset) = imm
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

    /// Verify a wide load instruction (64-bit immediate).
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

        // Result is scalar with known lower 32 bits
        state.set_scalar(dst, Some(ScalarValue::unknown()));

        Ok(())
    }

    /// Verify profile-specific constraints.
    fn verify_profile_constraints(&self, insns: &[BpfInsn]) -> VerifyResult<()> {
        #[cfg(feature = "embedded-profile")]
        {
            self.verify_embedded_constraints(insns)?;
        }

        #[cfg(feature = "cloud-profile")]
        {
            self.verify_cloud_constraints(insns)?;
        }

        Ok(())
    }

    /// Embedded profile specific constraints.
    #[cfg(feature = "embedded-profile")]
    fn verify_embedded_constraints(&self, insns: &[BpfInsn]) -> VerifyResult<()> {
        let cfg = self.cfg.as_ref().unwrap();

        // Check for unbounded loops
        if cfg.has_loops() {
            // In embedded profile, all loops must be bounded
            // For now, we simply reject programs with back edges
            // A more sophisticated analysis would compute loop bounds
            if let Some(&(from, _to)) = cfg.back_edges().first() {
                return Err(VerifyError::UnboundedLoop { insn_idx: from });
            }
        }

        // Check for dynamic allocation helpers
        for (idx, insn) in insns.iter().enumerate() {
            if insn.is_call() {
                // List of helpers that perform dynamic allocation
                const ALLOC_HELPERS: &[i32] = &[
                    // bpf_ringbuf_reserve, bpf_ringbuf_submit, etc.
                    // These would be defined based on your helper IDs
                ];

                if ALLOC_HELPERS.contains(&insn.imm) {
                    return Err(VerifyError::DynamicAllocationAttempted { insn_idx: idx });
                }
            }
        }

        Ok(())
    }

    /// Cloud profile specific constraints (more relaxed).
    #[cfg(feature = "cloud-profile")]
    fn verify_cloud_constraints(&self, _insns: &[BpfInsn]) -> VerifyResult<()> {
        // Cloud profile has minimal additional constraints
        // Could add JIT hints validation here
        Ok(())
    }
}

impl<P: PhysicalProfile> Default for Verifier<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of verifying a single instruction.
enum InsnResult {
    /// Continue to next instruction
    Continue,
    /// Jump to target instruction
    Jump(usize),
    /// Branch: verify both paths
    Branch { fallthrough: usize, target: usize },
    /// Program exit
    Exit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_empty_program() {
        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &[]);
        assert!(matches!(result, Err(VerifyError::EmptyProgram)));
    }

    #[test]
    fn verify_minimal_program() {
        let insns = [
            BpfInsn::mov64_imm(0, 0), // r0 = 0
            BpfInsn::exit(),          // exit
        ];

        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_no_exit() {
        let insns = [
            BpfInsn::mov64_imm(0, 0), // r0 = 0
            BpfInsn::nop(),           // nop (no exit)
        ];

        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::NoExit)));
    }

    #[test]
    fn verify_division_by_zero() {
        let insns = [
            BpfInsn::mov64_imm(0, 10),      // r0 = 10
            BpfInsn::new(0x37, 0, 0, 0, 0), // r0 /= 0 (div by zero)
            BpfInsn::exit(),
        ];

        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::DivisionByZero { .. })));
    }

    #[test]
    fn verify_write_to_r10() {
        let insns = [
            BpfInsn::mov64_imm(10, 0), // r10 = 0 (illegal!)
            BpfInsn::mov64_imm(0, 0),
            BpfInsn::exit(),
        ];

        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(result, Err(VerifyError::WriteToReadOnly { .. })));
    }

    #[test]
    fn verify_uninitialized_register() {
        let insns = [
            BpfInsn::add64_reg(0, 2), // r0 += r2 (r2 not init, r0 not init)
            BpfInsn::exit(),
        ];

        let result = Verifier::<ActiveProfile>::verify(BpfProgType::SocketFilter, &insns);
        assert!(matches!(
            result,
            Err(VerifyError::UninitializedRegister { .. })
        ));
    }
}
