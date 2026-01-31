//! Architecture-agnostic types for addresses and frames
//!
//! This module provides common types that work across all architectures,
//! similar to Linux's phys_addr_t and virt_addr_t

#[cfg(not(target_arch = "x86_64"))]
use kernel_virtual_memory::Segment;

#[cfg(not(target_arch = "x86_64"))]
mod non_x86 {
    use super::*;
    pub use kernel_physical_memory::{
        PageSize, PhysAddr, PhysFrame, PhysFrameRangeInclusive,
        PhysFrameRangeInclusive as PhysFrameRange, Size1GiB, Size2MiB, Size4KiB,
    };
    pub use kernel_virtual_memory::VirtAddr;

    #[cfg(target_arch = "aarch64")]
    pub use crate::arch::aarch64::paging::PageTableFlags;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(transparent)]
    pub struct Page<S: PageSize = Size4KiB> {
        pub start_address: VirtAddr,
        pub size: core::marker::PhantomData<S>,
    }

    impl<S: PageSize> Page<S> {
        pub const fn containing_address(address: VirtAddr) -> Self {
            Self {
                start_address: VirtAddr::new(address.as_u64() & !(S::SIZE - 1)),
                size: core::marker::PhantomData,
            }
        }

        pub const fn start_address(self) -> VirtAddr {
            self.start_address
        }

        pub const fn size(self) -> u64 {
            S::SIZE
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageRangeInclusive<S: PageSize = Size4KiB> {
        pub start: Page<S>,
        pub end: Page<S>,
    }

    impl<S: PageSize> Iterator for PageRangeInclusive<S> {
        type Item = Page<S>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.start.start_address.as_u64() > self.end.start_address.as_u64() {
                return None;
            }
            let page = self.start;
            self.start = Page::containing_address(self.start.start_address + S::SIZE);
            Some(page)
        }
    }

    impl<S: PageSize> From<Segment> for PageRangeInclusive<S> {
        fn from(segment: Segment) -> Self {
            Self {
                start: Page::containing_address(segment.start),
                end: Page::containing_address(segment.start + segment.len - 1),
            }
        }
    }

    impl<S: PageSize> From<&Segment> for PageRangeInclusive<S> {
        fn from(segment: &Segment) -> Self {
            Self {
                start: Page::containing_address(segment.start),
                end: Page::containing_address(segment.start + segment.len - 1),
            }
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub use non_x86::*;

#[cfg(target_arch = "x86_64")]
pub use x86_64_impl::*;

#[cfg(target_arch = "x86_64")]
mod x86_64_impl {
    pub use x86_64::structures::paging::frame::PhysFrameRangeInclusive;
    pub use x86_64::structures::paging::frame::PhysFrameRangeInclusive as PhysFrameRange;
    pub use x86_64::structures::paging::{
        PageSize, PageTableFlags, PhysFrame, Size1GiB, Size2MiB, Size4KiB,
    };
    pub use x86_64::{PhysAddr, VirtAddr};
}

// Extension traits to provide common methods if they are missing
pub trait PhysAddrExt {
    fn align_up(self, align: u64) -> Self;
    fn align_down(self, align: u64) -> Self;
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
        self.as_u64() % align == 0
    }
}

pub trait VirtAddrExt {
    fn align_up(self, align: u64) -> Self;
    fn align_down(self, align: u64) -> Self;
    fn is_aligned(self, align: u64) -> bool;
    fn as_mut_ptr<T>(self) -> *mut T;
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
    fn is_aligned(self, align: u64) -> bool {
        self.as_u64() % align == 0
    }

    #[inline]
    fn as_mut_ptr<T>(self) -> *mut T {
        self.as_u64() as *mut T
    }

    #[inline]
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
