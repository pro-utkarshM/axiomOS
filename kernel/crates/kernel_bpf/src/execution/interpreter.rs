//! BPF Bytecode Interpreter
//!
//! This module provides a pure interpreter for BPF bytecode.
//! The interpreter is available in both cloud and embedded profiles,
//! serving as the primary execution mode for embedded and fallback
//! for cloud (when JIT is unavailable).
//!
//! # Profile Constraints
//!
//! The interpreter enforces profile-specific limits:
//! - Instruction count bounded by `P::MAX_INSN_COUNT`
//! - Stack size bounded by `P::MAX_STACK_SIZE`

extern crate alloc;

use alloc::vec;
use core::marker::PhantomData;

use super::{BpfContext, BpfError, BpfExecutor, BpfResult};
use crate::bytecode::insn::BpfInsn;
use crate::bytecode::opcode::{AluOp, JmpOp, MemSize, OpcodeClass, SourceType};
use crate::bytecode::program::BpfProgram;
use crate::bytecode::registers::{Register, RegisterFile};
use crate::profile::{ActiveProfile, PhysicalProfile};

/// BPF bytecode interpreter.
///
/// The interpreter executes BPF programs instruction by instruction.
/// It is the simplest and most portable execution mode.
pub struct Interpreter<P: PhysicalProfile = ActiveProfile> {
    _profile: PhantomData<P>,
}

impl<P: PhysicalProfile> Interpreter<P> {
    /// Create a new interpreter.
    pub fn new() -> Self {
        Self {
            _profile: PhantomData,
        }
    }

    /// Execute a single instruction.
    fn execute_insn(
        &self,
        insn: &BpfInsn,
        regs: &mut RegisterFile,
        stack: &mut [u8],
        _ctx: &BpfContext,
    ) -> Result<InsnResult, BpfError> {
        // Exit instruction
        if insn.is_exit() {
            return Ok(InsnResult::Exit);
        }

        // Get opcode class
        let class = insn.class().ok_or(BpfError::InvalidInstruction)?;

        match class {
            OpcodeClass::Alu64 | OpcodeClass::Alu32 => {
                self.execute_alu(insn, regs, class == OpcodeClass::Alu64)?;
            }

            OpcodeClass::Jmp | OpcodeClass::Jmp32 => {
                return self.execute_jmp(insn, regs, class == OpcodeClass::Jmp);
            }

            OpcodeClass::Ldx => {
                self.execute_load(insn, regs, stack)?;
            }

            OpcodeClass::Stx | OpcodeClass::St => {
                self.execute_store(insn, regs, stack)?;
            }

            OpcodeClass::Ld => {
                // Wide load (64-bit immediate)
                return Ok(InsnResult::WideLoad);
            }
        }

        Ok(InsnResult::Continue)
    }

    /// Execute an ALU instruction.
    fn execute_alu(
        &self,
        insn: &BpfInsn,
        regs: &mut RegisterFile,
        is_64bit: bool,
    ) -> Result<(), BpfError> {
        let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;

        let src_val = if matches!(SourceType::from_opcode(insn.opcode), SourceType::Reg) {
            let src = Register::from_raw(insn.src_reg()).ok_or(BpfError::InvalidInstruction)?;
            regs.get(src)
        } else {
            insn.imm as i64 as u64
        };

        let dst_val = regs.get(dst);

        let alu_op = AluOp::from_opcode(insn.opcode).ok_or(BpfError::InvalidInstruction)?;

        let result = match alu_op {
            AluOp::Add => dst_val.wrapping_add(src_val),
            AluOp::Sub => dst_val.wrapping_sub(src_val),
            AluOp::Mul => dst_val.wrapping_mul(src_val),
            AluOp::Div => {
                if src_val == 0 {
                    return Err(BpfError::DivisionByZero);
                }
                dst_val / src_val
            }
            AluOp::Or => dst_val | src_val,
            AluOp::And => dst_val & src_val,
            AluOp::Lsh => dst_val << (src_val & 0x3f),
            AluOp::Rsh => dst_val >> (src_val & 0x3f),
            AluOp::Neg => (-(dst_val as i64)) as u64,
            AluOp::Mod => {
                if src_val == 0 {
                    return Err(BpfError::DivisionByZero);
                }
                dst_val % src_val
            }
            AluOp::Xor => dst_val ^ src_val,
            AluOp::Mov => src_val,
            AluOp::Arsh => ((dst_val as i64) >> (src_val & 0x3f)) as u64,
            AluOp::End => {
                // Byte swap
                match insn.imm {
                    16 => (dst_val as u16).swap_bytes() as u64,
                    32 => (dst_val as u32).swap_bytes() as u64,
                    64 => dst_val.swap_bytes(),
                    _ => return Err(BpfError::InvalidInstruction),
                }
            }
        };

        // Truncate to 32 bits for 32-bit ALU
        let result = if is_64bit {
            result
        } else {
            (result as u32) as u64
        };

        regs.set(dst, result);
        Ok(())
    }

