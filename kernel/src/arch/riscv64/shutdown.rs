/// Shutdown the system via SBI
pub fn shutdown() -> ! {
    unsafe {
        sbi_shutdown();
    }

    // If SBI shutdown fails, loop forever
    loop {
        riscv::asm::wfi();
    }
}

/// Reboot the system via SBI
pub fn reboot() -> ! {
    unsafe {
        sbi_reboot();
    }

    // If SBI reboot fails, loop forever
    loop {
        riscv::asm::wfi();
    }
}

/// SBI shutdown call
#[inline(always)]
unsafe fn sbi_shutdown() {
    // SBI SRST extension: shutdown
    sbi_call(0x53525354, 0, 0, 0, 0);
}

/// SBI reboot call
#[inline(always)]
unsafe fn sbi_reboot() {
    // SBI SRST extension: cold reboot
    sbi_call(0x53525354, 0, 1, 0, 0);
}

/// Generic SBI call
#[inline(always)]
unsafe fn sbi_call(
    extension: usize,
    function: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
) -> usize {
    let error: usize;
    core::arch::asm!(
        "ecall",
        in("a0") arg0,
        in("a1") arg1,
        in("a2") arg2,
        in("a6") function,
        in("a7") extension,
        lateout("a0") error,
    );
    error
}
