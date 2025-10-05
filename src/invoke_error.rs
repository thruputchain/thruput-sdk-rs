//! Errors resulting from cross-contract calls
//!
//! When a contract-client object is generated, if the
//! contract specifies #[contract(error = E)],
//! the contract methods on the client object will return
//! values of the form `ClientResult<T, E>`.  Otherwise
//! methods will return `InvokeResult<T>`, which simply
//! provides a byte-buffer in the case of contract-abort.

use thiserror::Error;
use core::error::Error;
use crate::SysErr;
use xdrs::{ReadXDROwned, ReadXDR, WriteXDR};

// XXX: "Client" error?  naming vs InvokeError?
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[derive(ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ClientError<E: Error + ReadXDROwned + WriteXDR> {
    #[error("client invocation failed: {0}")]
    Invoke(#[from] InvokeError),

    #[error("client aborted: {0}")] // XXX peer contract aborted?
    Aborted(#[source] E),

    // XXX peer contract aborted but _we_ could not decode the error value
    #[error("client aborted but could not decode error value")]
    AbortDecodeFailed,
}

pub type ClientResult<T, E> = Result<T, ClientError<E>>;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[derive(ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum InvokeError {
    #[error("error encoding arguments")]
    EncodeArgs,

    #[error("error decoding return value")]
    DecodeReturnVal,

    #[error("invoke() syscall failed: {0}")]
    Syscall(SysErr),

    #[error("contract aborted")]
    Aborted,
}

pub type InvokeResult<T> = Result<T, InvokeError>;
