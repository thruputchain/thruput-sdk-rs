//! The untyped result of a contract invocation.
//!
//! `ContractResult` can represent:
//! - contract success (with serialized return-value)
//! - contract exception (with serialized return-value)
//! - contract fault (with debugging message when `cfg(feature = "debugging")`)

use crate::xdr::*;
use crate::{AuthHandleBuf, exn::FaultString};
use core::fmt::{Debug, Display};

const ARGS_MAX: usize = crate::contract_op::CALLDATA_MAX;
pub type ArgsBuf = heapless::Vec<u8, ARGS_MAX>;

/// Buffer for cross-contract return/abort payloads.
///
/// With the `alloc` feature this is a heap `Vec` sized to the actual
/// payload, so any return up to the protocol's cap
/// (`kMaxReturnBufferLen` = 1024) is supported. Without `alloc` it is a
/// fixed 500-byte buffer and larger payloads fail to parse.
#[cfg(feature = "alloc")]
pub type Buf = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub type Buf = heapless::Vec<u8, 500>;
pub type ReturnBuf = Buf;
pub type AbortBuf = Buf;

/// Copy `bytes` into a fresh [`Buf`]. Without `alloc`, input beyond the
/// fixed capacity is truncated.
pub(crate) fn buf_from_slice(bytes: &[u8]) -> Buf {
    #[cfg(feature = "alloc")]
    {
        bytes.to_vec()
    }
    #[cfg(not(feature = "alloc"))]
    {
        let mut buf = Buf::new();
        let take = bytes.len().min(buf.capacity());
        buf.extend_from_slice(&bytes[..take]).ok();
        buf
    }
}

pub type ContractResult = Result<(ReturnBuf, AuthHandleBuf), ContractError>;

#[derive(Debug, PartialEq, ReadXDR, WriteXDR)]
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
