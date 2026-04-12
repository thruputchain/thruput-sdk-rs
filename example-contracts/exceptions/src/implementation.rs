use super::{BigExn, Interface, MyExn};
use sdk::*;

#[derive(Default)]
pub struct Contract;

#[contract_exception]
struct Thing;

#[contract_exception]
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

    fn throw_big_exn() {
        throw(BigExn {
            payload: [0xAB; 600],
        });
    }
}
