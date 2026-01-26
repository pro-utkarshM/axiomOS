#![no_std]
#![no_main]

use minilib::write;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let bytes = b"hello from init!\n";
    write(1, bytes);

    use kernel_abi::BpfAttr;
    use minilib::bpf;

    write(1, b"Loading Timer BPF program...\n");

    #[repr(C)]
    struct BpfInsn {
        code: u8,
        dst_src: u8,
        off: i16,
        imm: i32,
    }

    // Program:
    // r1 = 0x006b636954   ("Tick\0" in little endian is 'T'(54) 'i'(69) 'c'(63) 'k'(6b) '\0'(00))
    // r1 = 0x006b636954
    // *(u64 *)(r10 - 8) = r1
    // r1 = r10
    // r1 += -8
    // r2 = 5
    // call bpf_trace_printk (2)
    // exit

    let insns = [
        // r1 = 0x006b636954 ("Tick\0")
        // 'T'=0x54, 'i'=0x69, 'c'=0x63, 'k'=0x6b, '\0'=0x00
        // Value: 0x006B636954
        BpfInsn {
            code: 0xb7,
            dst_src: 0x01,
            off: 0,
            imm: 0x6c636954,
        }, // "Tick" (oops 'l' is 6c)
        // Wait: T=0x54, i=0x69, c=0x63, k=0x6b.
        // 0x6b636954
        // Let's print "Hi\0\0" simpler: 0x00006948 ('H'=48, 'i'=69)
        // Let's stick to "Tick" -> 0x6b636954. (Check ascii: T=84=0x54, i=105=0x69, c=99=0x63, k=107=0x6b)
        // So 0x006b636954.

        // r1 = 0x6b636954 (imm32)
        BpfInsn {
            code: 0xb7,
            dst_src: 0x01,
            off: 0,
            imm: 0x6b636954,
        },
        // *(u32 *)(r10 - 4) = r1 (STX, MEM, W=0 = 4 bytes? No, BPF_W=0, BPF_DW=3)
        // code = BPF_STX | BPF_MEM | BPF_W (0x63)
        BpfInsn {
            code: 0x63,
            dst_src: 0x1a,
            off: -4,
            imm: 0,
        }, // src=r1(1), dst=r10(a)
        // r1 = 0 (for null terminator if needed, but we wrote u32 above which has no null if we want 5 chars)
        // Wait, "Tick" is 4 chars. "Tick\0" is 5.
        // I need to write 0 to r10-4+4?
        // Let's just write 0 to r10-8 first.

        // r1 = 0
        BpfInsn {
            code: 0xb7,
            dst_src: 0x01,
            off: 0,
            imm: 0,
        },
        // *(u64 *)(r10 - 8) = r1  (STX, MEM, DW=3 -> 0x7b)
        BpfInsn {
            code: 0x7b,
            dst_src: 0x1a,
            off: -8,
            imm: 0,
        },
        // r1 = 0x6b636954 ("Tick")
        BpfInsn {
            code: 0xb7,
            dst_src: 0x01,
            off: 0,
            imm: 0x6b636954,
        },
        // *(u32 *)(r10 - 8) = r1 (STX, MEM, W=0 -> 0x63)
        // Writing to offset -8 puts "Tick" at -8. Nulls follow at -4.
        BpfInsn {
            code: 0x63,
            dst_src: 0x1a,
            off: -8,
            imm: 0,
        },
        // r1 = r10
        BpfInsn {
            code: 0xbf,
            dst_src: 0x0a,
            off: 0,
            imm: 0,
        },
        // r1 += -8
        BpfInsn {
            code: 0x07,
            dst_src: 0x01,
            off: 0,
            imm: -8,
        },
        // r2 = 5
        BpfInsn {
            code: 0xb7,
            dst_src: 0x02,
            off: 0,
            imm: 5,
        },
        // call bpf_trace_printk (2)
        BpfInsn {
            code: 0x85,
            dst_src: 0x00,
            off: 0,
            imm: 2,
        },
        // exit
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        },
    ];

    let attr = BpfAttr {
        prog_type: 1,
        insn_cnt: 8,
        insns: insns.as_ptr() as u64,
        ..Default::default()
    };

    let attr_ptr = &attr as *const BpfAttr as *const u8;

    // LOAD (cmd=5)
    let prog_id = bpf(5, attr_ptr, core::mem::size_of::<BpfAttr>() as i32);

    if prog_id >= 0 {
        write(1, b"BPF program loaded. Attaching to Timer...\n");

        let attach_attr = BpfAttr {
            attach_btf_id: 1, // Timer
            attach_prog_fd: prog_id as u32,
            ..Default::default()
        };

        let attach_ptr = &attach_attr as *const BpfAttr as *const u8;
        let res = bpf(8, attach_ptr, core::mem::size_of::<BpfAttr>() as i32);

        if res == 0 {
            write(1, b"Attached! Waiting for ticks...\n");
        } else {
            write(1, b"Attach failed.\n");
        }
    } else {
        write(1, b"Failed to load BPF program\n");
    }

    loop {
        unsafe {
            core::arch::asm!("pause");
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    loop {}
}
