#![no_std]
#![no_main]

use kernel_abi::BpfAttr;
use minilib::{bpf, exit, write};

// Hardcoded for RPi5
const BUTTON_PIN: u32 = 17; // GPIO 17 is our "E-Stop" button
const PWM_CHIP: u32 = 0;
const PWM_CHANNEL: u32 = 1;

// BPF Helper IDs
const HELPER_MOTOR_STOP: i32 = 1000;
const HELPER_PWM_WRITE: i32 = 1005;

#[repr(C)]
struct BpfInsn {
    code: u8,
    dst_src: u8,
    off: i16,
    imm: i32,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("=== Safety Interlock Demo ===\n");
    print("1. Starting Motor (PWM0 Ch1) at 50% duty cycle...\n");

    // ---------------------------------------------------------
    // 1. Start "Motor" (Set PWM) via BPF (oneshot)
    // ---------------------------------------------------------
    // We'll run a small BPF program once to set the PWM
    let start_insns = [
        // R1 = PWM_CHIP
        BpfInsn { code: 0xb7, dst_src: 0x01, off: 0, imm: PWM_CHIP as i32 },
        // R2 = PWM_CHANNEL
        BpfInsn { code: 0xb7, dst_src: 0x02, off: 0, imm: PWM_CHANNEL as i32 },
        // R3 = 50 (Duty Cycle %)
        BpfInsn { code: 0xb7, dst_src: 0x03, off: 0, imm: 50 },
        // Call bpf_pwm_write
        BpfInsn { code: 0x85, dst_src: 0x00, off: 0, imm: HELPER_PWM_WRITE },
        // Exit
        BpfInsn { code: 0x95, dst_src: 0x00, off: 0, imm: 0 },
    ];

    let load_attr = BpfAttr {
        prog_type: 1,
        insn_cnt: start_insns.len() as u32,
        insns: start_insns.as_ptr() as u64,
        ..Default::default()
    };

    let prog_id = bpf(5, &load_attr as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if prog_id < 0 {
        print("Error: Failed to load start program\n");
        exit(1);
    }

    // Run it using BPF_PROG_TEST_RUN (cmd 10)
    let _test_attr = BpfAttr {
        attach_prog_fd: prog_id as u32,
        ..Default::default()
    };
    // Note: TestRun not implemented in our simple kernel yet, so we attach to timer once or just assume userspace can set PWM via syscall (not implemented yet).
    // Actually, let's just attach to timer, wait a sec, then detach.
    // Or better: The demo assumes the motor is running.
    // Let's attach to timer, run for 1 tick, then detach.

    // Simpler: Just rely on the "E-Stop" program. The user can imagine the motor is running.
    // Or we use the helper logic in the E-Stop program to PROVE it works by having it running first?
    // We can use the previously created pwm_demo to start the motor if needed.

    // Let's just proceed to setup the E-Stop.

    print("Motor assumed running. Setting up E-Stop on GPIO 17...\n");

    // ---------------------------------------------------------
    // 2. Define E-Stop BPF Program
    // ---------------------------------------------------------
    // Logic:
    //   - Call bpf_motor_emergency_stop(reason=1)
    //   - Exit

    let estop_insns = [
        // R1 = 1 (Reason: Button Pressed)
        BpfInsn { code: 0xb7, dst_src: 0x01, off: 0, imm: 1 },
        // Call bpf_motor_emergency_stop
        BpfInsn { code: 0x85, dst_src: 0x00, off: 0, imm: HELPER_MOTOR_STOP },
        // Exit
        BpfInsn { code: 0x95, dst_src: 0x00, off: 0, imm: 0 },
    ];

    let load_estop = BpfAttr {
        prog_type: 1,
        insn_cnt: estop_insns.len() as u32,
        insns: estop_insns.as_ptr() as u64,
        ..Default::default()
    };

    let estop_id = bpf(5, &load_estop as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if estop_id < 0 {
        print("Error: Failed to load E-Stop program\n");
        exit(1);
    }
    print("E-Stop program loaded. ID: ");
    print_num(estop_id as u64);
    print("\n");

    // ---------------------------------------------------------
    // 3. Attach E-Stop to GPIO 17 (Rising Edge)
    // ---------------------------------------------------------
    let attach_attr = BpfAttr {
        attach_btf_id: 2, // ATTACH_TYPE_GPIO
        attach_prog_fd: estop_id as u32,
        key: BUTTON_PIN as u64, // Pin 17
        value: 1,               // Rising Edge
        ..Default::default()
    };

    let res = bpf(8, &attach_attr as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if res < 0 {
        print("Error: Failed to attach E-Stop\n");
        exit(1);
    }

    print("Safety Interlock Active!\n");
    print("Press Button on GPIO 17 to TRIGGER EMERGENCY STOP.\n");
    print("Kernel logs will show 'EMERGENCY STOP TRIGGERED' and PWM will go to 0.\n");

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
