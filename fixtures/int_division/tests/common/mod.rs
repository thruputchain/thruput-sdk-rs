//! Host arithmetic for each `IntDivision` method, and the operands the
//! tests divide.

#![allow(unused)]

pub use int_division::{
    SIGNED_QUOTIENT, SIGNED_REMAINDER, UNSIGNED_QUOTIENT, UNSIGNED_REMAINDER,
};

pub const KINDS: [u32; 4] = [
    UNSIGNED_QUOTIENT,
    UNSIGNED_REMAINDER,
    SIGNED_QUOTIENT,
    SIGNED_REMAINDER,
];

/// What `IntDivision::op64` returns, or `None` where the contract panics:
/// a zero divisor, or the one signed quotient that overflows.
pub fn op64(kind: u32, a: u64, b: u64) -> Option<u64> {
    match kind {
        UNSIGNED_QUOTIENT => a.checked_div(b),
        UNSIGNED_REMAINDER => a.checked_rem(b),
        SIGNED_QUOTIENT => (a as i64).checked_div(b as i64).map(|q| q as u64),
        _ => (a as i64).checked_rem(b as i64).map(|r| r as u64),
    }
}

/// 64-bit operands at the limb and half-limb boundaries, with both signs.
pub const OPERANDS_64: [u64; 20] = [
    1,
    3,
    10,
    0xFFFF,
    0x1_0000,
    10_000_000,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFF,
    0x1_0000_0000,
    0x1_0000_0001,
    0x8000_0000_FFFF_FFFF,
    1_000_000_000_000_000_000,
    0x7FFF_FFFF_FFFF_FFFF,
    0x8000_0000_0000_0000,
    0x8000_0000_0000_0001,
    0xFEDC_BA98_7654_3210,
    0xFFFF_FFFF_0000_0000,
    0xFFFF_FFFF_FFFF_FFFE,
    0xFFFF_FFFF_FFFF_FFFF,
];
