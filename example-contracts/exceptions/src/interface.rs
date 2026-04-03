use sdk::*;

#[contract_error]
pub struct MyExn;

#[contract_error]
pub struct BigExn {
    pub payload: [u8; 600],
}

#[contract(client = Client, throws(MyExn, BigExn))]
pub trait Interface {
    fn hello();

    fn add(n: u32) -> u32;

    fn throws_my_exn() -> u32;

    fn panics();

    fn syscall_err();

    fn throw_thing();

    fn throw_big_exn();
}
