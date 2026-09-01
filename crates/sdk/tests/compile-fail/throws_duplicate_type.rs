use sdk::*;

#[contract_exception]
pub enum DupError {
    Denied = 1,
}

// The same type twice would duplicate its entry in the contract spec.
#[contract(throws(DupError, DupError))]
pub trait ThrowsDuplicateTypeTrait {
    fn ping();
}

fn main() {}
