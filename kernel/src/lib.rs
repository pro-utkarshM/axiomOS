#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(negative_impls, vec_push_within_capacity)]
extern crate alloc;

use ::log::info;
use conquer_once::spin::OnceCell;
use spin::Mutex; // Added

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
#[cfg(target_arch = "x86_64")]
pub mod mcore;
pub mod mem;
mod serial; // Added

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
pub static BPF_MANAGER: OnceCell<Mutex<bpf::BpfManager>> = OnceCell::uninit(); // Added

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

    #[cfg(target_arch = "x86_64")]
    {
        mem::init();
        acpi::init();
        apic::init();
        hpet::init();
    }

    // Initialize BPF
    info!("Initializing BPF subsystem");
    BPF_MANAGER.init_once(|| {
        let mut manager = bpf::BpfManager::new();

        // Milestone 1: Verify BPF execution with hardcoded program
        // Program: bpf_trace_printk("Hello from BPF!", ...)
        use kernel_bpf::bytecode::insn::{BpfInsn, WideInsn};
        use kernel_bpf::execution::BpfContext;

        static HELLO: &[u8] = b"Hello from BPF!\0";
        let ptr = HELLO.as_ptr() as u64;

        // r1 = ptr (wide load)
        let wide = WideInsn::ld_dw_imm(1, ptr);

        let insns = alloc::vec![
            wide.insn,
            wide.next,
            BpfInsn::mov64_imm(2, HELLO.len() as i32), // r2 = len
            BpfInsn::call(2),                          // call bpf_trace_printk
            BpfInsn::exit()
        ];

        if let Ok(id) = manager.load_raw_program(insns) {
            info!("Test BPF program loaded (id={})", id);

            // Execute immediately to verify
            let ctx = BpfContext::empty();
            match manager.execute(id, &ctx) {
                Ok(res) => info!("Test BPF program executed successfully (res={})", res),
                Err(e) => info!("Test BPF program execution failed: {}", e),
            }
        } else {
            info!("Failed to load test BPF program");
        }

        Mutex::new(manager)
    });

    #[cfg(all(target_arch = "aarch64", feature = "aarch64_arch"))]
    {
        use arch::traits::Architecture;
        // Early init (exception vectors)
        arch::aarch64::Aarch64::early_init();
        // Full init (memory, interrupts, syscalls)
        arch::aarch64::Aarch64::init();
    }

    backtrace::init();

    file::init();

    #[cfg(target_arch = "x86_64")]
    {
        mcore::init();
        pci::init();
    }

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
