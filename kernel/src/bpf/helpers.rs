use crate::time::get_kernel_time_ns;

#[unsafe(no_mangle)]
pub extern "C" fn bpf_ktime_get_ns() -> u64 {
    get_kernel_time_ns()
}

/// BPF helper: Read GPIO pin value
///
/// Returns 1 if pin is high, 0 if low, -1 on error (invalid pin).
#[unsafe(no_mangle)]
pub extern "C" fn bpf_gpio_read(pin: u32) -> i64 {
    #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
    {
        if pin >= 28 {
            return -1;
        }
        let gpio = unsafe { crate::arch::aarch64::platform::rpi5::gpio::Rp1Gpio::new() };
        if gpio.read(pin as u8) { 1 } else { 0 }
    }
    #[cfg(not(all(target_arch = "aarch64", feature = "rpi5")))]
    {
        let _ = pin;
        -1
    }
}

/// BPF helper: Write GPIO pin value
///
/// Sets output pin high (value != 0) or low (value == 0).
/// Returns 0 on success, -1 on error (invalid pin).
///
/// Note: Pin must be configured as output first via syscall.
#[unsafe(no_mangle)]
pub extern "C" fn bpf_gpio_write(pin: u32, value: u32) -> i64 {
    #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
    {
        if pin >= 28 {
            return -1;
        }
        let gpio = unsafe { crate::arch::aarch64::platform::rpi5::gpio::Rp1Gpio::new() };
        if value != 0 {
            gpio.set_high(pin as u8);
        } else {
            gpio.set_low(pin as u8);
        }
        0
    }
    #[cfg(not(all(target_arch = "aarch64", feature = "rpi5")))]
    {
        let _ = (pin, value);
        -1
    }
}

/// BPF helper: Toggle GPIO pin
///
/// Toggles output pin state (high -> low or low -> high).
/// Returns new value (0 or 1) on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn bpf_gpio_toggle(pin: u32) -> i64 {
    #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
    {
        if pin >= 28 {
            return -1;
        }
        let gpio = unsafe { crate::arch::aarch64::platform::rpi5::gpio::Rp1Gpio::new() };
        gpio.toggle(pin as u8);
        // Return new value
        if gpio.read(pin as u8) { 1 } else { 0 }
    }
    #[cfg(not(all(target_arch = "aarch64", feature = "rpi5")))]
    {
        let _ = pin;
        -1
    }
}

/// BPF helper: Configure GPIO pin as output
///
/// Configures pin as output with specified initial value.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn bpf_gpio_set_output(pin: u32, initial_high: u32) -> i64 {
    #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
    {
        if pin >= 28 {
            return -1;
        }
        let gpio = unsafe { crate::arch::aarch64::platform::rpi5::gpio::Rp1Gpio::new() };
        gpio.configure_output(pin as u8, initial_high != 0);
        0
    }
    #[cfg(not(all(target_arch = "aarch64", feature = "rpi5")))]
    {
        let _ = (pin, initial_high);
        -1
    }
}

