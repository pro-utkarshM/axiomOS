use log::{info, warn};
use virtio_drivers::{Hal, transport::{Transport, mmio::MmioTransport, DeviceType}};

use crate::arch::aarch64::platform::virt::mmio::{VIRTIO_MMIO_BASE, VIRTIO_MMIO_SIZE, VIRTIO_MAX_DEVICES};
use crate::driver::virtio::hal::HalImpl;
use crate::driver::virtio::block;

/// Initialize VirtIO MMIO devices
///
/// Scans the fixed MMIO regions used by QEMU "virt" machine for VirtIO devices.
pub fn init() {
    info!("Scanning for VirtIO MMIO devices...");

    for i in 0..VIRTIO_MAX_DEVICES {
        let phys_addr = VIRTIO_MMIO_BASE + i * VIRTIO_MMIO_SIZE;

        // Map the device header
        // SAFETY: The address is within the valid MMIO region for QEMU virt.
        let virt_addr = unsafe {
            HalImpl::mmio_phys_to_virt(phys_addr as u64, VIRTIO_MMIO_SIZE)
        };

        // Try to initialize MMIO transport
        // MmioTransport::new validates the magic value ("virt") and version.
        // SAFETY: The virtual address is mapped and valid.
        match unsafe { MmioTransport::new(virt_addr.cast(), VIRTIO_MMIO_SIZE) } {
            Ok(transport) => {
                let device_type = transport.device_type();
                let version = transport.version();

                info!("Found VirtIO device at {:#x}: type {:?}, version {:?}", phys_addr, device_type, version);

                match device_type {
                    DeviceType::Block => {
                        info!("Initializing VirtIO Block device at {:#x}", phys_addr);
                        if let Err(e) = block::init_mmio(transport) {
                            warn!("Failed to initialize VirtIO Block device: {:?}", e);
                        }
                    }
                    _ => {
                        // TODO: Support other devices (Network, Console, GPU, etc.)
                        // For now, we just acknowledge existence
                    }
                }
            }
            Err(_e) => {
                // Not a VirtIO device or invalid magic, skip.
                // QEMU virt maps all 32 slots, but they might be empty (magic = 0)
                // log::trace!("No VirtIO device at {:#x}: {:?}", phys_addr, e);
            }
        }
    }
}
