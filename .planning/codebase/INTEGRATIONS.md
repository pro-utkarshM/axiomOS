# External Integrations

**Analysis Date:** 2026-01-21

## APIs & External Services

**Payment Processing:**
- Not applicable (bare-metal OS kernel)

**Email/SMS:**
- Not applicable (bare-metal OS kernel)

**External APIs:**
- Not applicable (no network stack implemented)

## Data Storage

**Databases:**
- Not applicable (bare-metal OS kernel)

**File Storage:**
- ext2 Filesystem - Primary storage format
  - Read support: `kernel/src/file/ext2.rs`
  - Write support: Not implemented (todo!)
  - Builder: `mkfs-ext2` crate from `https://github.com/tsatke/mkfs` - `Cargo.toml`
  - Disk image: Created via `mke2fs` external command - `build.rs`

**Caching:**
- Not applicable (no caching layer)

## Authentication & Identity

**Auth Provider:**
- Not applicable (no authentication system)

**OAuth Integrations:**
- Not applicable

## Monitoring & Observability

**Error Tracking:**
- Serial console logging via `uart_16550` - `kernel/src/serial.rs`
- Panic handler writes to serial - `kernel/src/main.rs`

**Analytics:**
- Not applicable

**Logs:**
- `log` crate (0.4) - Logging abstraction - `Cargo.toml`
- Output to serial console (COM1: 0x3F8 on x86_64)

## CI/CD & Deployment

**Hosting:**
- GitHub - Source control
- Repository: `axiom-ebpf`

**CI Pipeline:**
- GitHub Actions - `.github/workflows/build.yml`
- Workflows:
  - `lint` - rustfmt and clippy checks
  - `test` - cargo test (debug + release matrix)
  - `miri` - Undefined behavior detection per-crate
  - `miri-kernel-bpf` - BPF profile-specific Miri tests
  - `build` - Full release build with ISO artifact
- Schedule: Every push + twice daily (cron: `0 5,17 * * *`)

**BPF Profile CI:**
- Dedicated workflow: `.github/workflows/bpf-profiles.yml`
- Tests both cloud and embedded profiles separately
- Semantic consistency verification

## Environment Configuration

**Development:**
- Required env vars: None (all configuration in Cargo.toml and config files)
- Secrets location: Not applicable
- Tools: `xorriso`, `qemu-system`, `e2fsprogs`

**Staging:**
- Not applicable (bare-metal deployment)

**Production:**
- Boot via Limine bootloader
- Supports BIOS and UEFI boot modes
- ISO image or direct disk boot

## Webhooks & Callbacks

**Incoming:**
- Not applicable

**Outgoing:**
- Not applicable

## Hardware/Device Integrations

**VirtIO Drivers:**
- `virtio-drivers` crate (0.12) - Generic VirtIO device support
- Block device: `kernel/src/driver/virtio/block.rs`
- GPU: `kernel/src/driver/virtio/gpu.rs`
- HAL: `kernel/src/driver/virtio/hal.rs`

**PCI Enumeration:**
- `kernel_pci` subsystem - `kernel/crates/kernel_pci/`
- Device discovery and driver binding - `kernel/src/driver/pci.rs`

**Serial/UART:**
- UART 16550 driver - `uart_16550` crate
- Serial console at COM1 (0x3F8) - `kernel/src/serial.rs`

**Platform-Specific:**
- x86_64: APIC, x2APIC, ACPI, HPET - `kernel/src/apic.rs`, `kernel/src/hpet.rs`
- AArch64: GIC (interrupt controller), DTB parsing - `kernel/src/arch/aarch64/`
- Raspberry Pi 5: RP1 UART, GPIO - `kernel/src/arch/aarch64/platform/rpi5/`

## eBPF Subsystem

**kernel_bpf** - First-class eBPF kernel subsystem
- Location: `kernel/crates/kernel_bpf/`
- Optional feature in kernel: `dep:kernel_bpf` - `kernel/Cargo.toml`

**Profile-Aware Architecture:**

| Property | Cloud Profile | Embedded Profile |
|----------|---------------|------------------|
| Memory | Elastic (heap) | Static (64KB pool) |
| Stack | 512 KB | 8 KB |
| Instructions | 1,000,000 max | 100,000 max |
| JIT | Available | Erased at compile-time |
| Scheduling | Throughput | Deadline (EDF) |
| Map Resize | Available | Erased |

**eBPF Components:**
- Bytecode: `kernel/crates/kernel_bpf/src/bytecode/`
- Verifier: `kernel/crates/kernel_bpf/src/verifier/`
- Execution: `kernel/crates/kernel_bpf/src/execution/` (Interpreter + JIT)
- Maps: `kernel/crates/kernel_bpf/src/maps/`
- Scheduler: `kernel/crates/kernel_bpf/src/scheduler/`

## Boot & Firmware

**Limine Bootloader:**
- Protocol: Limine v9.x
- Configuration: `limine.conf`
- Cloned during build: `build.rs` line 222

**UEFI Support:**
- OVMF firmware for x86_64 emulation - `build.rs`
- `ovmf-prebuilt` crate (0.2.3) - `Cargo.toml`

**ISO Creation:**
- Tool: `xorriso` (external)
- Creates hybrid BIOS/UEFI bootable ISO - `build.rs` lines 166-197

## Not Detected

- Web framework (no HTTP server)
- Database clients (no SQL/NoSQL)
- Cloud provider SDKs (no AWS/GCP/Azure)
- Message queues
- Network stack (no TCP/IP)

---

*Integration audit: 2026-01-21*
*Update when adding/removing external services*
