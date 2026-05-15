//! ALU result computation for the verifier.
//!
//! `compute_alu_result(dst, op, rhs)` takes the verifier's view of the
//! destination register (`ScalarValue` with interval + tnum) and the
//! right-hand side (another register's `ScalarValue` for reg-mode, or a
//! constant `ScalarValue` for imm-mode) and returns the new `ScalarValue`
//! after the operation.
//!
//! This is the per-instruction precision lift wired into
//! [`super::core::Verifier::verify_alu`]. Without it the verifier collapses
//! `dst` to `ScalarValue::unknown()` after every ALU op, which loses every
//! bit of information the verifier had built up — including the bit-level
//! tnum that downstream checks (range refinement on conditionals, memory
//! bounds, state-pruning subsumption) want to consume.
//!
//! Tnum operations consumed here live on `TnumValue` (state.rs); interval
//! arithmetic is implemented inline since the range tracking is simple
//! enough that an extra module would be ceremony.
//!
//! Reference: Linux `kernel/bpf/verifier.c::adjust_scalar_min_max_vals`.
//! The strategy and arm-by-arm split are the same; the Rust types differ.

use super::state::{ScalarValue, TnumValue};
use crate::bytecode::opcode::AluOp;

/// Compute the new ScalarValue after `dst = op(dst, rhs)`.
///
/// Falls back to `ScalarValue::unknown()` for operations the lattice can't
/// represent precisely (e.g. integer division by a range, byte swap).
/// Conservative: any imprecision widens to fully unknown so the existing
/// memory-bounds / safety checks downstream see the widest reachable value.
pub fn compute_alu_result(dst: ScalarValue, op: AluOp, rhs: ScalarValue) -> ScalarValue {
    match op {
        AluOp::Mov => mov(rhs),
        AluOp::Add => add(dst, rhs),
        AluOp::Sub => sub(dst, rhs),
        AluOp::Mul => mul(dst, rhs),
        AluOp::Div => div_unsigned(dst, rhs),
        AluOp::Mod => mod_unsigned(dst, rhs),
        AluOp::And => bitwise_and(dst, rhs),
        AluOp::Or => bitwise_or(dst, rhs),
        AluOp::Xor => bitwise_xor(dst, rhs),
        AluOp::Lsh => lshift(dst, rhs),
        AluOp::Rsh => rshift(dst, rhs),
        AluOp::Arsh => arshift(dst, rhs),
        AluOp::Neg => negate(dst),
        AluOp::End => ScalarValue::unknown(),
    }
}

/// Build a `ScalarValue` for an immediate operand.
///
/// 32-bit immediates are sign-extended to 64 bits by the BPF spec.
pub fn scalar_from_imm(imm: i32) -> ScalarValue {
    let v = imm as i64 as u64;
    ScalarValue {
        value: Some(v),
        min: v,
        max: v,
        tnum: TnumValue::constant(v),
    }
}

fn mov(rhs: ScalarValue) -> ScalarValue {
    rhs
}

fn add(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let (min, min_overflow) = a.min.overflowing_add(b.min);
    let (max, max_overflow) = a.max.overflowing_add(b.max);
    let interval_ok = !min_overflow && !max_overflow;

    let tnum = a.tnum.add(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x.wrapping_add(y)),
        _ => None,
    };

    if interval_ok {
        ScalarValue {
            value,
            min,
            max,
            tnum,
        }
    } else {
        // Overflowed the unsigned interval — keep tnum (still sound) but
        // widen the interval to the full range so downstream checks don't
        // rely on a wrong bound.
        ScalarValue {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum,
        }
    }
}

fn sub(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let (min, min_under) = a.min.overflowing_sub(b.max);
    let (max, max_under) = a.max.overflowing_sub(b.min);
    let interval_ok = !min_under && !max_under;

    let tnum = a.tnum.sub(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x.wrapping_sub(y)),
        _ => None,
    };

    if interval_ok {
        ScalarValue {
            value,
            min,
            max,
            tnum,
        }
    } else {
        ScalarValue {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum,
        }
    }
}

fn mul(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let tnum = a.tnum.mul(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x.wrapping_mul(y)),
        _ => None,
    };

    // Interval: product is bounded by min*min..=max*max, but overflows
    // collapse to the full range. The cheap-and-correct thing is the
    // overflow-aware path.
    let (lo, lo_ovf) = a.min.overflowing_mul(b.min);
    let (hi, hi_ovf) = a.max.overflowing_mul(b.max);

    if !lo_ovf && !hi_ovf {
        ScalarValue {
            value,
            min: lo,
            max: hi,
            tnum,
        }
    } else {
        ScalarValue {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum,
        }
    }
}

