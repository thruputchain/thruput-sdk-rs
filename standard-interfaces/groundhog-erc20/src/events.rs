//! ERC20 Events
//!
//! See [EIP-20].
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
//!
//! [EIP-20]: https://eips.ethereum.org/EIPS/eip-20#events

use crate::CONTRACT_ADDR_ZERO;
use core::fmt;
use groundhog_xdr::standard_asset_events as stdevt;
use sdk::{
    ContractAddress, EmittedEvent, Event, ReadXDR, WriteXDR, hex_encode,
    log_event,
};

pub fn log_transfer_event(
    from: &ContractAddress,
    to: &ContractAddress,
    amount: u128,
) {
    // The event carries `amount` as i128 (matching the standard
    // `stdevt::TransferEvent`). `amount_to_diff` rejects values above
    // `i128::MAX` with `ERC20Error::TooLarge` — the same bound a transfer
    // already enforces — so the conversion to the standard form is infallible.
    let event =
        TransferEvent::new(from, to, crate::error::amount_to_diff(amount));

    log_event(&event);
}

/// An event emitted when ERC20 tokens are transferred between addresses.
/// If `from` is address zero, this is a "mint" event.
/// If `to` is address zero, is is a "burn" event.
#[derive(Event, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransferEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    #[topic]
    pub to: &'a ContractAddress,
    /// The transferred amount. Always non-negative; typed `i128` to match the
    /// standard [`stdevt::TransferEvent`], and bounded to `i128::MAX` on-chain
    /// (see [`log_transfer_event`]). For in-range values the CXS encoding is
    /// identical to the previous `u128`.
    pub amount: i128,
}

impl<'a> TransferEvent<'a> {
    pub fn new(
        from: &'a ContractAddress,
        to: &'a ContractAddress,
        amount: i128,
    ) -> Self {
        Self { from, to, amount }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for TransferEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != TransferEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let from: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let to: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let amount: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(TransferEvent::new(from, to, amount))
    }
}

impl<'a> From<&TransferEvent<'a>> for stdevt::TransferEvent {
    fn from(val: &TransferEvent<'a>) -> Self {
        stdevt::TransferEvent {
            from_address: *val.from,
            to_address: *val.to,
            // Both carry `amount` as i128 now, so this is infallible.
            amount: val.amount.into(),
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
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ApprovalEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub difference: i128,
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

impl<'a> TryFrom<EmittedEvent<'a>> for ApprovalEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != ApprovalEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let owner: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let spender: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let difference: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(ApprovalEvent::new(owner, spender, difference))
    }
}

impl<'a> From<&ApprovalEvent<'a>> for stdevt::ApprovalEvent {
    fn from(val: &ApprovalEvent<'a>) -> Self {
        stdevt::ApprovalEvent {
            owner_address: *val.owner,
            spender_address: *val.spender,
            amount: val.difference.into(),
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
            TransferEvent::SIGNATURE => {
                Ok(ERC20Event::Transfer(event.try_into()?))
            }
            ApprovalEvent::SIGNATURE => {
                Ok(ERC20Event::Approval(event.try_into()?))
            }
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

#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum StdERC20Event {
    Transfer(stdevt::TransferEvent),
    Approval(stdevt::ApprovalEvent),
}

impl<'a> From<&ERC20Event<'a>> for StdERC20Event {
    fn from(val: &ERC20Event<'a>) -> StdERC20Event {
        match val {
            ERC20Event::Transfer(e) => StdERC20Event::Transfer(e.into()),
            ERC20Event::Approval(e) => StdERC20Event::Approval(e.into()),
        }
    }
}
