use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::error::Error;
use core::fmt::{Debug, Formatter};

use kernel_device::block::{BlockBuf, BlockDevice};
use kernel_device::Device;
use spin::Mutex;

use crate::driver::KernelDeviceId;

#[cfg(feature = "rpi5")]
static EMBEDDED_DISK: &[u8] = include_bytes!(env!("EMBEDDED_DISK_PATH"));

pub struct RamBlockDevice {
    id: KernelDeviceId,
    data: Arc<Mutex<Vec<u8>>>,
}

impl RamBlockDevice {
    pub fn new(data: &[u8]) -> Self {
        Self {
            id: KernelDeviceId::new(),
            data: Arc::new(Mutex::new(data.to_vec())),
        }
    }
}

impl Debug for RamBlockDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RamBlockDevice")
            .field("id", &self.id)
            .field("size", &self.data.lock().len())
            .finish()
    }
}

impl Device<KernelDeviceId> for RamBlockDevice {
    fn id(&self) -> KernelDeviceId {
        self.id
    }
}

impl BlockDevice<KernelDeviceId, 512> for RamBlockDevice {
    fn block_count(&self) -> usize {
        self.data.lock().len() / 512
    }

    fn read_block(
        &mut self,
        block_num: usize,
        buf: &mut BlockBuf<512>,
    ) -> Result<(), Box<dyn Error>> {
        let data = self.data.lock();
        let offset = block_num * 512;
        buf[..].copy_from_slice(&data[offset..offset + 512]);
        Ok(())
    }

    fn write_block(&mut self, block_num: usize, buf: &BlockBuf<512>) -> Result<(), Box<dyn Error>> {
        let mut data = self.data.lock();
        let offset = block_num * 512;
        data[offset..offset + 512].copy_from_slice(&buf[..]);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl filesystem::BlockDevice for RamBlockDevice {
    type Error = ();

    fn sector_size(&self) -> usize {
        512
    }

    fn sector_count(&self) -> usize {
        self.data.lock().len() / 512
    }

    fn read_sector(&self, sector_index: usize, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let data = self.data.lock();
        let offset = sector_index * 512;
        let len = buf.len().min(512);
        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    fn write_sector(&mut self, sector_index: usize, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut data = self.data.lock();
        let offset = sector_index * 512;
        let len = buf.len().min(512);
        data[offset..offset + len].copy_from_slice(&buf[..len]);
        Ok(len)
    }
}

#[cfg(feature = "rpi5")]
pub fn init_embedded() {
    use log::info;
    use spin::RwLock;

    use crate::driver::block::BlockDevices;

    info!(
        "Copying embedded disk image ({} bytes) to heap...",
        EMBEDDED_DISK.len()
    );
    let device = RamBlockDevice::new(EMBEDDED_DISK);
    info!("RamBlockDevice created: {:?}", device);

    let device = Arc::new(RwLock::new(device));
    BlockDevices::register_block_device(device).expect("should be able to register ramdisk");
    info!("Embedded ramdisk registered as block device");
}
