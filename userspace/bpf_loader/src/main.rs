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
    write(1, b"[bpf_loader] Starting BPF loader...\n");

    // ------------------------------------------------------------------
    // Construct a BPF program that calls bpf_trace_printk (helper 2)
    // with the message "BPF tick!\0" stored on the BPF stack.
    //
    // Stack layout (relative to r10, the frame pointer):
    //   r10 - 16 : 8 bytes  (first 8 chars: "BPF tick")
    //   r10 - 8  : 8 bytes  (remaining: "!\0" padded with zeros)
    //
    // bpf_trace_printk(r1=fmt_ptr, r2=fmt_size) -> i32
    //   helper_id = 2
    //
    // The string "BPF tick!\0" is 10 bytes. We store it in two 8-byte
    // stack slots using store-double-word (STX DW) instructions.
    // ------------------------------------------------------------------

    #[repr(C)]
    struct BpfInsn {
        code: u8,
        dst_src: u8, // dst (low 4 bits) | src (high 4 bits)
        off: i16,
        imm: i32,
    }

    // Helper: encode dst and src register nibbles into dst_src byte.
    // dst goes in low 4 bits, src goes in high 4 bits.
    const fn regs(dst: u8, src: u8) -> u8 {
        (src << 4) | (dst & 0x0f)
    }

    // "BPF tick" as little-endian u64 = bytes [B, P, F, ' ', 't', 'i', 'c', 'k']
    let word1 = u64::from_le_bytes(*b"BPF tick");
    // "!\0\0\0\0\0\0\0" as little-endian u64
    let word2 = u64::from_le_bytes(*b"!\0\0\0\0\0\0\0");

    let insns = [
        // Store the string "BPF tick!\0" on the BPF stack
        //
        // Instruction 0: r1 = word1 (LD_DW_IMM, wide instruction, 2 slots)
        // LD_DW_IMM: opcode 0x18, dst=r1
        BpfInsn {
            code: 0x18,
            dst_src: regs(1, 0),
            off: 0,
            imm: word1 as i32,
        },
        // Instruction 1: second half of wide load
        BpfInsn {
            code: 0x00,
            dst_src: 0x00,
            off: 0,
            imm: (word1 >> 32) as i32,
        },
        // Instruction 2: *(u64 *)(r10 - 16) = r1   (STX DW)
        // STX DW: opcode 0x7b, dst=r10, src=r1
        BpfInsn {
            code: 0x7b,
            dst_src: regs(10, 1),
            off: -16,
            imm: 0,
        },
        // Instruction 3: r1 = word2 (LD_DW_IMM, wide instruction, 2 slots)
        BpfInsn {
            code: 0x18,
            dst_src: regs(1, 0),
            off: 0,
            imm: word2 as i32,
        },
        // Instruction 4: second half of wide load
        BpfInsn {
            code: 0x00,
            dst_src: 0x00,
            off: 0,
            imm: (word2 >> 32) as i32,
        },
        // Instruction 5: *(u64 *)(r10 - 8) = r1   (STX DW)
        BpfInsn {
            code: 0x7b,
            dst_src: regs(10, 1),
            off: -8,
            imm: 0,
        },
        // Now prepare arguments for bpf_trace_printk(r1=fmt_ptr, r2=fmt_size)
        //
        // Instruction 6: r1 = r10   (MOV64 reg)
        // MOV64 reg: opcode 0xbf
        BpfInsn {
            code: 0xbf,
            dst_src: regs(1, 10),
            off: 0,
            imm: 0,
        },
        // Instruction 7: r1 += -16  (ADD64 imm)
        // ADD64 imm: opcode 0x07
        BpfInsn {
            code: 0x07,
            dst_src: regs(1, 0),
            off: 0,
            imm: -16,
        },
        // Instruction 8: r2 = 10  (MOV64 imm) - length of "BPF tick!\0"
        // MOV64 imm: opcode 0xb7
        BpfInsn {
            code: 0xb7,
            dst_src: regs(2, 0),
            off: 0,
            imm: 10,
        },
        // Instruction 9: call helper 2 (bpf_trace_printk)
        // CALL: opcode 0x85, imm = helper_id
        BpfInsn {
            code: 0x85,
            dst_src: 0x00,
            off: 0,
            imm: 2,
        },
        // Instruction 10: r0 = 0  (MOV64 imm)
        BpfInsn {
            code: 0xb7,
            dst_src: regs(0, 0),
            off: 0,
            imm: 0,
        },
        // Instruction 11: exit
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        },
    ];

    // ------------------------------------------------------------------
    // Step 1: Load the BPF program via sys_bpf(BPF_PROG_LOAD=5, attr)
    // ------------------------------------------------------------------
    use kernel_abi::BpfAttr;

    let load_attr = BpfAttr {
        prog_type: 1, // SocketFilter (arbitrary, accepted by the kernel)
        insn_cnt: insns.len() as u32,
        insns: insns.as_ptr() as u64,
        ..Default::default()
    };

    write(1, b"[bpf_loader] Loading BPF program (");
    print_num(insns.len() as u64);
    write(1, b" instructions)...\n");

    let prog_id = bpf(
        5, // BPF_PROG_LOAD
        &load_attr as *const BpfAttr as *const u8,
        core::mem::size_of::<BpfAttr>() as i32,
    );

    if prog_id < 0 {
        write(1, b"[bpf_loader] FAILED to load BPF program (error ");
        print_num((-prog_id) as u64);
        write(1, b")\n");
        exit(1);
    }

    write(1, b"[bpf_loader] BPF program loaded with id=");
    print_num(prog_id as u64);
    write(1, b"\n");

    // ------------------------------------------------------------------
    // Step 2: Attach the program to the timer via sys_bpf(BPF_PROG_ATTACH=8)
    //
    // From kernel/src/syscall/bpf.rs:
    //   attach_btf_id  -> attach_type (ATTACH_TYPE_TIMER = 1)
    //   attach_prog_fd -> program id
    // ------------------------------------------------------------------
    let attach_attr = BpfAttr {
        attach_btf_id: 1,              // ATTACH_TYPE_TIMER
        attach_prog_fd: prog_id as u32, // program id from load step
        ..Default::default()
    };

    write(1, b"[bpf_loader] Attaching program to timer...\n");

    let attach_res = bpf(
        8, // BPF_PROG_ATTACH
        &attach_attr as *const BpfAttr as *const u8,
        core::mem::size_of::<BpfAttr>() as i32,
    );

    if attach_res != 0 {
        write(1, b"[bpf_loader] FAILED to attach BPF program (error ");
        print_num((-attach_res) as u64);
        write(1, b")\n");
        exit(1);
    }

    write(1, b"[bpf_loader] SUCCESS: BPF program attached to timer!\n");
    write(
        1,
        b"[bpf_loader] Timer ticks should now print 'BPF tick!' on serial console.\n",
    );

    // ------------------------------------------------------------------
    // Step 3: Loop to keep the process alive so timer ticks fire
    // ------------------------------------------------------------------
    loop {
        minilib::pause();
    }
}

fn print_num(mut n: u64) {
    if n == 0 {
        write(1, b"0");
        return;
    }

    let mut buf = [0u8; 20];
    let mut i = 0;

    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }

    // Reverse
    let mut j = 0;
    while j < i / 2 {
        buf.swap(j, i - 1 - j);
        j += 1;
    }

    write(1, &buf[..i]);
}
