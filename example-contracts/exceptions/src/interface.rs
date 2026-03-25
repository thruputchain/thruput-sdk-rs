use sdk::*;
use thiserror::Error;

#[derive(WriteXDR, ReadXDR, Copy, Clone, Debug, PartialEq, Error)]
#[error("My exception")]
pub struct MyExn;

#[contract(client = Client, throws = MyExn)]
pub trait Interface {
    fn hello();

    fn add(n: u32) -> u32;

    fn throws_my_exn() -> u32;

    fn panics();

    fn syscall_err();

    fn throw_thing();
}
