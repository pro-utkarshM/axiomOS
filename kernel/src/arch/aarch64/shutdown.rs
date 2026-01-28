/// Shutdown the system via PSCI
pub fn shutdown() -> ! {
    unsafe {
        psci_system_off();
    }

    // If PSCI shutdown fails, loop forever
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Reboot the system via PSCI
pub fn reboot() -> ! {
    unsafe {
        psci_system_reset();
    }

    // If PSCI reboot fails, loop forever
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// PSCI system off
#[inline(always)]
unsafe fn psci_system_off() {
    // PSCI 0.2+ function ID for SYSTEM_OFF
    let function_id: u32 = 0x84000008;

    unsafe {
        core::arch::asm!(
            "hvc #0",
            in("w0") function_id,
        );
    }
}

/// PSCI system reset
#[inline(always)]
unsafe fn psci_system_reset() {
    // PSCI 0.2+ function ID for SYSTEM_RESET
    let function_id: u32 = 0x84000009;

    unsafe {
        core::arch::asm!(
            "hvc #0",
            in("w0") function_id,
        );
    }
}
