#![no_std]
#![no_main]

use core::panic::PanicInfo;

use minilib::{bpf, clock_gettime, exit, msleep, timespec, write};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

#[repr(C)]
struct BpfInsn {
    code: u8,
    dst_src: u8,
    off: i16,
    imm: i32,
}

const fn regs(dst: u8, src: u8) -> u8 {
    (src << 4) | (dst & 0x0f)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write(1, b"\n");
    write(1, b"========================================\n");
    write(1, b"  AXIOM BENCHMARK RESULTS\n");
    write(1, b"========================================\n");
    write(1, b"\n");

    // Note: Boot time and memory footprint are printed by kernel during boot
    // We focus on BPF load time and timer interval measurements here

    let attr_size = core::mem::size_of::<kernel_abi::BpfAttr>() as i32;

    // ================================================================
    // Benchmark 1: BPF Program Load Time
    // ================================================================
    write(1, b"[Benchmark 1] BPF Program Load Time\n");
    write(1, b"Loading test program 10 times...\n");

    // Simple test program: just returns 42
    let test_insns = [
        BpfInsn {
            code: 0xb7,
            dst_src: regs(0, 0),
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

    let mut load_times_us = [0u64; 10];
    let mut min_us = u64::MAX;
    let mut max_us = 0u64;
    let mut total_us = 0u64;

    for (i, time) in load_times_us.iter_mut().enumerate() {
        let mut start = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        clock_gettime(0, &mut start as *mut timespec); // CLOCK_MONOTONIC = 0

        let load_attr = kernel_abi::BpfAttr {
            prog_type: 1,
            insn_cnt: test_insns.len() as u32,
            insns: test_insns.as_ptr() as u64,
            ..Default::default()
        };

        let prog_id = bpf(
            5, // BPF_PROG_LOAD
            &load_attr as *const kernel_abi::BpfAttr as *const u8,
            attr_size,
        );

        let mut end = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        clock_gettime(0, &mut end as *mut timespec);

        if prog_id < 0 {
            write(1, b"  [ERROR] Load failed\n");
            exit(1);
        }

        // Calculate elapsed time in microseconds
        let elapsed_ns =
            (end.tv_sec - start.tv_sec) * 1_000_000_000 + (end.tv_nsec - start.tv_nsec);
        let elapsed_us = (elapsed_ns / 1000) as u64;

        *time = elapsed_us;
        if elapsed_us < min_us {
            min_us = elapsed_us;
        }
        if elapsed_us > max_us {
            max_us = elapsed_us;
        }
        total_us += elapsed_us;

        write(1, b"  Run ");
        print_num((i + 1) as u64);
        write(1, b": ");
        print_num(elapsed_us);
        write(1, b" us\n");
    }

    let avg_us = total_us / 10;

    write(1, b"\nBPF Load Time Summary:\n");
    write(1, b"  Min: ");
    print_num(min_us);
    write(1, b" us\n");
    write(1, b"  Max: ");
    print_num(max_us);
    write(1, b" us\n");
    write(1, b"  Avg: ");
    print_num(avg_us);
    write(1, b" us\n");
    write(1, b"\n");

    // ================================================================
    // Benchmark 2: Timer Interrupt Interval (via BPF)
    // ================================================================
    write(1, b"[Benchmark 2] Timer Interrupt Interval\n");
    write(1, b"Measuring via BPF program attached to timer...\n");

    // Create ringbuf for timestamp events
    let ringbuf_attr = kernel_abi::BpfAttr {
        prog_type: 27, // BPF_MAP_TYPE_RINGBUF
        insn_cnt: 0,
        insns: (4096u64) << 32,
        ..Default::default()
    };

    let ringbuf_map_id = bpf(0, &ringbuf_attr as *const _ as *const u8, attr_size);
    if ringbuf_map_id < 0 {
        write(1, b"  [ERROR] Failed to create ringbuf\n");
        exit(1);
    }

    // BPF program that writes timestamp to ringbuf on each timer tick
    // Uses bpf_ktime_get_ns (helper 1) and bpf_ringbuf_output (helper 6)
    let timer_insns = [
        // Call bpf_ktime_get_ns
        BpfInsn {
            code: 0x85,
            dst_src: 0x00,
            off: 0,
            imm: 1,
        }, // call helper 1
        // r0 now contains timestamp
        // Store timestamp on stack
        BpfInsn {
            code: 0x7b,
            dst_src: regs(10, 0),
            off: -8,
            imm: 0,
        }, // *(u64*)(r10-8) = r0
        // Call bpf_ringbuf_output(map_id, &timestamp, 8, 0)
        BpfInsn {
            code: 0xb7,
            dst_src: regs(1, 0),
            off: 0,
            imm: ringbuf_map_id,
        }, // r1 = map_id
        BpfInsn {
            code: 0xbf,
            dst_src: regs(2, 10),
            off: 0,
            imm: 0,
        }, // r2 = r10
        BpfInsn {
            code: 0x07,
            dst_src: regs(2, 0),
            off: 0,
            imm: -8,
        }, // r2 += -8
        BpfInsn {
            code: 0xb7,
            dst_src: regs(3, 0),
            off: 0,
            imm: 8,
        }, // r3 = 8
        BpfInsn {
            code: 0xb7,
            dst_src: regs(4, 0),
            off: 0,
            imm: 0,
        }, // r4 = 0
        BpfInsn {
            code: 0x85,
            dst_src: 0x00,
            off: 0,
            imm: 6,
        }, // call bpf_ringbuf_output
        BpfInsn {
            code: 0xb7,
            dst_src: regs(0, 0),
            off: 0,
            imm: 0,
        }, // r0 = 0
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        }, // exit
    ];

    let timer_load_attr = kernel_abi::BpfAttr {
        prog_type: 1,
        insn_cnt: timer_insns.len() as u32,
        insns: timer_insns.as_ptr() as u64,
        ..Default::default()
    };

    let timer_prog_id = bpf(5, &timer_load_attr as *const _ as *const u8, attr_size);
    if timer_prog_id < 0 {
        write(1, b"  [ERROR] Failed to load timer program\n");
        exit(1);
    }

    // Attach to timer
    let attach_attr = kernel_abi::BpfAttr {
        attach_btf_id: 1, // ATTACH_TYPE_TIMER
        attach_prog_fd: timer_prog_id as u32,
        ..Default::default()
    };

    let attach_res = bpf(8, &attach_attr as *const _ as *const u8, attr_size);
    if attach_res != 0 {
        write(1, b"  [ERROR] Failed to attach timer program\n");
        exit(1);
    }

    write(1, b"  Collecting 100 timer samples...\n");

    let mut timestamps = [0u64; 100];
    let mut collected = 0;
    let mut buf = [0u8; 64];

    // Collect 100 timestamps
    while collected < 100 {
        let poll_attr = kernel_abi::BpfAttr {
            map_fd: ringbuf_map_id as u32,
            key: buf.as_mut_ptr() as u64,
            value: buf.len() as u64,
            ..Default::default()
        };

        let poll_res = bpf(37, &poll_attr as *const _ as *const u8, attr_size);

        if poll_res >= 8 {
            // Parse timestamp
            let ts = u64::from_ne_bytes([
                buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
            ]);
            timestamps[collected] = ts;
            collected += 1;
        }

        msleep(1); // Brief sleep between polls
    }

    // Calculate intervals between consecutive timestamps
    let mut intervals_us = [0u64; 99];
    let mut min_interval_us = u64::MAX;
    let mut max_interval_us = 0u64;
    let mut total_interval_us = 0u64;

    for i in 0..99 {
        let interval_ns = timestamps[i + 1].saturating_sub(timestamps[i]);
        let interval_us = interval_ns / 1000;
        intervals_us[i] = interval_us;
        if interval_us < min_interval_us {
            min_interval_us = interval_us;
        }
        if interval_us > max_interval_us {
            max_interval_us = interval_us;
        }
        total_interval_us += interval_us;
    }

    let avg_interval_us = total_interval_us / 99;

    write(1, b"\nTimer Interrupt Interval Summary:\n");
    write(1, b"  Min: ");
    print_num(min_interval_us);
    write(1, b" us\n");
    write(1, b"  Max: ");
    print_num(max_interval_us);
    write(1, b" us\n");
    write(1, b"  Avg: ");
    print_num(avg_interval_us);
    write(1, b" us\n");
    write(1, b"\n");

    // ================================================================
    // Summary Table
    // ================================================================
    write(1, b"========================================\n");
    write(1, b"  BENCHMARK SUMMARY\n");
    write(1, b"========================================\n");
    write(1, b"Boot to init:        [see kernel log]\n");
    write(1, b"Kernel memory:       [see kernel log]\n");
    write(1, b"BPF load time:       ");
    print_num(avg_us);
    write(1, b" us (avg of 10)\n");
    write(1, b"Timer interval:      ");
    print_num(avg_interval_us);
    write(1, b" us (avg of 99)\n");
    write(1, b"========================================\n");
    write(1, b"\n");

    exit(0);
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
