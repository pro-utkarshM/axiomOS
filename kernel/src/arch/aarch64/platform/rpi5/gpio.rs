//! RP1 GPIO Driver for Raspberry Pi 5
//!
//! The RP1 GPIO controller manages the 28 user-accessible GPIO pins
//! on the Raspberry Pi 5's 40-pin header.
//!
//! Each GPIO pin has:
//! - Function select (input, output, or alternate functions)
//! - Output level control
//! - Input level reading
//! - Pull-up/pull-down configuration
//! - Event detection (edges, levels)

use super::memory_map::RP1_GPIO_BASE;
use super::mmio::MmioReg;

/// GPIO function select values
///
/// Each GPIO pin can be configured to one of several functions.
/// The available alternate functions depend on the specific pin.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioFunction {
    /// Input mode
    Input = 0,
    /// Output mode
    Output = 1,
    /// Alternate function 0 (varies by pin)
    Alt0 = 4,
    /// Alternate function 1
    Alt1 = 5,
    /// Alternate function 2
    Alt2 = 6,
    /// Alternate function 3
    Alt3 = 7,
    /// Alternate function 4
    Alt4 = 3,
    /// Alternate function 5
    Alt5 = 2,
}

/// GPIO pull-up/pull-down configuration
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPull {
    /// No pull-up or pull-down
    None = 0,
    /// Pull-down resistor enabled
    Down = 1,
    /// Pull-up resistor enabled
    Up = 2,
}

/// RP1 GPIO register offsets per pin
///
/// Each GPIO pin has a set of registers at a fixed stride.
mod reg {
    /// GPIO status register (read-only)
    pub const STATUS: usize = 0x00;
    /// GPIO control register
    pub const CTRL: usize = 0x04;
}

/// Register stride per GPIO pin (8 bytes per pin)
const GPIO_REG_STRIDE: usize = 0x08;

/// Control register bit fields
mod ctrl {
    /// Function select mask (bits 4:0)
    pub const FUNCSEL_MASK: u32 = 0x1F;
    /// Output override (bit 13)
    pub const OUTOVER_SHIFT: u32 = 13;
    /// Output enable override (bit 15)
    pub const OEOVER_SHIFT: u32 = 15;
}

/// Status register bit fields
mod status {
    /// Input level (bit 17)
    pub const LEVEL_BIT: u32 = 17;
    /// Event detected (bit 19) - PROVISIONAL
    pub const EVENT_PENDING_BIT: u32 = 19;
}

mod irq_ctrl {
    /// Enable Rising Edge Detect
    pub const ENABLE_RIO: u32 = 1 << 20; // PROVISIONAL
    /// Enable Falling Edge Detect
    pub const ENABLE_FIO: u32 = 1 << 21; // PROVISIONAL
}

/// RP1 GPIO Driver
pub struct Rp1Gpio {
    base: usize,
}

impl Rp1Gpio {
    /// Number of GPIO pins available
    pub const NUM_PINS: u8 = 28;

    /// Create a new GPIO instance
    ///
    /// # Safety
    ///
    /// Must be called only once. The GPIO hardware must be accessible
    /// at the configured address.
    pub const unsafe fn new() -> Self {
        Self {
            base: RP1_GPIO_BASE,
        }
    }

    /// Set the function of a GPIO pin
    ///
    /// # Panics
    ///
    /// Panics if pin number is >= 28
    pub fn set_function(&self, pin: u8, func: GpioFunction) {
        assert!(pin < Self::NUM_PINS, "Invalid GPIO pin: {}", pin);

        let ctrl = self.reg_ctrl(pin);
        ctrl.modify(|v| (v & !ctrl::FUNCSEL_MASK) | (func as u32));
    }

    /// Get the current function of a GPIO pin
    pub fn get_function(&self, pin: u8) -> u32 {
        assert!(pin < Self::NUM_PINS, "Invalid GPIO pin: {}", pin);

        self.reg_ctrl(pin).read() & ctrl::FUNCSEL_MASK
    }

    /// Set a GPIO pin high (output mode)
    ///
    /// The pin must be configured as an output first.
    pub fn set_high(&self, pin: u8) {
        assert!(pin < Self::NUM_PINS, "Invalid GPIO pin: {}", pin);

        let ctrl = self.reg_ctrl(pin);
        // Set output override to drive high (value 3 = high)
        ctrl.modify(|v| v | (3 << ctrl::OUTOVER_SHIFT));
    }

