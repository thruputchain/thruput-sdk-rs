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
use crate::syscall::SysErr;
use xdrs::{ReadXDROwned, ReadXDR, WriteXDR};
use alloc::vec::Vec;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[derive(ReadXDR, WriteXDR)]
pub enum ClientError<E: Error + ReadXDROwned + WriteXDR> {
    #[error("client invocation failed: {0}")]
    Invoke(#[from] InvokeError),

    #[error("client aborted: {0}")]
    Aborted(#[source] E),

    #[error("client aborted but could not decode error value")]
    AbortDecodeFailed,
}

pub type ClientResult<T, E> = Result<T, ClientError<E>>;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[derive(ReadXDR, WriteXDR)]
pub enum InvokeError {
    #[error("invoke: error encoding arguments")]
    EncodeArgs,

    #[error("invoke: error decoding return value from {0:?}")]
    DecodeReturnVal(Vec<u8>),

    #[error("invoke: error making a syscall")]
    Syscall(#[from] SysErr),

    #[error("invoke: contract aborted")]
    Aborted(Vec<u8>),
}

pub type InvokeResult<T> = Result<T, InvokeError>;