/// BPF helper: Write to PWM channel
///
/// Arguments:
/// - pwm_id: 0 or 1
/// - channel: 1 or 2
/// - duty_percent: 0-100
///
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn bpf_pwm_write(pwm_id: u32, channel: u32, duty_percent: u32) -> i64 {
    #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
    {
        use crate::arch::aarch64::platform::rpi5::pwm::{PWM0, PWM1};

        if !(1..=2).contains(&channel) {
            return -1;
        }

        match pwm_id {
            0 => {
                let pwm = PWM0.lock();
                pwm.set_duty_cycle(channel as u8, duty_percent);
                0
            }
            1 => {
                let pwm = PWM1.lock();
                pwm.set_duty_cycle(channel as u8, duty_percent);
                0
            }
            _ => -1,
        }
    }
    #[cfg(not(all(target_arch = "aarch64", feature = "rpi5")))]
    {
        let _ = (pwm_id, channel, duty_percent);
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bpf_trace_printk(fmt: *const u8, _size: u32) -> i32 {
    // Safety: The verifier guarantees that the string is in valid memory.
    unsafe {
        let s = core::ffi::CStr::from_ptr(fmt as *const core::ffi::c_char);
        if let Ok(msg) = s.to_str() {
            log::info!("[BPF] {}", msg);
            return 0;
        }
    }
    -1
}

/// BPF helper: look up a map element by key.
///
/// # Safety
/// Called from verified BPF programs. The verifier ensures key_ptr is valid.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn bpf_map_lookup_elem(map_id: u32, key_ptr: *const u8) -> *mut u8 {
    use crate::BPF_MANAGER;
    if let Some(manager) = BPF_MANAGER.get() {
        let manager = manager.lock();
        if let Some(def) = manager.get_map_def(map_id) {
            let key_size = def.key_size as usize;
            // Safety: Verifier ensures valid memory access for key_ptr
            let key = unsafe { core::slice::from_raw_parts(key_ptr, key_size) };
            // Safety: Manager lock ensures map stability
            if let Some(ptr) = unsafe { manager.map_lookup_ptr(map_id, key) } {
                return ptr;
            }
        }
    }
    core::ptr::null_mut()
}

/// BPF helper: update a map element.
///
/// # Safety
/// Called from verified BPF programs. The verifier ensures pointers are valid.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn bpf_map_update_elem(
    map_id: u32,
    key_ptr: *const u8,
    value_ptr: *const u8,
    flags: u64,
) -> i32 {
    use crate::BPF_MANAGER;
    if let Some(manager) = BPF_MANAGER.get() {
        let manager = manager.lock();
        if let Some(def) = manager.get_map_def(map_id) {
            let key_size = def.key_size as usize;
            let value_size = def.value_size as usize;

            // Safety: Verifier ensures valid memory access
            let key = unsafe { core::slice::from_raw_parts(key_ptr, key_size) };
            let value = unsafe { core::slice::from_raw_parts(value_ptr, value_size) };

            if manager.map_update(map_id, key, value, flags).is_ok() {
                return 0;
            }
        }
    }
    -1
}

/// BPF helper: delete a map element.
///
/// # Safety
/// Called from verified BPF programs. The verifier ensures key_ptr is valid.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn bpf_map_delete_elem(map_id: u32, key_ptr: *const u8) -> i32 {
    use crate::BPF_MANAGER;
    if let Some(manager) = BPF_MANAGER.get() {
        let manager = manager.lock();
        if let Some(def) = manager.get_map_def(map_id) {
            let key_size = def.key_size as usize;
            let key = unsafe { core::slice::from_raw_parts(key_ptr, key_size) };
            if manager.map_delete(map_id, key).is_ok() {
                return 0;
            }
        }
    }
    -1
}

/// BPF helper: output data to a ring buffer map.
///
/// Writes event data to a ring buffer map for consumption by userspace.
///
/// # Arguments
/// * `map_id` - The ring buffer map ID
/// * `data_ptr` - Pointer to the event data
/// * `data_size` - Size of the event data in bytes
/// * `flags` - Reserved for future use (pass 0)
///
/// # Returns
/// 0 on success, negative error code on failure.
///
/// # Safety
/// Called from verified BPF programs. The verifier ensures data_ptr is valid.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn bpf_ringbuf_output(
    map_id: u32,
    data_ptr: *const u8,
    data_size: u64,
    flags: u64,
) -> i64 {
    use crate::BPF_MANAGER;

    if data_ptr.is_null() {
        return -1;
    }

    if let Some(manager) = BPF_MANAGER.get() {
        let manager = manager.lock();
        // Safety: Verifier ensures valid memory access for data_ptr
        let data = unsafe { core::slice::from_raw_parts(data_ptr, data_size as usize) };

        if manager.ringbuf_output(map_id, data, flags).is_ok() {
            return 0;
        }
    }
    -1
}
