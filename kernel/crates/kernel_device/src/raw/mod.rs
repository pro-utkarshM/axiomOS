use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use kernel_physical_memory::PhysFrameRangeInclusive;
use spin::RwLock;

use crate::{Device, DeviceId, RegisterDeviceError};

pub trait RawDevice<Id: DeviceId>: Device<Id> {
    fn physical_memory(&self) -> PhysFrameRangeInclusive;
}

pub struct RawDeviceRegistry<Id>
where
    Id: DeviceId + Ord + 'static,
{
    devices: BTreeMap<Id, Arc<RwLock<dyn RawDevice<Id>>>>,
}

// SAFETY: RawDeviceRegistry owns the devices and protects access via RwLock.
// The BTreeMap and Arc<RwLock> structure ensures that it is safe to send
// the registry to another thread.
unsafe impl<Id> Send for RawDeviceRegistry<Id> where Id: DeviceId + Ord + 'static {}

// SAFETY: RawDeviceRegistry protects its internal map with a BTreeMap (which is Send)
// and the individual devices are wrapped in Arc<RwLock>, ensuring thread-safe access.
unsafe impl<Id> Sync for RawDeviceRegistry<Id> where Id: DeviceId + Ord + 'static {}

impl<Id> Default for RawDeviceRegistry<Id>
where
    Id: DeviceId + Ord + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> RawDeviceRegistry<Id>
where
    Id: DeviceId + Ord + 'static,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }
}

impl<Id> RawDeviceRegistry<Id>
where
    Id: DeviceId + Ord + 'static,
{
    /// # Errors
    /// Returns an error if the device is already registered, returning the
    /// device that could not be registered.
    pub fn register_device<D>(&mut self, device: Arc<RwLock<D>>) -> Result<(), RegisterDeviceError>
    where
        D: RawDevice<Id>,
        D: 'static,
    {
        let id = device.read().id();
        if self.devices.contains_key(&id) {
            return Err(RegisterDeviceError::AlreadyRegistered);
        }

        self.devices.insert(id, device);

        Ok(())
    }

    pub fn all_devices(&self) -> impl Iterator<Item = &Arc<RwLock<dyn RawDevice<Id>>>> {
        self.devices.values()
    }
}
