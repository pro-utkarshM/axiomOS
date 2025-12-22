# Proposal: Porting Muffin OS to Raspberry Pi 5 with GPIO Support

**Author:** utkarsh  
**Date:** December 22, 2025  
**Version:** 1.0

---

## 1. Introduction

This document outlines a detailed plan for porting the **Muffin OS**, a hobby kernel written in Rust, to the **Raspberry Pi 5**. The primary goal is to achieve a successful bare-metal boot on the Pi 5's AArch64 architecture and implement a basic General-Purpose Input/Output (GPIO) driver to control an LED. This project will serve as a foundational step for more advanced operating system features on the Raspberry Pi platform.

The proposal provides a comprehensive checklist covering hardware and software prerequisites, a detailed analysis of the necessary modifications to Muffin OS, and a step-by-step implementation guide. It is intended for a developer with experience in systems programming and Rust.

---

## 2. Project Goals and Scope

### Primary Objectives

1.  **Bare-Metal Boot:** Successfully boot a modified Muffin OS kernel on the Raspberry Pi 5, executing in the AArch64 exception level (EL1).
2.  **GPIO Driver Implementation:** Develop a basic, memory-mapped GPIO driver for the Raspberry Pi 5's BCM2712 SoC.
3.  **LED Control:** Create a simple kernel-level application that utilizes the GPIO driver to blink an LED connected to one of the GPIO pins.
4.  **Serial Console Output:** Implement a minimal UART driver for serial communication to enable `printk`-style debugging.

### Out of Scope

-   Full POSIX compliance on ARM.
-   Advanced interrupt handling (beyond basic timer/GPIO).
-   Filesystem or block device support.
-   Multi-core support.
-   Userspace program execution.

---

## 3. Hardware and Software Requirements

| Category      | Item                                                                 | Purpose                                                    |
|---------------|----------------------------------------------------------------------|------------------------------------------------------------|
| **Hardware**  | Raspberry Pi 5 (any memory variant)                                  | Target device for the OS port.                             |
|               | MicroSD Card (16GB or larger)                                        | Storage for the bootloader and kernel image.               |
|               | USB-C Power Supply (5V/5A recommended)                               | Powering the Raspberry Pi 5.                             |
|               | LED and a resistor (e.g., 330Ω)                                      | Hardware for the GPIO test.                                |
|               | Breadboard and jumper wires                                          | Connecting the LED to the GPIO pins.                       |
|               | USB-to-TTL Serial Cable (e.g., PL2303, CP2102)                        | Essential for viewing kernel debug output.                 |
| **Software**  | Rust Nightly toolchain                                               | Muffin OS is built on nightly Rust features.               |
|               | `aarch64-none-elf` cross-compilation target                          | To build the kernel for the ARM64 architecture.            |
|               | QEMU (system-aarch64)                                                | Optional, for early-stage emulation and testing.           |
|               | Raspberry Pi firmware files (`bootcode.bin`, `start.elf`)            | Required for the Pi's boot process.                        |

---

## 4. Analysis of Muffin OS ARM Support

The Muffin OS repository already contains foundational work for multi-architecture support, which provides a strong starting point. The current status is as follows:

-   **Architecture Abstraction:** A `src/arch` module exists with a generic `Architecture` trait. An `aarch64` implementation of this trait is present.
-   **AArch64 Module:** The `src/arch/aarch64` directory contains stubs and basic implementations for `boot`, `context`, `exceptions`, `interrupts`, `paging`, `shutdown`, and `syscall`.
-   **Linker Script:** A linker script `linker-aarch64.ld` is available, which sets the kernel's base address to the higher half (`0xffffffff80000000`) and defines standard sections (`.text`, `.rodata`, `.data`, `.bss`).
-   **Build Configuration:** The `kernel/Cargo.toml` includes an `aarch64_arch` feature flag, which enables the compilation of AArch64-specific code and dependencies like the `aarch64-cpu` crate.

However, significant gaps remain for a functional Pi 5 port:

1.  **Platform-Specific Code:** The current AArch64 code is generic. It lacks drivers and initialization code specific to the Raspberry Pi 5 and its BCM2712 SoC.
2.  **Boot Process:** The `_start` function is generic. It needs to be adapted to the Raspberry Pi's boot convention, which involves receiving a device tree blob (DTB) address.
3.  **Peripheral Drivers:** There are no drivers for Pi-specific hardware, most critically the UART for serial output and the GPIO controller.
4.  **Memory Management:** The paging implementation is a stub and needs to be fully developed to correctly map the Pi 5's peripheral and RAM addresses.

---

## 5. Implementation Checklist and Proposal

This checklist provides a detailed, step-by-step plan to port Muffin OS to the Raspberry Pi 5.

### Phase 1: Environment Setup

1.  **Install Rust Nightly:** Ensure the correct Rust toolchain is active.
    ```bash
    rustup toolchain install nightly
    rustup default nightly
    ```
2.  **Add AArch64 Target:** Install the bare-metal AArch64 compilation target.
    ```bash
    rustup target add aarch64-unknown-none
    ```
3.  **Configure Cargo:** Create a `.cargo/config.toml` file in the project root to set the default target for AArch64 builds.
    ```toml
    [build]
    target = "aarch64-unknown-none"
    ```

### Phase 2: Kernel Modifications

1.  **Create a Platform Module:**
    -   Inside `kernel/src/arch/aarch64`, create a `platform/rpi5` module.
    -   This module will contain all Raspberry Pi 5-specific code, such as peripheral base addresses and drivers.