    /// Execute a jump instruction.
    fn execute_jmp(
        &self,
        insn: &BpfInsn,
        regs: &mut RegisterFile,
        is_64bit: bool,
    ) -> Result<InsnResult, BpfError> {
        let jmp_op = JmpOp::from_opcode(insn.opcode).ok_or(BpfError::InvalidInstruction)?;

        // Handle call and exit
        if matches!(jmp_op, JmpOp::Call) {
            return self.execute_call(insn, regs);
        }

        if matches!(jmp_op, JmpOp::Exit) {
            return Ok(InsnResult::Exit);
        }

        // Unconditional jump
        if matches!(jmp_op, JmpOp::Ja) {
            return Ok(InsnResult::Jump(insn.offset));
        }

        // Conditional jump
        let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;
        let dst_val = regs.get(dst);

        let src_val = if matches!(SourceType::from_opcode(insn.opcode), SourceType::Reg) {
            let src = Register::from_raw(insn.src_reg()).ok_or(BpfError::InvalidInstruction)?;
            regs.get(src)
        } else {
            insn.imm as i64 as u64
        };

        // Truncate to 32 bits for 32-bit jumps
        let (dst_val, src_val) = if is_64bit {
            (dst_val, src_val)
        } else {
            ((dst_val as u32) as u64, (src_val as u32) as u64)
        };

        let condition = match jmp_op {
            JmpOp::Jeq => dst_val == src_val,
            JmpOp::Jgt => dst_val > src_val,
            JmpOp::Jge => dst_val >= src_val,
            JmpOp::Jlt => dst_val < src_val,
            JmpOp::Jle => dst_val <= src_val,
            JmpOp::Jset => (dst_val & src_val) != 0,
            JmpOp::Jne => dst_val != src_val,
            JmpOp::Jsgt => (dst_val as i64) > (src_val as i64),
            JmpOp::Jsge => (dst_val as i64) >= (src_val as i64),
            JmpOp::Jslt => (dst_val as i64) < (src_val as i64),
            JmpOp::Jsle => (dst_val as i64) <= (src_val as i64),
            _ => return Err(BpfError::InvalidInstruction),
        };

        if condition {
            Ok(InsnResult::Jump(insn.offset))
        } else {
            Ok(InsnResult::Continue)
        }
    }

    /// Execute a call instruction.
    fn execute_call(
        &self,
        insn: &BpfInsn,
        regs: &mut RegisterFile,
    ) -> Result<InsnResult, BpfError> {
        let helper_id = insn.imm;

        // Get arguments from R1-R5
        let args = [
            regs.get(Register::R1),
            regs.get(Register::R2),
            regs.get(Register::R3),
            regs.get(Register::R4),
            regs.get(Register::R5),
        ];

        // Execute helper (simplified - would need helper registry)
        let result = self.call_helper(helper_id, args)?;

        // Store result in R0
        regs.set(Register::R0, result);

        Ok(InsnResult::Continue)
    }

    /// Call a helper function.
    fn call_helper(&self, helper_id: i32, args: [u64; 5]) -> Result<u64, BpfError> {
        unsafe {
            match helper_id {
                // bpf_ktime_get_ns
                1 => Ok(bpf_ktime_get_ns()),
                
                // bpf_trace_printk
                2 => Ok(bpf_trace_printk(args[0] as *const u8, args[1] as u32) as u64),

                // Unknown helper
                _ => Err(BpfError::InvalidHelper(helper_id)),
            }
        }
    }

