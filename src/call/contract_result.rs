//! The untyped result of a contract invocation.
//!
//! `ContractResult` can represent:
//! - contract success (with serialized return-value)
//! - contract abort (with serialized return-value)
//! - contract fault (with debugging message when `cfg(feature = "debugging")`)

use crate::exn::FaultString;
use crate::xdr::*;
use core::fmt::{Debug, Display};

const ARGS_MAX: usize = crate::contract_op::CALLDATA_MAX;
pub type ArgsBuf = heapless::Vec<u8, ARGS_MAX>;

pub type Buf = heapless::Vec<u8, 500>;
pub type ReturnBuf = Buf;
pub type AbortBuf = Buf;

pub type ContractResult = Result<ReturnBuf, ContractError>;

pub fn contract_result_should_commit(cr: &ContractResult) -> bool {
    cr.is_ok()
}

pub fn contract_result_should_revert(cr: &ContractResult) -> bool {
    cr.is_err()
}

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