    /// Set a GPIO pin low (output mode)
    ///
    /// The pin must be configured as an output first.
    pub fn set_low(&self, pin: u8) {
        assert!(pin < Self::NUM_PINS, "Invalid GPIO pin: {}", pin);

        let ctrl = self.reg_ctrl(pin);
        // Set output override to drive low (value 2 = low)
        ctrl.modify(|v| (v & !(3 << ctrl::OUTOVER_SHIFT)) | (2 << ctrl::OUTOVER_SHIFT));
    }

    /// Toggle a GPIO pin
    pub fn toggle(&self, pin: u8) {
        if self.read(pin) {
            self.set_low(pin);
        } else {
            self.set_high(pin);
        }
    }

    /// Read the current level of a GPIO pin
    ///
    /// Returns `true` if the pin is high, `false` if low.
    pub fn read(&self, pin: u8) -> bool {
        assert!(pin < Self::NUM_PINS, "Invalid GPIO pin: {}", pin);

        let status = self.reg_status(pin);
        (status.read() & (1 << status::LEVEL_BIT)) != 0
    }

    /// Configure a GPIO pin as output and set initial level
    pub fn configure_output(&self, pin: u8, initial_high: bool) {
        self.set_function(pin, GpioFunction::Output);
        if initial_high {
            self.set_high(pin);
        } else {
            self.set_low(pin);
        }
    }

    /// Configure a GPIO pin as input
    pub fn configure_input(&self, pin: u8) {
        self.set_function(pin, GpioFunction::Input);
    }

    /// Configure GPIO pins 14 and 15 for UART
    ///
    /// This sets them to Alt0 function (UART TXD/RXD).
    /// Note: With `enable_rp1_uart=1`, firmware already does this.
    pub fn setup_uart(&self) {
        self.set_function(14, GpioFunction::Alt0); // TXD0
        self.set_function(15, GpioFunction::Alt0); // RXD0
    }

    // Register accessors
    fn reg_status(&self, pin: u8) -> MmioReg<u32> {
        let offset = (pin as usize) * GPIO_REG_STRIDE + reg::STATUS;
        unsafe { MmioReg::new(self.base + offset) }
    }

    fn reg_ctrl(&self, pin: u8) -> MmioReg<u32> {
        let offset = (pin as usize) * GPIO_REG_STRIDE + reg::CTRL;
        unsafe { MmioReg::new(self.base + offset) }
    }

    /// Enable interrupt for a specific pin
    pub fn enable_interrupt(&self, pin: u8, rising: bool, falling: bool) {
        let ctrl = self.reg_ctrl(pin);
        let mut mask = 0;
        if rising { mask |= irq_ctrl::ENABLE_RIO; }
        if falling { mask |= irq_ctrl::ENABLE_FIO; }
        ctrl.modify(|v| v | mask);
    }

    /// Clear interrupt for a specific pin
    pub fn clear_interrupt(&self, pin: u8) {
        let status = self.reg_status(pin);
        // Write 1 to clear W1C (Write 1 to Clear) bits usually, 
        // or check if there is a separate clear register. 
        // For RP1, writing to STATUS might clear event.
        status.modify(|v| v | (1 << status::EVENT_PENDING_BIT));
    }
}

/// Handle GPIO interrupt
pub fn handle_interrupt() {
    let gpio = unsafe { Rp1Gpio::new() };
    
    // Scan all pins for events
    // TODO: Optimize by checking global IRQ status register if available
    for pin in 0..Rp1Gpio::NUM_PINS {
        let status = gpio.reg_status(pin).read();
        
        if (status & (1 << status::EVENT_PENDING_BIT)) != 0 {
            // Found active interrupt
            let is_rising = (status & (1 << status::LEVEL_BIT)) != 0; // Simplified assumption
            
            // 1. Clear interrupt
            gpio.clear_interrupt(pin);
            
            // 2. Prepare BPF Context
            let event = kernel_bpf::attach::GpioEvent {
                timestamp: 0, // TODO: Get system time
                chip_id: 0,   // gpiochip0
                line: pin as u32,
                edge: if is_rising { 1 } else { 2 },
                value: if is_rising { 1 } else { 0 },
            };
            
            // Safety: Transmuting struct to slice for read-only access
            let slice = unsafe {
                core::slice::from_raw_parts(
                    &event as *const _ as *const u8,
                    core::mem::size_of::<kernel_bpf::attach::GpioEvent>(),
                )
            };
            
            let ctx = kernel_bpf::execution::BpfContext::from_slice(slice);
            
            // 3. Invoke BPF hooks
            if let Some(manager) = crate::BPF_MANAGER.get() {
                // Ignore result
                let _ = manager.lock().execute_hooks(crate::bpf::ATTACH_TYPE_GPIO, &ctx);
            }
        }
    }
}
