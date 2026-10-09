//! Raw contract: entrypoints registered by hand, with no `#[contract]` macro.
//!
//! rv32 dispatches guest-side off the `groundhog_rv32_entrypoints` section,
//! which the macro normally populates. Writing the records by hand registers
//! these entrypoints outside the macro's calldata/Return glue — which is the
//! point of this fixture: it exercises the bytestring syscalls directly.
//!
//! The dispatcher traps (ebreak) when a wrapper returns, so each wrapper ends
//! in `raw_Return`.

#![no_std]

use sdk::*;

pub fn write_bytestring() {
    let mut calldata_buf = sdk::internal::new_scratch();
    fetch_calldata_and_auth_tokens(&mut calldata_buf);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let string = &calldata[32..];

    bytestring_set(&key, string);
}

pub fn log_bytestring() {
    let mut calldata_buf = sdk::internal::new_scratch();
    fetch_calldata_and_auth_tokens(&mut calldata_buf);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    if calldata.len() != 32 {
        panic!("bad calldata len");
    }

    let bstring = bytestring_get(&key);

    log(&bstring);
}

pub fn expect_bytestring() {
    let mut calldata_buf = sdk::internal::new_scratch();
    fetch_calldata_and_auth_tokens(&mut calldata_buf);
    let calldata: &[u8] = calldata_buf.as_ref();
    let key_array: [u8; 32] = calldata.try_into().unwrap();
    let key = key_array.into();

    let bstring = bytestring_get(&key);
    let expected_bstring = &calldata[32..];

    if bstring.as_ref() != expected_bstring {
        panic!("unexpected bytestring");
    }
}

#[cfg(target_arch = "riscv32")]
mod rv32_entries {
    use sdk::__rv32::Entrypoint;

    macro_rules! raw_entrypoint {
        ($static_name:ident, $wrapper:ident, $method_id:expr, $body:path) => {
            extern "C" fn $wrapper() {
                $body();
                sdk::sys::raw_Return(&[], false, &[])
            }

            #[used]
            #[unsafe(link_section = "groundhog_rv32_entrypoints")]
            static $static_name: Entrypoint = Entrypoint {
                method_id: $method_id,
                wrapper: $wrapper,
            };
        };
    }

    raw_entrypoint!(WRITE, write_bytestring, 0x0, super::write_bytestring);
    raw_entrypoint!(LOG, log_bytestring, 0x1, super::log_bytestring);
    raw_entrypoint!(EXPECT, expect_bytestring, 0x2, super::expect_bytestring);
}
