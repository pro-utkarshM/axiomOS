//! Raspberry Pi 5 Memory Map
//!
//! The Pi 5 has a complex memory layout with the RP1 southbridge
//! providing I/O peripherals through a PCIe connection.
//!
//! With firmware shortcuts enabled (pciex4_reset=0, enable_rp1_uart=1),
//! the RP1 peripherals are pre-mapped to the CPU's physical address space.

/// RP1 peripheral base address (PCIe BAR1 window)
///
/// The RP1's internal address space (starting at 0x4000_0000) is mapped
/// to this CPU physical address by the PCIe controller.
pub const RP1_PERIPHERAL_BASE: usize = 0x1F00_0000_0000;

/// RP1 internal offset for UART0
pub const RP1_UART0_OFFSET: usize = 0x0003_0000;

/// RP1 internal offset for UART1
pub const RP1_UART1_OFFSET: usize = 0x0003_4000;

/// RP1 internal offset for GPIO
pub const RP1_GPIO_OFFSET: usize = 0x000D_0000;

/// RP1 internal offset for I2C0
pub const RP1_I2C0_OFFSET: usize = 0x0007_0000;

/// RP1 internal offset for SPI0
pub const RP1_SPI0_OFFSET: usize = 0x0005_0000;

/// RP1 internal offset for PWM0
pub const RP1_PWM0_OFFSET: usize = 0x0009_8000;

/// RP1 internal offset for PWM1
pub const RP1_PWM1_OFFSET: usize = 0x0009_C000;

/// Calculate CPU physical address for an RP1 peripheral
#[inline]
pub const fn rp1_peripheral_addr(offset: usize) -> usize {
    RP1_PERIPHERAL_BASE + offset
}

/// UART0 base address (PL011-compatible)
pub const RP1_UART0_BASE: usize = rp1_peripheral_addr(RP1_UART0_OFFSET);

/// UART1 base address
pub const RP1_UART1_BASE: usize = rp1_peripheral_addr(RP1_UART1_OFFSET);

/// GPIO base address
pub const RP1_GPIO_BASE: usize = rp1_peripheral_addr(RP1_GPIO_OFFSET);

/// PWM0 base address
pub const RP1_PWM0_BASE: usize = rp1_peripheral_addr(RP1_PWM0_OFFSET);

/// PWM1 base address
pub const RP1_PWM1_BASE: usize = rp1_peripheral_addr(RP1_PWM1_OFFSET);

/// ARM GIC distributor base address (on BCM2712, not RP1)
pub const GICD_BASE: usize = 0xFF84_1000;

/// ARM GIC CPU interface base address (on BCM2712, not RP1)
pub const GICC_BASE: usize = 0xFF84_2000;

/// Physical memory (DRAM) start
pub const DRAM_BASE: usize = 0x0;

/// Kernel load address (where Pi firmware loads kernel8.img)
pub const KERNEL_LOAD_ADDR: usize = 0x8_0000;
