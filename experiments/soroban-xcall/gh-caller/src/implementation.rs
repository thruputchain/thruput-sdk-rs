//! Groundhog -> Soroban. A Soroban contract hosted by a soroban-xcall
//! Groundhog contract is called through that contract's method 0 with an XDR
//! `HostFunction::InvokeContract`; the reply is an XDR `ScVal`. The encoding
//! is small enough to write by hand, which is the point: any Groundhog
//! contract can do this with no Soroban dependency.
use sdk::call::{CallOutcome, contract_call};
use sdk::contract::ContractEntrypointID;
use sdk::types::ContractAddress;
use sdk::*;

extern crate alloc;
use alloc::vec::Vec;

#[contract_exception]
pub enum Error {
    /// The Soroban contract failed (its Groundhog frame reverted or trapped).
    SorobanCallFailed = 1,
    /// The reply was not an `ScVal::U32`.
    UnexpectedReturn = 2,
}

pub struct GhCaller;

/// XDR `HostFunction::InvokeContract { contract: target, fn, args: [U32(arg)] }`.
fn invoke_u32(target: &ContractAddress, func: &str, arg: u32) -> Vec<u8> {
    let mut x = Vec::new();
    x.extend_from_slice(&0u32.to_be_bytes()); // HOST_FUNCTION_TYPE_INVOKE_CONTRACT
    x.extend_from_slice(&1u32.to_be_bytes()); // SC_ADDRESS_TYPE_CONTRACT
    x.extend_from_slice(target.as_ref()); // contract id = Groundhog address
    x.extend_from_slice(&(func.len() as u32).to_be_bytes()); // ScSymbol
    x.extend_from_slice(func.as_bytes());
    x.resize(x.len() + (4 - func.len() % 4) % 4, 0);
    x.extend_from_slice(&1u32.to_be_bytes()); // args: 1 element
    x.extend_from_slice(&3u32.to_be_bytes()); // SCV_U32
    x.extend_from_slice(&arg.to_be_bytes());
    x
}

#[contract(throws = Error, client = GhCallerClient)]
impl GhCaller {
    /// Calls `increment(by)` on the Soroban contract at `target` and returns
    /// the new count.
    pub fn soroban_increment(target: ContractAddress, by: u32) -> u32 {
        let calldata = invoke_u32(&target, "increment", by);
        let mut ret = Vec::new();
        let mut ret_auth = Vec::new();
        let outcome = contract_call(
            &target,
            ContractEntrypointID::from_u64(0),
            u64::MAX,
            &calldata[..],
            &Vec::new(),
            Default::default(),
            &mut ret,
            &mut ret_auth,
        );
        match outcome {
            Ok(CallOutcome::Return) => {}
            _ => throw(Error::SorobanCallFailed),
        }
        // SCV_U32 = 3, then the value.
        match ret.as_slice() {
            [0, 0, 0, 3, a, b, c, d] => u32::from_be_bytes([*a, *b, *c, *d]),
            _ => throw(Error::UnexpectedReturn),
        }
    }
}
