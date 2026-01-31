//! Industrial I/O (IIO) Driver
//!
//! This module provides the interface for IIO sensors (accelerometers, gyroscopes, etc.)
//! and integrates them with the BPF subsystem.
//!
//! In the future, this will interface with actual I2C/SPI drivers.
//! For now, it provides the mechanism to inject sensor events and trigger BPF hooks.

use alloc::vec::Vec;
#[cfg(target_arch = "x86_64")]
use alloc::boxed::Box;
#[cfg(target_arch = "x86_64")]
use core::ffi::c_void;
use spin::Mutex;
use conquer_once::spin::OnceCell;

use kernel_bpf::attach::{IioChannel, IioEvent};
use kernel_bpf::execution::BpfContext;
#[cfg(target_arch = "x86_64")]
use crate::mcore::mtask::task::Task;
#[cfg(target_arch = "x86_64")]
use crate::mcore::mtask::process::Process;
#[cfg(target_arch = "x86_64")]
use crate::mcore::mtask::scheduler::global::GlobalTaskQueue;

/// Global IIO manager instance
pub static IIO_MANAGER: OnceCell<Mutex<IioManager>> = OnceCell::uninit();

/// Initialize the IIO subsystem
pub fn init() {
    IIO_MANAGER.init_once(|| Mutex::new(IioManager::new()));
}

/// Manages IIO devices and event dispatch
pub struct IioManager {
    /// Simulated devices for now
    devices: Vec<IioDevice>,
}

impl IioManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn register_device(&mut self, device: IioDevice) {
        self.devices.push(device);
    }

    /// Dispatch an IIO event to BPF hooks
    ///
    /// This is called by hardware drivers (or simulation) when new data is available.
    pub fn dispatch_event(&self, event: IioEvent) {
        // Create BPF context from the event
        // SAFETY: We are creating a slice from a stack-allocated struct.
        // The slice is only used within this scope to create the BpfContext.
        let slice = unsafe {
            core::slice::from_raw_parts(
                &event as *const _ as *const u8,
                core::mem::size_of::<IioEvent>(),
            )
        };

        let ctx = BpfContext::from_slice(slice);

        // Execute BPF hooks
        if let Some(manager) = crate::BPF_MANAGER.get() {
            manager
                .lock()
                .execute_hooks(crate::bpf::ATTACH_TYPE_IIO, &ctx);
        }
    }
}

/// Represents a physical IIO device
#[derive(Debug, Clone)]
pub struct IioDevice {
    pub id: u32,
    pub name: alloc::string::String,
    pub channels: Vec<IioChannel>,
}

impl IioDevice {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.into(),
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: IioChannel) {
        self.channels.push(channel);
    }
}

/// Simulation task entry point
#[cfg(target_arch = "x86_64")]
extern "C" fn iio_simulation_task(_arg: *mut c_void) {
    let mut counter = 0;
    loop {
        // Generate simulated accelerometer data
        let event = IioEvent {
            timestamp: 0, // In a real system, we'd use a timer
            device_id: 0,
            channel: 0, // AccelX
            value: counter,
            scale: 1_000_000,
            offset: 0,
        };

        if let Some(manager_lock) = IIO_MANAGER.get() {
            let manager = manager_lock.lock();
            manager.dispatch_event(event);
        }

        counter = (counter + 1) % 1000;

        // Simple delay - wait for a few interrupts
        for _ in 0..100 {
            #[cfg(target_arch = "x86_64")]
            unsafe { core::arch::asm!("hlt") };
            #[cfg(target_arch = "aarch64")]
            unsafe { core::arch::asm!("wfi") };
        }
    }
}

/// Initialize a simulated accelerometer for testing
pub fn init_simulated_device() {
    if let Some(manager_lock) = IIO_MANAGER.get() {
        let mut manager = manager_lock.lock();

        let mut accel = IioDevice::new(0, "simulated-accel");
        accel.add_channel(IioChannel::AccelX);
        accel.add_channel(IioChannel::AccelY);
        accel.add_channel(IioChannel::AccelZ);

        manager.register_device(accel);

        ::log::info!("Initialized simulated IIO accelerometer (id=0)");

        // Spawn simulation task
        #[cfg(target_arch = "x86_64")]
        {
            let task = Task::create_new(Process::root(), iio_simulation_task, core::ptr::null_mut())
                .expect("failed to create IIO simulation task");
            GlobalTaskQueue::enqueue(Box::pin(task));

            ::log::info!("Started IIO simulation background task");
        }
    }
}
