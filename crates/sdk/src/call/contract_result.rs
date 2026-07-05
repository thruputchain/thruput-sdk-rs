//! The untyped result of a contract invocation.
//!
//! `ContractResult` can represent:
//! - contract success (with serialized return-value)
//! - contract exception (with serialized return-value)
//! - contract fault (with debugging message when `cfg(feature = "debugging")`)

use crate::{AuthHandleBuf, exn::FaultString};
use core::fmt::{Debug, Display};

/// Buffer for cross-contract return/abort payloads — the SDK's single
/// [`ScratchBuf`](crate::internal::ScratchBuf). With `alloc` it is a heap
/// `Vec` sized to the actual payload; in the no-alloc guest it is the fixed
/// [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)-byte buffer, and larger
/// payloads fail to parse.
pub type Buf = crate::internal::ScratchBuf;
pub type ReturnBuf = Buf;
pub type AbortBuf = Buf;

/// Copy `bytes` into a fresh [`Buf`], growing to fit (always succeeds; the
/// `Result` shape is kept for call-site uniformity). `alloc`-only mock-side
/// plumbing: the no-alloc guest never copies payload bytes into a fresh
/// buffer.
#[cfg(feature = "alloc")]
pub(crate) fn buf_from_slice(bytes: &[u8]) -> Result<Buf, ()> {
    Ok(bytes.to_vec())
}

pub type ContractResult = Result<(ReturnBuf, AuthHandleBuf), ContractError>;

// Note: `ContractError` is an in-memory Rust `Result` variant only — it is
// never XDR-serialized (aborts cross the boundary as raw bytes), so it
// carries no `ReadXDR`/`WriteXDR` derive.
#[derive(Debug, PartialEq)]
#[repr(i32)]
pub enum ContractError {
    /// Requested abort, providing a serialized return value
    /// of type specific to the method.
    Abort(AbortBuf),

    /// Terminated without providing a return value.
    ///
    /// When `cfg(feature = "debugging")`, this contains a string describing
    /// the fault (e.g., what was passed to `panic`).
    ///
    Fault(FaultString),
}

impl ContractError {
    pub fn fault_via_display<V>(v: &V) -> Self
    where
        V: Display + ?Sized,
    {
        ContractError::Fault(FaultString::via_display(v))
    }

    pub fn fault_from_display<V>(v: V) -> Self
    where
        V: Display,
    {
        ContractError::Fault(FaultString::from_display(v))
    }
}
