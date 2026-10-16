#![allow(unexpected_cfgs)]

// 250 chars: exceeds the framed-domain budget
// (STATIC_KEYPATH_CAPACITY - 10 = 246).
#[derive(thruput_sdk::Storage)]
#[storage_domain = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"]
pub struct S {
    v: thruput_sdk::storage::NNInt,
}

fn main() {
    // Associated consts are lazy; force const-eval of the framed domain.
    let _ = <S as thruput_sdk::storage::StorageDomain>::DOMAIN;
}
