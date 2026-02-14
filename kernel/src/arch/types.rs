//! Architecture-agnostic types for addresses and frames
//!
//! This module provides common types that work across all architectures,
//! similar to Linux's phys_addr_t and virt_addr_t

pub use kernel_physical_memory::{
    PageSize, PhysAddr, PhysFrame, PhysFrameRangeInclusive,
    PhysFrameRangeInclusive as PhysFrameRange, Size1GiB, Size2MiB, Size4KiB,
};
pub use kernel_virtual_memory::{Page, PageRangeInclusive, VirtAddr};
#[cfg(target_arch = "x86_64")]
pub use x86_64::structures::paging::PageTableFlags;

#[cfg(target_arch = "aarch64")]
pub use crate::arch::aarch64::paging::PageTableFlags;

// Extension traits to provide common methods if they are missing
pub trait PhysAddrExt {
    fn align_up(self, align: u64) -> Self;
    fn align_down(self, align: u64) -> Self;
    #[allow(clippy::wrong_self_convention)]
    fn is_aligned(self, align: u64) -> bool;
}

impl PhysAddrExt for PhysAddr {
    #[inline]
    fn align_up(self, align: u64) -> Self {
        Self::new((self.as_u64() + align - 1) & !(align - 1))
    }

    #[inline]
    fn align_down(self, align: u64) -> Self {
        Self::new(self.as_u64() & !(align - 1))
    }

    #[inline]
    fn is_aligned(self, align: u64) -> bool {
        self.as_u64().is_multiple_of(align)
    }
}

pub trait VirtAddrExt {
    fn align_up(self, align: u64) -> Self;
    fn align_down(self, align: u64) -> Self;
    #[allow(clippy::wrong_self_convention)]
    fn is_aligned(self, align: u64) -> bool;
    #[allow(clippy::wrong_self_convention)]
    fn as_mut_ptr<T>(self) -> *mut T;
    #[allow(clippy::wrong_self_convention)]
    fn as_ptr<T>(self) -> *const T;
}

impl VirtAddrExt for VirtAddr {
    #[inline]
    fn align_up(self, align: u64) -> Self {
        Self::new((self.as_u64() + align - 1) & !(align - 1))
    }

    #[inline]
    fn align_down(self, align: u64) -> Self {
        Self::new(self.as_u64() & !(align - 1))
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)]
    fn is_aligned(self, align: u64) -> bool {
        self.as_u64().is_multiple_of(align)
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)]
    fn as_mut_ptr<T>(self) -> *mut T {
        self.as_u64() as *mut T
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)]
    fn as_ptr<T>(self) -> *const T {
        self.as_u64() as *const T
    }
}

pub trait PhysFrameExt {
    fn addr(self) -> u64;
}

impl<S: PageSize> PhysFrameExt for PhysFrame<S> {
    #[inline]
    fn addr(self) -> u64 {
        self.start_address().as_u64()
    }
}
