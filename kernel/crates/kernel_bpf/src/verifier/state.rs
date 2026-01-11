//! Verifier State Tracking
//!
//! This module defines the state tracked during BPF program verification,
//! including register types, stack slots, and the overall verifier state
//! at each program point.

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

use crate::bytecode::registers::Register;

/// Type of value held in a register.
///
/// The verifier tracks what type of value each register holds to ensure
/// type-safe operations and memory access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegType {
    /// Register has not been initialized
    #[default]
    NotInit,

    /// Scalar value (integer)
    Scalar,

    /// Pointer to stack frame (with offset from FP)
    PtrToStack,

    /// Pointer to map value
    PtrToMapValue,

    /// Pointer to map key
    PtrToMapKey,

    /// Pointer to context
    PtrToCtx,

    /// Pointer to packet data
    PtrToPacket,

    /// Pointer to packet end
    PtrToPacketEnd,

    /// Pointer to packet metadata
    PtrToPacketMeta,

    /// Constant pointer to map
    ConstPtrToMap,

    /// Frame pointer (R10, read-only)
    PtrToFp,

    /// Null pointer
    NullPtr,
}

impl RegType {
    /// Check if this is a pointer type.
    #[inline]
    pub const fn is_pointer(&self) -> bool {
        !matches!(self, Self::NotInit | Self::Scalar)
    }

    /// Check if this type can be dereferenced for read.
    #[inline]
    pub const fn can_read(&self) -> bool {
        matches!(
            self,
            Self::PtrToStack
                | Self::PtrToMapValue
                | Self::PtrToCtx
                | Self::PtrToPacket
                | Self::PtrToPacketMeta
        )
    }

    /// Check if this type can be dereferenced for write.
    #[inline]
    pub const fn can_write(&self) -> bool {
        matches!(
            self,
            Self::PtrToStack | Self::PtrToMapValue | Self::PtrToPacket
        )
    }
}

/// State of a single register during verification.
#[derive(Debug, Clone)]
pub struct RegState {
    /// Type of value in the register
    pub reg_type: RegType,

    /// For scalar values: known value if constant, None otherwise
    pub scalar_value: Option<ScalarValue>,

    /// For pointer types: offset from base
    pub ptr_offset: i64,

    /// For map pointers: map ID
    pub map_id: Option<u32>,
}

impl RegState {
    /// Create an uninitialized register state.
    pub const fn uninit() -> Self {
        Self {
            reg_type: RegType::NotInit,
            scalar_value: None,
            ptr_offset: 0,
            map_id: None,
        }
    }

    /// Create a scalar register state.
    pub fn scalar(value: Option<ScalarValue>) -> Self {
        Self {
            reg_type: RegType::Scalar,
            scalar_value: value,
            ptr_offset: 0,
            map_id: None,
        }
    }

    /// Create a stack pointer state.
    pub fn stack_ptr(offset: i64) -> Self {
        Self {
            reg_type: RegType::PtrToStack,
            scalar_value: None,
            ptr_offset: offset,
            map_id: None,
        }
    }

    /// Create a frame pointer state (R10).
    pub const fn frame_ptr() -> Self {
        Self {
            reg_type: RegType::PtrToFp,
            scalar_value: None,
            ptr_offset: 0,
            map_id: None,
        }
    }

    /// Create a context pointer state (R1 at entry).
    pub const fn ctx_ptr() -> Self {
        Self {
            reg_type: RegType::PtrToCtx,
            scalar_value: None,
            ptr_offset: 0,
            map_id: None,
        }
    }

    /// Check if the register is initialized.
    #[inline]
    pub fn is_init(&self) -> bool {
        !matches!(self.reg_type, RegType::NotInit)
    }
}

impl Default for RegState {
    fn default() -> Self {
        Self::uninit()
    }
}

/// Tracked scalar value with range information.
#[derive(Debug, Clone, Copy)]
pub struct ScalarValue {
    /// Known exact value (if constant)
    pub value: Option<u64>,

    /// Minimum possible value
    pub min: u64,

    /// Maximum possible value
    pub max: u64,

    /// Tracked number representation
    pub tnum: TnumValue,
}

