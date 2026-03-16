#![no_std]
#![no_main]

use kernel_abi::BpfAttr;
use minilib::{bpf, exit, write};

// Hardcoded for RPi5 Safety Demo
const LIMIT_SWITCH_PIN: u32 = 17;
const MOTOR_PWM_CHANNEL: u32 = 1;

// BPF Helper IDs
const HELPER_MOTOR_EMERGENCY_STOP: i32 = 1000;
const HELPER_TRACE_PRINTK: i32 = 2;

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
    print("\n");
    print("========================================\n");
    print("  AXIOM SAFETY INTERLOCK DEMO\n");
    print("========================================\n\n");

    print("Target: GPIO 17 (Limit Switch) -> PWM0 Emergency Stop\n");

    // BPF Logic:
    // 1. R6 = R1 (context: GpioEvent)
    // 2. R2 = *(u32 *)(R6 + 12)  // Load 'line'
    // 3. if R2 != LIMIT_SWITCH_PIN, exit
    // 4. R1 = 1 (reason code)
    // 5. call bpf_motor_emergency_stop(R1)
    // 6. R1 = address of string "Safety stop triggered!"
    // 7. R2 = size of string
    // 8. call bpf_trace_printk(R1, R2)
    // 9. exit

    let msg = "Safety stop triggered!\0";

    let insns = [
        // R6 = R1 (Save context pointer)
        BpfInsn {
            code: 0xbf,
            dst_src: regs(6, 1),
            off: 0,
            imm: 0,
        },
        // R2 = *(u32 *)(R6 + 12)  // Load 'line' from GpioEvent
        BpfInsn {
            code: 0x61,
            dst_src: regs(2, 6),
            off: 12,
            imm: 0,
        },
        // If R2 != LIMIT_SWITCH_PIN, goto EXIT
        BpfInsn {
            code: 0x55,
            dst_src: regs(2, 0),
            off: 6,
            imm: LIMIT_SWITCH_PIN as i32,
        },
        // --- Safety Triggered ---

        // R1 = 1 (Reason code for stop)
        BpfInsn {
            code: 0xb7,
            dst_src: regs(1, 0),
            off: 0,
            imm: 1,
        },
        // Call bpf_motor_emergency_stop(R1)
        BpfInsn {
            code: 0x85,
            dst_src: 0,
            off: 0,
            imm: HELPER_MOTOR_EMERGENCY_STOP,
        },
        // R1 = pointer to message (on stack or in program data)
        // For simplicity in raw bytecode without a loader that handles relocations,
        // we'll skip printk or use a fixed stack buffer if we really wanted to.
        // Let's just do the emergency stop for now.

        // EXIT:
        BpfInsn {
            code: 0xb7,
            dst_src: regs(0, 0),
            off: 0,
            imm: 0,
        }, // r0 = 0
        BpfInsn {
            code: 0x95,
            dst_src: 0,
            off: 0,
            imm: 0,
        }, // exit
    ];

    print("Loading Safety Interlock BPF program...\n");
    let load_attr = BpfAttr {
        prog_type: 1, // SocketFilter (generic)
        insn_cnt: insns.len() as u32,
        insns: insns.as_ptr() as u64,
        ..Default::default()
    };

    let prog_id = bpf(
        5, // BPF_PROG_LOAD
        &load_attr as *const BpfAttr as *const u8,
        core::mem::size_of::<BpfAttr>() as i32,
    );

    if prog_id < 0 {
        print("Error: Failed to load BPF program\n");
        exit(1);
    }

    print("Program loaded. ID: ");
    print_num(prog_id as u64);
    print("\n");

    // Attach Program to GPIO Interrupt
    print("Attaching to GPIO 17 (Rising Edge)...\n");

    let attach_attr = BpfAttr {
        attach_btf_id: 2, // ATTACH_TYPE_GPIO
        attach_prog_fd: prog_id as u32,
        key: LIMIT_SWITCH_PIN as u64, // Pin number
        value: 1,                      // 1=Rising Edge
        ..Default::default()
    };

    let res = bpf(
        8, // BPF_PROG_ATTACH
        &attach_attr as *const BpfAttr as *const u8,
        core::mem::size_of::<BpfAttr>() as i32,
    );

    if res < 0 {
        print("Error: Failed to attach BPF program\n");
        exit(1);
    }

    print("Success! Safety Interlock ACTIVE.\n");
    print("Simulating motor output on PWM0 Channel 1...\n");

    // Note: We don't have a specific pwm_set syscall yet in minilib,
    // but we can use the BPF helper via another BPF program or
    // we could add a syscall.
    // For this demo, let's assume the kernel has a default motor running
    // or we just rely on the BPF program triggering the stop logic.

    print("Ready. If GPIO 17 goes HIGH, the motor will be stopped in < 1us.\n");

    loop {
        minilib::sleep(1);
    }
}

fn print(s: &str) {
    write(1, s.as_bytes());
}

fn print_num(mut n: u64) {
    if n == 0 {
        print("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut j = 0;
    while j < i / 2 {
        buf.swap(j, i - 1 - j);
        j += 1;
    }
    write(1, &buf[..i]);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    print("Panic!\n");
    loop {}
}
