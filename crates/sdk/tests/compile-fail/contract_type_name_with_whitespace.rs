#![allow(unexpected_cfgs)]

// The id is spliced from the raw string, so a name that merely *parses* as an
// identifier is not enough — the whitespace would land in the id.
#[sdk::contract_type(name = " Meta ")]
pub struct Meta {
    pub count: u32,
}

fn main() {}
