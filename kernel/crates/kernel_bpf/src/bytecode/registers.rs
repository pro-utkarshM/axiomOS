//! BPF Register File
//!
//! eBPF has 11 64-bit registers (R0-R10):
//! - R0: Return value from functions and BPF program exit
//! - R1-R5: Function arguments (caller-saved)
//! - R6-R9: Callee-saved registers
//! - R10: Frame pointer (read-only)

use core::fmt;

/// BPF register identifiers.
///
/// eBPF uses 11 64-bit registers following a calling convention
/// similar to x86-64 System V ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Register {
    /// Return value / scratch register
    R0 = 0,
    /// Argument 1 / scratch (context pointer at entry)
    R1 = 1,
    /// Argument 2 / scratch
    R2 = 2,
    /// Argument 3 / scratch
    R3 = 3,
    /// Argument 4 / scratch
    R4 = 4,
    /// Argument 5 / scratch
    R5 = 5,
    /// Callee-saved
    R6 = 6,
    /// Callee-saved
    R7 = 7,
    /// Callee-saved
    R8 = 8,
    /// Callee-saved
    R9 = 9,
    /// Frame pointer (read-only)
    R10 = 10,
}

impl Register {
    /// Total number of BPF registers
    pub const COUNT: usize = 11;

    /// Try to create a register from a raw value.
    ///
    /// Returns `None` if the value is not a valid register (>= 11).
    #[inline]
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::R0),
            1 => Some(Self::R1),
            2 => Some(Self::R2),
            3 => Some(Self::R3),
            4 => Some(Self::R4),
            5 => Some(Self::R5),
            6 => Some(Self::R6),
            7 => Some(Self::R7),
            8 => Some(Self::R8),
            9 => Some(Self::R9),
            10 => Some(Self::R10),
            _ => None,
        }
    }

    /// Get the raw register number.
    #[inline]
    pub const fn as_raw(self) -> u8 {
        self as u8
    }

    /// Check if this is a caller-saved (scratch) register.
    ///
    /// R0-R5 are caller-saved and may be clobbered by function calls.
    #[inline]
    pub const fn is_caller_saved(self) -> bool {
        (self as u8) <= 5
    }

    /// Check if this is a callee-saved register.
    ///
    /// R6-R9 are callee-saved and must be preserved across function calls.
    #[inline]
    pub const fn is_callee_saved(self) -> bool {
        let val = self as u8;
        val >= 6 && val <= 9
    }

    /// Check if this is the frame pointer.
    ///
    /// R10 is the frame pointer and is read-only.
    #[inline]
    pub const fn is_frame_pointer(self) -> bool {
        matches!(self, Self::R10)
    }

    /// Check if this register can be written to.
    ///
    /// All registers except R10 (frame pointer) are writable.
    #[inline]
    pub const fn is_writable(self) -> bool {
        !self.is_frame_pointer()
    }
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", *self as u8)
    }
}

impl TryFrom<u8> for Register {
    type Error = InvalidRegister;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_raw(value).ok_or(InvalidRegister(value))
    }
}

/// Error returned when trying to create a register from an invalid value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRegister(pub u8);

impl fmt::Display for InvalidRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid register: {}", self.0)
    }
}

/// BPF register file containing all 11 registers.
///
/// This is the execution state that holds the current values of all registers
/// during BPF program execution.
#[derive(Clone)]
pub struct RegisterFile {
    /// Register values (R0-R10)
    values: [u64; Register::COUNT],
}

impl RegisterFile {
    /// Create a new register file with all registers initialized to zero.
    #[inline]
    pub const fn new() -> Self {
        Self {
            values: [0; Register::COUNT],
        }
    }

    /// Get the value of a register.
    #[inline]
    pub fn get(&self, reg: Register) -> u64 {
        self.values[reg as usize]
    }

    /// Set the value of a register.
    ///
    /// # Panics
    ///
    /// Debug builds will panic if attempting to write to R10 (frame pointer).
    #[inline]
    pub fn set(&mut self, reg: Register, value: u64) {
        debug_assert!(reg.is_writable(), "Cannot write to R10 (frame pointer)");
        self.values[reg as usize] = value;
    }

