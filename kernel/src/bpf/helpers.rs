use crate::time::get_kernel_time_ns;

#[unsafe(no_mangle)]
pub extern "C" fn bpf_ktime_get_ns() -> u64 {
    get_kernel_time_ns()
}

#[unsafe(no_mangle)]
pub extern "C" fn bpf_trace_printk(fmt: *const u8, _size: u32) -> i32 {
    // Safety: The verifier guarantees that the string is in valid memory.
    unsafe {
        let s = core::ffi::CStr::from_ptr(fmt as *const i8);
        if let Ok(msg) = s.to_str() {
            log::info!("[BPF] {}", msg);
            return 0;
        }
    }
    -1
}

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
