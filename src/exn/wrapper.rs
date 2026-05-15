use crate::exn::fault::*;
use crate::xdr::*;
use core::fmt::Debug;
#[cfg(feature = "debugging")]
use thiserror::Error;
#[cfg(all(feature = "spec", not(feature = "debugging")))]
use crate::ContractTypeSpec;

// TODO: Maybe make the fault-strings into Error sources,
// and figure out how to Display them at the right times.

/// Plumbing errors resulting from making a contract call:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(all(feature = "spec", not(feature = "debugging")), derive(ContractTypeSpec))]
#[repr(i32)]
pub enum CallingError {
    #[cfg_attr(
        feature = "debugging",
        error("Invalid call: contract not found (or bad auth tokens)")
    )]
    Invalid = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Decoding a successful result: {0}")
    )]
    DecodingSuccess(FaultString) = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Decoding an exception result: {0}")
    )]
    DecodingException(FaultString) = 2,

    #[cfg_attr(feature = "debugging", error("Encoding arguments: {0}"))]
    EncodingArgs(FaultString) = 3,
}

/// Plumbing errors resulting from handling a contract call:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(all(feature = "spec", not(feature = "debugging")), derive(ContractTypeSpec))]
#[repr(i32)]
pub enum HandlingError {
    #[cfg_attr(feature = "debugging", error("Decoding argument data: {0}"))]
    DecodingArgs(FaultString) = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Encoding successful result: {0}")
    )]
    EncodingSuccess(FaultString) = 1,

    #[cfg_attr(feature = "debugging", error("Encoding exception result: {0}"))]
    EncodingException(FaultString) = 2,

    #[cfg_attr(feature = "debugging", error("Encoding event: {0}"))]
    EncodingEvent(FaultString) = 3,
}

/// Plumbing errors resulting from working with auth tokens:
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(all(feature = "spec", not(feature = "debugging")), derive(ContractTypeSpec))]
#[repr(i32)]
pub enum AuthTokenError {
    #[cfg_attr(feature = "debugging", error("Encoding auth token: {0}"))]
    MintEncodeFailure(FaultString) = 0,
    #[cfg_attr(
        feature = "debugging",
        error("Decoding auth token payload: {0}")
    )]
    ConsumeDecodeFailure(FaultString) = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Receiving auth token handles: {0}")
    )]
    ReceiveAuthHandlesFailure(FaultString) = 2,
}

#[cfg(all(feature = "spec", not(feature = "debugging")))]
impl crate::__contract_spec::ContractTypeName for CallingError {
    const NAME: &'static str = "CallingError";
}

#[cfg(all(feature = "spec", not(feature = "debugging")))]
impl crate::__contract_spec::ContractTypeName for HandlingError {
    const NAME: &'static str = "HandlingError";
}

#[cfg(all(feature = "spec", not(feature = "debugging")))]
impl crate::__contract_spec::ContractTypeName for AuthTokenError {
    const NAME: &'static str = "AuthTokenError";
}
