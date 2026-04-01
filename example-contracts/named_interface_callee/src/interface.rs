use sdk::*;

#[contract(interface = "ExplicitNamedInterface")]
pub trait NamedInterfaceCallee {
    fn add_one(x: u32) -> u32;
}
