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

use crate::CONTRACT_ADDR_ZERO;
use core::fmt;
use groundhog_xdr::standard_asset_events as stdevt;
use sdk::{
    ContractAddress, EmittedEvent, Event, Topic, TopicSrc, WriteResult,
    WriteXDR, hash_to_array, hex_encode, log_event,
};
use xdrs::Writer;

pub fn log_transfer_event(
    from: &ContractAddress,
    to: &ContractAddress,
    amount: u128,
) {
    let event = TransferEvent::new(from, to, amount);

    log_event(&event);
}

/// An event emitted when ERC20 tokens are transferred between addresses.
/// If `from` is address zero, this is a "mint" event.
/// If `to` is address zero, is is a "burn" event.
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
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        // Don't serialize the addresses, which are topics
        self.amount.write_xdr(w)
    }
}

pub const TRANSFER_EVENT_SIG: &Topic = &hash_to_array!("TransferEvent");

impl<'a> Event for TransferEvent<'a> {
    fn num_topics(&self) -> u32 {
        3
    }

    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(TRANSFER_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.from)),
            2 => Some(TopicSrc::Shared(self.to)),
            _ => None,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for TransferEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != TRANSFER_EVENT_SIG {
            return Err("Unexpected event signature");
        }

        let from: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let to: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let amount: u128 = sdk::internal::decode_xdr(event.payload())
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(TransferEvent::new(from, to, amount))
    }
}

impl<'a> Into<stdevt::TransferEvent> for &TransferEvent<'a> {
    fn into(self) -> stdevt::TransferEvent {
        stdevt::TransferEvent {
            from_address: *self.from,
            to_address: *self.to,
            amount: TryInto::<i128>::try_into(self.amount).unwrap().into(),
        }
    }
}

impl<'a> fmt::Display for TransferEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.from == CONTRACT_ADDR_ZERO {
            write!(
                f,
                "MINT_TO({}, amount:{:?})",
                hex_encode(self.to),
                self.amount
            )
        } else if self.to == CONTRACT_ADDR_ZERO {
            write!(
                f,
                "BURN_FROM({}, amount:{:?})",
                hex_encode(self.from),
                self.amount
            )
        } else {
            write!(
                f,
                "TRANSFER({}, {}, amount:{:?})",
                hex_encode(self.from),
                hex_encode(self.to),
                self.amount,
            )
        }
    }
}

// ----------------------------------------------------------------------

pub fn log_approval_event(
    owner: &ContractAddress,
    spender: &ContractAddress,
    amount: i128,
) {
    let event = ApprovalEvent::new(owner, spender, amount);

    log_event(&event);
}

/// An event emitted when spending approval has been adjusted
/// by `owner` for `spender`.  *Note*: Contrary to traditional
/// ERC20 semantics, `amount` is the (signed) _difference_ in
/// tokens approved to spend.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovalEvent<'a> {
    pub owner: &'a ContractAddress,
    pub spender: &'a ContractAddress,
    pub difference: i128,
}

impl<'a> WriteXDR for ApprovalEvent<'a> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        // Don't serialize the addresses, which are topics
        self.difference.write_xdr(w)
    }
}

impl<'a> ApprovalEvent<'a> {
    pub fn new(
        owner: &'a ContractAddress,
        spender: &'a ContractAddress,
        difference: i128,
    ) -> Self {
        Self {
            owner,
            spender,
            difference,
        }
    }
}

pub const APPROVAL_EVENT_SIG: &Topic = &hash_to_array!("ApprovalEvent");

impl<'a> Event for ApprovalEvent<'a> {
    fn num_topics(&self) -> u32 {
        3
    }

    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(APPROVAL_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.owner)),
            2 => Some(TopicSrc::Shared(self.spender)),
            _ => None,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for ApprovalEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != APPROVAL_EVENT_SIG {
            return Err("Unexpected event signature");
        }

        let owner: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let spender: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let difference: i128 = sdk::internal::decode_xdr(event.payload())
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(ApprovalEvent::new(owner, spender, difference))
    }
}

impl<'a> Into<stdevt::ApprovalEvent> for &ApprovalEvent<'a> {
    fn into(self) -> stdevt::ApprovalEvent {
        stdevt::ApprovalEvent {
            owner_address: *self.owner,
            spender_address: *self.spender,
            amount: self.difference.into(),
        }
    }
}

impl<'a> fmt::Display for ApprovalEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "APPROVE({}, {}, diff:{:?})",
            hex_encode(self.owner),
            hex_encode(self.spender),
            self.difference
        )
    }
}

// ----------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ERC20Event<'a> {
    Transfer(TransferEvent<'a>),
    Approval(ApprovalEvent<'a>),
}

impl<'a> TryFrom<EmittedEvent<'a>> for ERC20Event<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        match sig {
            TRANSFER_EVENT_SIG => Ok(ERC20Event::Transfer(event.try_into()?)),
            APPROVAL_EVENT_SIG => Ok(ERC20Event::Approval(event.try_into()?)),
            _ => Err("Unexpected event signature"),
        }
    }
}

impl<'a> fmt::Display for ERC20Event<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ERC20_")?;

        match self {
            ERC20Event::Transfer(e) => e.fmt(f),
            ERC20Event::Approval(e) => e.fmt(f),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StdERC20Event {
    Transfer(stdevt::TransferEvent),
    Approval(stdevt::ApprovalEvent),
}

impl<'a> Into<StdERC20Event> for &ERC20Event<'a> {
    fn into(self) -> StdERC20Event {
        match self {
            ERC20Event::Transfer(e) => StdERC20Event::Transfer(e.into()),
            ERC20Event::Approval(e) => StdERC20Event::Approval(e.into()),
        }
    }
}