    /// Execute a load instruction.
    fn execute_load(
        &self,
        insn: &BpfInsn,
        regs: &mut RegisterFile,
        stack: &[u8],
    ) -> Result<(), BpfError> {
        let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;
        let src = Register::from_raw(insn.src_reg()).ok_or(BpfError::InvalidInstruction)?;

        let size = MemSize::from_opcode(insn.opcode).ok_or(BpfError::InvalidInstruction)?;

        let base = regs.get(src);
        let _addr = base.wrapping_add(insn.offset as i64 as u64);

        // For stack access (src = R10)
        if src == Register::R10 {
            let _fp = base;
            let offset = insn.offset as i64;
            let stack_offset = -(offset + (size.size_bytes() as i64));

            if stack_offset < 0 || stack_offset as usize + size.size_bytes() > stack.len() {
                return Err(BpfError::OutOfBounds);
            }

            let stack_idx = stack_offset as usize;
            let value = match size {
                MemSize::Byte => stack[stack_idx] as u64,
                MemSize::Half => {
                    let bytes: [u8; 2] = stack[stack_idx..stack_idx + 2]
                        .try_into()
                        .map_err(|_| BpfError::OutOfBounds)?;
                    u16::from_ne_bytes(bytes) as u64
                }
                MemSize::Word => {
                    let bytes: [u8; 4] = stack[stack_idx..stack_idx + 4]
                        .try_into()
                        .map_err(|_| BpfError::OutOfBounds)?;
                    u32::from_ne_bytes(bytes) as u64
                }
                MemSize::DWord => {
                    let bytes: [u8; 8] = stack[stack_idx..stack_idx + 8]
                        .try_into()
                        .map_err(|_| BpfError::OutOfBounds)?;
                    u64::from_ne_bytes(bytes)
                }
            };

            regs.set(dst, value);
            return Ok(());
        }

        // Generic memory access would require context pointer validation
        // For now, only stack access is fully implemented
        Err(BpfError::OutOfBounds)
    }

    /// Execute a store instruction.
    fn execute_store(
        &self,
        insn: &BpfInsn,
        regs: &RegisterFile,
        stack: &mut [u8],
    ) -> Result<(), BpfError> {
        let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;
        let class = insn.class().ok_or(BpfError::InvalidInstruction)?;

        let value = if matches!(class, OpcodeClass::St) {
            // Store immediate
            insn.imm as i64 as u64
        } else {
            // Store register
            let src = Register::from_raw(insn.src_reg()).ok_or(BpfError::InvalidInstruction)?;
            regs.get(src)
        };

        let size = MemSize::from_opcode(insn.opcode).ok_or(BpfError::InvalidInstruction)?;

        // For stack access (dst = R10)
        if dst == Register::R10 {
            let _fp = regs.get(Register::R10);
            let offset = insn.offset as i64;
            let stack_offset = -(offset + (size.size_bytes() as i64));

            if stack_offset < 0 || stack_offset as usize + size.size_bytes() > stack.len() {
                return Err(BpfError::OutOfBounds);
            }

            let stack_idx = stack_offset as usize;
            match size {
                MemSize::Byte => {
                    stack[stack_idx] = value as u8;
                }
                MemSize::Half => {
                    let bytes = (value as u16).to_ne_bytes();
                    stack[stack_idx..stack_idx + 2].copy_from_slice(&bytes);
                }
                MemSize::Word => {
                    let bytes = (value as u32).to_ne_bytes();
                    stack[stack_idx..stack_idx + 4].copy_from_slice(&bytes);
                }
                MemSize::DWord => {
                    let bytes = value.to_ne_bytes();
                    stack[stack_idx..stack_idx + 8].copy_from_slice(&bytes);
                }
            }
            return Ok(());
        }

        // Generic memory access would require context pointer validation
        Err(BpfError::OutOfBounds)
    }
}

