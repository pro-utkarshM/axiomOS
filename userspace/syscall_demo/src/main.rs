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

// ATTACH_TYPE_SYSCALL = 5
const ATTACH_TYPE_SYSCALL: u32 = 5;

// SAFETY: Entry point for the Syscall Trace demo.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("=== Syscall Trace Demo ===\n");
    print("Setting up BPF program to monitor syscalls...\n");

    // 1. Define BPF Program
    // Logic:
    //   - Read context (R1 = SyscallTraceContext*)
    //   - Load 'syscall_nr' field (offset 0, u64)
    //   - Return it in R0.
    //   - The kernel is configured to log non-zero return values from SYSCALL hooks.

    let insns = [
        // R0 = *(u64 *)(R1 + 0)  // Load 'syscall_nr' from SyscallTraceContext
        BpfInsn {
            code: 0x79,    // LDX DW (Load 64-bit Word from Memory)
            dst_src: 0x10, // dst=0 (R0), src=1 (R1) -> 0x10
            off: 0,        // offset of 'syscall_nr'
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
        prog_type: 1, // SocketFilter / Generic
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

    // 2. Attach Program to Syscall Entry
    print("Attaching to Global Syscall Hook...\n");

    let attach_attr = BpfAttr {
        attach_btf_id: ATTACH_TYPE_SYSCALL,
        attach_prog_fd: prog_id as u32,
        key: 0,
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

    print("Success! BPF program attached to syscalls.\n");
    print("The kernel will now log every syscall number (except 0) to the debug console.\n");
    print("Running indefinitely... (Press Ctrl-C to exit)\n");

    loop {
        // Trigger some syscalls to generate events
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
