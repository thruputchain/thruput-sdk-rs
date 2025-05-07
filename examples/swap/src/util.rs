#![allow(unused_variables)]

use core::cmp::Ord;
use core::ops::{Div, Mul};

pub const U112_MAX: u128 = 2u128.pow(112) - 1;

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
#[repr(transparent)]
pub struct U112(u128);

impl U112 {
    pub fn new_strict(n: u128) -> Self {
        n.try_into().unwrap()
    }
}

impl TryFrom<u128> for U112 {
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

impl PartialEq<u128> for U112 {
    fn eq(&self, other: &u128) -> bool {
        self.0 == *other
    }
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct U256(u128, u128);

impl U256 {
    pub const fn q(power: u32) -> Self {
        U256(0, 2u128.pow(power))
    }

    pub fn add_wrapping(&mut self, other: &Self) {
        todo!()
    }
}

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

impl Div for U256 {
    type Output = U256;

    fn div(self, other: Self) -> Self {
        todo!()
    }
}

impl Mul for U256 {
    type Output = U256;

    fn mul(self, other: Self) -> Self {
        todo!()
    }
}

impl Mul<u128> for U256 {
    type Output = U256;

    fn mul(self, other: u128) -> Self {
        todo!()
    }
}

// Compute exactly: (a*Q)/(b*Q) * p, where Q = 2^112
pub fn uq112x112_div_mul(a: U112, b: U112, p: u32) -> U256 {
    const Q: U256 = U256::q(112);

    let a: U256 = a.into();
    let b: U256 = b.into();
    let p: U256 = (p as u128).into();
    (a * Q) / (b * Q) * p
}
