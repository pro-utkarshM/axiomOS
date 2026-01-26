use jiff::Timestamp;

use crate::BOOT_TIME_SECONDS;
#[cfg(target_arch = "x86_64")]
use crate::hpet::hpet;

pub trait TimestampExt {
    fn now() -> Self;
}

#[cfg(target_arch = "x86_64")]
impl TimestampExt for Timestamp {
    fn now() -> Self {
        let counter = hpet().read().main_counter_value();
        let secs = BOOT_TIME_SECONDS.get().unwrap();
        let secs = secs + (counter / 1_000_000_000);
        Timestamp::new(
            i64::try_from(secs).expect("shouldn't have more seconds than i64::MAX"),
            (counter % 1_000_000_000) as i32,
        )
        .unwrap()
    }
}

#[cfg(not(target_arch = "x86_64"))]
impl TimestampExt for Timestamp {
    fn now() -> Self {
        // TODO: Implement proper time handling for aarch64/riscv64
        let secs = BOOT_TIME_SECONDS.get().unwrap();
        Timestamp::new(
            i64::try_from(*secs).expect("shouldn't have more seconds than i64::MAX"),
            0,
        )
        .unwrap()
    }
}

pub fn get_kernel_time_ns() -> u64 {
    let now = Timestamp::now();
    now.as_nanosecond().try_into().unwrap_or(0)
}
