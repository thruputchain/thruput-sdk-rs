use core::hint::black_box;
use thruput_sdk::*;

pub struct IntDivision;

/// The operation a `kind` argument selects.
pub const UNSIGNED_QUOTIENT: u32 = 0;
pub const UNSIGNED_REMAINDER: u32 = 1;
pub const SIGNED_QUOTIENT: u32 = 2;
pub const SIGNED_REMAINDER: u32 = 3;

/// Applies `f` to the operands `rounds` times and returns the last result.
///
/// `black_box` on the operands and the result keeps every round's division
/// in the loop. The difference in gas between two round counts is then the
/// cost of the extra divisions.
#[inline(always)]
fn repeat<T: Copy>(rounds: u32, a: T, b: T, f: impl Fn(T, T) -> T) -> T {
    let mut last = a;
    let mut i = 0;
    while i < rounds {
        last = black_box(f(black_box(a), black_box(b)));
        i += 1;
    }
    last
}

#[contract(client = IntDivisionClient)]
impl IntDivision {
    /// Divides `a` by `b`, `rounds` times. Signed kinds read both operands
    /// as two's complement.
    pub fn op64(kind: u32, a: u64, b: u64, rounds: u32) -> u64 {
        match kind {
            UNSIGNED_QUOTIENT => repeat(rounds, a, b, |a, b| a / b),
            UNSIGNED_REMAINDER => repeat(rounds, a, b, |a, b| a % b),
            SIGNED_QUOTIENT => {
                repeat(rounds, a, b, |a, b| (a as i64 / b as i64) as u64)
            }
            _ => repeat(rounds, a, b, |a, b| (a as i64 % b as i64) as u64),
        }
    }

    /// [`op64`](Self::op64) for 128-bit operands.
    pub fn op128(kind: u32, a: u128, b: u128, rounds: u32) -> u128 {
        match kind {
            UNSIGNED_QUOTIENT => repeat(rounds, a, b, |a, b| a / b),
            UNSIGNED_REMAINDER => repeat(rounds, a, b, |a, b| a % b),
            SIGNED_QUOTIENT => {
                repeat(rounds, a, b, |a, b| (a as i128 / b as i128) as u128)
            }
            _ => repeat(rounds, a, b, |a, b| (a as i128 % b as i128) as u128),
        }
    }
}
