#![no_std]
#![no_main]

use core::panic::PanicInfo;

use minilib::{bpf, exit, msleep, write};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

// SAFETY: Entry point for the BPF loader. Called by the startup code.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write(1, b"[bpf_loader] Starting BPF ringbuf demo...\n");

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

    use kernel_abi::BpfAttr;
    let attr_size = core::mem::size_of::<BpfAttr>() as i32;

    // ------------------------------------------------------------------
    // Step 1: Create a ringbuf map via sys_bpf(BPF_MAP_CREATE=0)
    //
    // For MAP_CREATE, BpfAttr fields are:
    //   prog_type -> map_type (27 = ringbuf)
    //   insn_cnt  -> key_size (0 for ringbuf)
    //   insns low 32  -> value_size (0 for ringbuf)
    //   insns high 32 -> max_entries (buffer size, must be power of 2)
    // ------------------------------------------------------------------
    let ringbuf_size: u32 = 4096; // 4 KB ringbuf
    let create_attr = BpfAttr {
        prog_type: 27, // BPF_MAP_TYPE_RINGBUF
        insn_cnt: 0,   // key_size = 0 for ringbuf
        insns: ((ringbuf_size as u64) << 32) | 0u64, // value_size=0, max_entries=4096
        ..Default::default()
    };

    write(1, b"[bpf_loader] Creating ringbuf map (4096 bytes)...\n");

    let map_id = bpf(
        0, // BPF_MAP_CREATE
        &create_attr as *const BpfAttr as *const u8,
        attr_size,
    );

    if map_id < 0 {
        write(1, b"[bpf_loader] FAILED to create ringbuf map (error ");
        print_num((-map_id) as u64);
        write(1, b")\n");
        exit(1);
    }

    write(1, b"[bpf_loader] Ringbuf map created with id=");
    print_num(map_id as u64);
    write(1, b"\n");

    // ------------------------------------------------------------------
    // Step 2: Construct a BPF program that:
    //   - Calls bpf_ktime_get_ns() to get a timestamp
    //   - Stores it on the stack
    //   - Calls bpf_ringbuf_output(map_id, data_ptr, data_size, flags)
    //   - Returns 0
    //
    // Helper IDs (from interpreter dispatch):
    //   1 = bpf_ktime_get_ns()
    //   6 = bpf_ringbuf_output(map_id, data_ptr, data_size, flags)
    //
    // Stack layout:
    //   r10 - 8 : 8 bytes (timestamp from bpf_ktime_get_ns)
    // ------------------------------------------------------------------

    let insns = [
        // Instruction 0: call bpf_ktime_get_ns (helper 1)
        // Result in r0 = timestamp (nanoseconds)
        BpfInsn {
            code: 0x85, // CALL
            dst_src: 0x00,
            off: 0,
            imm: 1, // bpf_ktime_get_ns
        },
        // Instruction 1: *(u64*)(r10 - 8) = r0  (store timestamp on stack)
        // STX DW: opcode 0x7b, dst=r10, src=r0
        BpfInsn {
            code: 0x7b,
            dst_src: regs(10, 0),
            off: -8,
            imm: 0,
        },
        // Instruction 2: r1 = map_id (0)
        // MOV64 imm: opcode 0xb7
        BpfInsn {
            code: 0xb7,
            dst_src: regs(1, 0),
            off: 0,
            imm: map_id,
        },
        // Instruction 3: r2 = r10 (frame pointer)
        // MOV64 reg: opcode 0xbf
        BpfInsn {
            code: 0xbf,
            dst_src: regs(2, 10),
            off: 0,
            imm: 0,
        },
        // Instruction 4: r2 += -8 (point to timestamp on stack)
        // ADD64 imm: opcode 0x07
        BpfInsn {
            code: 0x07,
            dst_src: regs(2, 0),
            off: 0,
            imm: -8,
        },
        // Instruction 5: r3 = 8 (data size = sizeof(u64))
        // MOV64 imm: opcode 0xb7
        BpfInsn {
            code: 0xb7,
            dst_src: regs(3, 0),
            off: 0,
            imm: 8,
        },
        // Instruction 6: r4 = 0 (flags)
        // MOV64 imm: opcode 0xb7
        BpfInsn {
            code: 0xb7,
            dst_src: regs(4, 0),
            off: 0,
            imm: 0,
        },
        // Instruction 7: call bpf_ringbuf_output (helper 6)
        BpfInsn {
            code: 0x85, // CALL
            dst_src: 0x00,
            off: 0,
            imm: 6, // bpf_ringbuf_output
        },
        // Instruction 8: r0 = 0 (return success)
        BpfInsn {
            code: 0xb7,
            dst_src: regs(0, 0),
            off: 0,
            imm: 0,
        },
        // Instruction 9: exit
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        },
    ];

    // ------------------------------------------------------------------
    // Step 3: Load the BPF program via sys_bpf(BPF_PROG_LOAD=5)
    // ------------------------------------------------------------------
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
        attr_size,
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
    // Step 4: Attach the program to the timer via sys_bpf(BPF_PROG_ATTACH=8)
    // ------------------------------------------------------------------
    let attach_attr = BpfAttr {
        attach_btf_id: 1,               // ATTACH_TYPE_TIMER
        attach_prog_fd: prog_id as u32, // program id from load step
        ..Default::default()
    };

    write(1, b"[bpf_loader] Attaching program to timer...\n");

    let attach_res = bpf(
        8, // BPF_PROG_ATTACH
        &attach_attr as *const BpfAttr as *const u8,
        attr_size,
    );

    if attach_res != 0 {
        write(1, b"[bpf_loader] FAILED to attach BPF program (error ");
        print_num((-attach_res) as u64);
        write(1, b")\n");
        exit(1);
    }

    write(
        1,
        b"[bpf_loader] SUCCESS: BPF program attached to timer!\n",
    );
    write(
        1,
        b"[bpf_loader] Polling ringbuf for events...\n",
    );

    // ------------------------------------------------------------------
    // Step 5: Poll the ringbuf in a loop
    //
    // BPF_RINGBUF_POLL (cmd=37) uses BpfAttr fields:
    //   map_fd -> map_id
    //   key    -> buf_ptr (userspace buffer)
    //   value  -> buf_size (buffer capacity)
    //
    // Returns: data length on success, 0 if empty, negative on error
    // ------------------------------------------------------------------
    let mut event_count: u64 = 0;
    let mut buf = [0u8; 64]; // Buffer for event data

    loop {
        let poll_attr = BpfAttr {
            map_fd: map_id as u32,
            key: buf.as_mut_ptr() as u64,
            value: buf.len() as u64,
            ..Default::default()
        };

        let poll_res = bpf(
            37, // BPF_RINGBUF_POLL
            &poll_attr as *const BpfAttr as *const u8,
            attr_size,
        );

        if poll_res > 0 {
            event_count += 1;

            // Parse the 8-byte timestamp from the event data
            if poll_res >= 8 {
                let timestamp = u64::from_ne_bytes([
                    buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
                ]);

                write(1, b"[bpf_loader] Event #");
                print_num(event_count);
                write(1, b": timestamp=");
                print_num(timestamp);
                write(1, b" (");
                print_num(poll_res as u64);
                write(1, b" bytes)\n");
            } else {
                write(1, b"[bpf_loader] Event #");
                print_num(event_count);
                write(1, b": ");
                print_num(poll_res as u64);
                write(1, b" bytes\n");
            }
        } else if poll_res < 0 {
            write(1, b"[bpf_loader] RINGBUF_POLL error: ");
            print_num((-poll_res) as u64);
            write(1, b"\n");
        }

        // Sleep 100ms between polls to avoid busy-waiting
        msleep(100);
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
