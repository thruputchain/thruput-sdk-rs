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
//!
//! The storage `try_` getters (`Value::try_get`, `NNInt::try_get`, the
//! `RCQ` `try_` reads, and their `sys_exn` free-function counterparts)
//! return `Result<T, LibraryExn>` where the `Err` is exactly the exception
//! the plain getter would have thrown (each plain getter is literally
//! `try_x().or_re_throw()`). Absence — a missing key or an empty queue —
//! is an ordinary, expected `Err` distinguished by
//! [`LibraryExn::is_storage_missing`]; use
//! [`NoneIfMissing::none_if_missing`] to treat just that case as `None`
//! while re-throwing everything else (wrong storage type at the key,
//! corrupt encodings, host faults).

pub(crate) mod abort_encode;
pub(crate) mod cross_exn;
/// Identity version carried by every spec id in the SDK's exception tree.
///
/// `LibraryExn` pins `#[exn(version = "1.0.0", wire_crate = "sdk")]` so its
/// selector holds still across SDK releases and package renames, and the
/// docs on `ContractTypeName` require an id to track whatever its type's
/// other identity derivations use. The types reachable from it
/// (`SyscallExn`, `FaultString`, and the `wrapper` errors) are part of that
/// same pinned ABI, so they take the same major rather than the crate's own
/// -- otherwise every breaking SDK release would silently rename them in
/// every spec that mentions them.
pub(crate) const EXN_ABI_MAJOR: &str = "1";

mod fault;
mod library;
mod wrapper;

pub use thruput_exn::{Exn, RawExceptionRef};

mod match_exn;
pub use match_exn::*;

mod raw;
pub use raw::*;

pub use fault::*;
pub use library::*;
pub use wrapper::*;
