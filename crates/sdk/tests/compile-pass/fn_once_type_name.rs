#![allow(unexpected_cfgs)]
use thruput_sdk::*;

type FnOnce = u32;

#[contract]
pub trait Existing {
    fn echo(value: FnOnce) -> FnOnce;
}

fn check(client: ExistingClient) {
    let _: u32 = client.echo(1);
    let _: Result<u32, CallError> = client.try_echo(2);
    let _: Result<u32, RawCallError<u8>> =
        client.map_exceptions(|_| 7u8).try_echo(3);
}

fn main() { let _ = check; }
