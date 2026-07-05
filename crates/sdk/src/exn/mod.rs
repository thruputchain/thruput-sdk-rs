//! Exceptions (part of the SDK's call-wrapper)

pub(crate) mod cross_exn;
mod fault;
mod library;
mod wrapper;

use crate::xdr::*;
use core::fmt::Debug;

/// A trait for exception types that can be serialized across contract calls.
///
/// Each implementing type gets a unique u64 selector derived from its
/// canonical type signature (via `#[exn]`). The selector is used
/// for dispatch when deserializing exceptions on the client side.
pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {
    const SELECTOR: u64;
}

/// Exn impl for Infallible — this type can never be instantiated,
/// so its selector is 0 (a reserved/unused value).
impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0;
}

mod raw;
pub use raw::*;

pub use fault::*;
pub use library::*;
pub use wrapper::*;
