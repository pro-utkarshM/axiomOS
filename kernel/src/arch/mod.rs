pub mod traits;

#[cfg(target_arch = "x86_64")]
pub mod gdt;

#[cfg(target_arch = "x86_64")]
pub mod idt;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

// Re-export the current architecture
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;

#[cfg(target_arch = "riscv64")]
pub use self::riscv64::*;

#[cfg(target_arch = "aarch64")]
pub use self::aarch64::*;