fn div_unsigned(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    // Division by zero is already rejected upstream in verify_alu. Here we
    // can assume b > 0. Result is bounded by a.max / max(1, b.min).
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => x.checked_div(y),
        _ => None,
    };

    // b.min == 0 but verifier proved b is nonzero — widen conservatively.
    let max = a.max.checked_div(b.min).unwrap_or(a.max);
    let min = a.min.checked_div(b.max).unwrap_or(0);

    ScalarValue {
        value,
        min,
        max,
        tnum: TnumValue::unknown(),
    }
}

fn mod_unsigned(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    // a % b ∈ [0, b - 1] when b > 0.
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => x.checked_rem(y),
        _ => None,
    };

    let max = if b.max == 0 {
        0
    } else {
        b.max.saturating_sub(1).min(a.max)
    };

    ScalarValue {
        value,
        min: 0,
        max,
        tnum: TnumValue::unknown(),
    }
}

fn bitwise_and(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let tnum = a.tnum.and(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x & y),
        _ => None,
    };
    // AND can only clear bits → result is bounded by min(a.max, b.max).
    let max = a.max.min(b.max);
    ScalarValue {
        value,
        min: 0,
        max,
        tnum,
    }
}

fn bitwise_or(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let tnum = a.tnum.or(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x | y),
        _ => None,
    };
    // OR can only set bits → result is at least max(a.min, b.min).
    let min = a.min.max(b.min);
    // Upper bound conservatively widens to u64::MAX since OR of two
    // unconstrained values can land anywhere; the tnum carries the real
    // precision here.
    ScalarValue {
        value,
        min,
        max: u64::MAX,
        tnum,
    }
}

fn bitwise_xor(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    let tnum = a.tnum.xor(b.tnum);
    let value = match (a.value, b.value) {
        (Some(x), Some(y)) => Some(x ^ y),
        _ => None,
    };
    // XOR doesn't constrain the interval beyond the entry range; the tnum
    // is where precision lives for XOR.
    ScalarValue {
        value,
        min: 0,
        max: u64::MAX,
        tnum,
    }
}

fn lshift(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    // Shifts by non-constant amounts conservatively widen; constant
    // shifts use the tnum lshift directly.
    if let Some(shift) = b.value {
        let s = (shift as u32 & 63) as u8;
        let tnum = a.tnum.lshift(s);
        let value = a.value.map(|v| v << s);
        let (min, min_ovf) = a.min.overflowing_shl(s as u32);
        let (max, max_ovf) = a.max.overflowing_shl(s as u32);
        if !min_ovf && !max_ovf {
            return ScalarValue {
                value,
                min,
                max,
                tnum,
            };
        }
        return ScalarValue {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum,
        };
    }
    ScalarValue {
        value: None,
        min: 0,
        max: u64::MAX,
        tnum: TnumValue::unknown(),
    }
}

fn rshift(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    if let Some(shift) = b.value {
        let s = (shift as u32 & 63) as u8;
        let tnum = a.tnum.rshift(s);
        let value = a.value.map(|v| v >> s);
        ScalarValue {
            value,
            min: a.min >> s,
            max: a.max >> s,
            tnum,
        }
    } else {
        ScalarValue {
            value: None,
            min: 0,
            max: u64::MAX,
            tnum: TnumValue::unknown(),
        }
    }
}

fn arshift(a: ScalarValue, b: ScalarValue) -> ScalarValue {
    if let Some(shift) = b.value {
        let s = (shift as u32 & 63) as u8;
        let tnum = a.tnum.arshift(s);
        let value = a.value.map(|v| (v as i64 >> s) as u64);
        // Signed shift can produce values across the full range when sign
        // bit is unknown. Widen interval to be safe.
        ScalarValue {
            value,
            min: 0,
            max: u64::MAX,
            tnum,
        }
    } else {
        ScalarValue::unknown()
    }
}

