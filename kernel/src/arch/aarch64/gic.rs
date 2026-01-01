//! ARM Generic Interrupt Controller (GICv2) Driver
//!
//! The GIC is the standard interrupt controller for ARM Cortex-A processors.
//! It consists of two main components:
//! - Distributor (GICD): Manages interrupt sources and routing
//! - CPU Interface (GICC): Per-CPU interrupt handling
//!
//! The Raspberry Pi 5 uses a GIC (likely GICv2) for interrupt management.

#[cfg(feature = "rpi5")]
use super::platform::rpi5::memory_map::{GICC_BASE, GICD_BASE};

/// GIC Distributor register offsets
mod gicd {
    /// Distributor Control Register
    pub const CTLR: usize = 0x000;
    /// Interrupt Controller Type Register
    pub const TYPER: usize = 0x004;
    /// Distributor Implementer Identification Register
    pub const IIDR: usize = 0x008;
    /// Interrupt Set-Enable Registers (32 bits each, 1 bit per IRQ)
    pub const ISENABLER: usize = 0x100;
    /// Interrupt Clear-Enable Registers
    pub const ICENABLER: usize = 0x180;
    /// Interrupt Set-Pending Registers
    pub const ISPENDR: usize = 0x200;
    /// Interrupt Clear-Pending Registers
    pub const ICPENDR: usize = 0x280;
    /// Interrupt Priority Registers (8 bits per IRQ)
    pub const IPRIORITYR: usize = 0x400;
    /// Interrupt Processor Targets Registers (8 bits per IRQ)
    pub const ITARGETSR: usize = 0x800;
    /// Interrupt Configuration Registers (2 bits per IRQ)
    pub const ICFGR: usize = 0xC00;
}

/// GIC CPU Interface register offsets
mod gicc {
    /// CPU Interface Control Register
    pub const CTLR: usize = 0x000;
    /// Interrupt Priority Mask Register
    pub const PMR: usize = 0x004;
    /// Binary Point Register
    pub const BPR: usize = 0x008;
    /// Interrupt Acknowledge Register
    pub const IAR: usize = 0x00C;
    /// End of Interrupt Register
    pub const EOIR: usize = 0x010;
    /// Running Priority Register
    pub const RPR: usize = 0x014;
    /// Highest Priority Pending Interrupt Register
    pub const HPPIR: usize = 0x018;
}

/// Special IRQ numbers
pub mod irq {
    /// Physical timer IRQ (PPI, ID 30)
    pub const TIMER_PHYS: u32 = 30;
    /// Virtual timer IRQ (PPI, ID 27)
    pub const TIMER_VIRT: u32 = 27;
    /// Spurious interrupt (no pending interrupt)
    pub const SPURIOUS: u32 = 1023;
}

/// GICD base address (set at runtime for flexibility)
#[cfg(feature = "rpi5")]
static GICD: usize = GICD_BASE;

/// GICC base address
#[cfg(feature = "rpi5")]
static GICC: usize = GICC_BASE;

/// Initialize the GIC
///
/// This configures both the Distributor and CPU Interface.
#[cfg(feature = "rpi5")]
pub fn init() {
    unsafe {
        // Disable distributor while configuring
        write_gicd(gicd::CTLR, 0);

        // Read how many IRQ lines are supported
        let typer = read_gicd(gicd::TYPER);
        let num_irqs = ((typer & 0x1F) + 1) * 32;
        log::debug!("GIC supports {} IRQs", num_irqs);

        // Disable all interrupts
        let num_regs = (num_irqs + 31) / 32;
        for i in 0..num_regs {
            write_gicd(gicd::ICENABLER + i as usize * 4, 0xFFFF_FFFF);
        }

        // Clear all pending interrupts
        for i in 0..num_regs {
            write_gicd(gicd::ICPENDR + i as usize * 4, 0xFFFF_FFFF);
        }

        // Set all interrupts to lowest priority (0xFF)
        let num_priority_regs = (num_irqs + 3) / 4;
        for i in 0..num_priority_regs {
            write_gicd(gicd::IPRIORITYR + i as usize * 4, 0xFFFF_FFFF);
        }

        // Route all SPIs to CPU 0
        let num_target_regs = (num_irqs + 3) / 4;
        for i in 8..num_target_regs {
            // Skip first 8 (SGIs/PPIs are per-CPU)
            write_gicd(gicd::ITARGETSR + i as usize * 4, 0x0101_0101);
        }

        // Configure all interrupts as level-triggered
        let num_cfg_regs = (num_irqs + 15) / 16;
        for i in 0..num_cfg_regs {
            write_gicd(gicd::ICFGR + i as usize * 4, 0);
        }

        // Enable distributor
        write_gicd(gicd::CTLR, 1);

        // Configure CPU interface
        // Set priority mask to accept all priorities
        write_gicc(gicc::PMR, 0xFF);

        // Enable CPU interface
        write_gicc(gicc::CTLR, 1);
    }

    log::info!("GICv2 initialized");
}

