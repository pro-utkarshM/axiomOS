# Raspberry Pi 5 Porting TODO

**Target:** ARM64 (aarch64) bare-metal kernel for Raspberry Pi 5
**Current Status:** Basic boot, UART, GPIO, GIC interrupts working
**Last Updated:** 2026-01-02

---

## ✅ Completed

- [x] ARM64 boot assembly (`kernel/src/arch/aarch64/boot.S`)
- [x] Exception vector table and handlers (`exceptions.rs`)
- [x] Generic Interrupt Controller (GICv2) driver (`gic.rs`)
- [x] RP1 UART0 driver (PL011-compatible) (`platform/rpi5/uart.rs`)
- [x] RP1 GPIO driver (28 pins, alt functions, pull resistors) (`platform/rpi5/gpio.rs`)
- [x] MMIO register abstraction (`platform/rpi5/mmio.rs`)
- [x] Memory map documentation (`platform/rpi5/memory_map.rs`)
- [x] Generic timer interrupts (100 Hz) (`interrupts.rs`)
- [x] Basic paging primitives (`paging.rs`)
- [x] Syscall interface (SVC handler) (`syscall.rs`)
- [x] Task context switching (`context.rs`)
- [x] Build script for aarch64 (`scripts/build-rpi5.sh`)
- [x] Deployment script (`scripts/deploy-rpi5.sh`)
- [x] Firmware config with shortcuts (`config/rpi5/config.txt`)

---

## 🔴 Critical (Blocking Further Development)

### Core Kernel Infrastructure

- [ ] **Page Table Setup** (`paging.rs:3`)
  - Currently relies on firmware identity mapping
  - Need to set up proper kernel page tables in TTBR1_EL1
  - Separate kernel/user address spaces
  - Map kernel at higher-half (e.g., 0xFFFF_8000_0000_0000)
  - Related: `kernel/linker-aarch64.ld` may need updates

- [ ] **Page Fault Handling** (`exceptions.rs:92`)
  - Implement data abort handler for recoverable faults
  - Parse ESR_EL1 to determine fault type
  - Parse FAR_EL1 for faulting address
  - Enable demand paging and copy-on-write
  - Handle permission faults vs. translation faults

- [ ] **Scheduler Integration** (`interrupts.rs:60`)
  - Hook timer interrupt to scheduler
  - Implement preemptive multitasking for aarch64
  - Task queue management
  - Context switch on timer tick
  - CPU time accounting

- [ ] **Memory Allocator for ARM64**
  - Verify physical memory allocator works on aarch64
  - Test heap allocator in bare-metal ARM context
  - Handle DRAM size detection (from DTB or probing)

---

## 🟡 High Priority (Core Functionality)

### Multi-Core Support

- [ ] **Wake Secondary Cores**
  - Currently parked in `boot.S` spin loop
  - Implement mailbox protocol to release cores 1-3
  - Per-core stack allocation
  - Per-core exception handlers

- [ ] **SMP Synchronization**
  - Spinlock primitives using ARM atomics (LDXR/STXR)
  - Per-CPU data structures
  - TLB shootdown for multi-core page table changes
  - Scheduler load balancing across cores

### Device Tree Parsing

- [ ] **DTB Parser**
  - Parse Device Tree Blob passed by firmware (address saved in `boot.S`)
  - Extract memory regions dynamically
  - Discover peripheral addresses
  - Get CPU count and core IDs
  - Replace hardcoded addresses in `memory_map.rs`

### Storage & Filesystem

- [ ] **SD Card Driver**
  - EMMC2 controller on RPi5
  - SD card initialization and detection
  - Block read/write operations
  - May need to use SPI mode as fallback

- [ ] **FAT32 Filesystem**
  - Read boot partition files
  - Persistent storage support
  - Integration with VFS (`kernel/src/file/`)

---

## 🟢 Medium Priority (Enhanced Functionality)

### RP1 Peripheral Drivers

- [ ] **I2C Driver** (RP1 offset `0x0007_0000`)
  - I2C master mode
  - Support standard (100kHz) and fast (400kHz) modes
  - Read/write transactions
  - Use cases: RTC, sensors, EEPROM

- [ ] **SPI Driver** (RP1 offset `0x0005_0000`)
  - SPI master mode
  - Configurable clock, mode, chip select
  - DMA support (optional)
  - Use cases: SD card, displays, sensors

- [ ] **UART1 Driver** (RP1 offset `0x0003_4000`)
  - Second serial port
  - Same PL011 interface as UART0
  - Use cases: GPS, modem, secondary console

- [ ] **PWM Driver**
  - RP1 has dedicated PWM blocks
  - Per-pin PWM for servos, LED dimming
  - Hardware-based timing

- [ ] **Watchdog Timer**
  - Auto-reset on kernel hang
  - Configurable timeout
  - System reliability feature

### GPIO Enhancements

- [ ] **GPIO Interrupts**
  - Edge detection (rising/falling)
  - Level detection (high/low)
  - Integration with GIC
  - Use cases: button presses, sensor signals

- [ ] **GPIO Event System**
  - Async edge detection
  - Debouncing logic
  - Event queue for userspace

### Graphics & Display

- [ ] **Framebuffer Support**
  - VideoCore mailbox interface
  - Request framebuffer from GPU
  - Pixel plotting primitives
  - Simple text console rendering

