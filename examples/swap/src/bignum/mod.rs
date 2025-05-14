mod u112;

pub use u112::U112;

pub use ::primitive_types::U256;

impl From<U112> for U256 {
    fn from(n: U112) -> Self {
        let n: u128 = n.into();
        n.into()
    }
}