impl ScalarValue {
    /// Create a constant scalar value.
    pub const fn constant(value: u64) -> Self {
        Self {
            value: Some(value),
            min: value,
            max: value,
            tnum: TnumValue::constant(value),
        }
    }

    /// Create an unknown scalar value.
    pub const fn unknown() -> Self {
        Self {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum: TnumValue::unknown(),
        }
    }

    /// Check if this is a known constant.
    #[inline]
    pub const fn is_constant(&self) -> bool {
        self.value.is_some()
    }

    /// Check if the value is known to be zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.value == Some(0)
    }

    /// Check if the value could be zero.
    #[inline]
    pub fn could_be_zero(&self) -> bool {
        self.min == 0 || self.value == Some(0)
    }
}

impl Default for ScalarValue {
    fn default() -> Self {
        Self::unknown()
    }
}

/// Tnum (tracked number) for partial value tracking.
///
/// A tnum represents partial knowledge about a value:
/// - `value`: bits that are known to be set
/// - `mask`: bits that are unknown (1 = unknown, 0 = known)
///
/// For a known constant, mask = 0 and value = the constant.
/// For completely unknown, mask = u64::MAX and value = 0.
#[derive(Debug, Clone, Copy)]
pub struct TnumValue {
    /// Known bit values (only valid where mask is 0)
    pub value: u64,
    /// Unknown bit mask (1 = unknown)
    pub mask: u64,
}

impl TnumValue {
    /// Create a tnum for a known constant.
    pub const fn constant(value: u64) -> Self {
        Self { value, mask: 0 }
    }

    /// Create a tnum for a completely unknown value.
    pub const fn unknown() -> Self {
        Self {
            value: 0,
            mask: u64::MAX,
        }
    }

    /// Check if this is a known constant.
    #[inline]
    pub const fn is_constant(&self) -> bool {
        self.mask == 0
    }

    /// Get the known constant value if fully known.
    #[inline]
    pub const fn as_constant(&self) -> Option<u64> {
        if self.mask == 0 {
            Some(self.value)
        } else {
            None
        }
    }
}

/// State of a stack slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackSlot {
    /// Slot is invalid/uninitialized
    #[default]
    Invalid,

    /// Slot contains spilled register
    Spill(Register),

    /// Slot contains scalar data
    Scalar,

    /// Slot contains zero
    Zero,
}

/// Stack state during verification.
#[derive(Clone)]
pub struct StackState {
    /// Stack slots (indexed by offset from FP, negative values)
    /// Index 0 = FP-1, Index 1 = FP-2, etc.
    slots: Vec<StackSlot>,

    /// Maximum stack depth used (positive value)
    max_depth: usize,
}

impl StackState {
    /// Create a new stack state with given capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            slots: alloc::vec![StackSlot::Invalid; max_size],
            max_depth: 0,
        }
    }

    /// Get the slot at the given offset from FP.
    ///
    /// Offset should be negative (stack grows down).
    pub fn get(&self, offset: i64) -> Option<StackSlot> {
        if offset >= 0 || offset < -(self.slots.len() as i64) {
            return None;
        }
        let idx = (-offset - 1) as usize;
        Some(self.slots[idx])
    }

    /// Set the slot at the given offset from FP.
    pub fn set(&mut self, offset: i64, slot: StackSlot) -> bool {
        if offset >= 0 || offset < -(self.slots.len() as i64) {
            return false;
        }
        let idx = (-offset - 1) as usize;
        self.slots[idx] = slot;

        // Update max depth
        let depth = idx + 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }

        true
    }

    /// Get the maximum stack depth used.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Check if access at offset with size is valid.
    pub fn is_valid_access(&self, offset: i64, size: usize) -> bool {
        // Stack access must be negative offset from FP
        if offset >= 0 {
            return false;
        }

        // Check bounds
        let end_offset = offset - (size as i64) + 1;
        if end_offset < -(self.slots.len() as i64) {
            return false;
        }

        true
    }
}

impl Default for StackState {
    fn default() -> Self {
        Self::new(512) // Default 512 bytes
    }
}

impl fmt::Debug for StackState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StackState")
            .field("max_depth", &self.max_depth)
            .field("capacity", &self.slots.len())
            .finish()
    }
}

