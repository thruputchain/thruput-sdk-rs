#![no_std]

use sdk::*;
use sdk::sys_exn::bytestring::*;

#[cfg_attr(target_family = "wasm", unsafe(export_name = "0"))]
pub extern "C" fn write_bytestring() -> i32 {
    let calldata = get_calldata();

    let calldata: &[u8] = calldata.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let string = &calldata[32..];

    bytestring_set(&key, string);

    0
}

#[cfg_attr(target_family = "wasm", unsafe(export_name = "1"))]
pub extern "C" fn log_bytestring() -> i32 {
    let calldata = get_calldata();

    let calldata: &[u8] = calldata.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    if calldata.len() != 32 {
        panic!("bad calldata len");
    }

    let bstring = bytestring_get(&key);

    log(&bstring);

    0
}

#[cfg_attr(target_family = "wasm", unsafe(export_name = "2"))]
pub extern "C" fn expect_bytestring() -> i32 {
    let calldata = get_calldata();

    let calldata: &[u8] = calldata.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let bstring = bytestring_get(&key);
    let expected_bstring = &calldata[32..];

    if bstring.as_ref() != expected_bstring {
        panic!("unexpected bytestring");
    }

    0
}
