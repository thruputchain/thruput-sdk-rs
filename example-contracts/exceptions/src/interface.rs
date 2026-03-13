use sdk::*;
use thiserror::Error;

#[derive(WriteXDR, ReadXDR, Copy, Clone, Debug, PartialEq, Error)]
#[error("MyErr")]
pub struct MyErr;

#[contract(client = Client, throws = MyErr)]
pub trait Interface {
    fn hello();

    fn add(n: u32) -> u32;

    fn aborts_with_my_err() -> u32;

    fn panics();

    fn syscall_err();

    fn throw_thing();
}
