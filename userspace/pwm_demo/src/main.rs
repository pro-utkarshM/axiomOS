#![no_std]
#![no_main]

use kernel_abi::BpfAttr;
use minilib::{bpf, exit, write};

// Helper IDs
const HELPER_PWM_WRITE: i32 = 1005;

// Attach Types
const ATTACH_TYPE_TIMER: u32 = 1;
const ATTACH_TYPE_PWM: u32 = 3;

#[repr(C)]
struct BpfInsn {
    code: u8,
    dst_src: u8,
    off: i16,
    imm: i32,
}

#[allow(dead_code)]
impl BpfInsn {
    const fn mov64_imm(dst: u8, imm: i32) -> Self {
        Self {
            code: 0xb7,
            dst_src: dst,
            off: 0,
            imm,
        }
    }

    const fn mov64_reg(dst: u8, src: u8) -> Self {
        Self {
            code: 0xbf,
            dst_src: (src << 4) | dst,
            off: 0,
            imm: 0,
        }
    }

    const fn ld_abs_w(imm: i32) -> Self {
        Self {
            code: 0x20,
            dst_src: 0,
            off: 0,
            imm,
        }
    }

    const fn ldx_w(dst: u8, src: u8, off: i16) -> Self {
        Self {
            code: 0x61,
            dst_src: (src << 4) | dst,
            off,
            imm: 0,
        }
    }

    const fn call(imm: i32) -> Self {
        Self {
            code: 0x85,
            dst_src: 0,
            off: 0,
            imm,
        }
    }

