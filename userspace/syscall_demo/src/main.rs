#![no_std]
#![no_main]

use core::panic::PanicInfo;
use minilib::{malloc, free, writev, iovec, exit, abort, write};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Test malloc
    let size = 128;
    let ptr = malloc(size);

    if ptr.is_null() {
        // Malloc failed
        let msg = "Malloc failed\n";
        write(1, msg.as_bytes());
        exit(1);
    }

    // 2. Write to allocated memory
    let slice = unsafe { core::slice::from_raw_parts_mut(ptr, size) };
    let message = "Hello from allocated memory!\n";
    for (i, b) in message.bytes().enumerate() {
        if i < size {
            slice[i] = b;
        }
    }

    // 3. Test writev
    let part1 = "Part 1: ";
    let part2 = "This is a ";
    let part3 = "scatter/gather write.\n";

    let iov = [
        iovec { iov_base: part1.as_ptr(), iov_len: part1.len() },
        iovec { iov_base: part2.as_ptr(), iov_len: part2.len() },
        iovec { iov_base: part3.as_ptr(), iov_len: part3.len() },
        iovec { iov_base: ptr, iov_len: message.len() }, // Print from heap
    ];

    let written = writev(1, &iov);

    if written < 0 {
         let msg = "Writev failed\n";
        write(1, msg.as_bytes());
        exit(1);
    }

    // 4. Test free
    free(ptr);

    let msg = "Memory freed. Exiting successfully.\n";
    write(1, msg.as_bytes());

    // 5. Test abort (uncomment to test, but we want clean exit for CI/checks)
    // abort();

    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort();
}
