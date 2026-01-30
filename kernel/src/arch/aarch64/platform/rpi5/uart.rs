//! RP1 UART Driver for Raspberry Pi 5
//!
//! The RP1 UART is a PL011-compatible UART peripheral. When using the
//! firmware shortcut `enable_rp1_uart=1`, the UART is pre-configured
//! with 115200 baud, 8N1 settings.
//!
//! Physical connection:
//! - GPIO14 (Pin 8) = TXD0 -> USB-TTL RX
//! - GPIO15 (Pin 10) = RXD0 -> USB-TTL TX
//! - GND (Pin 6) -> USB-TTL GND

use core::fmt::{self, Write};

use super::memory_map::RP1_UART0_BASE;
use super::mmio::MmioReg;

/// PL011 UART Register offsets
mod reg {
    /// Data Register - read/write data
    pub const DR: usize = 0x00;
    /// Receive Status / Error Clear Register
    #[allow(dead_code)]
    pub const RSRECR: usize = 0x04;
    /// Flag Register - status flags
    pub const FR: usize = 0x18;
    /// Integer Baud Rate Divisor
    #[allow(dead_code)]
    pub const IBRD: usize = 0x24;
    /// Fractional Baud Rate Divisor
    #[allow(dead_code)]
    pub const FBRD: usize = 0x28;
    /// Line Control Register
    pub const LCRH: usize = 0x2C;
    /// Control Register
    pub const CR: usize = 0x30;
    /// Interrupt FIFO Level Select Register
    #[allow(dead_code)]
    pub const IFLS: usize = 0x34;
    /// Interrupt Mask Set/Clear Register
    #[allow(dead_code)]
    pub const IMSC: usize = 0x38;
    /// Interrupt Clear Register
    pub const ICR: usize = 0x44;
}

/// Flag Register bits
mod fr {
    /// Transmit FIFO full
    pub const TXFF: u32 = 1 << 5;
    /// Receive FIFO empty
    pub const RXFE: u32 = 1 << 4;
    /// UART busy transmitting
    pub const BUSY: u32 = 1 << 3;
}

/// Line Control Register bits
mod lcrh {
    /// Enable FIFOs
    pub const FEN: u32 = 1 << 4;
    /// Word length: 8 bits
    pub const WLEN_8: u32 = 0b11 << 5;
    /// Word length: 7 bits
    #[allow(dead_code)]
    pub const WLEN_7: u32 = 0b10 << 5;
    /// Word length: 6 bits
    #[allow(dead_code)]
    pub const WLEN_6: u32 = 0b01 << 5;
    /// Word length: 5 bits
    #[allow(dead_code)]
    pub const WLEN_5: u32 = 0b00 << 5;
    /// Enable 2 stop bits
    #[allow(dead_code)]
    pub const STP2: u32 = 1 << 3;
    /// Even parity select
    #[allow(dead_code)]
    pub const EPS: u32 = 1 << 2;
    /// Parity enable
    #[allow(dead_code)]
    pub const PEN: u32 = 1 << 1;
    /// Send break
    #[allow(dead_code)]
    pub const BRK: u32 = 1 << 0;
}

/// Control Register bits
mod cr {
    /// UART enable
    pub const UARTEN: u32 = 1 << 0;
    /// Loopback enable
    #[allow(dead_code)]
    pub const LBE: u32 = 1 << 7;
    /// Transmit enable
    pub const TXE: u32 = 1 << 8;
    /// Receive enable
    pub const RXE: u32 = 1 << 9;
    /// Request to send
    #[allow(dead_code)]
    pub const RTS: u32 = 1 << 11;
}

/// RP1 PL011 UART Driver
pub struct Rp1Uart {
    base: usize,
}

impl Rp1Uart {
    /// Create a new UART instance
    ///
    /// # Safety
    ///
    /// Must be called only once per UART peripheral. The UART hardware
    /// must be accessible at the configured address.
    pub const unsafe fn new() -> Self {
        Self {
            base: RP1_UART0_BASE,
        }
    }

    /// Initialize the UART
    ///
    /// When using `enable_rp1_uart=1` in config.txt, the firmware has
    /// already configured the baud rate. We just need to ensure the
    /// UART is enabled with correct line settings.
    pub fn init(&mut self) {
        let cr = self.reg_cr();
        let lcrh = self.reg_lcrh();
        let icr = self.reg_icr();

        // Disable UART while configuring
        cr.write(0);

        // Wait for any pending transmission to complete
        self.reg_fr().wait_clear(fr::BUSY);

        // Flush FIFOs by disabling them
        lcrh.clear_bits(lcrh::FEN);

        // Clear all pending interrupts
        icr.write(0x7FF);

        // Configure line: 8 data bits, no parity, 1 stop bit, FIFOs enabled
        lcrh.write(lcrh::WLEN_8 | lcrh::FEN);

        // Enable UART, transmitter, and receiver
        cr.write(cr::UARTEN | cr::TXE | cr::RXE);
    }

    /// Send a single byte (blocking)
    pub fn putc(&self, c: u8) {
        // Wait for TX FIFO to have space
        self.reg_fr().wait_clear(fr::TXFF);

        // Write the byte
        self.reg_dr().write(c as u32);
    }

    /// Receive a single byte (blocking)
    pub fn getc(&self) -> u8 {
        // Wait for RX FIFO to have data
        self.reg_fr().wait_clear(fr::RXFE);

        // Read the byte (lower 8 bits of DR)
        (self.reg_dr().read() & 0xFF) as u8
    }

    /// Try to receive a byte (non-blocking)
    ///
    /// Returns `Some(byte)` if data is available, `None` otherwise.
    pub fn try_getc(&self) -> Option<u8> {
        if self.reg_fr().is_set(fr::RXFE) {
            None
        } else {
            Some((self.reg_dr().read() & 0xFF) as u8)
        }
    }

    /// Check if transmit FIFO has space
    pub fn can_write(&self) -> bool {
        !self.reg_fr().is_set(fr::TXFF)
    }

    /// Check if receive FIFO has data
    pub fn can_read(&self) -> bool {
        !self.reg_fr().is_set(fr::RXFE)
    }

    // Register accessors
    fn reg_dr(&self) -> MmioReg<u32> {
        unsafe { MmioReg::new(self.base + reg::DR) }
    }

    fn reg_fr(&self) -> MmioReg<u32> {
        unsafe { MmioReg::new(self.base + reg::FR) }
    }

    fn reg_lcrh(&self) -> MmioReg<u32> {
        unsafe { MmioReg::new(self.base + reg::LCRH) }
    }

    fn reg_cr(&self) -> MmioReg<u32> {
        unsafe { MmioReg::new(self.base + reg::CR) }
    }

    fn reg_icr(&self) -> MmioReg<u32> {
        unsafe { MmioReg::new(self.base + reg::ICR) }
    }
}

impl Write for Rp1Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // Convert \n to \r\n for proper terminal display
            if byte == b'\n' {
                self.putc(b'\r');
            }
            self.putc(byte);
        }
        Ok(())
    }
}
