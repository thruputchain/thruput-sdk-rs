//! ERC20 Events
//!
//! See (EIP-20)[https://eips.ethereum.org/EIPS/eip-20#events].
//!
//! # Transfer
//!
//! `event Transfer(address indexed _from, address indexed _to, uint256 _value)`
//!
//! "MUST trigger when tokens are transferred, including zero value transfers. A token contract
//! which creates new tokens SHOULD trigger a Transfer event with the `_from` address set to `0x0`
//! when tokens are created."
//!
//! # Approval
//!
//! `event Approval(address indexed _owner, address indexed _spender, uint256 _value)`
//!
//! "MUST trigger on any successful call to `approve(address _spender, uint256 _value)`."

use sdk::{
    ContractAddress, Event, Topic, TopicSrc, WriteResult, WriteXDR, log_event,
    sysresult::Result, topic_hashed_from,
};
use xdrs::Writer;

// TODO: conversion of the types defined below into those in `groundhog_xdr::standard_asset_events`

pub fn log_transfer_event(
    from: &ContractAddress,
    to: &ContractAddress,
    amount: u128,
) -> Result<()> {
    let event = TransferEvent::new(from, to, amount);

    log_event(&event)
}

pub fn log_approval_event(
    owner: &ContractAddress,
    spender: &ContractAddress,
    amount: i128,
) -> Result<()> {
    let event = ApprovalEvent::new(owner, spender, amount);

    log_event(&event)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransferEvent<'a> {
    pub from: &'a ContractAddress,
    pub to: &'a ContractAddress,
    pub amount: u128,
}

impl<'a> TransferEvent<'a> {
    pub fn new(
        from: &'a ContractAddress,
        to: &'a ContractAddress,
        amount: u128,
    ) -> Self {
        Self { from, to, amount }
    }
}

impl<'a> WriteXDR for TransferEvent<'a> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult {
        // Don't serialize the addresses, which are topics
        self.amount.write_xdr(w)
    }
}

pub const TRANSFER_EVENT_SIG: &Topic = &topic_hashed_from!("TransferEvent");

impl<'a> Event for TransferEvent<'a> {
    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(TRANSFER_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.from)),
            2 => Some(TopicSrc::Shared(self.to)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovalEvent<'a> {
    pub owner: &'a ContractAddress,
    pub spender: &'a ContractAddress,
    pub amount: i128,
}

impl<'a> WriteXDR for ApprovalEvent<'a> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult {
        // Don't serialize the addresses, which are topics
        self.amount.write_xdr(w)
    }
}

impl<'a> ApprovalEvent<'a> {
    pub fn new(
        owner: &'a ContractAddress,
        spender: &'a ContractAddress,
        amount: i128,
    ) -> Self {
        Self {
            owner,
            spender,
            amount,
        }
    }
}

pub const APPROVAL_EVENT_SIG: &Topic = &topic_hashed_from!("ApprovalEvent");

impl<'a> Event for ApprovalEvent<'a> {
    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(APPROVAL_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.owner)),
            2 => Some(TopicSrc::Shared(self.spender)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ERC20Event<'a> {
    Transfer(TransferEvent<'a>),
    Approval(ApprovalEvent<'a>),
}

// TODO: generate this code via the same macros that will derive `Event`, etc.
#[cfg(feature = "mock")]
mod emitted {
    use sdk::types::event::TxEmittedEvent;
    use super::*;
    use sdk::{event_topic, event_payload};
    use core::fmt;
    use crate::CONTRACT_ADDR_ZERO;

    struct Hex<'a>(&'a [u8]);

    impl<'a> fmt::Display for Hex<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
            // f.write_str("hex!(\"")?;
            for b in self.0 {
                write!(f, "{:02x}", b)?;
            }
            // f.write_str("\")")?;
            Ok(())
        }
    }

    fn hex_encode<'a, B: AsRef<[u8]>>(b: &'a B) -> Hex<'a> {
        Hex(b.as_ref())
    }

    pub fn to_erc20_event(e: &TxEmittedEvent) -> ERC20Event<'_> {
        let sig = event_topic(e, 0).expect("no event signature");
        match sig {
            TRANSFER_EVENT_SIG => {
                ERC20Event::Transfer(to_transfer_event(e))
            }
            APPROVAL_EVENT_SIG => {
                ERC20Event::Approval(to_approval_event(e))
            }
            _ => panic!("Unexpected event signature: {}", hex_encode(sig)),
        }
    }

    fn to_transfer_event(e: &TxEmittedEvent) -> TransferEvent<'_> {
        let sig = event_topic(e, 0).expect("no event signature");
        assert_eq!(sig, TRANSFER_EVENT_SIG, "Unexpected event signature");

        let from: &ContractAddress = event_topic(e, 1).expect("Missing topic").into();
        let to: &ContractAddress = event_topic(e, 2).expect("Missing topic").into();

        let payload = event_payload(e).expect("malformed event");
        let amount: u128 = sdk::internal::decode_xdr(payload).expect("couldn't parse event payload");

        TransferEvent::new(from, to, amount)
    }

    fn to_approval_event(e: &TxEmittedEvent) -> ApprovalEvent<'_> {
        let sig = event_topic(e, 0).expect("no event signature");
        assert_eq!(sig, APPROVAL_EVENT_SIG, "Unexpected event signature");

        let owner: &ContractAddress = event_topic(e, 1).expect("Missing topic").into();
        let spender: &ContractAddress = event_topic(e, 2).expect("Missing topic").into();

        let payload = event_payload(e).expect("malformed event");
        let amount: i128 = sdk::internal::decode_xdr(payload).expect("couldn't parse event payload");

        ApprovalEvent::new(owner, spender, amount)
    }

    pub fn print_event(e: &TxEmittedEvent) {
        let sig = event_topic(e, 0).expect("no event signature");

        match sig {
            TRANSFER_EVENT_SIG => {
                let from: &ContractAddress =
                    event_topic(e, 1).expect("no from address").into();
                let to: &ContractAddress =
                    event_topic(e, 2).expect("no to address").into();
                let payload = event_payload(e).expect("malformed event");
                let amount: u128 = sdk::internal::decode_xdr(payload).unwrap();

                if from == CONTRACT_ADDR_ZERO {
                    println!(
                        "\tMINT_TO {} amount:{:?}",
                        hex_encode(to),
                        amount
                    );
                } else if to == CONTRACT_ADDR_ZERO {
                    println!(
                        "\tBURN_FROM {} amount:{:?}",
                        hex_encode(from),
                        amount
                    );
                } else {
                    println!(
                        "\tTRANSFER {} {} amount:{:?}",
                        hex_encode(from),
                        hex_encode(to),
                        amount,
                    );
                }
            }
            APPROVAL_EVENT_SIG => {
                let payload = event_payload(e).expect("malformed event");
                let amount: i128 = sdk::internal::decode_xdr(payload).unwrap();
                println!("\tAPPROVE {:?}", amount);
            }
            _ => {
                panic!("Unexpected topic signature");
            }
        }
    }
}
pub use emitted::*;
