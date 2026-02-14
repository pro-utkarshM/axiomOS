//\! PL011 UART Driver for QEMU virt platform
//\!
//\! The virt platform UART is a standard PL011.

use core::fmt::{self, Write};

use super::mmio::MmioReg;

/// PL011 UART base address on QEMU virt
pub const UART_BASE: usize = 0x0900_0000;

/// PL011 UART Register offsets
mod reg {
    /// Data Register - read/write data
    pub const DR: usize = 0x00;
    /// Flag Register - status flags
    pub const FR: usize = 0x18;
    /// Line Control Register
    pub const LCRH: usize = 0x2C;
    /// Control Register
    pub const CR: usize = 0x30;
    /// Interrupt Clear Register
    pub const ICR: usize = 0x44;
}

/// Flag Register bits
mod fr {
    /// Transmit FIFO full
    pub const TXFF: u32 = 1 << 5;
    /// Receive FIFO empty
    #[allow(dead_code)]
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
}

/// Control Register bits
mod cr {
    /// UART enable
    pub const UARTEN: u32 = 1 << 0;
    /// Transmit enable
    pub const TXE: u32 = 1 << 8;
    /// Receive enable
    pub const RXE: u32 = 1 << 9;
}

/// PL011 UART Driver
pub struct PL011Uart {
    base: usize,
}

impl PL011Uart {
    /// Create a new UART instance
    ///
    /// # Safety
    ///
    /// Must be called only once per UART peripheral.
    pub const unsafe fn new(base: usize) -> Self {
        Self { base }
    }

    /// Initialize the UART
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

impl Write for PL011Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.putc(b'\r');
            }
            self.putc(byte);
        }
        Ok(())
    }
}
