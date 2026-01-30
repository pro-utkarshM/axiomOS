#![no_std]
#![no_main]

use core::panic::PanicInfo;

use minilib::{bpf, exit, write};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

// SAFETY: Entry point for the BPF loader. Called by the startup code.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let msg = b"Loading dynamic BPF program...\n";
    write(1, msg);

    // Program: mov64_imm(0, 42); exit()
    // Encoding:
    // mov64_imm(0, 42) -> 0xb7, 0x00, 0x00, 0x00, 42, 0, 0, 0
    // exit() -> 0x95, 0x00, 0x00, 0x00, 0, 0, 0, 0

    // We construct byte array manually since BpfInsn struct isn't fully exposed to userspace yet in a friendly way
    // (though we could use kernel_bpf crate if it was no_std compatible for userspace easily, but keeping it simple)

    #[repr(C)]
    struct BpfInsn {
        code: u8,
        dst_src: u8,
        off: i16,
        imm: i32,
    }

    let insns = [
        BpfInsn {
            code: 0xb7,
            dst_src: 0x00,
            off: 0,
            imm: 42,
        }, // r0 = 42
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        }, // exit
    ];

    use kernel_abi::BpfAttr;

    let attr = BpfAttr {
        prog_type: 1, // SOCKET_FILTER (arbitrary for now)
        insn_cnt: 2,
        insns: insns.as_ptr() as u64,
        ..Default::default()
    };

    let attr_ptr = &attr as *const BpfAttr as *const u8;

    // BPF_PROG_LOAD = 5
    let res = bpf(5, attr_ptr, core::mem::size_of::<BpfAttr>() as i32);

    if res >= 0 {
        write(1, b"BPF program loaded successfully!\n");
    } else {
        write(1, b"Failed to load BPF program\n");
    }

    exit(0);
}
