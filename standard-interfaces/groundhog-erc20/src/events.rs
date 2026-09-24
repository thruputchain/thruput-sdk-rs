//! ERC20 Events
//!
//! See [EIP-20] for `Transfer` and `Approval`:
//!
//! `event Transfer(address indexed _from, address indexed _to, uint256 _value)`
//! `event Approval(address indexed _owner, address indexed _spender, uint256 _value)`
//!
//! Supply changes have their own events here, [`MintEvent`] and
//! [`BurnEvent`], where Ethereum overloads `Transfer` with the zero address
//! as `_from` or `_to`. On Groundhog the zero address is an address like
//! any other, so a transfer involving it is exactly that, and a mint or
//! burn says so by name.
//!
//! [EIP-20]: https://eips.ethereum.org/EIPS/eip-20#events

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
/// Supply changes are [`MintEvent`] and [`BurnEvent`], not transfers.
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
        write!(
            f,
            "TRANSFER({}, {}, amount:{:?})",
            hex_encode(self.from),
            hex_encode(self.to),
            self.amount,
        )
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

pub fn log_approve_event(
    owner: &ContractAddress,
    spender: &ContractAddress,
    amount: u128,
) {
    let event = ApproveEvent::new(
        owner,
        spender,
        crate::error::amount_to_diff(amount),
    );

    log_event(&event);
}

/// Absolute remaining allowance set by `approve`. Its signature is distinct
/// from the legacy [`ApprovalEvent`], whose payload is a signed delta.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ApproveEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub amount: i128,
}

impl<'a> ApproveEvent<'a> {
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

impl<'a> TryFrom<EmittedEvent<'a>> for ApproveEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != ApproveEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let owner: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let spender: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let amount: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(ApproveEvent::new(owner, spender, amount))
    }
}

impl<'a> From<&ApproveEvent<'a>> for stdevt::ApprovalEvent {
    fn from(val: &ApproveEvent<'a>) -> Self {
        stdevt::ApprovalEvent {
            owner_address: *val.owner,
            spender_address: *val.spender,
            amount: val.amount.into(),
        }
    }
}

impl<'a> fmt::Display for ApproveEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "APPROVE({}, {}, amount:{:?})",
            hex_encode(self.owner),
            hex_encode(self.spender),
            self.amount
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `amount` tokens were created into `to`.
pub fn log_mint_event(to: &ContractAddress, amount: u128) {
    log_event(&MintEvent::new(to, crate::error::amount_to_diff(amount)));
}

/// An event emitted when tokens are created.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct MintEvent<'a> {
    #[topic]
    pub to: &'a ContractAddress,
    /// The created amount; non-negative, typed like
    /// [`TransferEvent::amount`].
    pub amount: i128,
}

impl<'a> MintEvent<'a> {
    pub fn new(to: &'a ContractAddress, amount: i128) -> Self {
        Self { to, amount }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for MintEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;
        if sig != MintEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let to: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let amount: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(MintEvent::new(to, amount))
    }
}

impl<'a> fmt::Display for MintEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MINT_TO({}, amount:{:?})",
            hex_encode(self.to),
            self.amount
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `amount` tokens were destroyed from `from`.
pub fn log_burn_event(from: &ContractAddress, amount: u128) {
    log_event(&BurnEvent::new(from, crate::error::amount_to_diff(amount)));
}

/// An event emitted when tokens are destroyed.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct BurnEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    /// The destroyed amount; non-negative, typed like
    /// [`TransferEvent::amount`].
    pub amount: i128,
}

impl<'a> BurnEvent<'a> {
    pub fn new(from: &'a ContractAddress, amount: i128) -> Self {
        Self { from, amount }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for BurnEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;
        if sig != BurnEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let from: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let amount: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(BurnEvent::new(from, amount))
    }
}

impl<'a> fmt::Display for BurnEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BURN_FROM({}, amount:{:?})",
            hex_encode(self.from),
            self.amount
        )
    }
}

// ----------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ERC20Event<'a> {
    Transfer(TransferEvent<'a>),
    Approval(ApprovalEvent<'a>),
    Mint(MintEvent<'a>),
    Burn(BurnEvent<'a>),
    Approve(ApproveEvent<'a>),
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
            ApproveEvent::SIGNATURE => {
                Ok(ERC20Event::Approve(event.try_into()?))
            }
            ApprovalEvent::SIGNATURE => {
                Ok(ERC20Event::Approval(event.try_into()?))
            }
            MintEvent::SIGNATURE => Ok(ERC20Event::Mint(event.try_into()?)),
            BurnEvent::SIGNATURE => Ok(ERC20Event::Burn(event.try_into()?)),
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
            ERC20Event::Mint(e) => e.fmt(f),
            ERC20Event::Burn(e) => e.fmt(f),
            ERC20Event::Approve(e) => e.fmt(f),
        }
    }
}

/// Owned, XDR-encodable form of [`MintEvent`], for event logs that cross a
/// contract boundary (there is no `standard_asset_events` counterpart).
#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct StdMintEvent {
    pub to_address: ContractAddress,
    pub amount: i128,
}

impl<'a> From<&MintEvent<'a>> for StdMintEvent {
    fn from(val: &MintEvent<'a>) -> Self {
        StdMintEvent {
            to_address: *val.to,
            amount: val.amount,
        }
    }
}

/// Owned, XDR-encodable form of [`BurnEvent`].
#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct StdBurnEvent {
    pub from_address: ContractAddress,
    pub amount: i128,
}

impl<'a> From<&BurnEvent<'a>> for StdBurnEvent {
    fn from(val: &BurnEvent<'a>) -> Self {
        StdBurnEvent {
            from_address: *val.from,
            amount: val.amount,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum StdERC20Event {
    Transfer(stdevt::TransferEvent),
    Approval(stdevt::ApprovalEvent),
    Mint(StdMintEvent),
    Burn(StdBurnEvent),
    /// Absolute allowance; `Approval` remains the legacy delta variant.
    Approve(stdevt::ApprovalEvent),
}

impl<'a> From<&ERC20Event<'a>> for StdERC20Event {
    fn from(val: &ERC20Event<'a>) -> StdERC20Event {
        match val {
            ERC20Event::Transfer(e) => StdERC20Event::Transfer(e.into()),
            ERC20Event::Approval(e) => StdERC20Event::Approval(e.into()),
            ERC20Event::Mint(e) => StdERC20Event::Mint(e.into()),
            ERC20Event::Burn(e) => StdERC20Event::Burn(e.into()),
            ERC20Event::Approve(e) => StdERC20Event::Approve(e.into()),
        }
    }
}
