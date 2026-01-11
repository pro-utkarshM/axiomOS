//! Architecture-agnostic types for addresses and frames
//!
//! This module provides common types that work across all architectures,
//! similar to Linux's phys_addr_t and virt_addr_t

use core::fmt;

/// Physical memory address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    /// Create a new physical address
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    /// Get the address as u64
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Check if address is aligned to given alignment
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0.is_multiple_of(align)
    }

    /// Align down to given alignment
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }

    /// Align up to given alignment
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PhysAddr(0x{:x})", self.0)
    }
}

impl From<u64> for PhysAddr {
    fn from(addr: u64) -> Self {
        Self(addr)
    }
}

impl From<usize> for PhysAddr {
    fn from(addr: usize) -> Self {
        Self(addr as u64)
    }
}

/// Virtual memory address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    /// Create a new virtual address
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    /// Get the address as u64
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Get the address as usize
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Get the address as a pointer
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    /// Get the address as a mutable pointer
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /// Check if address is aligned to given alignment
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0.is_multiple_of(align)
    }

    /// Align down to given alignment
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }

    /// Align up to given alignment
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VirtAddr(0x{:x})", self.0)
    }
}

impl From<u64> for VirtAddr {
    fn from(addr: u64) -> Self {
        Self(addr)
    }
}

impl From<usize> for VirtAddr {
    fn from(addr: usize) -> Self {
        Self(addr as u64)
    }
}

/// Physical memory frame (4KB page)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysFrame {
    start_address: PhysAddr,
}

impl PhysFrame {
    /// Size of a frame (4KB)
    pub const SIZE: u64 = 4096;

    /// Create a frame containing the given address
    #[inline]
    pub const fn containing_address(address: PhysAddr) -> Self {
        Self {
            start_address: PhysAddr(address.0 & !(Self::SIZE - 1)),
        }
    }

    /// Get the start address of the frame
    #[inline]
    pub const fn start_address(self) -> PhysAddr {
        self.start_address
    }

    /// Get the frame number
    #[inline]
    pub const fn number(self) -> u64 {
        self.start_address.0 / Self::SIZE
    }
}

/// Range of physical frames
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysFrameRange {
    pub start: PhysFrame,
    pub end: PhysFrame,
}

impl PhysFrameRange {
    /// Create a new frame range
    pub const fn new(start: PhysFrame, end: PhysFrame) -> Self {
        Self { start, end }
    }

    /// Check if the range is empty
    pub const fn is_empty(&self) -> bool {
        self.start.start_address.0 >= self.end.start_address.0
    }

    /// Get the number of frames in the range
    pub const fn count(&self) -> u64 {
        if self.is_empty() {
            0
        } else {
            (self.end.start_address.0 - self.start.start_address.0) / PhysFrame::SIZE
        }
    }
}

// Re-export x86_64 types when building for x86_64 for compatibility
#[cfg(target_arch = "x86_64")]
pub use x86_64::{
    PhysAddr as X86PhysAddr, VirtAddr as X86VirtAddr,
    structures::paging::{PhysFrame as X86PhysFrame, Size4KiB},
};

#[cfg(target_arch = "x86_64")]
impl From<X86PhysAddr> for PhysAddr {
    fn from(addr: X86PhysAddr) -> Self {
        Self(addr.as_u64())
    }
}

#[cfg(target_arch = "x86_64")]
impl From<PhysAddr> for X86PhysAddr {
    fn from(addr: PhysAddr) -> Self {
        X86PhysAddr::new(addr.0)
    }
}

#[cfg(target_arch = "x86_64")]
impl From<X86VirtAddr> for VirtAddr {
    fn from(addr: X86VirtAddr) -> Self {
        Self(addr.as_u64())
    }
}

#[cfg(target_arch = "x86_64")]
impl From<VirtAddr> for X86VirtAddr {
    fn from(addr: VirtAddr) -> Self {
        X86VirtAddr::new(addr.0)
    }
}

#[cfg(target_arch = "x86_64")]
impl From<X86PhysFrame<Size4KiB>> for PhysFrame {
    fn from(frame: X86PhysFrame<Size4KiB>) -> Self {
        Self {
            start_address: PhysAddr(frame.start_address().as_u64()),
        }
    }
}
