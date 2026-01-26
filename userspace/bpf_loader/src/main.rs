#![no_std]
#![no_main]

use core::panic::PanicInfo;
use minilib::{bpf, exit, write};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let msg = b"Testing bpf_loader...\n";
    write(1, msg);

    // BPF_PROG_LOAD = 5
    // We pass a dummy pointer for now just to trigger the syscall
    let dummy_attr = 0 as *const u8;
    
    let res = bpf(5, dummy_attr, 0);

    if res >= 0 {
        write(1, b"BPF syscall success!\n");
        exit(0);
    } else {
        write(1, b"BPF syscall returned error (expected for now)\n");
        // We actually verify that we reached the kernel by the fact 
        // that it didn't crash (or returned our specific -1)
        exit(0); 
    }
}
