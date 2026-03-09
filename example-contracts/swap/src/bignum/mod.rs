use xdrs::*;
use core::ops::Deref;

mod u112;

pub use u112::U112;

#[derive(ReadXDR, WriteXDR)]
#[repr(transparent)]
#[xdrs(as_type = "[u64; 4]")]
pub struct U256(
    pub primitive_types::U256
);

impl Default for U256 {
    fn default() -> Self {
        U256(primitive_types::U256::default())
    }
}

impl Deref for U256 {
    type Target = primitive_types::U256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<primitive_types::U256> for U256 {
    fn from(n: primitive_types::U256) -> Self {
        Self(n)
    }
}

impl From<u128> for U256 {
    fn from(n: u128) -> Self {
        Self(n.into())
    }
}

impl Into<[u64; 4]> for &U256 {
    fn into(self) -> [u64; 4] {
        self.0.0
    }
}

impl From<[u64; 4]> for U256 {
    fn from(array: [u64; 4]) -> Self {
        Self(primitive_types::U256(array))
    }
}

impl Into<[u64; 4]> for U256 {
    fn into(self) -> [u64; 4] {
        self.0.0
    }
}

impl From<U112> for U256 {
    fn from(n: U112) -> Self {
        let n: u128 = n.into();
        let n: primitive_types::U256 = n.into();
        n.into()
    }
}
