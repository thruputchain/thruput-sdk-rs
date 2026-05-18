use sdk::*;

mod errors {
    use sdk::*;

    #[contract_exception]
    pub struct Library;
}

#[contract(throws(errors::Library))]
pub trait ThrowsReservedVariantTrait {
    fn ping();
}

fn main() {}
