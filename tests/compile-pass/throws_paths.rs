use sdk::*;

mod alpha {
    use sdk::*;

    #[contract_error]
    pub enum AlphaError {
        Denied = 1,
    }
}

mod beta {
    use sdk::*;

    #[contract_error]
    pub enum BetaError {
        Overflow = 1,
    }
}

#[contract(throws(alpha::AlphaError, beta::BetaError))]
pub trait ThrowsPathTrait {
    fn ping() -> u32;
}

fn main() {
    let _alpha = ThrowsPathTraitClientError::AlphaError(alpha::AlphaError::Denied);
    let _beta = ThrowsPathTraitClientError::BetaError(beta::BetaError::Overflow);
}
