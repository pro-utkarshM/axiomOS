use jiff::Timestamp;

#[cfg(target_arch = "x86_64")]
use crate::hpet::hpet;
#[cfg(target_arch = "x86_64")]
use crate::BOOT_TIME_SECONDS;

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

#[cfg(target_arch = "aarch64")]
impl TimestampExt for Timestamp {
    fn now() -> Self {
        let counter: u64;
        unsafe { core::arch::asm!("mrs {}, cntvct_el0", out(reg) counter) };
        let freq: u64;
        unsafe { core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq) };
        if freq == 0 {
            return Timestamp::new(0, 0).unwrap();
        }
        let secs = counter / freq;
        let remainder = counter % freq;
        let nsec = (remainder * 1_000_000_000) / freq;
        Timestamp::new(secs as i64, nsec as i32).unwrap()
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl TimestampExt for Timestamp {
    fn now() -> Self {
        Timestamp::new(0, 0).unwrap()
    }
}

pub fn get_kernel_time_ns() -> u64 {
    let now = Timestamp::now();
    now.as_nanosecond().try_into().unwrap_or(0)
}
