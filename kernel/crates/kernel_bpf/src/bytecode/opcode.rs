//! BPF Opcode Definitions
//!
//! eBPF instructions use an 8-bit opcode with the following structure:
//!
//! ```text
//! +----------------+--------+--------------------+
//! |    4 bits      | 1 bit  |      3 bits        |
//! |   operation    | source |   instruction      |
//! |    code        |  type  |     class          |
//! +----------------+--------+--------------------+
//! ```
//!
//! - Instruction class (bits 0-2): Type of operation
//! - Source type (bit 3): Immediate (0) or register (1)
//! - Operation code (bits 4-7): Specific operation within class

use core::fmt;

/// Instruction class (bits 0-2 of opcode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpcodeClass {
    /// Load double word (LD class, legacy)
    Ld = 0x00,
    /// Load from memory (LDX class)
    Ldx = 0x01,
    /// Store immediate (ST class, legacy)
    St = 0x02,
    /// Store from register (STX class)
    Stx = 0x03,
    /// 32-bit ALU operations
    Alu32 = 0x04,
    /// 64-bit jumps
    Jmp = 0x05,
    /// 32-bit jumps
    Jmp32 = 0x06,
    /// 64-bit ALU operations
    Alu64 = 0x07,
}

impl OpcodeClass {
    /// Extract instruction class from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode & 0x07 {
            0x00 => Some(Self::Ld),
            0x01 => Some(Self::Ldx),
            0x02 => Some(Self::St),
            0x03 => Some(Self::Stx),
            0x04 => Some(Self::Alu32),
            0x05 => Some(Self::Jmp),
            0x06 => Some(Self::Jmp32),
            0x07 => Some(Self::Alu64),
            _ => None,
        }
    }

    /// Check if this is a memory load class.
    #[inline]
    pub const fn is_load(self) -> bool {
        matches!(self, Self::Ld | Self::Ldx)
    }

    /// Check if this is a memory store class.
    #[inline]
    pub const fn is_store(self) -> bool {
        matches!(self, Self::St | Self::Stx)
    }

    /// Check if this is a memory operation class.
    #[inline]
    pub const fn is_memory(self) -> bool {
        self.is_load() || self.is_store()
    }

    /// Check if this is an ALU class.
    #[inline]
    pub const fn is_alu(self) -> bool {
        matches!(self, Self::Alu32 | Self::Alu64)
    }

    /// Check if this is a jump class.
    #[inline]
    pub const fn is_jump(self) -> bool {
        matches!(self, Self::Jmp | Self::Jmp32)
    }
}

/// Source type (bit 3 of opcode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SourceType {
    /// Source is immediate value in instruction
    Imm = 0x00,
    /// Source is register
    Reg = 0x08,
}

impl SourceType {
    /// Extract source type from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Self {
        if opcode & 0x08 != 0 {
            Self::Reg
        } else {
            Self::Imm
        }
    }
}

/// ALU operation codes (bits 4-7 of opcode for ALU class).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AluOp {
    /// Add: dst += src
    Add = 0x00,
    /// Subtract: dst -= src
    Sub = 0x10,
    /// Multiply: dst *= src
    Mul = 0x20,
    /// Divide: dst /= src
    Div = 0x30,
    /// Bitwise OR: dst |= src
    Or = 0x40,
    /// Bitwise AND: dst &= src
    And = 0x50,
    /// Left shift: dst <<= src
    Lsh = 0x60,
    /// Logical right shift: dst >>= src
    Rsh = 0x70,
    /// Negate: dst = -dst
    Neg = 0x80,
    /// Modulo: dst %= src
    Mod = 0x90,
    /// Bitwise XOR: dst ^= src
    Xor = 0xa0,
    /// Move: dst = src
    Mov = 0xb0,
    /// Arithmetic right shift: dst >>= src (signed)
    Arsh = 0xc0,
    /// Byte swap: dst = bswap(dst)
    End = 0xd0,
}

impl AluOp {
    /// Extract ALU operation from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode & 0xf0 {
            0x00 => Some(Self::Add),
            0x10 => Some(Self::Sub),
            0x20 => Some(Self::Mul),
            0x30 => Some(Self::Div),
            0x40 => Some(Self::Or),
            0x50 => Some(Self::And),
            0x60 => Some(Self::Lsh),
            0x70 => Some(Self::Rsh),
            0x80 => Some(Self::Neg),
            0x90 => Some(Self::Mod),
            0xa0 => Some(Self::Xor),
            0xb0 => Some(Self::Mov),
            0xc0 => Some(Self::Arsh),
            0xd0 => Some(Self::End),
            _ => None,
        }
    }

    /// Check if this operation can divide by zero.
    #[inline]
    pub const fn can_divide_by_zero(self) -> bool {
        matches!(self, Self::Div | Self::Mod)
    }

    /// Check if this is a unary operation.
    #[inline]
    pub const fn is_unary(self) -> bool {
        matches!(self, Self::Neg | Self::End)
    }
}

