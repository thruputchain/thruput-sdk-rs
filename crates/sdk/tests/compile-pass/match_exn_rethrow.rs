//! `_ => e.re_throw()` is the documented way to propagate an exception
//! `match_exn!` did not handle. It *moves* the scrutinee that the macro is
//! holding a borrow of, so it has to typecheck — and it aborts the frame, so
//! it must not actually run here.
#![allow(unexpected_cfgs)]

use sdk::*;

#[contract_exception]
pub enum AlphaError {
    Denied = 1,
}

#[contract(client = RethrowClient, throws(AlphaError))]
pub trait Rethrow {
    fn ping();
}

/// Never called: `re_throw` aborts the frame.
#[allow(dead_code)]
fn handle(e: CallError) -> u32 {
    match_exn!(e, {
        AlphaError::Denied => 1,
        LibraryExn::NonNegativeIntOverflow => 2,
        _ => e.re_throw(),
    })
}

fn main() {}
