pub const UIO_MAXIOV: usize = 1024;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct iovec {
    pub iov_base: usize, // *const c_void
    pub iov_len: usize,
}
