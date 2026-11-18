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

/// What `IntDivision::op128` returns; see [`op64`].
pub fn op128(kind: u32, a: u128, b: u128) -> Option<u128> {
    match kind {
        UNSIGNED_QUOTIENT => a.checked_div(b),
        UNSIGNED_REMAINDER => a.checked_rem(b),
        SIGNED_QUOTIENT => {
            (a as i128).checked_div(b as i128).map(|q| q as u128)
        }
        _ => (a as i128).checked_rem(b as i128).map(|r| r as u128),
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

/// 128-bit operands at the limb boundaries, with both signs. The pairs
/// include every correction the 128-bit division can take: an estimate
/// capped at a limb's maximum, and one that overshoots and is added back.
pub const OPERANDS_128: [u128; 24] = [
    1,
    7,
    10_000,
    0xFFFF_FFFF,
    0x1_0000_0001,
    1_000_000_000_000_000_000,
    0xFFFF_FFFF_FFFF_FFFF,
    1 << 64,
    (1 << 64) + 1,
    (1 << 64) + 2,
    2 << 64,
    (1 << 95) + 12345,
    1 << 96,
    (1 << 96) + 1,
    (1 << 96) + 2,
    2 << 96,
    0x7FFF_FFFF_8000_0000_0000_0000_0000_0000,
    0x8000_0000_0000_0000_0000_0001,
    u128::MAX / 3,
    0x7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF,
    1 << 127,
    (1 << 127) + 1,
    u128::MAX - 1,
    u128::MAX,
];
