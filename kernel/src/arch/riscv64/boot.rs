/// Boot information passed from bootloader
pub struct BootInfo {
    pub hart_id: usize,
    pub dtb_addr: usize,
}

static mut BOOT_INFO: BootInfo = BootInfo {
    hart_id: 0,
    dtb_addr: 0,
};

/// Initialize boot information
pub unsafe fn init_boot_info(hart_id: usize, dtb_addr: usize) {
    BOOT_INFO.hart_id = hart_id;
    BOOT_INFO.dtb_addr = dtb_addr;
}

/// Get boot information
pub fn boot_info() -> &'static BootInfo {
    unsafe { &BOOT_INFO }
}

/// Early boot initialization (called from assembly)
/// Assembly entry point is in boot.S which calls this function
#[no_mangle]
pub unsafe extern "C" fn _start_rust(hart_id: usize, dtb_addr: usize) -> ! {
    // Initialize boot info
    init_boot_info(hart_id, dtb_addr);

    // Note: Hart filtering and BSS clearing is done in boot.S assembly

    // Jump to kernel main
    extern "Rust" {
        fn kernel_main() -> !;
    }

    kernel_main()
}
