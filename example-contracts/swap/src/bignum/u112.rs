use core::cmp::Ord;
use sdk::{ReadXDR, WriteXDR};

pub const U112_MAX: u128 = 2u128.pow(112) - 1;

#[derive(
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Copy,
    Clone,
    ReadXDR,
    WriteXDR,
)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(transparent)]
pub struct U112(u128);

impl U112 {
    pub const MAX: u128 = 2u128.pow(112) - 1;

    pub fn new_strict(n: u128) -> Self {
        n.try_into().unwrap()
    }

    pub const fn zero() -> Self {
        Self(0)
    }
}

impl TryFrom<u128> for U112 {
    type Error = ();

    fn try_from(n: u128) -> Result<Self, ()> {
        if n <= U112_MAX { Ok(Self(n)) } else { Err(()) }
    }
}

impl From<U112> for u128 {
    fn from(n: U112) -> u128 {
        n.0
    }
}

impl PartialEq<u128> for U112 {
    fn eq(&self, other: &u128) -> bool {
        self.0 == *other
    }
}
