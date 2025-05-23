//! An implementation of {{ crate_name }}

use super::interface::{{ crate_name | capitalize }};
use sdk::storage_prelude::*;

#[derive(Storage, Debug, Serialize)]
pub struct {{ crate_name | capitalize }}Contract {
}

impl {{ crate_name | capitalize }}Contract {
    pub fn new() -> Self {
        todo!("Initialize contract storage here");
    }
}

// This implements the ERC20 interface.  The `contract` macro provides
// an exported entrypoint for each method; these will be placed in a
// module with the specified name ("guest_exports").
#[contract(exports = "guest_exports")]
impl {{ crate_name | capitalize }} for {{ crate_name | capitalize }}Contract {
    fn initialize(
        this: Option<&mut Self>,
    ) -> Result<()> {
        todo!("Initialize contract here");
    }
    // TODO: add method implementations here
}