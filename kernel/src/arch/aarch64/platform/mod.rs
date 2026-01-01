//! Platform-specific code for AArch64 boards
//!
//! Each platform (Raspberry Pi 5, QEMU virt, etc.) has its own module
//! with board-specific drivers and initialization.

#[cfg(feature = "rpi5")]
pub mod rpi5;

#[cfg(feature = "rpi5")]
pub use rpi5::*;
