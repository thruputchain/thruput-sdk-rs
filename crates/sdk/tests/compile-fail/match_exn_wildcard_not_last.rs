use thruput_sdk::*;

#[contract_exception]
pub enum E {
    A = 1,
}

fn main() {
    let e = CallError::Invalid;
    // Arms after `_` could never be reached.
    let _n: u32 = match_exn!(e, {
        _ => 0,
        E::A => 1,
    });
}
