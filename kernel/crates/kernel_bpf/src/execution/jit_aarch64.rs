//! ARM64 (AArch64) JIT Compiler for BPF Programs
//!
//! This module provides Just-In-Time compilation of BPF bytecode to
//! ARM64 machine code. This is particularly important for robotics
//! platforms like Jetson and Raspberry Pi.
//!
//! # Register Mapping
//!
//! BPF registers are mapped to ARM64 registers:
//!
//! | BPF    | ARM64  | Purpose                    |
//! |--------|--------|----------------------------|
//! | R0     | X7     | Return value               |
//! | R1     | X0     | Arg 1 / context ptr        |
//! | R2     | X1     | Arg 2                      |
//! | R3     | X2     | Arg 3                      |
//! | R4     | X3     | Arg 4                      |
//! | R5     | X4     | Arg 5                      |
//! | R6     | X19    | Callee-saved               |
//! | R7     | X20    | Callee-saved               |
//! | R8     | X21    | Callee-saved               |
//! | R9     | X22    | Callee-saved               |
//! | R10    | X25    | Frame pointer (read-only)  |
//!
//! # Stack Layout
//!
//! ```text
//! High Address
//! ┌─────────────────────┐
//! │ Saved LR (X30)      │
//! ├─────────────────────┤
//! │ Saved FP (X29)      │
//! ├─────────────────────┤
//! │ Saved X19-X25       │
//! ├─────────────────────┤
//! │ BPF stack space     │
//! │ (profile max)       │
//! ├─────────────────────┤  ← BPF R10 (frame pointer)
//! │                     │
//! Low Address
//! ```

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::bytecode::insn::BpfInsn;
use crate::bytecode::opcode::{AluOp, JmpOp, MemSize, OpcodeClass, SourceType};
use crate::bytecode::program::BpfProgram;
use crate::execution::{BpfContext, BpfExecutor, BpfResult};
use crate::profile::{ActiveProfile, PhysicalProfile};

// ARM64 register numbers
const X0: u8 = 0;
const X1: u8 = 1;
const X2: u8 = 2;
const X3: u8 = 3;
const X4: u8 = 4;
const X7: u8 = 7;
const X19: u8 = 19;
const X20: u8 = 20;
const X21: u8 = 21;
const X22: u8 = 22;
const X25: u8 = 25;
const X29: u8 = 29; // Frame pointer
const X30: u8 = 30; // Link register
const SP: u8 = 31; // Stack pointer (when used as base)

/// BPF to ARM64 register mapping.
const BPF_TO_ARM64: [u8; 11] = [
    X7,  // R0 -> X7 (return value)
    X0,  // R1 -> X0 (arg1/context)
    X1,  // R2 -> X1 (arg2)
    X2,  // R3 -> X2 (arg3)
    X3,  // R4 -> X3 (arg4)
    X4,  // R5 -> X4 (arg5)
    X19, // R6 -> X19 (callee-saved)
    X20, // R7 -> X20 (callee-saved)
    X21, // R8 -> X21 (callee-saved)
    X22, // R9 -> X22 (callee-saved)
    X25, // R10 -> X25 (frame pointer)
];

/// ARM64 code emitter.
struct Arm64Emitter {
    /// Emitted code bytes
    code: Vec<u8>,
    /// Jump targets that need patching (offset -> target instruction index)
    jump_patches: Vec<(usize, usize)>,
    /// Instruction offsets (BPF insn index -> code offset)
    insn_offsets: Vec<usize>,
}

impl Arm64Emitter {
    fn new(capacity: usize) -> Self {
        Self {
            code: Vec::with_capacity(capacity),
            jump_patches: Vec::new(),
            insn_offsets: Vec::new(),
        }
    }

    /// Emit a 32-bit instruction.
    fn emit(&mut self, insn: u32) {
        self.code.extend_from_slice(&insn.to_le_bytes());
    }

    /// Get current code offset.
    fn offset(&self) -> usize {
        self.code.len()
    }

    /// Record the start of a BPF instruction.
    fn mark_insn(&mut self) {
        self.insn_offsets.push(self.offset());
    }

