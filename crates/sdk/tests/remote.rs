//! `#[contract]` with the `remote` feature generates `<Iface>Remote<I>`. Its
//! methods must send the guest client's selector and argument encoding, and
//! route only declared `pure` and `readOnly` methods to views.

#![allow(unexpected_cfgs)]

use std::marker::PhantomData;

use thruput_contract_util::{NormalizedMethodSignature, contract_entrypoint_id};
use thruput_sdk::__xdrs::{ReadXDROwned, WriteXDR, cxs};
use thruput_sdk::remote::{MethodEffect, RemoteInvoker, RemoteMethod};
use thruput_sdk::*;

#[contract(version = "1.2.3")]
pub trait Ledger {
    /// Balance held by `owner`.
    #[contract(effect = "readOnly")]
    fn balance(&self, owner: &ContractAddress) -> u64;

    #[contract(effect = "pure")]
    fn fee(&self) -> u64;

    #[contract(effect = "stateChanging")]
    fn transfer(&mut self, to: &ContractAddress, amount: u64);

    // Declared `&self`, but undeclared effects are never treated as reads.
    fn request(&self, amount: u64) -> u32;
}

/// An interface method named like a proxy accessor replaces that accessor.
#[allow(dead_code)]
mod shadowed {
    use thruput_sdk::*;

    #[contract(version = "1.0.0")]
    pub trait Located {
        #[contract(effect = "readOnly")]
        fn address(&self) -> u32;
    }
}

/// Records what the proxy hands its invoker.
#[derive(Debug, Clone)]
struct Recorder;

#[derive(Debug)]
struct Recorded<R> {
    kind: &'static str,
    target: ContractAddress,
    method: RemoteMethod,
    calldata: Vec<u8>,
    returns: PhantomData<fn() -> R>,
}

impl Recorder {
    fn record<R, A: WriteXDR + ?Sized>(
        kind: &'static str,
        target: &ContractAddress,
        method: RemoteMethod,
        args: &A,
    ) -> Recorded<R> {
        Recorded {
            kind,
            target: target.clone(),
            method,
            calldata: cxs::encode(args).unwrap(),
            returns: PhantomData,
        }
    }
}

impl RemoteInvoker for Recorder {
    type Call<R> = Recorded<R>;
    type View<R> = Recorded<R>;

    fn call<R: ReadXDROwned, A: WriteXDR + ?Sized>(
        &self,
        target: &ContractAddress,
        method: RemoteMethod,
        args: &A,
    ) -> Recorded<R> {
        Self::record("call", target, method, args)
    }

    fn view<R: ReadXDROwned, A: WriteXDR + ?Sized>(
        &self,
        target: &ContractAddress,
        method: RemoteMethod,
        args: &A,
    ) -> Recorded<R> {
        Self::record("view", target, method, args)
    }
}

fn entrypoint(method: &str) -> contract::ContractEntrypointID {
    contract_entrypoint_id(
        Some("Ledger"),
        "1.2.3",
        &NormalizedMethodSignature::new(method),
    )
}

#[test]
fn methods_carry_guest_selectors_and_encoding() {
    let target = ContractAddress::from([7; 32]);
    let owner = ContractAddress::from([9; 32]);
    let ledger = LedgerRemote::new(Recorder, &target);
    assert_eq!(ledger.address(), target);

    let balance: Recorded<u64> = ledger.balance(&owner);
    assert_eq!(balance.kind, "view");
    assert_eq!(balance.target, target);
    assert_eq!(
        balance.method,
        RemoteMethod {
            interface: Some("Ledger"),
            version: "1.2.3",
            name: "balance",
            entrypoint: entrypoint("balance"),
            effect: Some(MethodEffect::ReadOnly),
        }
    );
    // A single argument is sent by itself, not as a 1-tuple.
    assert_eq!(balance.calldata, cxs::encode(&owner).unwrap());

    let fee: Recorded<u64> = ledger.fee();
    assert_eq!(fee.kind, "view");
    assert_eq!(fee.method.effect, Some(MethodEffect::Pure));
    assert_eq!(fee.calldata, cxs::encode(&()).unwrap());

    let transfer: Recorded<()> = ledger.transfer(&owner, 5);
    assert_eq!(transfer.kind, "call");
    assert_eq!(transfer.method.entrypoint, entrypoint("transfer"));
    assert_eq!(transfer.calldata, cxs::encode(&(&owner, 5u64)).unwrap());
}

#[test]
fn undeclared_effects_are_calls() {
    let ledger = LedgerRemote::new(Recorder, &ContractAddress::from([7; 32]));
    let request: Recorded<u32> = ledger.request(3);
    assert_eq!(request.kind, "call");
    assert_eq!(request.method.effect, None);
    assert!(!request.method.is_read_only());
}

#[test]
fn interface_method_replaces_accessor() {
    let located =
        shadowed::LocatedRemote::new(Recorder, &ContractAddress::from([1; 32]));
    let address: Recorded<u32> = located.address();
    assert_eq!(address.kind, "view");
    assert_eq!(address.method.name, "address");
}
