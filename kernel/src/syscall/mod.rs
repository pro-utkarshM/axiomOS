use core::ops::Neg;
use core::slice::{from_raw_parts, from_raw_parts_mut};

#[cfg(target_arch = "x86_64")]
use access::KernelAccess;
use kernel_abi::{EINVAL, Errno, syscall_name};
use kernel_syscall::access::FileAccess;
use kernel_syscall::fcntl::sys_open;
use kernel_syscall::mman::sys_mmap;
use kernel_syscall::stat::sys_fstat;
use kernel_syscall::unistd::{sys_close, sys_getcwd, sys_lseek, sys_read, sys_write};
use kernel_syscall::{UserspaceMutPtr, UserspacePtr};
use log::{error, trace};
#[cfg(target_arch = "x86_64")]
use x86_64::instructions::hlt;

#[cfg(not(target_arch = "x86_64"))]
fn hlt() {
    #[cfg(target_arch = "riscv64")]
    // SAFETY: wfi (wait for interrupt) is a privileged instruction that halts the CPU
    // until an interrupt occurs. We are in kernel context with interrupts properly
    // configured, so this is safe to execute.
    unsafe {
        riscv::asm::wfi();
    }
    #[cfg(all(target_arch = "aarch64", feature = "aarch64_arch"))]
    // SAFETY: wfi (wait for interrupt) is a privileged instruction that halts the CPU
    // until an interrupt occurs. We are in kernel context with interrupts properly
    // configured, so this is safe to execute.
    unsafe {
        core::arch::asm!("wfi");
    }
}

#[cfg(target_arch = "x86_64")]
mod access;
pub mod bpf;
#[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
pub mod pwm;
mod validation;

