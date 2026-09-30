#![allow(unexpected_cfgs)]

// Identical declarations expanded from one macro have identical source
// locations. Their private guard helpers must still have distinct names.
macro_rules! interface {
    ($module:ident) => {
        pub mod $module {
            #[sdk::contract(version = "4.5.6")]
            pub trait SameName {
                fn ping() -> u32;
            }
        }
    };
}

interface!(first);
interface!(second);

pub use first::SameName as Alias;
pub struct Contract;

#[sdk::contract]
impl Alias for Contract {
    fn ping() -> u32 {
        42
    }
}

fn main() {}
