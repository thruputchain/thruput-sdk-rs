use crate::ContractTypeSpec;
use crate::exn::fault::*;
use crate::xdr::*;
use core::fmt::Debug;
#[cfg(feature = "debugging")]
use thiserror::Error;

// TODO: Maybe make the fault-strings into Error sources,
// and figure out how to Display them at the right times.

/// Plumbing errors resulting from making a contract call:
#[derive(
    Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(feature = "debugging", derive(Error))]
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

/// `Result<T, CallingError>::or_re_throw()` aborts with the typed
/// `LibraryExn::Calling` exception (client-side interface path).
impl crate::re_throw::ReThrow for CallingError {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn re_throw(self) -> ! {
        crate::exn::client_library_throw(self)
    }
}

/// Plumbing errors resulting from handling a contract call:
#[derive(
    Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(feature = "debugging", derive(Error))]
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
#[derive(
    Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(feature = "debugging", derive(Error))]
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

impl crate::__contract_spec::ContractTypeName for CallingError {
    const NAME: &'static str = "CallingError";
}

impl crate::__contract_spec::ContractTypeName for HandlingError {
    const NAME: &'static str = "HandlingError";
}

impl crate::__contract_spec::ContractTypeName for AuthTokenError {
    const NAME: &'static str = "AuthTokenError";
}
