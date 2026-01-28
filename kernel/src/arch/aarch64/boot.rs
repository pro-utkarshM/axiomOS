/// Boot information passed from bootloader
pub struct BootInfo {
    pub dtb_addr: usize,
}

static mut BOOT_INFO: BootInfo = BootInfo { dtb_addr: 0 };

/// Initialize boot information
pub unsafe fn init_boot_info(dtb_addr: usize) {
    BOOT_INFO.dtb_addr = dtb_addr;
}

/// Get boot information
pub fn boot_info() -> &'static BootInfo {
    unsafe { &*(&raw const BOOT_INFO) }
}

/// Early boot initialization (called from assembly)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(dtb_addr: usize) -> ! {
    // Initialize boot info
    unsafe {
        init_boot_info(dtb_addr);
    }

    // BSS is already cleared by assembly, but we define the symbols
    // for reference
    unsafe extern "C" {
        static __bss_start: u8;
        static __bss_end: u8;
    }

    // Initialize platform-specific hardware (UART, etc.)
    #[cfg(feature = "rpi5")]
    super::platform::rpi5::init();

    // Parse device tree to get memory information
    if let Err(e) = unsafe { super::dtb::parse(dtb_addr) } {
        // Log error but continue - we can fall back to hardcoded values
        log::warn!("Failed to parse DTB: {}", e);
    }

    // Jump to kernel main
    unsafe extern "Rust" {
        fn kernel_main() -> !;
    }

    unsafe {
        kernel_main()
    }
}
