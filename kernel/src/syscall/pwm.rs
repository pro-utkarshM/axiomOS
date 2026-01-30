//! PWM Syscall Implementation

use kernel_abi::syscall::{SYS_PWM_CONFIG, SYS_PWM_ENABLE, SYS_PWM_WRITE};

use crate::arch::aarch64::platform::rpi5::pwm::{PWM0, PWM1};

/// Configure PWM period/frequency
///
/// Arguments:
/// - `pwm_id`: 0 or 1 (PWM controller)
/// - `freq_hz`: Frequency in Hz
pub fn sys_pwm_config(pwm_id: usize, freq_hz: usize) -> isize {
    match pwm_id {
        0 => {
            let mut pwm = PWM0.lock();
            pwm.set_frequency(1, freq_hz as u32);
            pwm.set_frequency(2, freq_hz as u32); // Set both channels to same freq for now
            0
        }
        1 => {
            let mut pwm = PWM1.lock();
            pwm.set_frequency(1, freq_hz as u32);
            pwm.set_frequency(2, freq_hz as u32);
            0
        }
        _ => -1, // Invalid PWM ID
    }
}

/// Set PWM duty cycle
///
/// Arguments:
/// - `pwm_id`: 0 or 1
/// - `channel`: 1 or 2
/// - `duty_percent`: 0-100 (percentage)
pub fn sys_pwm_write(pwm_id: usize, channel: usize, duty_percent: usize) -> isize {
    if channel < 1 || channel > 2 {
        return -1;
    }

    match pwm_id {
        0 => {
            let mut pwm = PWM0.lock();
            pwm.set_duty_cycle(channel as u8, duty_percent as u32);
            0
        }
        1 => {
            let mut pwm = PWM1.lock();
            pwm.set_duty_cycle(channel as u8, duty_percent as u32);
            0
        }
        _ => -1,
    }
}

/// Enable/Disable PWM channel
///
/// Arguments:
/// - `pwm_id`: 0 or 1
/// - `channel`: 1 or 2
/// - `enable`: 0 (disable) or 1 (enable)
pub fn sys_pwm_enable(pwm_id: usize, channel: usize, enable: usize) -> isize {
    if channel < 1 || channel > 2 {
        return -1;
    }

    match pwm_id {
        0 => {
            let mut pwm = PWM0.lock();
            if enable != 0 {
                pwm.enable(channel as u8);
            } else {
                pwm.disable(channel as u8);
            }
            0
        }
        1 => {
            let mut pwm = PWM1.lock();
            if enable != 0 {
                pwm.enable(channel as u8);
            } else {
                pwm.disable(channel as u8);
            }
            0
        }
        _ => -1,
    }
}
