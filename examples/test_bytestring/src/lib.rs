#![no_std]

use sdk::invoke::get_calldata;
use sdk::prelude::*;

#[cfg_attr(target_family = "wasm", export_name = "0")]
pub extern "C" fn write_bytestring() -> i32 {
    let calldata = get_calldata().unwrap();

    let key = StorageKey::new(calldata[0..32].try_into().unwrap());

    let string = &calldata[32..];

    bytestring_set(&key, string).unwrap();

    0
}

#[cfg_attr(target_family = "wasm", export_name = "1")]
pub extern "C" fn log_bytestring() -> i32 {
    let calldata = get_calldata().unwrap();

    let key = StorageKey::new(calldata[0..32].try_into().unwrap());

    if calldata.len() != 32 {
        panic!("bad calldata len");
    }

    let bstring = bytestring_get(&key).unwrap();

    log(&bstring).unwrap();

    0
}

#[cfg_attr(target_family = "wasm", export_name = "2")]
pub extern "C" fn expect_bytestring() -> i32 {
    let calldata = get_calldata().unwrap();

    let key = StorageKey::new(calldata[0..32].try_into().unwrap());

    let bstring = bytestring_get(&key).unwrap();
    let expected_bstring = &calldata[32..];

    if bstring.as_ref() != expected_bstring {
        panic!("unexpected bytestring");
    }

    0
}
