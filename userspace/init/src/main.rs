#![no_std]
#![no_main]

use minilib::{exit, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    let bytes = b"hello from init!\n";
    write(1, bytes);
    
    // We can't spawn processes yet in this simplified environment easily without fork/exec syscalls exposed nicely or filesystem.
    // However, we can construct the same syscall test here or rely on the fact that bpf_loader is built.
    // Since I can't easily run a separate binary from init without filesystem support (which is minimal),
    // I will duplicate the test logic here for verification purposes or leave it as is if I can run bpf_loader directly from qemu.
    
    // Assuming the user wants to run `bpf_loader` as the init process or similar.
    // For now, let's add the bpf test call directly to init to verify it works in userspace.
    
    use minilib::bpf;
    write(1, b"Testing BPF syscall from init...\n");
    let res = bpf(5, core::ptr::null(), 0);
    if res == -1 {
         write(1, b"BPF syscall responded correctly with -1 (placeholder)\n");
    } else {
         write(1, b"BPF syscall response unexpected\n");
    }

    exit(0);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    loop {}
}
