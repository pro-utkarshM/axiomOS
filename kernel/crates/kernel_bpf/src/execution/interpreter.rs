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

unsafe extern "C" {
    fn bpf_ktime_get_ns() -> u64;
    fn bpf_trace_printk(fmt: *const u8, size: u32) -> i32;
    fn bpf_map_lookup_elem(map_id: u32, key: *const u8) -> *mut u8;
    fn bpf_map_update_elem(map_id: u32, key: *const u8, value: *const u8, flags: u64) -> i32;
    fn bpf_map_delete_elem(map_id: u32, key: *const u8) -> i32;
    fn bpf_ringbuf_output(map_id: u32, data: *const u8, size: u64, flags: u64) -> i64;
}

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
        ctx: &BpfContext,
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
                self.execute_load(insn, regs, stack, ctx)?;
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

                // bpf_map_lookup_elem
                3 => Ok(bpf_map_lookup_elem(args[0] as u32, args[1] as *const u8) as u64),

                // bpf_map_update_elem
                4 => Ok(bpf_map_update_elem(
                    args[0] as u32,
                    args[1] as *const u8,
                    args[2] as *const u8,
                    args[3],
                ) as u64),

                // bpf_map_delete_elem
                5 => Ok(bpf_map_delete_elem(args[0] as u32, args[1] as *const u8) as u64),

                // bpf_ringbuf_output
                6 => Ok(bpf_ringbuf_output(
                    args[0] as u32,
                    args[1] as *const u8,
                    args[2],
                    args[3],
                ) as u64),

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
        ctx: &BpfContext,
    ) -> Result<(), BpfError> {
        let dst = Register::from_raw(insn.dst_reg()).ok_or(BpfError::InvalidInstruction)?;
        let src = Register::from_raw(insn.src_reg()).ok_or(BpfError::InvalidInstruction)?;

        let size = MemSize::from_opcode(insn.opcode).ok_or(BpfError::InvalidInstruction)?;

        let base = regs.get(src);
        // wrapping_add is correct for address calculation
        let addr = base.wrapping_add(insn.offset as i64 as u64);

        // 1. Stack access (src = R10)
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

        // 2. Context access
        // Check if address is within the BpfContext struct
        let ctx_addr = ctx as *const _ as u64;
        let ctx_size = core::mem::size_of::<BpfContext>() as u64;

        if addr >= ctx_addr && addr + size.size_bytes() as u64 <= ctx_addr + ctx_size {
            let value = unsafe {
                match size {
                    MemSize::Byte => core::ptr::read_unaligned(addr as *const u8) as u64,
                    MemSize::Half => core::ptr::read_unaligned(addr as *const u16) as u64,
                    MemSize::Word => core::ptr::read_unaligned(addr as *const u32) as u64,
                    MemSize::DWord => core::ptr::read_unaligned(addr as *const u64),
                }
            };
            regs.set(dst, value);
            return Ok(());
        }

        // 3. Data access
        // Check if address is within [ctx.data, ctx.data_end)
        let data_start = ctx.data as u64;
        let data_end = ctx.data_end as u64;

        if !ctx.data.is_null() && addr >= data_start {
            if addr + size.size_bytes() as u64 <= data_end {
                let value = unsafe {
                    match size {
                        MemSize::Byte => core::ptr::read_unaligned(addr as *const u8) as u64,
                        MemSize::Half => core::ptr::read_unaligned(addr as *const u16) as u64,
                        MemSize::Word => core::ptr::read_unaligned(addr as *const u32) as u64,
                        MemSize::DWord => core::ptr::read_unaligned(addr as *const u64),
                    }
                };
                regs.set(dst, value);
                return Ok(());
            }
        }

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

#[cfg(test)]
#[allow(clippy::missing_safety_doc)]
mod helpers_stub {
    use core::sync::atomic::{AtomicU64, Ordering};

    // Simple test map: single u64 value at key 0
    static TEST_MAP_VALUE: AtomicU64 = AtomicU64::new(0);

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_ktime_get_ns() -> u64 {
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_trace_printk(_fmt: *const u8, _len: u32) -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_map_lookup_elem(_map_id: u32, _key: *const u8) -> *mut u8 {
        // Return pointer to our test value
        TEST_MAP_VALUE.as_ptr() as *mut u8
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_map_update_elem(
        _map_id: u32,
        _key: *const u8,
        value: *const u8,
        _flags: u64,
    ) -> i32 {
        // Update test value from the 8-byte value pointer (handle null for tests)
        if !value.is_null() {
            let val = unsafe { *(value as *const u64) };
            TEST_MAP_VALUE.store(val, Ordering::SeqCst);
        }
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_map_delete_elem(_map_id: u32, _key: *const u8) -> i32 {
        TEST_MAP_VALUE.store(0, Ordering::SeqCst);
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn bpf_ringbuf_output(
        _map_id: u32,
        _data: *const u8,
        _size: u64,
        _flags: u64,
    ) -> i64 {
        // Test stub: always succeed
        0
    }

    pub fn get_test_map_value() -> u64 {
        TEST_MAP_VALUE.load(Ordering::SeqCst)
    }

    pub fn reset_test_map() {
        TEST_MAP_VALUE.store(0, Ordering::SeqCst);
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

    #[test]
    fn execute_map_lookup_helper() {
        // Test that calling bpf_map_lookup_elem helper works
        // Helper 3 = bpf_map_lookup_elem(map_id, key_ptr) -> value_ptr
        helpers_stub::reset_test_map();

        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(1, 0)) // r1 = map_id (0)
            .insn(BpfInsn::mov64_imm(2, 0)) // r2 = key_ptr (dummy)
            .insn(BpfInsn::call(3)) // r0 = bpf_map_lookup_elem(r1, r2)
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        // Result should be a non-null pointer (the address of TEST_MAP_VALUE)
        assert!(result.is_ok());
        assert_ne!(result.unwrap(), 0);
    }

    #[test]
    fn execute_map_update_helper() {
        // Test that calling bpf_map_update_elem helper works
        // Helper 4 = bpf_map_update_elem(map_id, key_ptr, value_ptr, flags) -> result
        helpers_stub::reset_test_map();
        assert_eq!(helpers_stub::get_test_map_value(), 0);

        // We need to put a value on the stack and pass its pointer
        // For this test, we'll just verify the helper is called and returns 0
        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(1, 0)) // r1 = map_id (0)
            .insn(BpfInsn::mov64_imm(2, 0)) // r2 = key_ptr (dummy)
            .insn(BpfInsn::mov64_imm(3, 0)) // r3 = value_ptr (dummy)
            .insn(BpfInsn::mov64_imm(4, 0)) // r4 = flags (0)
            .insn(BpfInsn::call(4)) // r0 = bpf_map_update_elem(r1, r2, r3, r4)
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        // Helper should return 0 on success
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn execute_map_delete_helper() {
        // Test that calling bpf_map_delete_elem helper works
        // Helper 5 = bpf_map_delete_elem(map_id, key_ptr) -> result
        helpers_stub::reset_test_map();

        let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
            .insn(BpfInsn::mov64_imm(1, 0)) // r1 = map_id (0)
            .insn(BpfInsn::mov64_imm(2, 0)) // r2 = key_ptr (dummy)
            .insn(BpfInsn::call(5)) // r0 = bpf_map_delete_elem(r1, r2)
            .exit()
            .build()
            .expect("valid program");

        let interpreter = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        let result = interpreter.execute(&program, &ctx);
        // Helper should return 0 on success
        assert_eq!(result, Ok(0));
    }
}