    const fn exit() -> Self {
        Self {
            code: 0x95,
            dst_src: 0,
            off: 0,
            imm: 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("=== PWM Demo ===\n");

    // ---------------------------------------------------------
    // 1. Load PWM Observer Program
    // ---------------------------------------------------------
    print("Loading PWM Observer program...\n");

    // Logic:
    // PwmEvent context is at R1
    // struct PwmEvent {
    //    timestamp: u64, // 0
    //    chip_id: u32,   // 8
    //    channel: u32,   // 12
    //    period_ns: u32, // 16
    //    duty_ns: u32,   // 20
    //    polarity: u32,  // 24
    //    enabled: u32,   // 28
    // }

    let observer_insns = [
        // R6 = R1 (Save context)
        BpfInsn::mov64_reg(6, 1),

        // R1 = fmt_str pointer (stack) - simplifying by putting it on stack or using map
        // For simplicity in this demo without map strings, let's just print fixed values or use simple trace_printk
        // Note: trace_printk in our kernel takes (fmt, size) and verifies fmt is in RO memory.
        // We can't easily pass a pointer to our stack/rodata from userspace unless we use a map.
        // BUT, our JIT/Interpreter implementation of trace_printk:
        //    fn bpf_trace_printk(fmt: *const u8, size: u32) -> i32;
        // In the interpreter, it reads the pointer. If the pointer comes from the BPF program (e.g. stack), it needs to be valid.

        // Let's use a simpler approach: Just read the values and if duty > 50%, write to GPIO (LED) just to see something?
        // Or assume trace_printk works if we pass a pointer that the kernel can resolve?
        // Actually, the verifier checks trace_printk arguments.

        // Let's try to just print the duty cycle using trace_printk if possible,
        // or just accept that we can't easily print strings without map support for strings yet.

        // Alternative: Write to a map (PerfEvent/RingBuf) and have userspace read it.
        // We have `bpf_ringbuf_output`.

        // Let's use RingBuf.

        // But first, let's make the observer extremely simple:
        // Read duty_ns (offset 20)
        // If duty_ns > 500000 (50%), return 1, else 0.
        // The return value is logged by execute_hooks in bpf/mod.rs if attach_type == PWM?
        // mod.rs:
        // if attach_type == ATTACH_TYPE_IIO { log... }
        // else if attach_type == ATTACH_TYPE_SYSCALL { log... }
        // It doesn't log PWM results.

        // Okay, so we need a side effect.
        // Let's toggle the LED (GPIO 18) if duty cycle is high.
        // Helper 1003: bpf_gpio_write(pin, value)

        // R1 = 18 (LED Pin)
        BpfInsn::mov64_imm(1, 18),

        // R2 = *(u32 *)(R6 + 20) (duty_ns)
        BpfInsn::ldx_w(2, 6, 20),

        // R3 = 0 (Default value)
        BpfInsn::mov64_imm(3, 0),

        // if R2 > 50 (let's say 50% of 100), R3 = 1
        // Wait, duty is in ns.
        // Let's just say if duty_ns > 0, turn LED on.
        // if R2 > 0 goto SET_ON
        BpfInsn { code: 0x2d, dst_src: 0x20, off: 1, imm: 0 }, // JGT R2 > 0, +1
        BpfInsn { code: 0x05, dst_src: 0, off: 1, imm: 0 },    // JA +1
        // SET_ON:
        BpfInsn::mov64_imm(3, 1),

        // R2 = R3 (Value to write)
        BpfInsn::mov64_reg(2, 3),

        // Call bpf_gpio_write(18, value)
        BpfInsn::call(HELPER_PWM_WRITE - 2), // 1005 - 2 = 1003 (GPIO_WRITE)

        BpfInsn::mov64_imm(0, 0),
        BpfInsn::exit(),
    ];

    let load_attr = BpfAttr {
        prog_type: 1,
        insn_cnt: observer_insns.len() as u32,
        insns: observer_insns.as_ptr() as u64,
        ..Default::default()
    };

    let obs_id = bpf(5, &load_attr as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if obs_id < 0 {
        print("Error: Failed to load observer\n");
        exit(1);
    }
    print("Observer loaded. ID: ");
    print_num(obs_id as u64);
    print("\n");

    // ---------------------------------------------------------
    // 2. Attach Observer to PWM
    // ---------------------------------------------------------
    print("Attaching Observer to PWM0 Channel 1...\n");
    let attach_attr = BpfAttr {
        attach_btf_id: ATTACH_TYPE_PWM,
        attach_prog_fd: obs_id as u32,
        key: 0, // Chip 0
        value: 1, // Channel 1
        ..Default::default()
    };
    let res = bpf(8, &attach_attr as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if res < 0 {
        print("Error: Failed to attach observer\n");
        exit(1);
    }

    // ---------------------------------------------------------
    // 3. Load Controller Program (Timer)
    // ---------------------------------------------------------
    print("Loading Controller program...\n");

    // Logic:
    // static int counter = 0;
    // counter++;
    // int duty = counter % 100;
    // bpf_pwm_write(0, 1, duty);

    // Since we don't have maps easily in this raw assembly demo for state,
    // we can read the random number or time to vary the duty cycle.
    // R1 = bpf_ktime_get_ns()

    let ctrl_insns = [
        // R0 = bpf_ktime_get_ns()
        BpfInsn::call(1),

        // R1 = R0
        BpfInsn::mov64_reg(1, 0),

        // R1 = R1 / 100000000 (Reduce to slower changing value)
        // Division is hard in BPF assembly without registers.
        // Let's just mask it.
        // R1 = R1 & 127
        BpfInsn { code: 0x47, dst_src: 0, off: 0, imm: 127 }, // AND R1, 127

        // If R1 > 100, R1 = 100
        BpfInsn { code: 0x25, dst_src: 0x01, off: 1, imm: 100 }, // JGT R1 > 100, +1
        BpfInsn { code: 0x05, dst_src: 0, off: 1, imm: 0 },      // JA +1
        BpfInsn::mov64_imm(1, 100),

        // R3 = R1 (Duty %)
        BpfInsn::mov64_reg(3, 1),

        // R1 = 0 (PWM Chip)
        BpfInsn::mov64_imm(1, 0),

        // R2 = 1 (Channel)
        BpfInsn::mov64_imm(2, 1),

        // Call bpf_pwm_write(0, 1, duty)
        BpfInsn::call(HELPER_PWM_WRITE),

        BpfInsn::mov64_imm(0, 0),
        BpfInsn::exit(),
    ];

    let load_ctrl = BpfAttr {
        prog_type: 1,
        insn_cnt: ctrl_insns.len() as u32,
        insns: ctrl_insns.as_ptr() as u64,
        ..Default::default()
    };

    let ctrl_id = bpf(5, &load_ctrl as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if ctrl_id < 0 {
        print("Error: Failed to load controller\n");
        exit(1);
    }
    print("Controller loaded. ID: ");
    print_num(ctrl_id as u64);
    print("\n");

    // ---------------------------------------------------------
    // 4. Attach Controller to Timer
    // ---------------------------------------------------------
    print("Attaching Controller to Timer...\n");
    let attach_ctrl = BpfAttr {
        attach_btf_id: ATTACH_TYPE_TIMER,
        attach_prog_fd: ctrl_id as u32,
        ..Default::default()
    };
    let res = bpf(8, &attach_ctrl as *const _ as *const u8, core::mem::size_of::<BpfAttr>() as i32);
    if res < 0 {
        print("Error: Failed to attach controller\n");
        exit(1);
    }

    print("Success! Running. Check LED on GPIO 18.\n");

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
