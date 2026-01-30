//! Raspberry Pi 5 platform support
//!
//! The Pi 5 uses the BCM2712 SoC with the RP1 southbridge chip connected via PCIe.
//! When using firmware shortcuts (pciex4_reset=0, enable_rp1_uart=1),
//! RP1 peripherals are pre-mapped at fixed addresses in the CPU's physical
//! address space.
//!
//! Key addresses:
//! - RP1 peripheral base: 0x1F00_0000_0000
//! - UART0: 0x1F00_0030_0000
//! - GPIO: 0x1F00_00D0_0000

pub mod gpio;
pub mod memory_map;
pub mod mmio;
pub mod pwm;
pub mod uart;

use conquer_once::spin::Lazy;
use pwm::Rp1Pwm;
use spin::Mutex;
use uart::Rp1Uart;

/// Global UART instance for debug output
pub static UART: Lazy<Mutex<Rp1Uart>> = Lazy::new(|| {
    let mut uart = unsafe { Rp1Uart::new() };
    uart.init();
    Mutex::new(uart)
});

/// Global PWM0 instance
pub static PWM0: Lazy<Mutex<Rp1Pwm>> = Lazy::new(|| {
    let pwm = unsafe { Rp1Pwm::pwm0() };
    pwm.init();
    Mutex::new(pwm)
});

/// Global PWM1 instance
pub static PWM1: Lazy<Mutex<Rp1Pwm>> = Lazy::new(|| {
    let pwm = unsafe { Rp1Pwm::pwm1() };
    pwm.init();
    Mutex::new(pwm)
});

/// Initialize Raspberry Pi 5 platform
///
/// This should be called early in boot to set up essential peripherals
/// like UART for debug output.
pub fn init() {
    // Force lazy initialization of UART
    let _ = &*UART;

    // Print boot banner
    use core::fmt::Write;
    let _ = writeln!(UART.lock(), "\n=== axiom-ebpf on Raspberry Pi 5 ===");
    let _ = writeln!(UART.lock(), "Platform initialized");
}
