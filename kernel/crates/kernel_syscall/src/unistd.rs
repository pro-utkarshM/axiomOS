use core::slice::from_raw_parts_mut;

use kernel_abi::{
    EBADF, EFAULT, EINVAL, EMFILE, ENAMETOOLONG, ENOENT, ERANGE, ESPIPE, Errno, PATH_MAX,
    UIO_MAXIOV, iovec,
};
use kernel_vfs::path::{AbsolutePath, Path};

use crate::access::{CwdAccess, FileAccess};
use crate::ptr::{UserspaceMutPtr, UserspacePtr};

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

pub fn sys_writev<Cx: FileAccess>(
    cx: &Cx,
    fildes: Cx::Fd,
    iov_ptr: UserspacePtr<iovec>,
    iovcnt: usize,
) -> Result<usize, Errno> {
    if iovcnt > UIO_MAXIOV {
        return Err(EINVAL);
    }
    if iovcnt == 0 {
        return Ok(0);
    }

    // Validate iov array range
    iov_ptr.validate_range(iovcnt * core::mem::size_of::<iovec>())?;

    // SAFETY: We validated the range above. We trust the caller to keep the memory valid
    // during the call.
    let iov_slice = unsafe { core::slice::from_raw_parts(iov_ptr.as_ptr(), iovcnt) };

    let mut total_written = 0;

    // We need to copy Fd because we use it in the loop
    // But Fd trait doesn't enforce Copy. However, it is From<c_int> + Into<c_int>.
    // We can convert back and forth if needed, or assume it's cheap to clone if possible.
    // Actually, FileAccess::Fd is an associated type.
    // We can just call write multiple times.
    // But `fildes` is moved if it's not Copy.
    // Let's assume Fd is Copy since it's usually an integer.
    // If not, we have to convert it.
    let fd_int: core::ffi::c_int = fildes.into();

    for iov in iov_slice {
        let base = iov.iov_base;
        let len = iov.iov_len;

        if len == 0 {
            continue;
        }

        // Validate buffer
        let buf_ptr = unsafe { UserspacePtr::<u8>::try_from_usize(base)? };
        buf_ptr.validate_range(len)?;

        // SAFETY: Range validated.
        let buf_slice = unsafe { core::slice::from_raw_parts(base as *const u8, len) };

        // We reconstruct Fd from int each time to satisfy the borrow checker/trait bounds
        // if Fd is not Copy.
        let current_fd = Cx::Fd::from(fd_int);

        let written = cx.write(current_fd, buf_slice).map_err(|_| EINVAL)?;
        total_written += written;

        if written < len {
            // Partial write
            break;
        }
    }

    Ok(total_written)
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

/// Create a pipe.
pub fn sys_pipe<Cx: FileAccess>(cx: &Cx, mut pipefd: UserspaceMutPtr<i32>) -> Result<usize, Errno> {
    pipefd
        .validate_range(2 * core::mem::size_of::<i32>())
        .map_err(|_| EFAULT)?;

    // SAFETY: We validated the range above.
    let slice = unsafe { core::slice::from_raw_parts_mut(pipefd.as_mut_ptr(), 2) };

    let (read_fd, write_fd) = cx.pipe().map_err(|_| EMFILE)?;

    slice[0] = read_fd.into();
    slice[1] = write_fd.into();

    Ok(0)
}

/// Duplicate a file descriptor.
pub fn sys_dup<Cx: FileAccess>(cx: &Cx, oldfd: Cx::Fd) -> Result<usize, Errno> {
    let newfd = cx.dup(oldfd).map_err(|_| EBADF)?;
    Ok(newfd.into() as usize)
}

/// Duplicate a file descriptor to a specific value.
pub fn sys_dup2<Cx: FileAccess>(cx: &Cx, oldfd: Cx::Fd, newfd: Cx::Fd) -> Result<usize, Errno> {
    let res = cx.dup2(oldfd, newfd).map_err(|_| EBADF)?;
    Ok(res.into() as usize)
}

fn resolve_path<Cx: CwdAccess>(
    cx: &Cx,
    path: UserspacePtr<u8>,
    path_len: usize,
) -> Result<alloc::borrow::Cow<'static, AbsolutePath>, Errno> {
    use alloc::borrow::{Cow, ToOwned};

    if path_len > PATH_MAX {
        return Err(ENAMETOOLONG);
    }

    path.validate_range(path_len).map_err(|_| EFAULT)?;

    // SAFETY: We verified path_len is within reasonable limits (PATH_MAX).
    // The pointer comes from a UserspacePtr which we assume points to valid memory
    // for the specified length.
    let path_bytes = unsafe { core::slice::from_raw_parts(path.as_ptr(), path_len) };
    let path_str = core::str::from_utf8(path_bytes).map_err(|_| EINVAL)?;
    let path = Path::new(path_str);

    if let Ok(p) = AbsolutePath::try_new(path) {
        Ok(Cow::Owned(p.to_owned()))
    } else {
        let mut p = cx.current_working_directory().read().clone();
        p.push(path);
        Ok(Cow::Owned(p))
    }
}

pub fn sys_chdir<Cx: CwdAccess>(
    cx: &Cx,
    path: UserspacePtr<u8>,
    path_len: usize,
) -> Result<usize, Errno> {
    let path = resolve_path(cx, path, path_len)?;
    cx.chdir(&path)?;
    Ok(0)
}

pub fn sys_mkdir<Cx: CwdAccess + FileAccess>(
    cx: &Cx,
    path: UserspacePtr<u8>,
    path_len: usize,
    _mode: usize,
) -> Result<usize, Errno> {
    let path = resolve_path(cx, path, path_len)?;
    cx.mkdir(&path).map_err(|_| ENOENT)?; // TODO: Better error mapping from MkdirError
    Ok(0)
}

pub fn sys_rmdir<Cx: CwdAccess + FileAccess>(
    cx: &Cx,
    path: UserspacePtr<u8>,
    path_len: usize,
) -> Result<usize, Errno> {
    let path = resolve_path(cx, path, path_len)?;
    cx.rmdir(&path).map_err(|_| ENOENT)?; // TODO: Better error mapping from RmdirError
    Ok(0)
}

#[cfg(test)]
mod tests {
    use alloc::borrow::ToOwned;
    use alloc::vec;

    use kernel_abi::{Errno, EINVAL, ERANGE};
    use kernel_vfs::path::{AbsoluteOwnedPath, AbsolutePath};
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

            fn chdir(&self, path: &AbsolutePath) -> Result<(), Errno> {
                *self.0.write() = path.to_owned();
                Ok(())
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
