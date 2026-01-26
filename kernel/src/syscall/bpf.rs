use alloc::vec::Vec;

use kernel_abi::{BPF_MAP_CREATE, BPF_PROG_ATTACH, BPF_PROG_LOAD, BpfAttr};
use kernel_bpf::bytecode::insn::BpfInsn;

use crate::BPF_MANAGER;

pub fn sys_bpf(cmd: usize, attr_ptr: usize, _size: usize) -> isize {
    // Basic permissions check would go here

    let cmd_u32 = cmd as u32;

    match cmd_u32 {
        BPF_PROG_ATTACH => {
            log::info!("sys_bpf: PROG_ATTACH");
            if attr_ptr == 0 {
                return -1; // EFAULT
            }
            let attr = unsafe { &*(attr_ptr as *const BpfAttr) };

            // In Linux, attach is separate, but we reuse target_fd etc or simplied logic.
            // target_fd -> attach_type for our MVP?
            // Actually BpfAttr has `attach_bpf_fd` and `attach_type` isn't a top level field in our struct yet, let's check ABI.
            // Linux BpfAttr uses anonymous union.
            // We defined `attach_btf_id` and `attach_prog_fd` in `BpfAttr`.

            // Let's assume for MVP: target of attachment is passed via `target_fd` (not in our struct yet) or `attach_btf_id`?
            // Wait, standard `BPF_PROG_ATTACH` uses `target_fd`, `attach_bpf_fd`, `attach_type`.

            // I need to check my BpfAttr definition again.
            // It has `attach_prog_fd`.

            // Let's rely on `attach_btf_id` as the TYPE for now, or add `attach_type` to struct if missing.
            // Checking `kernel_abi/src/bpf.rs`...

            // I added `expected_attach_type`? No.

            // Implementation plan said:
            // let target = attr.target_fd; // Use as attach_type
            // let prog_id = attr.attach_bpf_fd;

            // My struct update earlier added:
            // pub attach_btf_id: u32,
            // pub attach_prog_fd: u32,

            // I will use `attach_btf_id` as the "Attach Type" and `attach_prog_fd` as the Program ID for this specific MVP.
            // This is slightly non-standard naming but works for our internal ABI.

            let attach_type = attr.attach_btf_id;
            let prog_id = attr.attach_prog_fd;

            if let Some(manager) = BPF_MANAGER.get() {
                match manager.lock().attach(attach_type, prog_id) {
                    Ok(_) => {
                        log::info!("sys_bpf: attached prog {} to type {}", prog_id, attach_type);
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

            // Safety: We assume the pointer is valid for this MVP.
            // In a real kernel we would use copy_from_user and validate ranges.
            if attr_ptr == 0 {
                return -1; // EFAULT
            }

            let attr = unsafe { &*(attr_ptr as *const BpfAttr) };

            let insn_cnt = attr.insn_cnt as usize;
            let insns_ptr = attr.insns as *const BpfInsn;

            if insns_ptr.is_null() || insn_cnt == 0 || insn_cnt > 4096 {
                log::error!(
                    "sys_bpf: invalid instructions (ptr={:p}, cnt={})",
                    insns_ptr,
                    insn_cnt
                );
                return -1; // EINVAL
            }

            log::info!("sys_bpf: loading {} instructions", insn_cnt);

            // Copy instructions from userspace
            let mut insns = Vec::with_capacity(insn_cnt);
            for i in 0..insn_cnt {
                unsafe {
                    insns.push(*insns_ptr.add(i));
                }
            }

            // Load into manager
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
        BPF_MAP_CREATE => {
            log::info!("sys_bpf: MAP_CREATE");
            -1
        }
        _ => {
            log::warn!("sys_bpf: Unknown command {}", cmd);
            -1
        }
    }
}