    /// Set the value of a register without checking if it's writable.
    ///
    /// # Safety
    ///
    /// Caller must ensure they are not writing to R10 in a way that
    /// violates BPF semantics.
    #[inline]
    pub unsafe fn set_unchecked(&mut self, reg: Register, value: u64) {
        self.values[reg as usize] = value;
    }

    /// Get a mutable reference to a register value.
    ///
    /// # Panics
    ///
    /// Debug builds will panic if attempting to get mutable reference to R10.
    #[inline]
    pub fn get_mut(&mut self, reg: Register) -> &mut u64 {
        debug_assert!(reg.is_writable(), "Cannot get mutable reference to R10");
        &mut self.values[reg as usize]
    }

    /// Initialize registers for program entry.
    ///
    /// Sets R1 to the context pointer and R10 to the frame pointer.
    #[inline]
    pub fn init_for_entry(&mut self, ctx_ptr: u64, frame_ptr: u64) {
        // Clear all registers
        self.values = [0; Register::COUNT];

        // R1 = context pointer
        self.values[Register::R1 as usize] = ctx_ptr;

        // R10 = frame pointer (stack base)
        self.values[Register::R10 as usize] = frame_ptr;
    }

    /// Get the return value (R0).
    #[inline]
    pub fn return_value(&self) -> u64 {
        self.get(Register::R0)
    }

    /// Get the context pointer (R1 at entry).
    #[inline]
    pub fn context_ptr(&self) -> u64 {
        self.get(Register::R1)
    }

    /// Get the frame pointer (R10).
    #[inline]
    pub fn frame_ptr(&self) -> u64 {
        self.get(Register::R10)
    }

    /// Get all register values as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u64; Register::COUNT] {
        &self.values
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RegisterFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisterFile")
            .field("r0", &format_args!("{:#018x}", self.values[0]))
            .field("r1", &format_args!("{:#018x}", self.values[1]))
            .field("r2", &format_args!("{:#018x}", self.values[2]))
            .field("r3", &format_args!("{:#018x}", self.values[3]))
            .field("r4", &format_args!("{:#018x}", self.values[4]))
            .field("r5", &format_args!("{:#018x}", self.values[5]))
            .field("r6", &format_args!("{:#018x}", self.values[6]))
            .field("r7", &format_args!("{:#018x}", self.values[7]))
            .field("r8", &format_args!("{:#018x}", self.values[8]))
            .field("r9", &format_args!("{:#018x}", self.values[9]))
            .field("r10 (fp)", &format_args!("{:#018x}", self.values[10]))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_from_raw() {
        assert_eq!(Register::from_raw(0), Some(Register::R0));
        assert_eq!(Register::from_raw(10), Some(Register::R10));
        assert_eq!(Register::from_raw(11), None);
        assert_eq!(Register::from_raw(255), None);
    }

    #[test]
    fn register_properties() {
        // Caller-saved
        assert!(Register::R0.is_caller_saved());
        assert!(Register::R5.is_caller_saved());
        assert!(!Register::R6.is_caller_saved());

        // Callee-saved
        assert!(!Register::R5.is_callee_saved());
        assert!(Register::R6.is_callee_saved());
        assert!(Register::R9.is_callee_saved());
        assert!(!Register::R10.is_callee_saved());

        // Frame pointer
        assert!(!Register::R9.is_frame_pointer());
        assert!(Register::R10.is_frame_pointer());

        // Writable
        assert!(Register::R0.is_writable());
        assert!(Register::R9.is_writable());
        assert!(!Register::R10.is_writable());
    }

    #[test]
    fn register_file_operations() {
        let mut regs = RegisterFile::new();

        // Initial values should be zero
        assert_eq!(regs.get(Register::R0), 0);
        assert_eq!(regs.get(Register::R10), 0);

        // Set and get
        regs.set(Register::R1, 0x1234);
        assert_eq!(regs.get(Register::R1), 0x1234);

        // Init for entry
        regs.init_for_entry(0xdead, 0xbeef);
        assert_eq!(regs.context_ptr(), 0xdead);
        assert_eq!(regs.frame_ptr(), 0xbeef);
        assert_eq!(regs.return_value(), 0);
    }
}