#[must_use]
pub fn dispatch_syscall(
    n: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    arg6: usize,
) -> isize {
    trace!(
        "syscall: {} ({n}) {arg1} {arg2} {arg3} {arg4} {arg5} {arg6}",
        syscall_name(n)
    );

    // Run BPF hooks (AttachType::Syscall = 2) at syscall entry
    #[cfg(target_arch = "x86_64")]
    if let Some(manager) = crate::BPF_MANAGER.get() {
        let ctx = kernel_bpf::execution::BpfContext::empty();
        manager.lock().execute_hooks(2, &ctx);
    }

    let result: Result<usize, Errno> = match n {
        #[cfg(target_arch = "x86_64")]
        kernel_abi::SYS_EXIT => {
            let status = i32::try_from(arg1).unwrap_or(0);
            let task = crate::mcore::context::ExecutionContext::load().current_task();
            let process = task.process();
            *process.exit_code().write() = Some(status);
            task.set_should_terminate(true);
            loop {
                hlt();
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        kernel_abi::SYS_EXIT => {
            error!("SYS_EXIT not implemented for aarch64/riscv64");
            loop {
                hlt();
            }
        }
        kernel_abi::SYS_GETCWD => dispatch_sys_getcwd(arg1, arg2),
        kernel_abi::SYS_MMAP => dispatch_sys_mmap(arg1, arg2, arg3, arg4, arg5, arg6),
        kernel_abi::SYS_OPEN => dispatch_sys_open(arg1, arg2, arg3, arg4),
        kernel_abi::SYS_READ => dispatch_sys_read(arg1, arg2, arg3),
        kernel_abi::SYS_WRITE => dispatch_sys_write(arg1, arg2, arg3),
        kernel_abi::SYS_CLOSE => dispatch_sys_close(arg1),
        kernel_abi::SYS_FSTAT => dispatch_sys_fstat(arg1, arg2),
        kernel_abi::SYS_LSEEK => dispatch_sys_lseek(arg1, arg2, arg3),
        kernel_abi::SYS_BPF => dispatch_sys_bpf(arg1, arg2, arg3),
        #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
        kernel_abi::SYS_PWM_CONFIG => dispatch_sys_pwm_config(arg1, arg2),
        #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
        kernel_abi::SYS_PWM_WRITE => dispatch_sys_pwm_write(arg1, arg2, arg3),
        #[cfg(all(target_arch = "aarch64", feature = "rpi5"))]
        kernel_abi::SYS_PWM_ENABLE => dispatch_sys_pwm_enable(arg1, arg2, arg3),
        _ => {
            error!("unimplemented syscall: {} ({n})", syscall_name(n));
            loop {
                hlt();
            }
        }
    };

    match result {
        Ok(ret) => {
            trace!("syscall {} ({n}) returned {ret}", syscall_name(n));
            ret as isize
        }
        Err(e) => {
            error!("syscall {} ({n}) failed with error: {e:?}", syscall_name(n));
            Into::<isize>::into(e).neg()
        }
    }
}

/// Create a slice from a raw pointer and length.
///
/// # Safety
///
/// The caller must ensure:
/// - `ptr` points to valid, initialized memory for `len` elements of type `T`
/// - The memory is properly aligned for type `T`
/// - The memory remains valid for the lifetime `'a`
/// - No mutable references to the memory exist during the slice's lifetime
unsafe fn slice_from_ptr_and_len<'a, T>(ptr: usize, len: usize) -> Result<&'a [T], Errno> {
    if ptr == 0 || len == 0 {
        return Err(EINVAL);
    }
    // SAFETY: Caller guarantees ptr points to valid memory for len elements of T,
    // is properly aligned, and no mutable references exist. The null check above
    // ensures ptr is non-null.
    let slice = unsafe { from_raw_parts(ptr as *mut T, len) };
    Ok(slice)
}

/// Create a mutable slice from a raw pointer and length.
///
/// # Safety
///
/// The caller must ensure:
/// - `ptr` points to valid, initialized memory for `len` elements of type `T`
/// - The memory is properly aligned for type `T`
/// - The memory remains valid for the lifetime `'a`
/// - No other references (mutable or immutable) to the memory exist
unsafe fn slice_from_ptr_and_len_mut<'a, T>(ptr: usize, len: usize) -> Result<&'a mut [T], Errno> {
    if ptr == 0 || len == 0 {
        return Err(EINVAL);
    }
    // SAFETY: Caller guarantees ptr points to valid memory for len elements of T,
    // is properly aligned, and no other references exist. The null check above
    // ensures ptr is non-null.
    let slice = unsafe { from_raw_parts_mut(ptr as *mut T, len) };
    Ok(slice)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_getcwd(path: usize, size: usize) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    // SAFETY: path comes from userspace syscall arguments. UserspaceMutPtr::try_from_usize
    // validates that the address is in the userspace address range (canonical lower half).
    let path = unsafe { UserspaceMutPtr::try_from_usize(path)? };
    sys_getcwd(&cx, path, size)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_mmap(
    addr: usize,
    len: usize,
    prot: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    // SAFETY: addr comes from userspace syscall arguments. UserspacePtr::try_from_usize
    // validates that the address is in the userspace address range (canonical lower half).
    let addr = unsafe { UserspacePtr::try_from_usize(addr)? };
    let prot = i32::try_from(prot)?;
    let flags = i32::try_from(flags)?;
    let fd = i32::try_from(fd)?;
    sys_mmap(&cx, addr, len, prot, flags, fd, offset)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_open(
    path: usize,
    path_len: usize,
    oflag: usize,
    mode: usize,
) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    // SAFETY: path comes from userspace syscall arguments. UserspacePtr::try_from_usize
    // validates that the address is in the userspace address range (canonical lower half).
    let path = unsafe { UserspacePtr::try_from_usize(path)? };
    sys_open(&cx, path, path_len, oflag as i32, mode as i32)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_read(fd: usize, buf: usize, nbyte: usize) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    let fd = i32::try_from(fd).map_err(|_| EINVAL)?;
    let fd = <KernelAccess as FileAccess>::Fd::from(fd);

    // SAFETY: buf comes from userspace syscall arguments. The slice_from_ptr_and_len_mut
    // function validates that buf is non-null. The caller (userspace) is responsible for
    // ensuring the buffer is valid and writable for nbyte bytes.
    let slice = unsafe { slice_from_ptr_and_len_mut(buf, nbyte) }?;
    sys_read(&cx, fd, slice)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_write(fd: usize, buf: usize, nbyte: usize) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    let fd = i32::try_from(fd).map_err(|_| EINVAL)?;
    let fd = <KernelAccess as FileAccess>::Fd::from(fd);

    // SAFETY: buf comes from userspace syscall arguments. The slice_from_ptr_and_len
    // function validates that buf is non-null. The caller (userspace) is responsible for
    // ensuring the buffer is valid and readable for nbyte bytes.
    let slice = unsafe { slice_from_ptr_and_len(buf, nbyte) }?;
    sys_write(&cx, fd, slice)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_bpf(cmd: usize, attr: usize, size: usize) -> Result<usize, Errno> {
    let ret = bpf::sys_bpf(cmd, attr, size);
    Ok(ret as usize)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_close(fd: usize) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    let fd = i32::try_from(fd).map_err(|_| EINVAL)?;
    let fd = <KernelAccess as FileAccess>::Fd::from(fd);

    sys_close(&cx, fd)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_lseek(fd: usize, offset: usize, whence: usize) -> Result<usize, Errno> {
    let cx = KernelAccess::new();

    let fd = i32::try_from(fd).map_err(|_| EINVAL)?;
    let fd = <KernelAccess as FileAccess>::Fd::from(fd);
    let offset = offset as i64;
    let whence = i32::try_from(whence).map_err(|_| EINVAL)?;

    sys_lseek(&cx, fd, offset, whence)
}

#[cfg(target_arch = "x86_64")]
fn dispatch_sys_fstat(fd: usize, statbuf: usize) -> Result<usize, Errno> {
    use kernel_syscall::stat::UserStat;

    let cx = KernelAccess::new();

    let fd = i32::try_from(fd).map_err(|_| EINVAL)?;
    let fd = <KernelAccess as FileAccess>::Fd::from(fd);
    // SAFETY: statbuf comes from userspace syscall arguments. UserspaceMutPtr::try_from_usize
    // validates that the address is in the userspace address range (canonical lower half).
    // The caller (userspace) is responsible for providing a valid, writable buffer.
    let buf = unsafe { UserspaceMutPtr::<UserStat>::try_from_usize(statbuf)? };

    sys_fstat::<KernelAccess>(&cx, fd, buf)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_getcwd(_path: usize, _size: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_mmap(
    _addr: usize,
    _len: usize,
    _prot: usize,
    _flags: usize,
    _fd: usize,
    _offset: usize,
) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_open(
    _path: usize,
    _path_len: usize,
    _oflag: usize,
    _mode: usize,
) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_read(_fd: usize, _buf: usize, _nbyte: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_write(_fd: usize, _buf: usize, _nbyte: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_bpf(_cmd: usize, _attr: usize, _size: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_close(_fd: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_lseek(_fd: usize, _offset: usize, _whence: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

#[cfg(not(target_arch = "x86_64"))]
fn dispatch_sys_fstat(_fd: usize, _statbuf: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}
