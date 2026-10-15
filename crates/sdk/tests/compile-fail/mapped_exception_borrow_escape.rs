#![allow(unexpected_cfgs)]
use thruput_sdk::*;

#[contract]
pub trait Example {
    fn call();
}

fn main() {
    let client = ExampleClient::new(&ContractAddress::default());
    let _ = client.map_exceptions(|exn| exn.payload()).try_call();
}
