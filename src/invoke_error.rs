//! Errors resulting from cross-contract calls
//!
//! When a contract-client object is generated, if the
//! contract specifies #[contract(error = E)],
//! the contract methods on the client object will return
//! values of the form `ClientResult<T, E>`.  Otherwise
//! methods will return `InvokeResult<T>`, which simply
//! provides a byte-buffer in the case of contract-abort.

use crate::SysErr;
use crate::internal::{ReadXDR, ReadXDROwned, WriteXDR};
use core::error::Error;
use thiserror::Error;

// TODO: Reconsider naming of `ClientError`, `InvokeError`

#[derive(Debug, Clone, Error, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ClientError<E: Error + ReadXDROwned + WriteXDR> {
    #[error("contract invocation failed: {0}")]
    Invoke(#[from] InvokeError),

    #[error("the called contract aborted: {0}")]
    #[allow(clippy::large_enum_variant)]
    Aborted(#[source] E),

    #[error(
        "the called contract aborted, but we couldn't decode the \
            returned error value"
    )]
    AbortDecodeFailed,
}

pub type ClientResult<T, E> = Result<T, ClientError<E>>;

#[derive(Debug, Clone, Error, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum InvokeError {
    #[error("error encoding contract-call arguments")]
    EncodeArgs,

    #[error("error decoding contract-call return-value")]
    DecodeReturnVal,

    #[error("invoke() syscall failed: {0}")]
    Syscall(SysErr),

    #[error("the called contract aborted")]
    Aborted,

    // XXX temporary
    #[error("the called contract aborted with return-data of length {0}")]
    AbortedWith(u32),
}

pub type InvokeResult<T> = Result<T, InvokeError>;
