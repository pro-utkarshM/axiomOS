#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}
