#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(negative_impls, vec_push_within_capacity)]
extern crate alloc;

use ::log::info;
use conquer_once::spin::OnceCell;
use spin::Mutex;

#[cfg(target_arch = "x86_64")]
use crate::driver::pci;
#[cfg(target_arch = "x86_64")]
use crate::limine::BOOT_TIME;

#[cfg(target_arch = "x86_64")]
mod acpi;
#[cfg(target_arch = "x86_64")]
mod apic;
pub mod arch;
pub mod backtrace;
pub mod bpf;
pub mod driver;
pub mod file;
#[cfg(target_arch = "x86_64")]
pub mod hpet;
#[cfg(target_arch = "x86_64")]
pub mod limine;
mod log;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub mod mcore;
pub mod mem;
mod serial;

// Provide a dummy allocator for non-x86_64 and non-aarch64 targets
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
struct DummyAllocator;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
unsafe impl core::alloc::GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[cfg(target_arch = "x86_64")]
pub mod sse;
pub mod syscall;
pub mod time;

static BOOT_TIME_SECONDS: OnceCell<u64> = OnceCell::uninit();
pub static BPF_MANAGER: OnceCell<Mutex<bpf::BpfManager>> = OnceCell::uninit();

fn init_boot_time() {
    #[cfg(target_arch = "x86_64")]
    BOOT_TIME_SECONDS.init_once(|| BOOT_TIME.get_response().unwrap().timestamp().as_secs());
    #[cfg(not(target_arch = "x86_64"))]
    BOOT_TIME_SECONDS.init_once(|| 0);
}

pub fn init() {
    init_boot_time();

    #[cfg(target_arch = "x86_64")]
    let init_start = {
        use crate::hpet::hpet;
        hpet().read().main_counter_value()
    };

    log::init();
    info!("Logging initialized");

    #[cfg(target_arch = "x86_64")]
    {
        mem::init();
        acpi::init();
        apic::init();
        hpet::init();
    }

    #[cfg(target_arch = "aarch64")]
    {
        use arch::traits::Architecture;
        info!("Initializing architecture...");
        arch::aarch64::Aarch64::early_init();
        arch::aarch64::Aarch64::init();
        info!("Architecture initialized");
    }

    info!("Initializing BPF subsystem...");
    BPF_MANAGER.init_once(|| {
        let manager = bpf::BpfManager::new();
        Mutex::new(manager)
    });
    info!("BPF subsystem initialized");

    info!("Initializing backtrace...");
    backtrace::init();
    info!("Backtrace initialized");

    info!("Initializing VFS...");
    file::init();
    info!("VFS initialized");

    info!("Initializing IIO...");
    driver::iio::init();
    info!("IIO initialized");

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        info!("Initializing multicore/scheduler...");
        mcore::init();
        info!("Multicore/scheduler initialized");
    }

    #[cfg(target_arch = "x86_64")]
    {
        pci::init();
    }

    #[cfg(all(target_arch = "aarch64", feature = "virt"))]
    {
        info!("Initializing VirtIO MMIO...");
        driver::virtio::mmio::init();
        info!("VirtIO MMIO initialized");
    }

    info!("Initializing simulated devices...");
    driver::iio::init_simulated_device();
    info!("Simulated devices initialized");

    info!("kernel initialized");

    // Print benchmark metrics
    print_benchmark_metrics();

    #[cfg(target_arch = "x86_64")]
    {
        use crate::hpet::hpet;
        let init_end = hpet().read().main_counter_value();
        let init_time_ns = init_end - init_start;
        let init_time_ms = init_time_ns / 1_000_000;
        info!("Kernel init completed in {} ms", init_time_ms);
    }
}

fn print_benchmark_metrics() {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        use crate::mem::heap::Heap;

        info!("");
        info!("========================================");
        info!("  AXIOM KERNEL METRICS");
        info!("========================================");

        // Memory footprint
        let heap_size = Heap::size();
        let heap_used = Heap::used();
        let heap_free = Heap::free();

        info!("Heap total:          {} KB ({} MB)", heap_size / 1024, heap_size / 1024 / 1024);
        info!("Heap used:           {} KB", heap_used / 1024);
        info!("Heap free:           {} KB", heap_free / 1024);

        // Estimate total kernel memory (heap + kernel code/data)
        // For now, we'll report heap usage as the dynamic component
        info!("Kernel heap usage:   {} KB", heap_used / 1024);

        info!("========================================");
        info!("");
    }
}

#[cfg(target_pointer_width = "64")]
pub trait U64Ext {
    fn into_usize(self) -> usize;
}

#[cfg(target_pointer_width = "64")]
impl U64Ext for u64 {
    #[allow(clippy::cast_possible_truncation)]
    fn into_usize(self) -> usize {
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
