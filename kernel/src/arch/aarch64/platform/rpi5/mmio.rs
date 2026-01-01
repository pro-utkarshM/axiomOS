//! Memory-Mapped I/O Abstraction for RP1 peripherals
//!
//! Provides type-safe volatile register access for hardware peripherals.
//! All register accesses use volatile operations to prevent compiler
//! optimizations that could reorder or eliminate hardware accesses.

use core::ptr::{read_volatile, write_volatile};

/// Memory-mapped I/O register wrapper
///
/// Provides volatile read/write access to a hardware register at a fixed address.
/// The type parameter `T` determines the register width (typically u32 for ARM).
#[repr(transparent)]
pub struct MmioReg<T: Copy> {
    addr: *mut T,
}

impl<T: Copy> MmioReg<T> {
    /// Create a new MMIO register wrapper at the given address
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - The address is valid and properly aligned for type T
    /// - The address points to a memory-mapped hardware register
    /// - No other code accesses this register without proper synchronization
    #[inline]
    pub const unsafe fn new(addr: usize) -> Self {
        Self {
            addr: addr as *mut T,
        }
    }

    /// Read the current value of the register
    #[inline(always)]
    pub fn read(&self) -> T {
        // Safety: Address was validated at construction time
        unsafe { read_volatile(self.addr) }
    }

    /// Write a value to the register
    #[inline(always)]
    pub fn write(&self, value: T) {
        // Safety: Address was validated at construction time
        unsafe { write_volatile(self.addr, value) }
    }

    /// Read-modify-write the register using a closure
    ///
    /// This is not atomic! If atomicity is required, disable interrupts
    /// or use proper synchronization.
    #[inline(always)]
    pub fn modify<F>(&self, f: F)
    where
        F: FnOnce(T) -> T,
    {
        let val = self.read();
        let new_val = f(val);
        self.write(new_val);
    }
}

// Additional methods for u32 registers (most common case)
impl MmioReg<u32> {
    /// Set specific bits in the register (OR operation)
    #[inline(always)]
    pub fn set_bits(&self, mask: u32) {
        self.modify(|v| v | mask);
    }

    /// Clear specific bits in the register (AND NOT operation)
    #[inline(always)]
    pub fn clear_bits(&self, mask: u32) {
        self.modify(|v| v & !mask);
    }

    /// Check if specific bits are set
    #[inline(always)]
    pub fn is_set(&self, mask: u32) -> bool {
        (self.read() & mask) != 0
    }

    /// Wait until specific bits are set
    #[inline]
    pub fn wait_set(&self, mask: u32) {
        while !self.is_set(mask) {
            core::hint::spin_loop();
        }
    }

    /// Wait until specific bits are clear
    #[inline]
    pub fn wait_clear(&self, mask: u32) {
        while self.is_set(mask) {
            core::hint::spin_loop();
        }
    }
}

// MmioReg is Send+Sync because MMIO access is inherently thread-safe
// when properly synchronized at higher levels (which is the caller's
// responsibility).
unsafe impl<T: Copy> Send for MmioReg<T> {}
unsafe impl<T: Copy> Sync for MmioReg<T> {}
