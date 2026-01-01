#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(negative_impls, vec_push_within_capacity)]
extern crate alloc;

use ::log::info;
use conquer_once::spin::OnceCell;

#[cfg(target_arch = "x86_64")]
use crate::driver::pci;
#[cfg(target_arch = "x86_64")]
use crate::limine::BOOT_TIME;

#[cfg(target_arch = "x86_64")]
mod acpi;
#[cfg(target_arch = "x86_64")]
mod apic;
mod arch;
pub mod backtrace;
pub mod driver;
pub mod file;
#[cfg(target_arch = "x86_64")]
pub mod hpet;
#[cfg(target_arch = "x86_64")]
pub mod limine;
mod log;
#[cfg(target_arch = "x86_64")]
pub mod mcore;
pub mod mem;
mod serial;

// Provide a dummy allocator for non-x86_64 targets
#[cfg(not(target_arch = "x86_64"))]
#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;

#[cfg(not(target_arch = "x86_64"))]
struct DummyAllocator;

#[cfg(not(target_arch = "x86_64"))]
unsafe impl core::alloc::GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // no-op
    }
}
#[cfg(target_arch = "x86_64")]
pub mod sse;
pub mod syscall;
pub mod time;

static BOOT_TIME_SECONDS: OnceCell<u64> = OnceCell::uninit();

/// # Panics
/// Panics if there was no boot time provided by limine.
fn init_boot_time() {
    #[cfg(target_arch = "x86_64")]
    BOOT_TIME_SECONDS.init_once(|| BOOT_TIME.get_response().unwrap().timestamp().as_secs());
    #[cfg(not(target_arch = "x86_64"))]
    BOOT_TIME_SECONDS.init_once(|| 0); // TODO: Get boot time from device tree
}

pub fn init() {
    init_boot_time();

    log::init();
    mem::init();
    
    #[cfg(target_arch = "x86_64")]
    {
        acpi::init();
        apic::init();
        hpet::init();
    }
    
    backtrace::init();

    #[cfg(target_arch = "x86_64")]
    {
        mcore::init();
        pci::init();
    }

    file::init();

    info!("kernel initialized");
}

#[cfg(target_pointer_width = "64")]
pub trait U64Ext {
    fn into_usize(self) -> usize;
}

#[cfg(target_pointer_width = "64")]
impl U64Ext for u64 {
    #[allow(clippy::cast_possible_truncation)]
    fn into_usize(self) -> usize {
        // Safety: we know that we are on 64-bit, so this is correct
        unsafe { usize::try_from(self).unwrap_unchecked() }
    }
}

#[cfg(target_pointer_width = "64")]
pub trait UsizeExt {
    fn into_u64(self) -> u64;
}

#[cfg(target_pointer_width = "64")]
impl UsizeExt for usize {
    fn into_u64(self) -> u64 {
        self as u64
    }
}