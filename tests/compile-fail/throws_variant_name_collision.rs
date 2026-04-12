use sdk::*;

mod alpha {
    use sdk::*;

    #[contract_exception]
    pub struct Error;
}

mod beta {
    use sdk::*;

    #[contract_exception]
    pub struct Error;
}

#[contract(throws(alpha::Error, beta::Error))]
pub trait ThrowsCollisionTrait {
    fn ping();
}

fn main() {}
