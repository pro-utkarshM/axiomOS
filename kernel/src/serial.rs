// x86_64 serial implementation
#[cfg(target_arch = "x86_64")]
mod x86_64_impl {
    use conquer_once::spin::Lazy;
    use spin::Mutex;
    use uart_16550::SerialPort;

    static SERIAL1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    });

    pub fn internal_print(args: core::fmt::Arguments) {
        use core::fmt::Write;

        use x86_64::instructions::interrupts;

        // disable interrupts while holding a lock on the WRITER
        // so that no deadlock can occur when we want to print
        // something in an interrupt handler
        interrupts::without_interrupts(|| {
            SERIAL1
                .lock()
                .write_fmt(args)
                .expect("Printing to serial failed");
        });
    }
}

// aarch64 serial implementation
#[cfg(all(target_arch = "aarch64", feature = "aarch64_arch"))]
mod aarch64_impl {
    pub fn internal_print(args: core::fmt::Arguments) {
        use core::fmt::Write;

        #[cfg(feature = "rpi5")]
        {
            use crate::arch::aarch64::Aarch64;
            use crate::arch::aarch64::platform::rpi5::UART;
            use crate::arch::traits::Architecture;

            // Disable interrupts while printing to avoid deadlock
            let were_enabled = Aarch64::are_interrupts_enabled();
            if were_enabled {
                Aarch64::disable_interrupts();
            }

            let _ = UART.lock().write_fmt(args);

            if were_enabled {
                Aarch64::enable_interrupts();
            }
        }
    }
}

#[cfg(all(target_arch = "aarch64", feature = "aarch64_arch"))]
#[doc(hidden)]
pub use aarch64_impl::internal_print;

// Stub for aarch64 without aarch64_arch feature
#[cfg(all(target_arch = "aarch64", not(feature = "aarch64_arch")))]
#[doc(hidden)]
pub fn internal_print(_args: core::fmt::Arguments) {
    // No-op when aarch64_arch feature is not enabled
}
#[cfg(target_arch = "x86_64")]
#[doc(hidden)]
pub use x86_64_impl::internal_print;

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::serial::internal_print(format_args!($($arg)*)));
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