fn negate(a: ScalarValue) -> ScalarValue {
    let value = a.value.map(|v| v.wrapping_neg());
    ScalarValue {
        value,
        min: 0,
        max: u64::MAX,
        tnum: TnumValue::unknown(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::opcode::AluOp;

    fn unknown_in(min: u64, max: u64) -> ScalarValue {
        ScalarValue {
            value: None,
            min,
            max,
            tnum: TnumValue::unknown(),
        }
    }

    #[test]
    fn mov_passes_rhs_through() {
        let r = compute_alu_result(
            ScalarValue::constant(7),
            AluOp::Mov,
            ScalarValue::constant(42),
        );
        assert_eq!(r.value, Some(42));
        assert_eq!(r.min, 42);
        assert_eq!(r.max, 42);
    }

    #[test]
    fn add_constants() {
        let r = compute_alu_result(
            ScalarValue::constant(10),
            AluOp::Add,
            ScalarValue::constant(5),
        );
        assert_eq!(r.value, Some(15));
        assert_eq!(r.min, 15);
        assert_eq!(r.max, 15);
    }

    #[test]
    fn add_unknown_widens_interval_correctly() {
        let r = compute_alu_result(unknown_in(0, 10), AluOp::Add, ScalarValue::constant(5));
        assert_eq!(r.min, 5);
        assert_eq!(r.max, 15);
    }

    #[test]
    fn and_caps_max_by_mask() {
        // dst = unknown; rhs = 0xff (constant)
        // result.max ≤ 0xff
        let r = compute_alu_result(
            ScalarValue::unknown(),
            AluOp::And,
            ScalarValue::constant(0xff),
        );
        assert_eq!(r.max, 0xff);
        assert_eq!(r.min, 0);
        assert_eq!(r.tnum.mask, 0xff);
    }

    #[test]
    fn and_then_add_one_carries_precision() {
        // r1 = unknown; r1 &= 0xff; r2 = r1 + 1
        // Expect r2.max = 256, r2.min = 1
        let r1 = compute_alu_result(
            ScalarValue::unknown(),
            AluOp::And,
            ScalarValue::constant(0xff),
        );
        let r2 = compute_alu_result(r1, AluOp::Add, ScalarValue::constant(1));
        assert_eq!(r2.min, 1);
        assert_eq!(r2.max, 256);
    }

    #[test]
    fn sub_constants() {
        let r = compute_alu_result(
            ScalarValue::constant(20),
            AluOp::Sub,
            ScalarValue::constant(8),
        );
        assert_eq!(r.value, Some(12));
    }

    #[test]
    fn sub_underflow_widens() {
        let r = compute_alu_result(unknown_in(0, 5), AluOp::Sub, unknown_in(0, 10));
        assert_eq!(r.min, 0);
        assert_eq!(r.max, u64::MAX);
    }

    #[test]
    fn or_lifts_min_by_known_bits() {
        let r = compute_alu_result(
            ScalarValue::constant(0),
            AluOp::Or,
            ScalarValue::constant(0xff),
        );
        assert_eq!(r.value, Some(0xff));
    }

    #[test]
    fn xor_constants() {
        let r = compute_alu_result(
            ScalarValue::constant(0xaaaa),
            AluOp::Xor,
            ScalarValue::constant(0xff00),
        );
        assert_eq!(r.value, Some(0xaaaa ^ 0xff00));
    }

    #[test]
    fn lshift_constant_shifts_interval() {
        let r = compute_alu_result(unknown_in(0, 0xff), AluOp::Lsh, ScalarValue::constant(8));
        assert_eq!(r.min, 0);
        assert_eq!(r.max, 0xff00);
        // tnum low 8 bits should be known zero.
        assert!(r.tnum.contains(0));
        assert!(r.tnum.contains(0xff00));
        assert!(!r.tnum.contains(0x01));
    }

    #[test]
    fn rshift_constant_shifts_interval() {
        let r = compute_alu_result(unknown_in(0, 0xff00), AluOp::Rsh, ScalarValue::constant(8));
        assert_eq!(r.min, 0);
        assert_eq!(r.max, 0xff);
    }

    #[test]
    fn div_caps_by_divisor() {
        // dst ∈ [0, 100], rhs = 5 → result ∈ [0, 20]
        let r = compute_alu_result(unknown_in(0, 100), AluOp::Div, ScalarValue::constant(5));
        assert_eq!(r.max, 20);
        assert_eq!(r.min, 0);
    }

    #[test]
    fn mod_bounded_by_divisor_minus_one() {
        // dst ∈ [0, 1000], rhs = 256 → result ∈ [0, 255]
        let r = compute_alu_result(unknown_in(0, 1000), AluOp::Mod, ScalarValue::constant(256));
        assert_eq!(r.max, 255);
        assert_eq!(r.min, 0);
    }

    #[test]
    fn mul_constants() {
        let r = compute_alu_result(
            ScalarValue::constant(6),
            AluOp::Mul,
            ScalarValue::constant(7),
        );
        assert_eq!(r.value, Some(42));
    }

    #[test]
    fn scalar_from_imm_sign_extends() {
        // i32 -1 should become u64::MAX as a sign-extended scalar.
        let r = scalar_from_imm(-1);
        assert_eq!(r.value, Some(u64::MAX));
    }
}
