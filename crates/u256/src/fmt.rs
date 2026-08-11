//! Formatting.
//!
//! `Debug` is unconditional; everything else is behind the `fmt` feature.
//!
//! The split is not arbitrary. `Debug` has to be always available because
//! `#[derive(Debug)]` on any struct holding a `U256` would otherwise fail to
//! compile in a guest build -- and contract state structs do derive it. So
//! `Debug` renders hexadecimal using its own nibble extraction, which costs
//! shifts and a lookup and no division.
//!
//! `Display` and `FromStr` are the expensive pair: decimal needs a repeated
//! division loop. Nothing in a release contract build should be formatting, so
//! they are compiled out by default.

use crate::{LIMBS, U256};
use core::fmt;

/// Writes the value as hex with leading zeros suppressed.
///
/// `prefix` forces the `0x`; otherwise it follows the alternate flag, so
/// `{:x}` is bare and `{:#x}` is prefixed, as for the primitive integers.
fn write_hex(
    n: &U256,
    f: &mut fmt::Formatter<'_>,
    digits: &[u8; 16],
    prefix: bool,
) -> fmt::Result {
    if prefix || f.alternate() {
        f.write_str("0x")?;
    }

    let mut started = false;
    let mut limb = LIMBS;
    while limb > 0 {
        limb -= 1;
        let mut nibble = 8;
        while nibble > 0 {
            nibble -= 1;
            let d = ((n.0[limb] >> (nibble * 4)) & 0xf) as usize;
            if d != 0 || started || (limb == 0 && nibble == 0) {
                started = true;
                f.write_str(
                    // SAFETY-free: `digits` is ASCII, so a one-byte slice of
                    // it is valid UTF-8 by construction.
                    core::str::from_utf8(&digits[d..d + 1])
                        .expect("hex digit table is ASCII"),
                )?;
            }
        }
    }

    Ok(())
}

const LOWER: [u8; 16] = *b"0123456789abcdef";

// Only `UpperHex` uses this, and that is behind `fmt` -- so in a guest build
// the table itself must go too, or it is dead rodata.
#[cfg(feature = "fmt")]
const UPPER: [u8; 16] = *b"0123456789ABCDEF";

impl fmt::Debug for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(self, f, &LOWER, true)
    }
}

#[cfg(feature = "fmt")]
impl fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(self, f, &LOWER, false)
    }
}

#[cfg(feature = "fmt")]
impl fmt::UpperHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(self, f, &UPPER, false)
    }
}

#[cfg(feature = "fmt")]
impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }

        // 2^256 - 1 is 78 decimal digits.
        let mut buf = [0u8; 78];
        let mut pos = buf.len();
        let mut n = *self;

        // Nine digits at a time: 10^9 is the largest power of ten fitting in a
        // u32, so each step is one `div_rem_small` rather than nine.
        while !n.is_zero() {
            let (q, r) = n.div_rem_small(1_000_000_000);
            n = q;
            let mut r = r;
            let chunk = if n.is_zero() {
                // Leading chunk: emit only its significant digits.
                let mut d = 0;
                let mut t = r;
                while t != 0 {
                    d += 1;
                    t /= 10;
                }
                d
            } else {
                9
            };
            let mut i = 0;
            while i < chunk {
                pos -= 1;
                buf[pos] = b'0' + (r % 10) as u8;
                r /= 10;
                i += 1;
            }
        }

        f.write_str(
            core::str::from_utf8(&buf[pos..])
                .expect("decimal digits are ASCII"),
        )
    }
}

/// Returned when a string cannot be parsed as a [`U256`].
#[cfg(feature = "fmt")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseU256Error {
    /// The input was empty.
    Empty,
    /// The input contained a character that is not a decimal digit.
    InvalidDigit,
    /// The value does not fit in 256 bits.
    Overflow,
}

#[cfg(feature = "fmt")]
impl fmt::Display for ParseU256Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "cannot parse an empty string as U256",
            Self::InvalidDigit => "invalid decimal digit",
            Self::Overflow => "value does not fit in 256 bits",
        })
    }
}

#[cfg(all(feature = "fmt", feature = "std"))]
impl std::error::Error for ParseU256Error {}

#[cfg(feature = "fmt")]
impl core::str::FromStr for U256 {
    type Err = ParseU256Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseU256Error::Empty);
        }

        let mut acc = U256::ZERO;
        for byte in s.bytes() {
            if !byte.is_ascii_digit() {
                return Err(ParseU256Error::InvalidDigit);
            }
            let (scaled, overflow) = acc.overflowing_mul_u32(10);
            if overflow {
                return Err(ParseU256Error::Overflow);
            }
            let (next, overflow) =
                scaled.overflowing_add(U256::from(byte - b'0'));
            if overflow {
                return Err(ParseU256Error::Overflow);
            }
            acc = next;
        }

        Ok(acc)
    }
}