/// Complete verifier state at a program point.
#[derive(Clone)]
pub struct VerifierState {
    /// Register states
    pub regs: [RegState; Register::COUNT],

    /// Stack state
    pub stack: StackState,

    /// Current instruction pointer
    pub insn_idx: usize,

    /// Number of instructions processed (for bounds checking)
    pub insn_processed: usize,
}

impl VerifierState {
    /// Create initial verifier state for program entry.
    pub fn new_entry(stack_size: usize) -> Self {
        let mut regs = core::array::from_fn(|_| RegState::uninit());

        // R1 = context pointer at entry
        regs[Register::R1 as usize] = RegState::ctx_ptr();

        // R10 = frame pointer (always valid)
        regs[Register::R10 as usize] = RegState::frame_ptr();

        Self {
            regs,
            stack: StackState::new(stack_size),
            insn_idx: 0,
            insn_processed: 0,
        }
    }

    /// Get register state.
    pub fn reg(&self, reg: Register) -> &RegState {
        &self.regs[reg as usize]
    }

    /// Get mutable register state.
    pub fn reg_mut(&mut self, reg: Register) -> &mut RegState {
        &mut self.regs[reg as usize]
    }

    /// Check if a register is initialized.
    pub fn is_reg_init(&self, reg: Register) -> bool {
        self.regs[reg as usize].is_init()
    }

    /// Mark a register as containing a scalar value.
    pub fn set_scalar(&mut self, reg: Register, value: Option<ScalarValue>) {
        self.regs[reg as usize] = RegState::scalar(value);
    }

    /// Advance to next instruction.
    pub fn advance(&mut self) {
        self.insn_idx += 1;
        self.insn_processed += 1;
    }

    /// Jump to a specific instruction.
    pub fn jump_to(&mut self, idx: usize) {
        self.insn_idx = idx;
        self.insn_processed += 1;
    }
}

impl fmt::Debug for VerifierState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifierState")
            .field("insn_idx", &self.insn_idx)
            .field("insn_processed", &self.insn_processed)
            .field("stack_depth", &self.stack.max_depth())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_type_properties() {
        assert!(!RegType::NotInit.is_pointer());
        assert!(!RegType::Scalar.is_pointer());
        assert!(RegType::PtrToStack.is_pointer());
        assert!(RegType::PtrToCtx.is_pointer());

        assert!(RegType::PtrToStack.can_read());
        assert!(RegType::PtrToStack.can_write());
        assert!(RegType::PtrToCtx.can_read());
        assert!(!RegType::PtrToCtx.can_write());
    }

    #[test]
    fn scalar_value_tracking() {
        let constant = ScalarValue::constant(42);
        assert!(constant.is_constant());
        assert_eq!(constant.value, Some(42));

        let unknown = ScalarValue::unknown();
        assert!(!unknown.is_constant());
        assert!(unknown.could_be_zero());
    }

    #[test]
    fn stack_state_operations() {
        let mut stack = StackState::new(256);

        // Valid negative offsets
        assert!(stack.set(-1, StackSlot::Scalar));
        assert!(stack.set(-8, StackSlot::Zero));

        assert_eq!(stack.get(-1), Some(StackSlot::Scalar));
        assert_eq!(stack.get(-8), Some(StackSlot::Zero));
        assert_eq!(stack.max_depth(), 8);

        // Invalid positive offset
        assert!(!stack.set(0, StackSlot::Scalar));
        assert!(!stack.set(1, StackSlot::Scalar));
    }

    #[test]
    fn verifier_state_entry() {
        let state = VerifierState::new_entry(512);

        // R1 should be context pointer
        assert!(state.is_reg_init(Register::R1));
        assert_eq!(state.reg(Register::R1).reg_type, RegType::PtrToCtx);

        // R10 should be frame pointer
        assert!(state.is_reg_init(Register::R10));
        assert_eq!(state.reg(Register::R10).reg_type, RegType::PtrToFp);

        // Other registers should be uninitialized
        assert!(!state.is_reg_init(Register::R0));
        assert!(!state.is_reg_init(Register::R2));
    }
}
