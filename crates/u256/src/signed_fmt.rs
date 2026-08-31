//! [`I256`] formatting.
//!
//! Split the same way the unsigned side is: `Debug` is unconditional, because
//! contract state structs `#[derive(Debug)]` over their fields and gating it
//! would break guest builds, and everything that needs the decimal division
//! loop sits behind `fmt`.
//!
//! `Debug` renders sign-magnitude hex (`-0x2a`), not the two's complement bits
//! -- a diagnostic that prints `-1` as sixty-four `f`s is not one. `LowerHex`
//! and `UpperHex` do print the raw bits, matching the primitives, where
//! `format!("{:x}", -1i8)` is `"ff"`.

use crate::I256;
use crate::fmt::{HEX_DIGITS, LOWER, hex_digits};
use core::fmt;

#[cfg(feature = "fmt")]
use crate::fmt::{DEC_DIGITS, UPPER, dec_digits};

impl fmt::Debug for I256 {
    /// Renders as sign-magnitude `0x`-prefixed hex.
    ///
    /// Like [`U256`](crate::U256)'s `Debug`, this does not honour width or
    /// fill flags; use `{:x}` when the exact textual shape matters.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        if self.is_negative() {
            f.write_str("-")?;
        }
        f.write_str("0x")?;
        f.write_str(hex_digits(&self.unsigned_abs(), &mut buf, &LOWER))
    }
}

#[cfg(feature = "fmt")]
impl fmt::LowerHex for I256 {
    /// The two's complement bits, as the primitives do it: `-1` is all `f`s,
    /// not `-1`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        let digits = hex_digits(&self.cast_unsigned(), &mut buf, &LOWER);
        f.pad_integral(true, "0x", digits)
    }
}

#[cfg(feature = "fmt")]
impl fmt::UpperHex for I256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        let digits = hex_digits(&self.cast_unsigned(), &mut buf, &UPPER);
        // Lowercase `0x` even here: `format!("{:#X}", 255u8)` is `"0xFF"`.
        f.pad_integral(true, "0x", digits)
    }
}

#[cfg(feature = "fmt")]
impl fmt::Display for I256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; DEC_DIGITS];
        let digits = dec_digits(&self.unsigned_abs(), &mut buf);
        // `pad_integral` writes the sign itself, and honours `+`, width, fill
        // and zero-padding the way it does for the primitives -- so
        // `format!("{:+06}", -42)` lines up.
        f.pad_integral(!self.is_negative(), "", digits)
    }
}

/// Returned when a string cannot be parsed as an [`I256`].
#[cfg(feature = "fmt")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseI256Error {
    /// The input was empty.
    Empty,
    /// The input contained a character that is neither a leading sign nor a
    /// decimal digit -- including a sign with no digits after it.
    InvalidDigit,
    /// The value does not fit in a signed 256-bit integer.
    Overflow,
}

#[cfg(feature = "fmt")]
impl fmt::Display for ParseI256Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "cannot parse an empty string as I256",
            Self::InvalidDigit => "invalid decimal digit",
            Self::Overflow => "value does not fit in a signed 256-bit integer",
        })
    }
}

#[cfg(all(feature = "fmt", feature = "std"))]
impl std::error::Error for ParseI256Error {}

#[cfg(feature = "fmt")]
impl core::str::FromStr for I256 {
    type Err = ParseI256Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use crate::U256;

        let (negative, digits) = match s.as_bytes().first() {
            None => return Err(ParseI256Error::Empty),
            Some(b'-') => (true, &s[1..]),
            Some(b'+') => (false, &s[1..]),
            _ => (false, s),
        };
        // A bare sign is an invalid digit, not an empty string -- the same
        // answer `"-".parse::<i32>()` gives.
        if digits.is_empty() {
            return Err(ParseI256Error::InvalidDigit);
        }

        let mut acc = U256::ZERO;
        for byte in digits.bytes() {
            if !byte.is_ascii_digit() {
                return Err(ParseI256Error::InvalidDigit);
            }
            let (scaled, overflow) = acc.overflowing_mul_u32(10);
            if overflow {
                return Err(ParseI256Error::Overflow);
            }
            let (next, overflow) =
                scaled.overflowing_add(U256::from_u32((byte - b'0') as u32));
            if overflow {
                return Err(ParseI256Error::Overflow);
            }
            acc = next;
        }

        // The negative range reaches one further, so the bound is on the
        // magnitude rather than on the value it would become.
        let limit = if negative {
            U256::ONE.wrapping_shl(255)
        } else {
            Self::MAX.cast_unsigned()
        };
        if acc > limit {
            return Err(ParseI256Error::Overflow);
        }

        Ok(if negative {
            acc.wrapping_neg().cast_signed()
        } else {
            acc.cast_signed()
        })
    }
}
