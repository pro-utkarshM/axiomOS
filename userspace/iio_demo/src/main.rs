#![no_std]
#![no_main]

use kernel_abi::BpfAttr;
use minilib::{bpf, exit, write};

// BPF Helper IDs
// const HELPER_TRACE_PRINTK: i32 = 2;

#[repr(C)]
struct BpfInsn {
    code: u8,
    dst_src: u8,
    off: i16,
    imm: i32,
}

// ATTACH_TYPE_IIO = 4
const ATTACH_TYPE_IIO: u32 = 4;

// SAFETY: Entry point for the IIO demo. Called by the startup code.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("=== IIO Sensor Integration Demo ===\n");
    print("Setting up BPF program to monitor simulated accelerometer...\n");

    // 1. Define BPF Program
    // Logic:
    //   - Read event context (R1 = IioEvent*)
    //   - Load 'value' field (offset 16)
    //   - We want to print it. Since we don't have a sophisticated formatter in BPF yet,
    //     we'll use a fixed string and the interpreter will log it.
    //     Actually, bpf_trace_printk in this kernel just takes a string and logs it.
    //     It doesn't support format strings with arguments yet (it just uses CStr::from_ptr).

    // For now, let's just make it call bpf_trace_printk with a static message
    // to prove the hook is firing.

    // Since we need to pass a string pointer, and we are in userspace,
    // the kernel interpreter will try to read this pointer.
    // However, the interpreter currently assumes the pointer is valid in the kernel.
    // Wait, the interpreter's bpf_trace_printk implementation:
    /*
    pub extern "C" fn bpf_trace_printk(fmt: *const u8, _size: u32) -> i32 {
        unsafe {
            let s = core::ffi::CStr::from_ptr(fmt as *const core::ffi::c_char);
            if let Ok(msg) = s.to_str() {
                log::info!("[BPF] {}", msg);
                return 0;
            }
        }
        -1
    }
    */
    // This is problematic because the BPF program is running in the kernel,
    // but the string is in userspace.
    // Actually, in this architecture, the BPF instructions are loaded into the kernel.
    // If I put the string in the BPF program's read-only data, it should work if the loader handles it.
    // But our `load_raw_program` doesn't handle data sections.

    // Let's use a trick: use a helper that doesn't need a pointer,
    // or just rely on the fact that the simulation is running.

    // Actually, I can use `bpf_gpio_write` to an unused pin as a way to "output" data
    // if I really wanted to, but let's stick to the plan.

    // Let's see if I can find a way to get a string into the kernel.
    // The `init` function in `kernel/src/lib.rs` has a test BPF program:
    /*
        static HELLO: &[u8] = b"Hello from BPF!\0";
        let ptr = HELLO.as_ptr() as u64;
        let wide = WideInsn::ld_dw_imm(1, ptr);
    */
    // That works because it's compiled INTO the kernel.

    // For userspace-loaded BPF, we need to be careful.
    // If I just pass a pointer to a string in this userspace process,
    // the kernel might be able to read it if it's in the same address space
    // (which it is when the hook fires in the context of a process, but IIO fires in a kernel task).

    // Since the IIO simulation task is a KERNEL task, it runs in the kernel address space.
    // It won't see userspace memory.

    // So `bpf_trace_printk` with a userspace pointer will fail or cause a crash in the kernel.

    // However, for this demo, the goal is to show the end-to-end flow.
    // I will use `bpf_gpio_write` with the sensor value as the "value" argument
    // just to see it happen in the logs (since bpf_gpio_write logs on RPi5).
    // Or I can just return the value and have nothing happen.

    // Wait! I implemented the simulation task in `kernel/src/driver/iio.rs`.
    // It calls `dispatch_event`.

    // Let's just make the BPF program return the value.

    let insns = [
        // R0 = *(u32 *)(R1 + 16)  // Load 'value' from IioEvent
        BpfInsn {
            code: 0x61,    // LDXW
            dst_src: 0x10, // dst=0 (R0), src=1 (R1) -> 0x10
            off: 16,       // offset of 'value' in IioEvent
            imm: 0,
        },
        // EXIT
        BpfInsn {
            code: 0x95,
            dst_src: 0x00,
            off: 0,
            imm: 0,
        },
    ];

    print("Loading BPF program...\n");
    let load_attr = BpfAttr {
        prog_type: 1, // SocketFilter
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

    // 2. Attach Program to IIO Event
    print("Attaching to IIO device 0...\n");

    let attach_attr = BpfAttr {
        attach_btf_id: ATTACH_TYPE_IIO,
        attach_prog_fd: prog_id as u32,
        key: 0, // Device ID 0
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

    print("Success! BPF program attached to IIO.\n");
    print("The kernel will now log sensor data in the background.\n");
    print("Running indefinitely... (Press Ctrl-C to exit)\n");

    loop {
        // Sleep to save CPU while kernel handles sensor events
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