/// Placeholder for non-rpi5 builds
#[cfg(not(feature = "rpi5"))]
pub fn init() {
    log::warn!("GIC not initialized (no platform selected)");
}

/// Enable a specific interrupt
#[cfg(feature = "rpi5")]
pub fn enable_irq(irq: u32) {
    let reg_index = (irq / 32) as usize;
    let bit = 1u32 << (irq % 32);

    unsafe {
        write_gicd(gicd::ISENABLER + reg_index * 4, bit);
    }

    log::debug!("Enabled IRQ {}", irq);
}

#[cfg(not(feature = "rpi5"))]
pub fn enable_irq(_irq: u32) {}

/// Disable a specific interrupt
#[cfg(feature = "rpi5")]
pub fn disable_irq(irq: u32) {
    let reg_index = (irq / 32) as usize;
    let bit = 1u32 << (irq % 32);

    unsafe {
        write_gicd(gicd::ICENABLER + reg_index * 4, bit);
    }
}

#[cfg(not(feature = "rpi5"))]
pub fn disable_irq(_irq: u32) {}

/// Acknowledge an interrupt (read IAR)
///
/// Returns the interrupt ID. A value of 1023 indicates a spurious interrupt.
#[cfg(feature = "rpi5")]
pub fn acknowledge() -> u32 {
    unsafe { read_gicc(gicc::IAR) }
}

#[cfg(not(feature = "rpi5"))]
pub fn acknowledge() -> u32 {
    irq::SPURIOUS
}

/// Signal end of interrupt handling
#[cfg(feature = "rpi5")]
pub fn end_of_interrupt(irq: u32) {
    unsafe {
        write_gicc(gicc::EOIR, irq);
    }
}

#[cfg(not(feature = "rpi5"))]
pub fn end_of_interrupt(_irq: u32) {}

/// Set interrupt priority (0 = highest, 255 = lowest)
#[cfg(feature = "rpi5")]
pub fn set_priority(irq: u32, priority: u8) {
    let reg_index = (irq / 4) as usize;
    let byte_offset = (irq % 4) as usize;

    unsafe {
        let mut val = read_gicd(gicd::IPRIORITYR + reg_index * 4);
        val &= !(0xFF << (byte_offset * 8));
        val |= (priority as u32) << (byte_offset * 8);
        write_gicd(gicd::IPRIORITYR + reg_index * 4, val);
    }
}

#[cfg(not(feature = "rpi5"))]
pub fn set_priority(_irq: u32, _priority: u8) {}

// Low-level register access
#[cfg(feature = "rpi5")]
unsafe fn read_gicd(offset: usize) -> u32 {
    core::ptr::read_volatile((GICD + offset) as *const u32)
}

#[cfg(feature = "rpi5")]
unsafe fn write_gicd(offset: usize, value: u32) {
    core::ptr::write_volatile((GICD + offset) as *mut u32, value);
}

#[cfg(feature = "rpi5")]
unsafe fn read_gicc(offset: usize) -> u32 {
    core::ptr::read_volatile((GICC + offset) as *const u32)
}

#[cfg(feature = "rpi5")]
unsafe fn write_gicc(offset: usize, value: u32) {
    core::ptr::write_volatile((GICC + offset) as *mut u32, value);
}
