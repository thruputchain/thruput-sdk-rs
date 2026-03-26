#![no_std]

use core::fmt::Debug;
use xdrs::{ReadXDROwned, WriteXDR};

/// A trait for error types that can be serialized across contract calls.
///
/// Each implementing type gets a unique u64 selector derived from its
/// canonical type signature (via `#[derive(Exn)]`). The selector is used
/// for dispatch when deserializing errors on the client side.
pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {
    const SELECTOR: u64;
}

/// Exn impl for Infallible -- this type can never be instantiated,
/// so its selector is 0 (a reserved/unused value).
impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0;
}
