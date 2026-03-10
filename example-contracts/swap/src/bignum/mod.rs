mod u112;
use sdk::{ReadXDR, WriteXDR};
use primitive_types as prim;
use core::ops::*;

pub use u112::U112;

// Wrap for serialization :-(

#[derive(Clone, Default, Debug, ReadXDR, WriteXDR)]
#[xdrs(via = "[u64; 4]")]
#[repr(transparent)]
pub struct U256(prim::U256);

impl U256 {
    pub const fn zero() -> Self {
        Self(prim::U256::zero())
    }
}

/*
impl core::ops::Deref for U256 {
    type Target = prim::U256;

    fn deref(&self) -> &prim::U256 {
        &self.0
    }
}
*/

impl From<[u64; 4]> for U256 {
    fn from(array: [u64; 4]) -> Self {
        Self(prim::U256(array))
    }
}

impl Into<[u64; 4]> for U256 {
    fn into(self) -> [u64; 4] {
        let p: prim::U256 = self.0;
        p.0
    }
}

// Forward many operations to inner type ...

impl From<u32> for U256 {
    fn from(n: u32) -> Self {
        Self(n.into())
    }
}

impl From<u128> for U256 {
    fn from(n: u128) -> Self {
        Self(n.into())
    }
}

impl<T> AddAssign<T> for U256
where
    prim::U256: AddAssign<T>
{
    fn add_assign(&mut self, rhs: T) {
        self.0 += rhs
    }
}

impl From<U112> for U256 {
    fn from(n: U112) -> Self {
        let n: u128 = n.into();
        n.into()
    }
}
