//! Panic-handling for the test environment

pub fn init() {
    // std::panic::set_hook(Box::new(&panic_hook));
}

/*
use crate::*;
use std::panic::PanicHookInfo;

// Translate panic messages into a `Panic` payload
//
fn panic_hook(info: &PanicHookInfo) {

    if let Some(message) = info.payload_as_str() {
        // This was the result of `panic!(...)`

        let exn = Panic::from_hook_info(message, info.location());

        library_panic(exn);
    } else {
        let exn = Panic::from_hook_info("<unrecognized payload>", info.location());

        library_panic(exn);
    }
}
*/