2.  **Implement a UART Driver:**
    -   **Goal:** Get `printk!` or a similar macro working for debug output.
    -   **Action:** Research the BCM2712 datasheet or community resources for the physical base address of one of the UARTs (e.g., PL011) [1].
    -   Create a `uart.rs` file in the new platform module.
    -   Implement functions to initialize the UART and write a single character. This will involve writing to memory-mapped registers.
    -   Hook this driver into a global `WRITER` instance, similar to how it's done for the x86_64 VGA driver.

3.  **Adapt the Boot Process:**
    -   **Goal:** Align the kernel entry point with the Raspberry Pi bootloader.
    -   **Action:** Modify the `_start` function in `src/arch/aarch64/boot.rs`. The Pi bootloader passes the address of the device tree blob (DTB) in register `x0`. The kernel must be prepared to receive and store this address.
    -   The linker script `linker-aarch64.ld` already places the kernel in the higher half, which is good practice. Ensure the entry point `_start` is correctly exposed.

4.  **Implement a GPIO Driver:**
    -   **Goal:** Control a GPIO pin.
    -   **Action:** Create a `gpio.rs` file in the platform module.
    -   Define constants for the GPIO peripheral base address on the BCM2712 [2].
    -   Implement functions to:
        -   Set a GPIO pin's function (e.g., to an output).
        -   Set a GPIO pin's state (high or low).
    -   This will involve calculating the correct register addresses for a given pin and performing volatile writes.

5.  **Develop the Main Kernel Logic:**
    -   **Goal:** Tie everything together to blink an LED.
    -   **Action:** In the `kernel_main` function (when compiled for `aarch64_arch`):
        1.  Initialize the UART driver.
        2.  Print a boot message (e.g., "Muffin OS for RPi5 booted!").
        3.  Initialize the GPIO driver.
        4.  Configure a specific GPIO pin (e.g., GPIO 22) as an output.
        5.  Enter an infinite loop that toggles the GPIO pin high and low, with a delay in between. A simple busy-wait loop can be used for the delay initially.

### Phase 3: Build and Deployment

1.  **Build the Kernel:**
    -   **Goal:** Create a raw binary kernel image.
    -   **Action:** Build the kernel in release mode using the AArch64 feature flag.
        ```bash
        cargo build --release --features "aarch64_arch"
        ```
    -   This will produce an ELF file. Use `cargo-objcopy` (or `llvm-objcopy`) to convert it to a flat binary image.
        ```bash
        cargo objcopy -- -O binary target/aarch64-unknown-none/release/kernel target/aarch64-unknown-none/release/kernel8.img
        ```

2.  **Prepare the SD Card:**
    -   **Goal:** Create a bootable SD card for the Pi 5.
    -   **Action:**
        1.  Format the MicroSD card with a single FAT32 partition.
        2.  Download the latest Raspberry Pi firmware files (`bootcode.bin`, `start.elf`, and `.dat` files) from the official repository [3].
        3.  Copy the firmware files to the root of the SD card.
        4.  Copy your compiled `kernel8.img` to the root of the SD card.
        5.  Create a `config.txt` file on the SD card with the following content:
            ```
            arm_64bit=1
            kernel=kernel8.img
            enable_uart=1
            ```

### Phase 4: Testing

1.  **Connect Hardware:**
    -   Connect the USB-to-TTL serial cable to the Pi's UART pins (GPIO 14/15) and your computer.
    -   Connect the LED (with its resistor) to the chosen GPIO pin and a ground pin.
2.  **Boot the Pi:**
    -   Insert the SD card and power on the Raspberry Pi 5.
3.  **Verify:**
    -   Open a serial terminal (e.g., `minicom`, `screen`, PuTTY) on your computer, connected to the serial cable.
    -   Look for the boot message from your kernel.
    -   Observe the connected LED. It should start blinking.

---

## 6. Risks and Mitigation

| Risk                               | Mitigation                                                                                                                              |
|------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| **Undocumented Hardware**          | BCM2712 documentation is not as public as other SoCs. Rely on official Raspberry Pi documentation, community forums, and reverse-engineering existing open-source code (e.g., Linux kernel, U-Boot). |
| **Incorrect Peripheral Addresses** | Peripheral base addresses are critical. Cross-reference addresses from multiple sources. Start with UART, as it provides immediate feedback for debugging. |
| **MMU and Caching Issues**         | Bare-metal MMU setup is complex. Initially, run with the MMU disabled or use a simple identity mapping. Introduce more complex paging and caching once the basic boot is stable. |
| **Build/Linker Script Errors**     | The linker script is crucial for memory layout. Start with a simple script and add complexity as needed. Use `readelf` and `objdump` to inspect the generated kernel binary. |

---

## 7. Conclusion

Porting Muffin OS to the Raspberry Pi 5 is an ambitious but achievable project that will significantly expand the OS's capabilities and provide a valuable learning experience in bare-metal ARM64 development. The existing multi-architecture foundation in Muffin OS provides a solid starting point.

By following the detailed checklist in this proposal, a developer can systematically implement the necessary drivers and boot logic to bring Muffin OS to life on this new hardware platform. The successful completion of this project will pave the way for future development, including interrupt handling, multi-core support, and a full userspace environment.

---

## 8. References

[1] Raspberry Pi Documentation - Peripherals. (https://www.raspberrypi.com/documentation/computers/raspberry-pi.html#bcm2712-peripherals)

[2] OSDev Wiki - Raspberry Pi Bare Bones. (https://wiki.osdev.org/Raspberry_Pi_Bare_Bones)

[3] Raspberry Pi Firmware Repository. (https://github.com/raspberrypi/firmware)