impl fmt::Display for AluOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Or => "or",
            Self::And => "and",
            Self::Lsh => "lsh",
            Self::Rsh => "rsh",
            Self::Neg => "neg",
            Self::Mod => "mod",
            Self::Xor => "xor",
            Self::Mov => "mov",
            Self::Arsh => "arsh",
            Self::End => "end",
        };
        write!(f, "{}", s)
    }
}

/// Jump operation codes (bits 4-7 of opcode for JMP class).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JmpOp {
    /// Unconditional jump
    Ja = 0x00,
    /// Jump if equal: if dst == src
    Jeq = 0x10,
    /// Jump if greater than (unsigned): if dst > src
    Jgt = 0x20,
    /// Jump if greater or equal (unsigned): if dst >= src
    Jge = 0x30,
    /// Jump if bits set: if dst & src
    Jset = 0x40,
    /// Jump if not equal: if dst != src
    Jne = 0x50,
    /// Jump if greater than (signed): if dst > src
    Jsgt = 0x60,
    /// Jump if greater or equal (signed): if dst >= src
    Jsge = 0x70,
    /// Function call
    Call = 0x80,
    /// Program exit
    Exit = 0x90,
    /// Jump if less than (unsigned): if dst < src
    Jlt = 0xa0,
    /// Jump if less or equal (unsigned): if dst <= src
    Jle = 0xb0,
    /// Jump if less than (signed): if dst < src
    Jslt = 0xc0,
    /// Jump if less or equal (signed): if dst <= src
    Jsle = 0xd0,
}

impl JmpOp {
    /// Extract jump operation from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode & 0xf0 {
            0x00 => Some(Self::Ja),
            0x10 => Some(Self::Jeq),
            0x20 => Some(Self::Jgt),
            0x30 => Some(Self::Jge),
            0x40 => Some(Self::Jset),
            0x50 => Some(Self::Jne),
            0x60 => Some(Self::Jsgt),
            0x70 => Some(Self::Jsge),
            0x80 => Some(Self::Call),
            0x90 => Some(Self::Exit),
            0xa0 => Some(Self::Jlt),
            0xb0 => Some(Self::Jle),
            0xc0 => Some(Self::Jslt),
            0xd0 => Some(Self::Jsle),
            _ => None,
        }
    }

    /// Check if this is a conditional jump.
    #[inline]
    pub const fn is_conditional(self) -> bool {
        !matches!(self, Self::Ja | Self::Call | Self::Exit)
    }

    /// Check if this is an unconditional jump.
    #[inline]
    pub const fn is_unconditional(self) -> bool {
        matches!(self, Self::Ja)
    }

    /// Check if this is a control flow terminator.
    #[inline]
    pub const fn is_terminator(self) -> bool {
        matches!(self, Self::Exit)
    }

    /// Check if this is a function call.
    #[inline]
    pub const fn is_call(self) -> bool {
        matches!(self, Self::Call)
    }

    /// Check if this jump uses signed comparison.
    #[inline]
    pub const fn is_signed(self) -> bool {
        matches!(self, Self::Jsgt | Self::Jsge | Self::Jslt | Self::Jsle)
    }
}

impl fmt::Display for JmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ja => "ja",
            Self::Jeq => "jeq",
            Self::Jgt => "jgt",
            Self::Jge => "jge",
            Self::Jset => "jset",
            Self::Jne => "jne",
            Self::Jsgt => "jsgt",
            Self::Jsge => "jsge",
            Self::Call => "call",
            Self::Exit => "exit",
            Self::Jlt => "jlt",
            Self::Jle => "jle",
            Self::Jslt => "jslt",
            Self::Jsle => "jsle",
        };
        write!(f, "{}", s)
    }
}

/// Memory access size (bits 3-4 of opcode for memory class).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MemSize {
    /// 32-bit word
    Word = 0x00,
    /// 16-bit half word
    Half = 0x08,
    /// 8-bit byte
    Byte = 0x10,
    /// 64-bit double word
    DWord = 0x18,
}

