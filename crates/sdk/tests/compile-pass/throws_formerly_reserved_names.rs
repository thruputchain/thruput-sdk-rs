//! Names that `throws(...)` used to reject.
//!
//! The generated client error enum gave each declared type a variant named
//! after its last path segment, so `Library`/`Fault`/`Unknown` were reserved
//! (they named built-in variants), two types sharing a last segment
//! collided, and re-declaring `LibraryExn` aliased the implicit `Library`
//! arm. Exceptions are matched by selector now, there is no enum, and none
//! of those are collisions any more.
#![allow(unexpected_cfgs)]

use thruput_sdk::*;

mod errors {
    use thruput_sdk::*;

    #[contract_exception]
    pub struct Library;

    #[contract_exception]
    pub struct Fault;

    #[contract_exception]
    pub struct Unknown;
}

#[contract(throws(
    errors::Library,
    errors::Fault,
    errors::Unknown,
    thruput_sdk::exn::LibraryExn
))]
pub trait FormerlyReservedNames {
    fn ping();
}

fn classify(e: &CallError) -> &'static str {
    match_exn!(e, {
        errors::Library => "library",
        errors::Fault => "fault",
        errors::Unknown => "unknown",
        LibraryExn::NonNegativeIntOverflow => "sdk-overflow",
        _ => "other",
    })
}

fn main() {
    assert_eq!(classify(&CallError::Invalid), "other");
}
