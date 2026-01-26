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
