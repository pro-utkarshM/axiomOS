use alloc::vec::Vec;
use core::mem::size_of;

use kernel_abi::{
    BPF_MAP_CREATE, BPF_MAP_DELETE_ELEM, BPF_MAP_LOOKUP_ELEM, BPF_MAP_UPDATE_ELEM, BPF_PROG_ATTACH,
    BPF_PROG_LOAD, BpfAttr,
};
use kernel_bpf::bytecode::insn::BpfInsn;

use super::validation::{copy_from_userspace, copy_to_userspace, read_userspace_slice};
use crate::BPF_MANAGER;

pub fn sys_bpf(cmd: usize, attr_ptr: usize, _size: usize) -> isize {
    // Basic permissions check would go here

    let cmd_u32 = cmd as u32;

    match cmd_u32 {
        BPF_MAP_CREATE => {
            log::info!("sys_bpf: MAP_CREATE");
            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            // For MAP_CREATE, fields are:
            // prog_type -> map_type
            // insn_cnt -> key_size
            // insns (low u32) -> value_size
            // insns (high u32) -> max_entries
            let map_type = attr.prog_type;
            let key_size = attr.insn_cnt;
            let value_size = (attr.insns & 0xFFFFFFFF) as u32;
            let max_entries = ((attr.insns >> 32) & 0xFFFFFFFF) as u32;

            if let Some(manager) = BPF_MANAGER.get() {
                match manager
                    .lock()
                    .create_map(map_type, key_size, value_size, max_entries)
                {
                    Ok(map_id) => map_id as isize,
                    Err(e) => {
                        log::error!("sys_bpf: MAP_CREATE failed: {}", e);
                        -1
                    }
                }
            } else {
                -1
            }
        }
        BPF_MAP_LOOKUP_ELEM => {
            log::debug!("sys_bpf: MAP_LOOKUP_ELEM");
            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            let map_id = attr.map_fd;
            let key_ptr = attr.key as *const u8;
            let value_ptr = attr.value as *mut u8;

            if key_ptr.is_null() || value_ptr.is_null() {
                return -1;
            }

            if let Some(manager) = BPF_MANAGER.get() {
                let mgr = manager.lock();
                // Get key size from map (for now, assume 4 bytes)
                let key_size = 4usize; // TODO: get from map def

                let key = match read_userspace_slice(key_ptr as usize, key_size) {
                    Ok(k) => k,
                    Err(_) => return -1,
                };

                if let Some(value) = mgr.map_lookup(map_id, &key) {
                    // Copy value to user buffer
                    if copy_to_userspace(value_ptr as usize, &value).is_err() {
                        return -1;
                    }
                    0
                } else {
                    -2 // ENOENT
                }
            } else {
                -1
            }
        }
        BPF_MAP_UPDATE_ELEM => {
            log::debug!("sys_bpf: MAP_UPDATE_ELEM");
            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            let map_id = attr.map_fd;
            let key_ptr = attr.key as *const u8;
            let value_ptr = attr.value as *const u8;
            let flags = attr.flags;

            if key_ptr.is_null() || value_ptr.is_null() {
                return -1;
            }

            if let Some(manager) = BPF_MANAGER.get() {
                let mgr = manager.lock();
                // For now, assume fixed sizes (TODO: get from map def)
                let key_size = 4usize;
                let value_size = 8usize;

                let key = match read_userspace_slice(key_ptr as usize, key_size) {
                    Ok(k) => k,
                    Err(_) => return -1,
                };

                let value = match read_userspace_slice(value_ptr as usize, value_size) {
                    Ok(v) => v,
                    Err(_) => return -1,
                };

                match mgr.map_update(map_id, &key, &value, flags) {
                    Ok(_) => 0,
                    Err(e) => {
                        log::error!("sys_bpf: MAP_UPDATE failed: {}", e);
                        -1
                    }
                }
            } else {
                -1
            }
        }
        BPF_MAP_DELETE_ELEM => {
            log::debug!("sys_bpf: MAP_DELETE_ELEM");
            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            let map_id = attr.map_fd;
            let key_ptr = attr.key as *const u8;

            if key_ptr.is_null() {
                return -1;
            }

            if let Some(manager) = BPF_MANAGER.get() {
                let mgr = manager.lock();
                let key_size = 4usize;

                let key = match read_userspace_slice(key_ptr as usize, key_size) {
                    Ok(k) => k,
                    Err(_) => return -1,
                };

                match mgr.map_delete(map_id, &key) {
                    Ok(_) => 0,
                    Err(_) => -2, // ENOENT
                }
            } else {
                -1
            }
        }
        BPF_PROG_ATTACH => {
            log::info!("sys_bpf: PROG_ATTACH");
            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            let attach_type = attr.attach_btf_id;
            let prog_id = attr.attach_prog_fd;

            if let Some(manager) = BPF_MANAGER.get() {
                match manager.lock().attach(attach_type, prog_id) {
                    Ok(_) => {
                        log::info!("sys_bpf: attached prog {} to type {}", prog_id, attach_type);

                        // For GPIO attach type, also configure hardware interrupts
                        #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
                        if attach_type == crate::bpf::ATTACH_TYPE_GPIO {
                            // Use key as GPIO pin number, value as edge flags
                            // edge flags: 1 = rising, 2 = falling, 3 = both
                            let pin = attr.key as u8;
                            let edge_flags = attr.value as u32;

                            if pin < 28 {
                                // SAFETY: Rp1Gpio::new() creates an interface to memory-mapped
                                // GPIO registers. This is safe because:
                                // 1. We are on aarch64 with rpi5 feature enabled (checked by cfg)
                                // 2. The GPIO base address is hardcoded for RPi5 platform
                                // 3. We have validated the pin number is in range 0-27
                                // 4. The kernel has exclusive access to GPIO hardware
                                let gpio = unsafe {
                                    crate::arch::aarch64::platform::rpi5::gpio::Rp1Gpio::new()
                                };

                                // Configure pin as input for edge detection
                                gpio.configure_input(pin);

                                // Enable interrupts based on edge flags
                                let rising = (edge_flags & 1) != 0;
                                let falling = (edge_flags & 2) != 0;

                                // Default to both edges if none specified
                                let (rising, falling) = if !rising && !falling {
                                    (true, true)
                                } else {
                                    (rising, falling)
                                };

                                gpio.enable_interrupt(pin, rising, falling);
                                log::info!(
                                    "sys_bpf: enabled GPIO{} interrupt (rising={}, falling={})",
                                    pin,
                                    rising,
                                    falling
                                );
                            } else {
                                log::warn!("sys_bpf: invalid GPIO pin {} (must be 0-27)", pin);
                            }
                        }

                        0
                    }
                    Err(e) => {
                        log::error!("sys_bpf: attach failed: {}", e);
                        -1
                    }
                }
            } else {
                -1
            }
        }
        BPF_PROG_LOAD => {
            log::info!("sys_bpf: PROG_LOAD");

            let attr = match copy_from_userspace::<BpfAttr>(attr_ptr) {
                Ok(a) => a,
                Err(_) => return -1,
            };

            let insn_cnt = attr.insn_cnt as usize;
            let insns_ptr = attr.insns as *const BpfInsn;

            if insns_ptr.is_null() || insn_cnt == 0 || insn_cnt > 4096 {
                log::error!(
                    "sys_bpf: invalid instructions (ptr={:p}, cnt={})",
                    insns_ptr,
                    insn_cnt
                );
                return -1;
            }

            log::info!("sys_bpf: loading {} instructions", insn_cnt);

            let mut insns = Vec::with_capacity(insn_cnt);
            // safe cast because we validate the pointer below
            let insns_bytes_ptr = insns_ptr as usize;

            // Validate the entire instruction buffer first
            if read_userspace_slice(insns_bytes_ptr, insn_cnt * size_of::<BpfInsn>()).is_err() {
                log::error!("sys_bpf: invalid instruction buffer");
                return -1;
            }

            for i in 0..insn_cnt {
                match copy_from_userspace::<BpfInsn>(insns_bytes_ptr + i * size_of::<BpfInsn>()) {
                    Ok(insn) => insns.push(insn),
                    Err(_) => return -1,
                }
            }

            if let Some(manager) = BPF_MANAGER.get() {
                match manager.lock().load_raw_program(insns) {
                    Ok(id) => {
                        log::info!("sys_bpf: program loaded with id {}", id);
                        id as isize
                    }
                    Err(e) => {
                        log::error!("sys_bpf: failed to load program: {}", e);
                        -1
                    }
                }
            } else {
                log::error!("sys_bpf: BPF_MANAGER not initialized");
                -1
            }
        }
        _ => {
            log::warn!("sys_bpf: Unknown command {}", cmd);
            -1
        }
    }
}
