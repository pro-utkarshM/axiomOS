use log::info;

#[cfg(target_arch = "x86_64")]
use crate::limine::MEMORY_MAP_REQUEST;
#[cfg(target_arch = "x86_64")]
use crate::mem::address_space::AddressSpace;
#[cfg(target_arch = "x86_64")]
use crate::mem::heap::Heap;

#[cfg(target_arch = "x86_64")]
pub mod address_space;
#[cfg(target_arch = "x86_64")]
pub mod heap;
#[cfg(target_arch = "x86_64")]
pub mod memapi;
#[cfg(target_arch = "x86_64")]
pub mod phys;
#[cfg(target_arch = "x86_64")]
pub mod virt;

#[cfg(target_arch = "x86_64")]
#[allow(clippy::missing_panics_doc)]
pub fn init() {
    let response = MEMORY_MAP_REQUEST
        .get_response()
        .expect("should have a memory map response");

    let usable_physical_memory = phys::init_stage1(response.entries());

    address_space::init();

    let address_space = AddressSpace::kernel();

    heap::init(address_space, usable_physical_memory);

    virt::init();

    phys::init_stage2();

    heap::init_stage2();

    info!("memory initialized, {Heap:x?}");
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init() {
    info!("memory initialization (aarch64/riscv64 stub)");
    // TODO: Implement proper memory management for aarch64/riscv64
}
