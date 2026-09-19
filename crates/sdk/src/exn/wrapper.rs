use crate::ContractTypeSpec;
use crate::exn::fault::*;
use crate::xdr::*;
use core::fmt::Debug;
#[cfg(not(target_arch = "riscv32"))]
use thiserror::Error;

// TODO: Maybe make the fault-strings into Error sources,
// and figure out how to Display them at the right times.

/// Plumbing errors resulting from making a contract call:
#[derive(
    Debug, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(not(target_arch = "riscv32"), derive(Error))]
#[repr(i32)]
pub enum CallingError {
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Invalid call: contract not found (or bad auth tokens)")
    )]
    Invalid = 0,

    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Decoding a successful result: {0}")
    )]
    DecodingSuccess(FaultString) = 1,

    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Decoding an exception result: {0}")
    )]
    DecodingException(FaultString) = 2,

    #[cfg_attr(not(target_arch = "riscv32"), error("Encoding arguments: {0}"))]
    EncodingArgs(FaultString) = 3,

    /// The target address already had a frame open on the invocation stack,
    /// so the runtime refused the call as reentrant. The callee never ran.
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Invalid call: target already has an open frame (reentrant)")
    )]
    Reentrance = 4,

    /// Admitting the call would have exceeded the network's cross-invoke
    /// depth bound. The callee never ran; the same call succeeds from a
    /// shallower frame.
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Invalid call: cross-invoke depth bound reached")
    )]
    DepthExceeded = 5,
}

/// `Result<T, CallingError>::or_re_throw()` aborts with the typed
/// `LibraryExn::Calling` exception (client-side interface path).
impl crate::re_throw::ReThrow for CallingError {
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> ! {
        crate::exn::client_library_throw(self)
    }
}

/// Plumbing errors resulting from handling a contract call:
#[derive(
    Debug, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(not(target_arch = "riscv32"), derive(Error))]
#[repr(i32)]
pub enum HandlingError {
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Decoding argument data: {0}")
    )]
    DecodingArgs(FaultString) = 0,

    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Encoding successful result: {0}")
    )]
    EncodingSuccess(FaultString) = 1,

    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Encoding exception result: {0}")
    )]
    EncodingException(FaultString) = 2,

    #[cfg_attr(not(target_arch = "riscv32"), error("Encoding event: {0}"))]
    EncodingEvent(FaultString) = 3,
}

/// Plumbing errors resulting from working with auth tokens:
#[derive(
    Debug, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec, ConstXDRSize,
)]
#[cfg_attr(not(target_arch = "riscv32"), derive(Error))]
#[repr(i32)]
pub enum AuthTokenError {
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Encoding auth token: {0}")
    )]
    MintEncodeFailure(FaultString) = 0,
    #[cfg_attr(
        not(target_arch = "riscv32"),
        error("Decoding auth token payload: {0}")
    )]
    ConsumeDecodeFailure(FaultString) = 1,

    #[cfg_attr(
        not(target_arch = "riscv32"),
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
