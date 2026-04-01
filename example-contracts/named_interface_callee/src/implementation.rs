use crate::NamedInterfaceCallee;
use sdk::*;

pub struct NamedInterfaceCalleeContract;

#[contract(interface = "ExplicitNamedInterface")]
impl NamedInterfaceCallee for NamedInterfaceCalleeContract {
    fn add_one(x: u32) -> u32 {
        x + 1
    }
}
