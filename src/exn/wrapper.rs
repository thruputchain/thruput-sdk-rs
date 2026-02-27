use crate::exn::Exn;
use crate::exn::fault::*;
use crate::sys::SysErr;
use crate::xdr::*;
use core::fmt::Debug;
use thiserror::Error;

pub trait Throws {
    type Error: Exn;
}

// TODO: Maybe make the fault-strings into Error sources,
// and figure out how to Display them at the right times.

/// Plumbing errors resulting from making a contract call:
#[derive(Debug, Clone, PartialEq, Error, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum CallingError {
    #[error("Decoding a successful result: {0}")]
    DecodingSuccess(FaultString),

    #[error("Decoding an abort result: {0}")]
    DecodingAbort(FaultString),

    #[error("Encoding arguments: {0}")]
    EncodingArgs(FaultString),

    #[error("Invoke syscall failed: {0}")]
    Syscall(SysErr),
}

/// Plumbing errors resulting from handling a contract call:
#[derive(Debug, Clone, PartialEq, Error, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum HandlingError {
    #[error("Decoding argument data: {0}")]
    DecodingArgs(FaultString),

    #[error("Encoding successful result: {0}")]
    EncodingSuccess(FaultString),

    #[error("Encoding abort result: {0}")]
    EncodingAbort(FaultString),
}
