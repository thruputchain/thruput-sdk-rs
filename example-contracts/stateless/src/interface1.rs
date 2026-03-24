use crate::Stateless;
use sdk::*;

#[contract(interface = "Interface1", client = Client)]
pub trait Interface {
    fn perform_task() -> u32;
}

#[contract(interface = "Interface1")]
impl Interface for Stateless {
    fn perform_task() -> u32 {
        1
    }
}
