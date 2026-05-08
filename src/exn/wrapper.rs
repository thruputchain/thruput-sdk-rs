use crate::exn::fault::*;
use crate::xdr::*;
use core::fmt::Debug;
#[cfg(feature = "debugging")]
use thiserror::Error;

// TODO: Maybe make the fault-strings into Error sources,
// and figure out how to Display them at the right times.

/// Plumbing errors resulting from making a contract call:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[repr(i32)]
pub enum CallingError {
    #[cfg_attr(
        feature = "debugging",
        error("Invalid call: contract not found (or bad auth tokens)")
    )]
    Invalid,

    #[cfg_attr(
        feature = "debugging",
        error("Decoding a successful result: {0}")
    )]
    DecodingSuccess(FaultString),

    #[cfg_attr(
        feature = "debugging",
        error("Decoding an exception result: {0}")
    )]
    DecodingException(FaultString),

    #[cfg_attr(feature = "debugging", error("Encoding arguments: {0}"))]
    EncodingArgs(FaultString),
}

/// Plumbing errors resulting from handling a contract call:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[repr(i32)]
pub enum HandlingError {
    #[cfg_attr(feature = "debugging", error("Decoding argument data: {0}"))]
    DecodingArgs(FaultString),

    #[cfg_attr(
        feature = "debugging",
        error("Encoding successful result: {0}")
    )]
    EncodingSuccess(FaultString),

    #[cfg_attr(feature = "debugging", error("Encoding exception result: {0}"))]
    EncodingException(FaultString),

    #[cfg_attr(feature = "debugging", error("Encoding event: {0}"))]
    EncodingEvent(FaultString),
}

/// Plumbing errors resulting from working with auth tokens:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[repr(i32)]
pub enum AuthTokenError {
    #[cfg_attr(feature = "debugging", error("Encoding auth token: {0}"))]
    MintEncodeFailure(FaultString),
    #[cfg_attr(
        feature = "debugging",
        error("Decoding auth token payload: {0}")
    )]
    ConsumeDecodeFailure(FaultString),

    #[cfg_attr(
        feature = "debugging",
        error("Receiving auth token handles: {0}")
    )]
    ReceiveAuthHandlesFailure(FaultString),
}
