use crate::Stateless;
use sdk::*;

#[contract(interface = "Interface2", client = Client)]
trait Interface {
    fn perform_task() -> u32;
}

#[contract(interface = "Interface2")]
impl Interface for Stateless {
    fn perform_task() -> u32 {
        2
    }
}
