use sdk::prelude::*;

pub use sdk::prelude::{Error, Result};

#[contract(client = "client")]
pub trait {{ crate_name | capitalize }} {
    fn initialize(maybe_self: Option<&mut Self>) -> Result<()>;
    // TODO: add method declarations here
}
