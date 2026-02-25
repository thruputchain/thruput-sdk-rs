//! Panic-handling for the test environment

/*
pub fn init() {
    // std::panic::set_hook(Box::new(&panic_hook));
}
*/

/* XXX
use crate::*;
use heapless::String;
use groundhog_native_exec::invoke_error::InvokeError;
use std::boxed::Box;
use std::eprintln;
use std::fmt::Write;
use std::panic::{Location, PanicHookInfo, resume_unwind};

// Translate panic messages into a `Panic` payload
//
fn panic_hook(info: &PanicHookInfo) {

    if let Some(message) = info.payload_as_str() {
        // This was the result of `panic!(...)`

        let exn = Panic::from_hook_info(message, info.location());

        // stash exn somewhere?

        // resume_unwind(Box::new(exn));
    } else {
        // resume_unwind(Box::new(info.payload()));
    }
}

impl Panic {
    pub fn from_hook_info(message: &str, location: Option<&Location>) -> Self {
        let mut write_failed = false;

        let mut msg = String::new();

        if write!(&mut msg, "{}", message).is_err() {
            write_failed = true;
        }

        if let Some(loc) = location
            && write!(&mut msg, " at {}", loc).is_err()
        {
            write_failed = true;
        }

        Self { msg, write_failed }
    }
}
*/
