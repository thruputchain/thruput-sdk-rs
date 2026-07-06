//! Exceptions (part of the SDK's call-wrapper)
//!
//! **The `try_` law**: every `pub fn try_*` in this SDK either cannot abort
//! the VM at all, or can abort it only by invoking a caller-supplied
//! closure — never on its own account. Concretely, that means a `try_`
//! function's own control flow (buffer fetches, syscalls it issues itself,
//! decoding) reports failure through its return type (`Option`/`Result`);
//! it does not call `library_throw`/`contract_abort` internally except by
//! running `f`. This is what makes `try_` a meaningful opt-out of the
//! throwing convention: callers can rely on the `try_` spelling to never
//! surprise them with an abort that their own closure didn't cause.
//! [`RawExceptionRef::try_as`], [`RawException::try_as`], and
//! [`crate::call::try_call_raw_with`] hold this law
//! (the list above is illustrative, not exhaustive). Some longer-established
//! `try_` storage getters are the one exception class: they predate this law
//! and instead follow an older, narrower contract — "`None` on a missing
//! key, but still aborts on other storage/decode invariant violations" —
//! documented at each such call site.

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