- [ ] **Boot Splash**
  - Display logo during boot
  - Visual confirmation of boot progress

---

## 🔵 Low Priority (Nice to Have)

### Networking

- [ ] **Ethernet Driver**
  - Gigabit Ethernet on RPi5
  - MAC layer implementation
  - DMA ring buffers for RX/TX
  - Link status detection

- [ ] **Network Stack**
  - ARP, ICMP, UDP (minimum viable)
  - TCP implementation
  - Socket API for userspace
  - DHCP client

### USB

- [ ] **USB Host Controller (XHCI)**
  - Complex but high-value
  - USB 3.0 + USB 2.0 support
  - Root hub enumeration

- [ ] **USB HID Driver**
  - Keyboard and mouse support
  - Boot protocol for early init

- [ ] **USB Mass Storage**
  - Block device interface
  - Bulk-only transport

### Audio

- [ ] **I2S Audio Driver**
  - PCM output
  - Integration with audio codec on RPi5
  - Waveform playback

### Power Management

- [ ] **CPU Frequency Scaling**
  - DVFS support via mailbox interface
  - Power/performance modes
  - Thermal throttling

- [ ] **Suspend/Resume**
  - Low-power sleep states
  - Wake sources (GPIO, timer)

- [ ] **Thermal Monitoring**
  - Read CPU temperature
  - Thermal zone management

### Advanced Features

- [ ] **DMA Controller**
  - RP1 DMA engine driver
  - Offload memory copies
  - Peripheral-to-memory transfers

- [ ] **Real-Time Clock**
  - Persistent timekeeping
  - Wake alarms
  - Integration with I2C RTC chips

- [ ] **PCIe Driver (Native)**
  - Replace firmware shortcuts (`pciex4_reset=0`)
  - Proper PCIe enumeration
  - Support for PCIe add-on cards

---

## 📋 Testing & Validation

- [ ] **Unit Tests for aarch64 Code**
  - Extract testable logic to separate crates
  - Test page table manipulation
  - Test exception handling logic

- [ ] **Hardware Validation Tests**
  - GPIO loopback tests
  - UART echo test
  - Timer accuracy measurement
  - Multi-core synchronization tests

- [ ] **Stress Tests**
  - Multi-core scheduler under load
  - Memory allocator torture test
  - Interrupt storm handling

- [ ] **Compatibility Matrix**
  - Test on different RPi5 RAM configurations (4GB/8GB)
  - Test with different firmware versions
  - Test with/without config.txt shortcuts

---

## 🛠️ Build & Tooling

- [ ] **GDB Debugging Setup**
  - QEMU ARM64 GDB stub
  - Hardware debugging via JTAG/SWD
  - Crash dump analysis

- [ ] **Continuous Integration**
  - Cross-compile tests in GitHub Actions
  - Automated build verification
  - Boot tests in QEMU (when aarch64 QEMU config ready)

- [ ] **Documentation**
  - Architecture guide for aarch64 port
  - Driver development guide
  - Memory map reference
  - Boot process documentation

---

## 🔬 Research & Exploration

- [ ] **QEMU aarch64 Testing**
  - Create QEMU virt machine configuration
  - Enable testing without physical hardware
  - Automate boot tests like x86_64 version

- [ ] **U-Boot Integration**
  - Alternative to direct firmware boot
  - Standard ARM boot protocol
  - Netboot support

- [ ] **Virtualization (EL2)**
  - Stay in EL2 instead of dropping to EL1
  - Hypervisor capabilities
  - VM hosting

---

## 📝 Notes

### Firmware Dependencies
Current implementation relies on Pi firmware shortcuts:
- `pciex4_reset=0` - Keeps RP1 mapped (no PCIe driver needed)
- `enable_rp1_uart=1` - Pre-configures UART0 (115200, 8N1)

Long-term goal: Remove these dependencies with native drivers.

### Memory Layout
- Kernel loaded at `0x80000` by firmware
- Stack at `0x10000` (64KB, grows down)
- RP1 peripherals at `0x1F00_0000_0000`
- GIC at `0xFF84_1000` (GICD) / `0xFF84_2000` (GICC)

### Reference Documentation
- BCM2712 datasheet (not publicly available - reverse engineering)
- ARM Cortex-A76 TRM
- GICv2 Architecture Specification
- PL011 UART Technical Reference Manual
- RP1 peripherals: inferred from Linux kernel drivers

---

## 🎯 Suggested Development Path

**Phase 1: Stabilize Core**
1. Page table setup
2. Page fault handling
3. Scheduler integration
4. Memory allocator validation

**Phase 2: Enable Multiprocessing**
1. Wake secondary cores
2. SMP synchronization primitives
3. Multi-core scheduler

**Phase 3: Expand Peripherals**
1. I2C driver (easiest, high utility)
2. Device tree parsing (removes hardcoding)
3. SD card driver (storage foundation)
4. Framebuffer (visual feedback)

**Phase 4: Advanced Features**
1. USB support (huge unlock)
2. Networking (Ethernet + stack)
3. Remove firmware dependencies (native PCIe)

**Phase 5: Production Hardening**
1. Power management
2. Comprehensive testing
3. Performance optimization
4. Documentation
