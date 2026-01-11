use x86_64::instructions::port::Port;

pub fn shutdown() -> ! {
    let mut port = Port::new(0xf4);
    unsafe {
        port.write(0x00_u32);
    }
    loop {
        x86_64::instructions::hlt();
    }
}
