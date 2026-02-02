use core::slice::from_raw_parts_mut;

use kernel_abi::{EBADF, EINVAL, ERANGE, ESPIPE, Errno};

use crate::access::{CwdAccess, FileAccess};
use crate::ptr::UserspaceMutPtr;

/// Whence values for lseek
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

pub fn sys_getcwd<Cx: CwdAccess>(
    cx: &Cx,
    buf: UserspaceMutPtr<u8>,
    size: usize,
) -> Result<usize, Errno> {
    if buf.as_ptr().is_null() {
        return Err(EINVAL);
    }
    if size == 0 {
        return Err(EINVAL);
    }

    buf.validate_range(size).map_err(|_| EINVAL)?;

    let mut buf = buf;
    // SAFETY: We checked that the buffer pointer is not null and size is non-zero.
    // The UserspaceMutPtr type ensures basic validity, and we trust the syscall caller
    // to provide a valid range for the duration of the call.
    let slice = unsafe { from_raw_parts_mut(buf.as_mut_ptr(), size) };

    let cwd = cx.current_working_directory();
    let guard = cwd.read();
    let bytelen = guard.len();
    if size <= bytelen {
        return Err(ERANGE);
    }
    slice.iter_mut().zip(guard.bytes()).for_each(|(s, b)| {
        *s = b;
    });
    slice[bytelen] = 0; // Null-terminate the string

    Ok(buf.addr())
}

pub fn sys_read<Cx: FileAccess>(cx: &Cx, fildes: Cx::Fd, buf: &mut [u8]) -> Result<usize, Errno> {
    cx.read(fildes, buf).map_err(|_| EINVAL)
}

pub fn sys_write<Cx: FileAccess>(cx: &Cx, fildes: Cx::Fd, buf: &[u8]) -> Result<usize, Errno> {
    cx.write(fildes, buf).map_err(|_| EINVAL)
}

/// Close a file descriptor.
pub fn sys_close<Cx: FileAccess>(cx: &Cx, fildes: Cx::Fd) -> Result<usize, Errno> {
    cx.close(fildes).map_err(|_| EBADF)?;
    Ok(0)
}

/// Reposition read/write file offset.
pub fn sys_lseek<Cx: FileAccess>(
    cx: &Cx,
    fildes: Cx::Fd,
    offset: i64,
    whence: i32,
) -> Result<usize, Errno> {
    cx.lseek(fildes, offset, whence).map_err(|_| ESPIPE)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use kernel_abi::{EINVAL, ERANGE};
    use kernel_vfs::path::AbsoluteOwnedPath;
    use spin::rwlock::RwLock;

    use crate::access::CwdAccess;
    use crate::unistd::sys_getcwd;

    #[test]
    fn test_getcwd() {
        struct Cwd<'a>(&'a RwLock<AbsoluteOwnedPath>);
        impl CwdAccess for Cwd<'_> {
            fn current_working_directory(&self) -> &RwLock<AbsoluteOwnedPath> {
                self.0
            }
        }

        for args in [
            (("/test/path", 0), Err(EINVAL)),
            (("/test/path", 10), Err(ERANGE)),
            (("/test/path", 11), Ok(())),
        ] {
            let ((path, size), expected) = args;
            let cwd = AbsoluteOwnedPath::try_from(path).unwrap().into();
            let access = Cwd(&cwd);
            let mut buf = vec![0u8; size];
            let ptr = buf.as_mut_ptr();
            let res = sys_getcwd(&access, ptr.try_into().unwrap(), buf.len());
            match expected {
                Ok(()) => match res {
                    Ok(addr) => {
                        assert_eq!(addr, ptr as usize);
                        assert_eq!(path.as_bytes(), &buf[..path.len()]);
                        assert_eq!(0, buf[path.len()]);
                    }
                    Err(e) => panic!("failed with {e} but expected success"),
                },
                Err(e) => {
                    assert_eq!(res, Err(e));
                }
            }
        }
    }
}
