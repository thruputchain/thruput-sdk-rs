use super::{Interface, MyErr};
use sdk::*;
use thiserror::Error;

#[derive(Default)]
pub struct Contract;

#[derive(Debug, Error)]
#[error("Thing")]
struct Thing;

#[contract(throws = MyErr)]
impl Interface for Contract {
    fn hello() {}

    fn add(n: u32) -> u32 {
        n + 1
    }

    fn aborts_with_my_err() -> u32 {
        abort(&MyErr);
    }

    fn panics() {
        panic!("uh oh!");
    }

    fn syscall_err() {
        // This should throw an exception
        bytestring_get_len(&StorageKey::from([7; 32]));
    }

    fn throw_thing() {
        abort(&Thing);
    }
}
