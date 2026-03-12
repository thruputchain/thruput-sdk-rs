mod u112;
mod u256;

pub use u112::U112;
pub use u256::U256;

impl From<U112> for U256 {
    fn from(n: U112) -> Self {
        let n: u128 = n.into();
        n.into()
    }
}