impl<P: PhysicalProfile> Default for Interpreter<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: PhysicalProfile> BpfExecutor<P> for Interpreter<P> {
    fn execute(&self, program: &BpfProgram<P>, ctx: &BpfContext) -> BpfResult {
        let insns = program.instructions();

        if insns.is_empty() {
            return Err(BpfError::NotLoaded);
        }

        // Initialize register file
        let mut regs = RegisterFile::new();

        // Allocate stack
        let mut stack = vec![0u8; P::MAX_STACK_SIZE];

        // R1 = context pointer
        regs.set(Register::R1, ctx as *const _ as u64);

        // R10 = frame pointer (top of stack)
        let fp = stack.as_ptr() as u64 + P::MAX_STACK_SIZE as u64;
        unsafe {
            regs.set_unchecked(Register::R10, fp);
        }

        // Execute
        let mut pc = 0usize;
        let mut insn_count = 0usize;
        let insn_limit = P::MAX_INSN_COUNT;

        loop {
            // Check bounds
            if pc >= insns.len() {
                return Err(BpfError::OutOfBounds);
            }

            // Check instruction limit
            insn_count += 1;
            if insn_count > insn_limit {
                return Err(BpfError::Timeout);
            }

            let insn = &insns[pc];

            // Handle wide instruction
            if insn.is_wide() {
                if pc + 1 >= insns.len() {
                    return Err(BpfError::InvalidInstruction);
                }
                let next_insn = &insns[pc + 1];
                let imm64 = (insn.imm as u32 as u64) | ((next_insn.imm as u32 as u64) << 32);

                let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;
                regs.set(dst, imm64);

                pc += 2;
                continue;
            }

            // Execute instruction
            match self.execute_insn(insn, &mut regs, &mut stack, ctx)? {
                InsnResult::Continue => {
                    pc += 1;
                }
                InsnResult::Jump(offset) => {
                    pc = ((pc as i64) + 1 + (offset as i64)) as usize;
                }
                InsnResult::Exit => {
                    return Ok(regs.return_value());
                }
                InsnResult::WideLoad => {
                    // Handled above, shouldn't reach here
                    return Err(BpfError::InvalidInstruction);
                }
            }
        }
    }
}

/// Result of executing a single instruction.
enum InsnResult {
    /// Continue to next instruction
    Continue,
    /// Jump by offset
    Jump(i16),
    /// Program exit
    Exit,
    /// Wide load (64-bit immediate)
    WideLoad,
}

unsafe extern "C" {
    fn bpf_ktime_get_ns() -> u64;
    fn bpf_trace_printk(fmt: *const u8, len: u32) -> i32;
}

#[cfg(test)]
#[allow(clippy::missing_safety_doc)]
mod helpers_stub {
    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_ktime_get_ns() -> u64 {
        0
    }
    
    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_trace_printk(_fmt: *const u8, _len: u32) -> i32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::program::{BpfProgType, ProgramBuilder};

    #[test]
    fn execute_simple_program() {
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 42)) // r0 = 42
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn execute_arithmetic() {
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 10)) // r0 = 10
            .insn(BpfInsn::add64_imm(0, 5)) // r0 += 5
            .insn(BpfInsn::mov64_imm(1, 3)) // r1 = 3
            .insn(BpfInsn::add64_reg(0, 1)) // r0 += r1
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        assert_eq!(result, Ok(18)); // 10 + 5 + 3 = 18
    }

    #[test]
    fn execute_conditional_jump() {
        // if r0 == 0, return 1, else return 2
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 0)) // r0 = 0
            .insn(BpfInsn::jeq_imm(0, 0, 2)) // if r0 == 0, skip 2
            .insn(BpfInsn::mov64_imm(0, 2)) // r0 = 2 (skipped)
            .insn(BpfInsn::ja(1)) // skip next
            .insn(BpfInsn::mov64_imm(0, 1)) // r0 = 1
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        assert_eq!(result, Ok(1));
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Too slow under Miri - tests instruction limit timeout
    fn execute_timeout() {
        // Infinite loop (would timeout)
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 0)) // r0 = 0
            .insn(BpfInsn::ja(-1)) // infinite loop
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        assert_eq!(result, Err(BpfError::Timeout));
    }

    #[test]
    fn execute_division_by_zero() {
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(0, 10)) // r0 = 10
            .insn(BpfInsn::mov64_imm(1, 0)) // r1 = 0
            .insn(BpfInsn::new(0x3f, 0, 1, 0, 0)) // r0 /= r1
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        assert_eq!(result, Err(BpfError::DivisionByZero));
    }
}
