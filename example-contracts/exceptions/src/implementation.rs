use super::{Interface, MyExn};
use sdk::*;

#[derive(Default)]
pub struct Contract;

#[contract_error]
#[error("Thing")]
struct Thing;

#[contract_error]
#[error("not authorized")]
pub struct Unauthorized;

#[contract]
impl Interface for Contract {
    fn hello() {}

    fn add(n: u32) -> u32 {
        n + 1
    }

    fn throws_my_exn() -> u32 {
        throw(MyExn);
    }

    fn panics() {
        panic!("uh oh!");
    }

    fn syscall_err() {
        // This should throw an exception
        bytestring_get_len(&StorageKey::from([7; 32]));
    }

    fn throw_thing() {
        throw(Thing);
    }
}
