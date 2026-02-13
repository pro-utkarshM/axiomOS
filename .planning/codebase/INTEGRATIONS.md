# External Integrations

**Analysis Date:** 2026-02-13

## APIs & External Services

Not applicable — bare-metal kernel with no external API dependencies.

## Bootloader

**Limine v9.x-binary:**
- Purpose: BIOS/UEFI boot for x86_64 kernel
- Integration: Downloaded during build via git clone — `build.rs`
- Config: `limine.conf` (protocol: limine, timeout: 0)
- Files staged: `limine-bios.sys`, `limine-bios-cd.bin`, `limine-uefi-cd.bin`, EFI boot files
- Repository: https://github.com/limine-bootloader/limine.git

**Device Tree (AArch64):**
- Purpose: Hardware discovery on ARM platforms
- Integration: DTB address passed in x0 register at boot — `kernel/src/arch/aarch64/boot.S`
- Parsed via `fdt` crate — `kernel/src/arch/aarch64/dtb.rs`

## Emulation

**QEMU System Emulators:**
- `qemu-system-x86_64` - x86_64 with UEFI/BIOS boot — `src/main.rs`
- `qemu-system-aarch64` - ARM64 virt machine and RPi5 emulation — `scripts/run-virt.sh`
- Parameters: SMP, memory, disk image, debug port (localhost:1234 for GDB)
- Disk: Generated 10M ext2 filesystem — `build.rs`

**OVMF (UEFI Firmware):**
- Prebuilt x86_64 firmware images (Code + Vars) — `ovmf-prebuilt` crate in `build.rs`
- Used for UEFI boot in QEMU

## Hardware Targets

**x86_64 (Primary Dev Target):**
- QEMU emulation via Limine bootloader
- Kernel loaded at 0xffffffff80000000
- ACPI for hardware discovery, APIC for interrupts, HPET for timers

**ARM64 / AArch64:**
- QEMU virt machine (generic ARM SoC) — `scripts/run-virt.sh`
- Raspberry Pi 5 (RP1 SoC) — `scripts/build-rpi5.sh`, `scripts/deploy-rpi5.sh`
- GIC (Generic Interrupt Controller) for interrupts
- Platform-specific UART, GPIO, PWM drivers for RPi5

**RISC-V 64-bit (Experimental):**
- Demo kernel in `kernel/demos/riscv/`
- Separate Cargo workspace — `kernel/demos/riscv/.cargo/config.toml`
- QEMU execution — `scripts/run-riscv.sh`

## Device Drivers

**VirtIO:**
- Block device (disk.img mounting) — `virtio-drivers = "0.12"`
- GPU (framebuffer) — `kernel/src/driver/virtio/gpu.rs`
- MMIO transport for AArch64 — `kernel/src/driver/virtio/mmio.rs`

**PCI:**
- Enumeration and device discovery — `kernel/crates/kernel_pci/`
- Device tree parsing for ARM platforms

**Serial/UART:**
- 16550-compatible UART — `uart_16550 = "0.4"`
- RP1 UART on RPi5, QEMU UART on virt/x86_64

**Platform-Specific (RPi5):**
- RP1 GPIO controller — `kernel/src/arch/aarch64/platform/rpi5/gpio.rs`
- RP1 PWM controller — `kernel/src/arch/aarch64/platform/rpi5/pwm.rs`
- RP1 UART — `kernel/src/arch/aarch64/platform/rpi5/uart.rs`

## Filesystem

**ext2:**
- Build-time: 10M ext2 disk image created via `mke2fs` — `build.rs`
- Runtime: Mounted as root filesystem via VirtIO block device
- VFS implementation — `kernel/crates/kernel_vfs/`, `kernel/src/file/ext2.rs`

## Build Artifacts

**Bootable ISO** (x86_64):
- `target/*/build/*/out/muffin.iso` - Hybrid BIOS+UEFI
- Created via xorriso + Limine installation — `build.rs`

**Disk Image:**
- `target/*/build/*/out/disk.img` - 10M ext2
- Contains: init, demos (gpio_demo, pwm_demo, bpf_loader, fork_test, etc.)

**Kernel Binaries:**
- x86_64: `target/x86_64-unknown-none/release/kernel` (ELF)
- AArch64: `target/aarch64-unknown-none/release/kernel8.img` (raw binary for RPi5)

## CI/CD Pipeline

**GitHub Actions:** `.github/workflows/`
- `build.yml`: Lint (rustfmt/clippy), test (debug/release), Miri, full build
- `bpf-profiles.yml`: BPF profile testing, mutual exclusion verification
- Runs on: ubuntu-latest, scheduled (5 AM, 5 PM UTC)
- Artifacts: `muffin-boot-images` (ISO)

**Dependabot:** `.github/dependabot.yml` for dependency updates

## Development & Deployment Scripts

- `scripts/build-rpi5.sh` - Compile kernel with rpi5 feature, create kernel8.img
- `scripts/deploy-rpi5.sh` - Copy kernel8.img to SD card boot partition
- `scripts/run-virt.sh` - Run ARM64 virt machine in QEMU
- `scripts/build-riscv.sh` - RISC-V demo kernel build
- `scripts/run-riscv.sh` - RISC-V QEMU execution

---

*Integration audit: 2026-02-13*
*Update when adding/removing external services*
