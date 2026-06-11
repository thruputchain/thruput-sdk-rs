#![allow(unexpected_cfgs)]

// Version strings must be full semver triplets; a bad one must produce a
// compile error, not a proc-macro panic.
#[sdk::exn(version = "1")]
pub enum MyExn {
    Boom = 1,
}

fn main() {}