    /// Record a jump that needs patching.
    fn record_jump(&mut self, target_insn: usize) {
        self.jump_patches.push((self.offset() - 4, target_insn));
    }

    // ============================================================
    // ARM64 Instruction Encoding
    // ============================================================

    /// MOV (register): Rd = Rm
    fn emit_mov_reg(&mut self, rd: u8, rm: u8) {
        // ORR Xd, XZR, Xm
        let insn = 0xAA000000 | ((rm as u32) << 16) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// MOV (immediate): Rd = imm16
    fn emit_mov_imm(&mut self, rd: u8, imm: u16, shift: u8) {
        // MOVZ Xd, #imm, LSL #shift
        let hw = (shift / 16) as u32;
        let insn = 0xD2800000 | (hw << 21) | ((imm as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// MOVK (keep other bits): Rd |= (imm16 << shift)
    fn emit_movk(&mut self, rd: u8, imm: u16, shift: u8) {
        let hw = (shift / 16) as u32;
        let insn = 0xF2800000 | (hw << 21) | ((imm as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// Load 64-bit immediate into register.
    fn emit_mov64_imm(&mut self, rd: u8, imm: i64) {
        let imm = imm as u64;

        // Try single instruction encoding for common values
        if imm == 0 {
            // MOV Xd, XZR
            self.emit_mov_reg(rd, 31);
            return;
        }

        // Use MOVZ + MOVK sequence
        let parts = [
            (imm & 0xFFFF) as u16,
            ((imm >> 16) & 0xFFFF) as u16,
            ((imm >> 32) & 0xFFFF) as u16,
            ((imm >> 48) & 0xFFFF) as u16,
        ];

        // Find first non-zero part
        let mut first = true;
        for (i, &part) in parts.iter().enumerate() {
            if part != 0 {
                if first {
                    self.emit_mov_imm(rd, part, (i * 16) as u8);
                    first = false;
                } else {
                    self.emit_movk(rd, part, (i * 16) as u8);
                }
            }
        }

        if first {
            // All parts were zero (shouldn't happen, caught above)
            self.emit_mov_reg(rd, 31);
        }
    }

    /// ADD (register): Rd = Rn + Rm
    fn emit_add_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0x8B000000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// ADD (immediate): Rd = Rn + imm12
    fn emit_add_imm(&mut self, rd: u8, rn: u8, imm: u16) {
        let insn =
            0x91000000 | (((imm as u32) & 0xFFF) << 10) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// SUB (register): Rd = Rn - Rm
    fn emit_sub_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0xCB000000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// SUB (immediate): Rd = Rn - imm12
    fn emit_sub_imm(&mut self, rd: u8, rn: u8, imm: u16) {
        let insn =
            0xD1000000 | (((imm as u32) & 0xFFF) << 10) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// MUL: Rd = Rn * Rm
    fn emit_mul(&mut self, rd: u8, rn: u8, rm: u8) {
        // MADD Xd, Xn, Xm, XZR (multiply-add with zero)
        let insn = 0x9B007C00 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// UDIV: Rd = Rn / Rm (unsigned)
    fn emit_udiv(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0x9AC00800 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// SDIV: Rd = Rn / Rm (signed)
    fn emit_sdiv(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0x9AC00C00 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// AND (register): Rd = Rn & Rm
    fn emit_and_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0x8A000000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// ORR (register): Rd = Rn | Rm
    fn emit_orr_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0xAA000000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// EOR (register): Rd = Rn ^ Rm
    fn emit_eor_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        let insn = 0xCA000000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// LSL (register): Rd = Rn << Rm
    fn emit_lsl_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        // LSLV Xd, Xn, Xm
        let insn = 0x9AC02000 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// LSR (register): Rd = Rn >> Rm (unsigned)
    fn emit_lsr_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        // LSRV Xd, Xn, Xm
        let insn = 0x9AC02400 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// ASR (register): Rd = Rn >> Rm (signed)
    fn emit_asr_reg(&mut self, rd: u8, rn: u8, rm: u8) {
        // ASRV Xd, Xn, Xm
        let insn = 0x9AC02800 | ((rm as u32) << 16) | ((rn as u32) << 5) | ((rd as u32) & 0x1f);
        self.emit(insn);
    }

    /// NEG: Rd = -Rm
    fn emit_neg(&mut self, rd: u8, rm: u8) {
        // SUB Xd, XZR, Xm
        self.emit_sub_reg(rd, 31, rm);
    }

    /// LDR (register + immediate offset)
    fn emit_ldr(&mut self, rt: u8, rn: u8, offset: i16, size: MemSize) {
        let (opc, scale) = match size {
            MemSize::Byte => (0b00, 0),
            MemSize::Half => (0b01, 1),
            MemSize::Word => (0b10, 2),
            MemSize::DWord => (0b11, 3),
        };

        // Use pre-index for negative offsets, unsigned offset for positive
        if offset < 0 {
            // LDUR (unscaled immediate)
            let imm9 = (offset as u32) & 0x1FF;
            let insn =
                (opc << 30) | 0x38400000 | (imm9 << 12) | ((rn as u32) << 5) | ((rt as u32) & 0x1f);
            self.emit(insn);
        } else {
            // LDR (unsigned offset)
            let imm12 = ((offset as u32) >> scale) & 0xFFF;
            let insn = (opc << 30)
                | 0x39400000
                | (imm12 << 10)
                | ((rn as u32) << 5)
                | ((rt as u32) & 0x1f);
            self.emit(insn);
        }
    }

    /// STR (register + immediate offset)
    fn emit_str(&mut self, rt: u8, rn: u8, offset: i16, size: MemSize) {
        let (opc, scale) = match size {
            MemSize::Byte => (0b00, 0),
            MemSize::Half => (0b01, 1),
            MemSize::Word => (0b10, 2),
            MemSize::DWord => (0b11, 3),
        };

        if offset < 0 {
            // STUR (unscaled immediate)
            let imm9 = (offset as u32) & 0x1FF;
            let insn =
                (opc << 30) | 0x38000000 | (imm9 << 12) | ((rn as u32) << 5) | ((rt as u32) & 0x1f);
            self.emit(insn);
        } else {
            // STR (unsigned offset)
            let imm12 = ((offset as u32) >> scale) & 0xFFF;
            let insn = (opc << 30)
                | 0x39000000
                | (imm12 << 10)
                | ((rn as u32) << 5)
                | ((rt as u32) & 0x1f);
            self.emit(insn);
        }
    }

    /// CMP (register): flags = Rn - Rm
    fn emit_cmp_reg(&mut self, rn: u8, rm: u8) {
        // SUBS XZR, Xn, Xm
        let insn = 0xEB00001F | ((rm as u32) << 16) | ((rn as u32) << 5);
        self.emit(insn);
    }

    /// CMP (immediate): flags = Rn - imm12
    fn emit_cmp_imm(&mut self, rn: u8, imm: u16) {
        // SUBS XZR, Xn, #imm
        let insn = 0xF100001F | (((imm as u32) & 0xFFF) << 10) | ((rn as u32) << 5);
        self.emit(insn);
    }

    /// Conditional branch: B.cond offset
    fn emit_b_cond(&mut self, cond: u8, offset: i32) {
        // B.cond: imm19 offset
        let imm19 = ((offset >> 2) as u32) & 0x7FFFF;
        let insn = 0x54000000 | (imm19 << 5) | (cond as u32);
        self.emit(insn);
    }

    /// Unconditional branch: B offset
    fn emit_b(&mut self, offset: i32) {
        // B: imm26 offset
        let imm26 = ((offset >> 2) as u32) & 0x3FFFFFF;
        let insn = 0x14000000 | imm26;
        self.emit(insn);
    }

    /// Branch to link register: RET
    fn emit_ret(&mut self) {
        // RET (X30)
        self.emit(0xD65F03C0);
    }

    /// STP (store pair): store two registers
    fn emit_stp(&mut self, rt1: u8, rt2: u8, rn: u8, offset: i16) {
        let imm7 = ((offset >> 3) as u32) & 0x7F;
        let insn = 0xA9000000
            | (imm7 << 15)
            | ((rt2 as u32) << 10)
            | ((rn as u32) << 5)
            | ((rt1 as u32) & 0x1f);
        self.emit(insn);
    }

    /// LDP (load pair): load two registers
    fn emit_ldp(&mut self, rt1: u8, rt2: u8, rn: u8, offset: i16) {
        let imm7 = ((offset >> 3) as u32) & 0x7F;
        let insn = 0xA9400000
            | (imm7 << 15)
            | ((rt2 as u32) << 10)
            | ((rn as u32) << 5)
            | ((rt1 as u32) & 0x1f);
        self.emit(insn);
    }
}

/// ARM64 JIT-compiled BPF program.
pub struct Arm64JitProgram {
    /// Executable code
    code: Vec<u8>,
    /// Entry point function
    #[allow(dead_code)]
    entry: usize,
}

/// ARM64 JIT compiler error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm64JitError {
    /// Unsupported instruction
    UnsupportedInstruction,
    /// Code too large
    CodeTooLarge,
    /// Memory allocation failed
    AllocationFailed,
}

/// ARM64 JIT compiler.
pub struct Arm64JitCompiler<P: PhysicalProfile = ActiveProfile> {
    _profile: PhantomData<P>,
}

impl<P: PhysicalProfile> Arm64JitCompiler<P> {
    /// Create a new ARM64 JIT compiler.
    pub fn new() -> Self {
        Self {
            _profile: PhantomData,
        }
    }

    /// Compile a BPF program to ARM64 machine code.
    pub fn compile(&self, program: &BpfProgram<P>) -> Result<Arm64JitProgram, Arm64JitError> {
        let insns = program.instructions();

        // Estimate code size (roughly 4 ARM64 instructions per BPF instruction)
        let estimated_size = insns.len() * 16 + 256; // Extra for prologue/epilogue
        let mut emitter = Arm64Emitter::new(estimated_size);

        // Emit prologue
        self.emit_prologue(&mut emitter, P::MAX_STACK_SIZE);

        // Compile each BPF instruction
        for (idx, insn) in insns.iter().enumerate() {
            emitter.mark_insn();
            self.compile_insn(&mut emitter, insn, idx)?;
        }

        // Patch jumps
        self.patch_jumps(&mut emitter)?;

        Ok(Arm64JitProgram {
            code: emitter.code,
            entry: 0,
        })
    }

    /// Emit function prologue.
    fn emit_prologue(&self, emitter: &mut Arm64Emitter, stack_size: usize) {
        // Save frame pointer and link register
        emitter.emit_stp(X29, X30, SP, -16);

        // Set up frame pointer
        emitter.emit_mov_reg(X29, SP);

        // Save callee-saved registers (X19-X25)
        emitter.emit_stp(X19, X20, SP, -32);
        emitter.emit_stp(X21, X22, SP, -48);
        emitter.emit_stp(X25, X25, SP, -56); // X25 is BPF R10

        // Allocate BPF stack space
        let stack_alloc = ((stack_size + 15) & !15) as u16; // 16-byte aligned
        emitter.emit_sub_imm(SP, SP, stack_alloc + 64);

        // Set up BPF frame pointer (R10 -> X25)
        // Points to the top of BPF stack
        emitter.emit_mov_reg(X25, SP);
        emitter.emit_add_imm(X25, X25, stack_alloc);
    }

    /// Emit function epilogue.
    fn emit_epilogue(&self, emitter: &mut Arm64Emitter, stack_size: usize) {
        let stack_alloc = ((stack_size + 15) & !15) as u16;

        // Deallocate stack
        emitter.emit_add_imm(SP, SP, stack_alloc + 64);

        // Restore callee-saved registers
        emitter.emit_ldp(X25, X25, SP, -56);
        emitter.emit_ldp(X21, X22, SP, -48);
        emitter.emit_ldp(X19, X20, SP, -32);

        // Restore frame pointer and link register
        emitter.emit_ldp(X29, X30, SP, -16);

        // Return
        emitter.emit_ret();
    }

    /// Compile a single BPF instruction.
    fn compile_insn(
        &self,
        emitter: &mut Arm64Emitter,
        insn: &BpfInsn,
        idx: usize,
    ) -> Result<(), Arm64JitError> {
        let class = insn.class().ok_or(Arm64JitError::UnsupportedInstruction)?;

        match class {
            OpcodeClass::Alu64 | OpcodeClass::Alu32 => {
                self.compile_alu(emitter, insn)?;
            }
            OpcodeClass::Jmp | OpcodeClass::Jmp32 => {
                self.compile_jmp(emitter, insn, idx)?;
            }
            OpcodeClass::Ldx => {
                self.compile_ldx(emitter, insn)?;
            }
            OpcodeClass::Stx | OpcodeClass::St => {
                self.compile_st(emitter, insn)?;
            }
            OpcodeClass::Ld => {
                self.compile_ld(emitter, insn)?;
            }
        }

        Ok(())
    }

    /// Compile ALU instruction.
    fn compile_alu(&self, emitter: &mut Arm64Emitter, insn: &BpfInsn) -> Result<(), Arm64JitError> {
        let dst = BPF_TO_ARM64[insn.dst_reg() as usize];
        let alu_op = insn.alu_op().ok_or(Arm64JitError::UnsupportedInstruction)?;

        match insn.source_type() {
            SourceType::Imm => {
                // Immediate operand
                match alu_op {
                    AluOp::Add => {
                        if insn.imm >= 0 && insn.imm < 4096 {
                            emitter.emit_add_imm(dst, dst, insn.imm as u16);
                        } else {
                            // Load immediate to temp register
                            emitter.emit_mov64_imm(X7, insn.imm as i64);
                            emitter.emit_add_reg(dst, dst, X7);
                        }
                    }
                    AluOp::Sub => {
                        if insn.imm >= 0 && insn.imm < 4096 {
                            emitter.emit_sub_imm(dst, dst, insn.imm as u16);
                        } else {
                            emitter.emit_mov64_imm(X7, insn.imm as i64);
                            emitter.emit_sub_reg(dst, dst, X7);
                        }
                    }
                    AluOp::Mov => {
                        emitter.emit_mov64_imm(dst, insn.imm as i64);
                    }
                    AluOp::Mul
                    | AluOp::Div
                    | AluOp::Mod
                    | AluOp::And
                    | AluOp::Or
                    | AluOp::Xor
                    | AluOp::Lsh
                    | AluOp::Rsh
                    | AluOp::Arsh => {
                        // Load immediate to temp, then do reg op
                        emitter.emit_mov64_imm(X7, insn.imm as i64);
                        self.emit_alu_reg(emitter, alu_op, dst, X7)?;
                    }
                    AluOp::Neg => {
                        emitter.emit_neg(dst, dst);
                    }
                    AluOp::End => {
                        // Endian conversion - not commonly used
                        return Err(Arm64JitError::UnsupportedInstruction);
                    }
                }
            }
            SourceType::Reg => {
                let src = BPF_TO_ARM64[insn.src_reg() as usize];
                match alu_op {
                    AluOp::Mov => {
                        emitter.emit_mov_reg(dst, src);
                    }
                    AluOp::Neg => {
                        emitter.emit_neg(dst, dst);
                    }
                    _ => {
                        self.emit_alu_reg(emitter, alu_op, dst, src)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Emit register-to-register ALU operation.
    fn emit_alu_reg(
        &self,
        emitter: &mut Arm64Emitter,
        op: AluOp,
        dst: u8,
        src: u8,
    ) -> Result<(), Arm64JitError> {
        match op {
            AluOp::Add => emitter.emit_add_reg(dst, dst, src),
            AluOp::Sub => emitter.emit_sub_reg(dst, dst, src),
            AluOp::Mul => emitter.emit_mul(dst, dst, src),
            AluOp::Div => emitter.emit_udiv(dst, dst, src),
            AluOp::Mod => {
                // ARM64 doesn't have MOD, compute: dst = dst - (dst/src)*src
                emitter.emit_udiv(X7, dst, src); // X7 = dst / src
                emitter.emit_mul(X7, X7, src); // X7 = X7 * src
                emitter.emit_sub_reg(dst, dst, X7); // dst = dst - X7
            }
            AluOp::And => emitter.emit_and_reg(dst, dst, src),
            AluOp::Or => emitter.emit_orr_reg(dst, dst, src),
            AluOp::Xor => emitter.emit_eor_reg(dst, dst, src),
            AluOp::Lsh => emitter.emit_lsl_reg(dst, dst, src),
            AluOp::Rsh => emitter.emit_lsr_reg(dst, dst, src),
            AluOp::Arsh => emitter.emit_asr_reg(dst, dst, src),
            _ => return Err(Arm64JitError::UnsupportedInstruction),
        }
        Ok(())
    }

    /// Compile jump instruction.
    fn compile_jmp(
        &self,
        emitter: &mut Arm64Emitter,
        insn: &BpfInsn,
        _idx: usize,
    ) -> Result<(), Arm64JitError> {
        // Check for EXIT
        if insn.is_exit() {
            // Move return value from BPF R0 (X7) to ARM64 return register (X0)
            emitter.emit_mov_reg(X0, X7);
            self.emit_epilogue(emitter, 512); // TODO: use actual stack size
            return Ok(());
        }

        let jmp_op = insn.jmp_op().ok_or(Arm64JitError::UnsupportedInstruction)?;
        let target = insn.offset; // Will be patched later

        if jmp_op.is_unconditional() {
            // JA: unconditional jump
            emitter.emit_b(target as i32 * 4); // Placeholder, will patch
            emitter.record_jump((insn.offset as usize).wrapping_add(_idx).wrapping_add(1));
        } else {
            // Conditional jump
            let dst = BPF_TO_ARM64[insn.dst_reg() as usize];

            match insn.source_type() {
                SourceType::Imm => {
                    emitter.emit_mov64_imm(X7, insn.imm as i64);
                    emitter.emit_cmp_reg(dst, X7);
                }
                SourceType::Reg => {
                    let src = BPF_TO_ARM64[insn.src_reg() as usize];
                    emitter.emit_cmp_reg(dst, src);
                }
            }

            // Emit conditional branch
            let cond = match jmp_op {
                JmpOp::Jeq => 0,   // EQ
                JmpOp::Jne => 1,   // NE
                JmpOp::Jgt => 8,   // HI (unsigned greater)
                JmpOp::Jge => 2,   // HS/CS (unsigned greater or equal)
                JmpOp::Jlt => 3,   // LO/CC (unsigned less)
                JmpOp::Jle => 9,   // LS (unsigned less or equal)
                JmpOp::Jsgt => 12, // GT (signed greater)
                JmpOp::Jsge => 10, // GE (signed greater or equal)
                JmpOp::Jslt => 11, // LT (signed less)
                JmpOp::Jsle => 13, // LE (signed less or equal)
                JmpOp::Jset => {
                    // Test bits: use TST instead of CMP
                    // This is a simplification - real impl would need TST
                    return Err(Arm64JitError::UnsupportedInstruction);
                }
                _ => return Err(Arm64JitError::UnsupportedInstruction),
            };

            emitter.emit_b_cond(cond, target as i32 * 4);
            emitter.record_jump((insn.offset as usize).wrapping_add(_idx).wrapping_add(1));
        }

        Ok(())
    }

    /// Compile load instruction (LDX).
    fn compile_ldx(&self, emitter: &mut Arm64Emitter, insn: &BpfInsn) -> Result<(), Arm64JitError> {
        let dst = BPF_TO_ARM64[insn.dst_reg() as usize];
        let src = BPF_TO_ARM64[insn.src_reg() as usize];
        let size = insn
            .mem_size()
            .ok_or(Arm64JitError::UnsupportedInstruction)?;

        emitter.emit_ldr(dst, src, insn.offset, size);

        Ok(())
    }

    /// Compile store instruction (STX, ST).
    fn compile_st(&self, emitter: &mut Arm64Emitter, insn: &BpfInsn) -> Result<(), Arm64JitError> {
        let dst = BPF_TO_ARM64[insn.dst_reg() as usize];
        let size = insn
            .mem_size()
            .ok_or(Arm64JitError::UnsupportedInstruction)?;

        match insn.class() {
            Some(OpcodeClass::Stx) => {
                let src = BPF_TO_ARM64[insn.src_reg() as usize];
                emitter.emit_str(src, dst, insn.offset, size);
            }
            Some(OpcodeClass::St) => {
                // Store immediate - load to temp first
                emitter.emit_mov64_imm(X7, insn.imm as i64);
                emitter.emit_str(X7, dst, insn.offset, size);
            }
            _ => return Err(Arm64JitError::UnsupportedInstruction),
        }

        Ok(())
    }

    /// Compile LD (wide immediate load).
    fn compile_ld(&self, emitter: &mut Arm64Emitter, insn: &BpfInsn) -> Result<(), Arm64JitError> {
        // LD_IMM64: load 64-bit immediate (uses two instructions)
        let dst = BPF_TO_ARM64[insn.dst_reg() as usize];

        // The full 64-bit value comes from imm (low 32) and next instruction's imm (high 32)
        // For now, just load the low 32 bits sign-extended
        emitter.emit_mov64_imm(dst, insn.imm as i64);

        Ok(())
    }

    /// Patch jump targets.
    fn patch_jumps(&self, emitter: &mut Arm64Emitter) -> Result<(), Arm64JitError> {
        for (code_offset, target_insn) in &emitter.jump_patches {
            if *target_insn >= emitter.insn_offsets.len() {
                return Err(Arm64JitError::UnsupportedInstruction);
            }

            let target_offset = emitter.insn_offsets[*target_insn];
            let branch_offset = (target_offset as i32) - (*code_offset as i32);

            // Patch the instruction
            let insn = u32::from_le_bytes(
                emitter.code[*code_offset..*code_offset + 4]
                    .try_into()
                    .map_err(|_| Arm64JitError::UnsupportedInstruction)?,
            );

            // Determine if it's a conditional or unconditional branch
            let patched = if insn & 0xFC000000 == 0x14000000 {
                // Unconditional branch
                let imm26 = ((branch_offset >> 2) as u32) & 0x3FFFFFF;
                0x14000000 | imm26
            } else {
                // Conditional branch
                let imm19 = ((branch_offset >> 2) as u32) & 0x7FFFF;
                (insn & 0xFF00001F) | (imm19 << 5)
            };

            emitter.code[*code_offset..*code_offset + 4].copy_from_slice(&patched.to_le_bytes());
        }

        Ok(())
    }
}

impl<P: PhysicalProfile> Default for Arm64JitCompiler<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// ARM64 JIT executor.
pub struct Arm64JitExecutor<P: PhysicalProfile = ActiveProfile> {
    compiler: Arm64JitCompiler<P>,
}

impl<P: PhysicalProfile> Arm64JitExecutor<P> {
    /// Create a new ARM64 JIT executor.
    pub fn new() -> Self {
        Self {
            compiler: Arm64JitCompiler::new(),
        }
    }

    /// Compile a program.
    pub fn compile(&self, program: &BpfProgram<P>) -> Result<Arm64JitProgram, Arm64JitError> {
        self.compiler.compile(program)
    }
}

impl<P: PhysicalProfile> Default for Arm64JitExecutor<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: PhysicalProfile> BpfExecutor<P> for Arm64JitExecutor<P> {
    fn execute(&self, program: &BpfProgram<P>, ctx: &BpfContext) -> BpfResult {
        // Try to compile
        match self.compile(program) {
            Ok(_jit_prog) => {
                // In a full implementation, we would:
                // 1. Map the code to executable memory
                // 2. Call the entry point
                // For now, fall back to interpreter
                let interp = crate::execution::Interpreter::<P>::new();
                interp.execute(program, ctx)
            }
            Err(_) => {
                // Fall back to interpreter
                let interp = crate::execution::Interpreter::<P>::new();
                interp.execute(program, ctx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_mapping() {
        // Verify register mapping is valid
        for (bpf_reg, arm_reg) in BPF_TO_ARM64.iter().enumerate() {
            assert!(
                *arm_reg <= 31,
                "Invalid ARM64 register for BPF R{}",
                bpf_reg
            );
        }
    }

    #[test]
    fn test_emitter_mov_imm() {
        let mut emitter = Arm64Emitter::new(64);
        emitter.emit_mov64_imm(X0, 42);
        assert!(!emitter.code.is_empty());
    }

    #[test]
    fn test_emitter_add_reg() {
        let mut emitter = Arm64Emitter::new(64);
        emitter.emit_add_reg(X0, X1, X2);
        assert_eq!(emitter.code.len(), 4);
    }
}
