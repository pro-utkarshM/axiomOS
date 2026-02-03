#![no_std]

use core::arch::asm;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::_mm_pause;
use core::ffi::c_int;

// --- Syscall Wrappers ---

pub fn syscall0(n: usize) -> usize {
    let mut result: usize = 0;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov rax, {n}",
        "int 0x80",
        "mov {result}, rax",
        n = in(reg) n,
        result = lateout(reg) result,
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            lateout("x0") result,
            options(nostack)
        );
    }
    result
}

pub fn syscall_debug(n: usize) -> usize {
    let r0: usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            lateout("x0") r0,
            options(nostack)
        );
    }
    #[cfg(target_arch = "x86_64")]
    {
        // Debug syscall not implemented for x86_64
        let _ = n;
        r0 = 0;
    }

    r0
}

pub fn syscall1(n: usize, arg1: usize) -> usize {
    let mut result;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov rax,{n}",
        "mov rdi, {arg1}",
        "int 0x80",
        "mov {result}, rax",
        n = in(reg) n,
        arg1 = in(reg) arg1,
        result = lateout(reg) result,
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inout("x0") arg1 => result,
            options(nostack)
        );
    }
    result
}

pub fn syscall2(n: usize, arg1: usize, arg2: usize) -> usize {
    let mut result;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov rax,{n}",
        "mov rdi, {arg1}",
        "mov rsi, {arg2}",
        "int 0x80",
        "mov {result}, rax",
        n = in(reg) n,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        result = lateout(reg) result,
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inout("x0") arg1 => result,
            in("x1") arg2,
            options(nostack)
        );
    }
    result
}

pub fn syscall3(n: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let mut result;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov rax,{n}",
        "mov rdi, {arg1}",
        "mov rsi, {arg2}",
        "mov rdx, {arg3}",
        "int 0x80",
        "mov {result}, rax",
        n = in(reg) n,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        arg3 = in(reg) arg3,
        result = lateout(reg) result,
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inout("x0") arg1 => result,
            in("x1") arg2,
            in("x2") arg3,
            options(nostack)
        );
    }
    result
}

pub fn syscall4(n: usize, arg1: usize, arg2: usize, arg3: usize, arg4: usize) -> usize {
    let mut result;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov rax,{n}",
        "mov rdi, {arg1}",
        "mov rsi, {arg2}",
        "mov rdx, {arg3}",
        "mov rcx, {arg4}",
        "int 0x80",
        "mov {result}, rax",
        n = in(reg) n,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        arg3 = in(reg) arg3,
        arg4 = in(reg) arg4,
        result = lateout(reg) result,
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inout("x0") arg1 => result,
            in("x1") arg2,
            in("x2") arg3,
            in("x3") arg4,
            options(nostack)
        );
    }
    result
}

// --- libc-like functions ---

pub fn exit(code: i32) -> ! {
    syscall1(1, code as usize);
    loop {
        #[cfg(target_arch = "x86_64")]
        _mm_pause();
        #[cfg(target_arch = "aarch64")]
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn read(fd: c_int, buf: &mut [u8]) -> c_int {
    syscall3(36, fd as usize, buf.as_mut_ptr() as usize, buf.len()) as i32
}

pub fn write(fd: c_int, buf: &[u8]) -> c_int {
    syscall3(37, fd as usize, buf.as_ptr() as usize, buf.len()) as i32
}

pub fn bpf(cmd: c_int, attr: *const u8, size: c_int) -> c_int {
    syscall3(50, cmd as usize, attr as usize, size as usize) as i32
}

// --- Time ---

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

pub fn clock_gettime(clock_id: c_int, tp: *mut timespec) -> c_int {
    syscall2(54, clock_id as usize, tp as usize) as i32
}

pub fn nanosleep(req: *const timespec, rem: *mut timespec) -> c_int {
    syscall2(55, req as usize, rem as usize) as i32
}

pub fn sleep(secs: u64) {
    let req = timespec {
        tv_sec: secs as i64,
        tv_nsec: 0,
    };
    nanosleep(&req, core::ptr::null_mut());
}

pub fn msleep(msecs: u64) {
    let req = timespec {
        tv_sec: (msecs / 1000) as i64,
        tv_nsec: ((msecs % 1000) * 1_000_000) as i64,
    };
    nanosleep(&req, core::ptr::null_mut());
}

pub fn pause() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("pause");
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("isb");
    }
}

// --- Filesystem ---

pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

pub fn lseek(fd: c_int, offset: i64, whence: c_int) -> i64 {
    syscall3(39, fd as usize, offset as usize, whence as usize) as i64
}

pub fn close(fd: c_int) -> c_int {
    syscall1(40, fd as usize) as i32
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

pub fn fstat(fd: c_int, buf: *mut stat) -> c_int {
    syscall2(5, fd as usize, buf as usize) as i32
}

pub const O_CREAT: i32 = 1 << 2;
pub const O_RDONLY: i32 = 1 << 16;
pub const O_RDWR: i32 = 1 << 17;
pub const O_WRONLY: i32 = 1 << 19;

pub fn open(path: &str, flags: c_int, mode: c_int) -> c_int {
    syscall4(
        3,
        path.as_ptr() as usize,
        path.len(),
        flags as usize,
        mode as usize,
    ) as i32
}

pub fn spawn(path: &str) -> c_int {
    syscall2(56, path.as_ptr() as usize, path.len()) as i32
}

pub fn abort() -> ! {
    syscall0(32);
    loop {
        // Should not be reached
        unsafe { asm!("nop") };
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct iovec {
    pub iov_base: *const u8,
    pub iov_len: usize,
}

pub fn writev(fd: c_int, iov: &[iovec]) -> c_int {
    syscall3(38, fd as usize, iov.as_ptr() as usize, iov.len()) as i32
}

// --- Memory ---

pub fn malloc(size: usize) -> *mut u8 {
    syscall1(27, size) as *mut u8
}

pub fn free(ptr: *mut u8) {
    syscall1(28, ptr as usize);
}
