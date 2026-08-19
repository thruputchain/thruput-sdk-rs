#![no_std]

use sdk::*;

pub extern "C" fn write_bytestring() -> i32 {
    let mut calldata_buf = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    fetch_calldata_and_auth_tokens(&mut calldata_buf, &mut auth_ids);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let string = &calldata[32..];

    bytestring_set(&key, string);

    0
}

pub extern "C" fn log_bytestring() -> i32 {
    let mut calldata_buf = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    fetch_calldata_and_auth_tokens(&mut calldata_buf, &mut auth_ids);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    if calldata.len() != 32 {
        panic!("bad calldata len");
    }

    let bstring = bytestring_get(&key);

    log(&bstring);

    0
}

pub extern "C" fn expect_bytestring() -> i32 {
    let mut calldata_buf = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    fetch_calldata_and_auth_tokens(&mut calldata_buf, &mut auth_ids);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let bstring = bytestring_get(&key);
    let expected_bstring = &calldata[32..];

    if bstring.as_ref() != expected_bstring {
        panic!("unexpected bytestring");
    }

    0
}
