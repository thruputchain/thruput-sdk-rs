use crate::exn::LibraryExn;
use crate::re_throw::*;
use crate::xdr::*;

/// An unsigned integer, no larger than `i128::MAX`
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[derive(WriteXDR, ReadXDR)]
#[repr(transparent)]
pub struct u127(u128);

impl u127 {
    pub const ZERO: u127 = u127(0u128);
    pub const MAX: u127 = u127(i128::MAX as u128);

    pub fn new(n: u128) -> Self {
        Self::try_new(n).or_re_throw()
    }

    pub fn try_new(n: u128) -> Result<Self, LibraryExn> {
        n.try_into().map_err(|_| sdk::LibraryExn::NonNegativeIntOverflow)
    }

    pub const fn max() -> Self {
        Self::MAX
    }

    pub const fn zero() -> Self {
        Self::ZERO
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl TryFrom<u128> for u127 {
    type Error = LibraryExn;

    fn try_from(n: u128) -> Result<Self, LibraryExn> {
        if n <= i128::MAX as u128 {
            Ok(Self(n))
        } else {
            Err(LibraryExn::NonNegativeIntOverflow)
        }
    }
}

impl TryFrom<i128> for u127 {
    type Error = LibraryExn;

    fn try_from(i: i128) -> Result<Self, LibraryExn> {
        i.try_into()
            .map(|n| Self(n))
            .map_err(|_| LibraryExn::NonNegativeIntUnderflow)
    }
}

impl From<u127> for u128 {
    fn from(n: u127) -> Self {
        n.0
    }
}

impl From<u127> for i128 {
    fn from(n: u127) -> Self {
        // The inner value is always <= i128::MAX
        n.0 as i128
    }
}

impl From<u32> for u127 {
    fn from(n: u32) -> Self {
        Self(n.into())
    }
}

#[test]
fn i128_max_abs_fits_u127() {
    let n = i128::MAX.unsigned_abs();

    let r: Result<u127, LibraryExn> = n.try_into();
    assert!(r.is_ok());
}
