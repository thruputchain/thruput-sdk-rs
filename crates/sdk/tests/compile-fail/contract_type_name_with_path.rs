#![allow(unexpected_cfgs)]

// `name` becomes the final segment of the spec id, so it must be one segment.
// A value carrying `::` would splice extra segments in and could claim the id
// of a real type at that path.
#[sdk::contract_type(name = "inner::Meta")]
pub struct Meta {
    pub count: u32,
}

fn main() {}
