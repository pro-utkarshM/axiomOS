#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::fmt::Write;

// SBI console functions
const SBI_CONSOLE_PUTCHAR: usize = 1;

fn sbi_call(eid: usize, fid: usize, arg0: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            lateout("a0") ret,
        );
    }
    ret
}

fn sbi_console_putchar(c: u8) {
    sbi_call(SBI_CONSOLE_PUTCHAR, 0, c as usize);
}

struct SbiConsole;

impl Write for SbiConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            sbi_console_putchar(byte);
        }
        Ok(())
    }
}

fn print(args: core::fmt::Arguments) {
    let mut console = SbiConsole;
    console.write_fmt(args).unwrap();
}

macro_rules! println {
    ($($arg:tt)*) => {
        print(format_args!($($arg)*));
        print(format_args!("\n"));
    };
}

#[no_mangle]
pub unsafe extern "C" fn _start_rust(hart_id: usize, dtb_addr: usize) -> ! {
    println!("Muffin OS RISC-V Kernel");
    println!("=======================");
    println!("Hart ID: {}", hart_id);
    println!("DTB Address: 0x{:x}", dtb_addr);
    println!("");
    println!("OpenSBI integration successful!");
    println!("Kernel booted via OpenSBI firmware");
    println!("");
    println!("This is a minimal demonstration kernel showing:");
    println!("  - Boot assembly entry point");
    println!("  - OpenSBI SBI calls (console output)");
    println!("  - Device tree blob address capture");
    println!("  - Proper linker script usage");
    println!("");
    println!("Halting...");
    
    loop {
        riscv::asm::wfi();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("PANIC: {}", info);
    loop {
        unsafe { riscv::asm::wfi(); }
    }
}
