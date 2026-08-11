//! Formatting.
//!
//! `Debug` is unconditional -- contract state structs `#[derive(Debug)]` over
//! `U256`, and gating it would break them in guest builds -- so it renders hex,
//! which needs no division. `Display`/`FromStr` need a decimal division loop no
//! release guest build should carry, so they sit behind `fmt`.

use crate::U256;
use core::fmt;

/// 2^256 - 1 is 64 hexadecimal digits.
const HEX_DIGITS: usize = 64;

/// 2^256 - 1 is 78 decimal digits. Only the decimal path uses this, and that
/// is behind `fmt`.
#[cfg(feature = "fmt")]
const DEC_DIGITS: usize = 78;

const LOWER: [u8; 16] = *b"0123456789abcdef";

// Only `UpperHex` uses this, and that is behind `fmt` -- so in a guest build
// the table itself must go too, or it is dead rodata.
#[cfg(feature = "fmt")]
const UPPER: [u8; 16] = *b"0123456789ABCDEF";

/// Renders the significant hex digits into `buf`, returning them as a string.
/// Digits are written from the back, so the return value borrows the tail.
fn hex_digits<'a>(
    n: &U256,
    buf: &'a mut [u8; HEX_DIGITS],
    table: &[u8; 16],
) -> &'a str {
    let mut pos = buf.len();
    let mut v = *n;

    if v.is_zero() {
        pos -= 1;
        buf[pos] = b'0';
    } else {
        while !v.is_zero() {
            pos -= 1;
            buf[pos] = table[(v.to_limbs()[0] & 0xf) as usize];
            v = v.wrapping_shr(4);
        }
    }

    core::str::from_utf8(&buf[pos..]).expect("hex digits are ASCII")
}

impl fmt::Debug for U256 {
    /// Renders as `0x`-prefixed hex.
    ///
    /// Unlike the `LowerHex`/`UpperHex` impls this does not honour width or
    /// fill flags -- `Debug` is for diagnostics, and a padded `{:?}` on an
    /// integer is not a thing callers ask for. Use `{:x}` when the exact
    /// textual shape matters.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        f.write_str("0x")?;
        f.write_str(hex_digits(self, &mut buf, &LOWER))
    }
}

#[cfg(feature = "fmt")]
impl fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        let digits = hex_digits(self, &mut buf, &LOWER);
        // `pad_integral` applies width, fill, alignment and zero-padding, and
        // emits the prefix only under `#` -- so `{:064x}` and `{:#x}` behave
        // as they do for the primitive integers. Fixed-width hex is the
        // common spelling for a 256-bit word, so this has to be right.
        f.pad_integral(true, "0x", digits)
    }
}

#[cfg(feature = "fmt")]
impl fmt::UpperHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; HEX_DIGITS];
        let digits = hex_digits(self, &mut buf, &UPPER);
        // Lowercase `0x` even here: `format!("{:#X}", 255u8)` is `"0xFF"`.
        f.pad_integral(true, "0x", digits)
    }
}

/// Renders the decimal digits into `buf`, returning them as a string.
#[cfg(feature = "fmt")]
fn dec_digits<'a>(n: &U256, buf: &'a mut [u8; DEC_DIGITS]) -> &'a str {
    let mut pos = buf.len();
    let mut v = *n;

    if v.is_zero() {
        pos -= 1;
        buf[pos] = b'0';
    }

    // Nine digits at a time: 10^9 is the largest power of ten fitting in a
    // u32, so each step is one `div_rem_small` rather than nine.
    while !v.is_zero() {
        let (q, r) = v.div_rem_small(1_000_000_000);
        v = q;
        let mut r = r;
        // The leading chunk emits only its significant digits; every other
        // chunk is zero-padded to nine, or interior zeros would be lost.
        let count = if v.is_zero() {
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
        while i < count {
            pos -= 1;
            buf[pos] = b'0' + (r % 10) as u8;
            r /= 10;
            i += 1;
        }
    }

    core::str::from_utf8(&buf[pos..]).expect("decimal digits are ASCII")
}

#[cfg(feature = "fmt")]
impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; DEC_DIGITS];
        let digits = dec_digits(self, &mut buf);
        f.pad_integral(true, "", digits)
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
