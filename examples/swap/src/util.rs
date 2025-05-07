use core::cmp::Ord;

pub const U112_MAX: u128 = 2u128.checked_pow(112) - 1;

#[derive(Default, Ord, Copy, Clone)]
#[repr(transparent)]
pub struct U112(u128);

impl TryFrom<128> for U112 {
    type Error = ();

    fn try_from(n: u128) -> Result<Self, ()> {
        if n <= U112_MAX {
            Ok(Self(n))
        } else {
            Err(())
        }
    }
}

impl From<U112> for u128 {
    fn from(n: U112) -> u128 {
        n.0
    }
}

#[derive(Default, Ord, Copy, Clone)]
#[repr(transparent)]
pub struct U256(u128, u128);

impl From<u128> for U256 {
    fn from(n: u128) -> Self {
        Self(0, n)
    }
}

impl From<U112> for U256 {
    fn from(n: U112) -> Self {
        let n: u128 = n.into();
        n.into()
    }
}

use core::ops::Mul;
impl Mul<u128> for U256 {
    type Output = U256;

    fn mul(self, other: u128) -> Self {
        todo!()
    }
}

// Compute exactly: (a*Q)/(b*Q) * p, where Q = 2^112
pub fn uq112x112_div_mul(a: U112, b: U112, p: u32) -> U256 {
    const Q: U256 = 2u128.pow(112).into();
    let a: U256 = a.into();
    let b: U256 = b.into();
    let p: U256 = p.into();
    (a * Q)/(b * Q) * p
}
