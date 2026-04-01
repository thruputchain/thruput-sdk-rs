use sdk::*;

mod errors {
    use sdk::*;

    #[contract_error]
    pub struct Library;
}

#[contract(throws(errors::Library))]
pub trait ThrowsReservedVariantTrait {
    fn ping();
}

fn main() {}
