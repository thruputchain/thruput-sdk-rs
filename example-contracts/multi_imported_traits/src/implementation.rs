pub struct MultiImportedTraitsContract;
use thruput_sdk::*;

#[contract(interface = "ExplicitNamedInterface")]
impl named_interface_callee::NamedInterfaceCallee
    for MultiImportedTraitsContract
{
    fn add_one(x: u32) -> u32 {
        x + 1
    }
}

#[contract]
impl versioned_callee::VersionedCallee for MultiImportedTraitsContract {
    fn add_one(x: u32) -> u32 {
        x + 2
    }
}