impl MemSize {
    /// Extract memory size from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode & 0x18 {
            0x00 => Some(Self::Word),
            0x08 => Some(Self::Half),
            0x10 => Some(Self::Byte),
            0x18 => Some(Self::DWord),
            _ => None,
        }
    }

    /// Get the size in bytes.
    #[inline]
    pub const fn size_bytes(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Half => 2,
            Self::Word => 4,
            Self::DWord => 8,
        }
    }
}

/// Memory access mode (bits 5-7 of opcode for memory class).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MemMode {
    /// Immediate (for LD class)
    Imm = 0x00,
    /// Absolute address
    Abs = 0x20,
    /// Indirect address
    Ind = 0x40,
    /// Memory at register + offset
    Mem = 0x60,
    /// Atomic operations
    Atomic = 0xc0,
}

impl MemMode {
    /// Extract memory mode from opcode.
    #[inline]
    pub const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode & 0xe0 {
            0x00 => Some(Self::Imm),
            0x20 => Some(Self::Abs),
            0x40 => Some(Self::Ind),
            0x60 => Some(Self::Mem),
            0xc0 => Some(Self::Atomic),
            _ => None,
        }
    }
}

/// Atomic operation codes (in imm field for atomic memory operations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AtomicOp {
    /// Atomic add
    Add = 0x00,
    /// Atomic or
    Or = 0x40,
    /// Atomic and
    And = 0x50,
    /// Atomic xor
    Xor = 0xa0,
    /// Atomic exchange
    Xchg = 0xe0,
    /// Atomic compare and exchange
    Cmpxchg = 0xf0,
}

impl AtomicOp {
    /// Extract atomic operation from immediate value.
    #[inline]
    pub const fn from_imm(imm: i32) -> Option<Self> {
        match (imm as u32) & 0xf0 {
            0x00 => Some(Self::Add),
            0x40 => Some(Self::Or),
            0x50 => Some(Self::And),
            0xa0 => Some(Self::Xor),
            0xe0 => Some(Self::Xchg),
            0xf0 => Some(Self::Cmpxchg),
            _ => None,
        }
    }

    /// Check if this operation returns the old value.
    #[inline]
    pub const fn fetches_value(self) -> bool {
        matches!(self, Self::Xchg | Self::Cmpxchg)
    }
}

/// Decoded opcode information.
#[derive(Debug, Clone, Copy)]
pub struct DecodedOpcode {
    /// Raw opcode byte
    pub raw: u8,
    /// Instruction class
    pub class: OpcodeClass,
    /// Source type (immediate or register)
    pub source: SourceType,
}

impl DecodedOpcode {
    /// Decode an opcode byte.
    #[inline]
    pub const fn decode(opcode: u8) -> Option<Self> {
        let class = match OpcodeClass::from_opcode(opcode) {
            Some(c) => c,
            None => return None,
        };
        let source = SourceType::from_opcode(opcode);

        Some(Self {
            raw: opcode,
            class,
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_class_extraction() {
        assert_eq!(OpcodeClass::from_opcode(0x07), Some(OpcodeClass::Alu64));
        assert_eq!(OpcodeClass::from_opcode(0x04), Some(OpcodeClass::Alu32));
        assert_eq!(OpcodeClass::from_opcode(0x05), Some(OpcodeClass::Jmp));
        assert_eq!(OpcodeClass::from_opcode(0x61), Some(OpcodeClass::Ldx));
    }

    #[test]
    fn source_type_extraction() {
        assert_eq!(SourceType::from_opcode(0x07), SourceType::Imm);
        assert_eq!(SourceType::from_opcode(0x0f), SourceType::Reg);
    }

    #[test]
    fn alu_op_extraction() {
        assert_eq!(AluOp::from_opcode(0x07), Some(AluOp::Add));
        assert_eq!(AluOp::from_opcode(0x17), Some(AluOp::Sub));
        assert_eq!(AluOp::from_opcode(0xb7), Some(AluOp::Mov));
    }

    #[test]
    fn jmp_op_extraction() {
        assert_eq!(JmpOp::from_opcode(0x05), Some(JmpOp::Ja));
        assert_eq!(JmpOp::from_opcode(0x95), Some(JmpOp::Exit));
        assert_eq!(JmpOp::from_opcode(0x85), Some(JmpOp::Call));
    }

    #[test]
    fn mem_size_bytes() {
        assert_eq!(MemSize::Byte.size_bytes(), 1);
        assert_eq!(MemSize::Half.size_bytes(), 2);
        assert_eq!(MemSize::Word.size_bytes(), 4);
        assert_eq!(MemSize::DWord.size_bytes(), 8);
    }
}
