//! Groundhog token events.
//!
//! [`AllowanceChangeEvent`] carries a signed delta, not an absolute approval.
//! Supply changes use [`MintEvent`] and [`BurnEvent`]; a [`TransferEvent`]
//! involving the zero address is still an ordinary transfer.

use core::fmt;
use groundhog_xdr::standard_asset_events as stdevt;
use sdk::{
    BlockHeight, ContractAddress, EmittedEvent, Event, ReadXDR, WriteXDR,
    hex_encode, log_event,
};

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
    /// by the token implementation. For in-range values the CXS encoding is
    /// identical to the previous `u128`.
    pub amount: i128,
}

impl<'a> TransferEvent<'a> {
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

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

/// Signed delta from `adjust_allowance` or a signed grant. Deletes and resets
/// emit [`AllowanceDeleteEvent`] and [`AllowanceResetEvent`] instead. Spending
/// emits only its token movement; query `allowance` for the remaining value.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct AllowanceChangeEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub difference: i128,
}

impl<'a> AllowanceChangeEvent<'a> {
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

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

impl<'a> TryFrom<EmittedEvent<'a>> for AllowanceChangeEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != AllowanceChangeEvent::SIGNATURE {
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

        Ok(AllowanceChangeEvent::new(owner, spender, difference))
    }
}

impl<'a> From<&AllowanceChangeEvent<'a>> for StdAllowanceChangeEvent {
    fn from(val: &AllowanceChangeEvent<'a>) -> Self {
        StdAllowanceChangeEvent {
            owner_address: *val.owner,
            spender_address: *val.spender,
            difference: val.difference,
        }
    }
}

impl<'a> fmt::Display for AllowanceChangeEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ALLOWANCE_CHANGE({}, {}, difference:{:?})",
            hex_encode(self.owner),
            hex_encode(self.spender),
            self.difference
        )
    }
}

// ----------------------------------------------------------------------

/// An allowance deleted by `delete_allowance`, leaving it exactly zero with no
/// expiry. `removed` is the stored amount deleted, even if it had expired.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct AllowanceDeleteEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub removed: i128,
}

impl<'a> AllowanceDeleteEvent<'a> {
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

    pub fn new(
        owner: &'a ContractAddress,
        spender: &'a ContractAddress,
        removed: i128,
    ) -> Self {
        Self {
            owner,
            spender,
            removed,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for AllowanceDeleteEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != AllowanceDeleteEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let owner: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let spender: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let removed: i128 = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(AllowanceDeleteEvent::new(owner, spender, removed))
    }
}

/// Owned form of [`AllowanceDeleteEvent`].
#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct StdAllowanceDeleteEvent {
    pub owner_address: ContractAddress,
    pub spender_address: ContractAddress,
    pub removed: i128,
}

impl<'a> From<&AllowanceDeleteEvent<'a>> for StdAllowanceDeleteEvent {
    fn from(val: &AllowanceDeleteEvent<'a>) -> Self {
        StdAllowanceDeleteEvent {
            owner_address: *val.owner,
            spender_address: *val.spender,
            removed: val.removed,
        }
    }
}

impl<'a> fmt::Display for AllowanceDeleteEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ALLOWANCE_DELETE({}, {}, removed:{:?})",
            hex_encode(self.owner),
            hex_encode(self.spender),
            self.removed
        )
    }
}

// ----------------------------------------------------------------------

/// An allowance reset by `ERC20AllowanceExpiry::reset_allowance`, leaving it
/// exactly zero and spendable through block `until` once refilled. `removed`
/// is the stored amount deleted, even if it had expired.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct AllowanceResetEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub removed: i128,
    pub until: BlockHeight,
}

impl<'a> AllowanceResetEvent<'a> {
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

    pub fn new(
        owner: &'a ContractAddress,
        spender: &'a ContractAddress,
        removed: i128,
        until: BlockHeight,
    ) -> Self {
        Self {
            owner,
            spender,
            removed,
            until,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for AllowanceResetEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != AllowanceResetEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let owner: &ContractAddress =
            event.topic(1).ok_or("Missing topic")?.into();
        let spender: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let payload = AllowanceResetEvent::decode_payload(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(AllowanceResetEvent::new(
            owner,
            spender,
            payload.removed,
            payload.until,
        ))
    }
}

/// Owned form of [`AllowanceResetEvent`].
#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct StdAllowanceResetEvent {
    pub owner_address: ContractAddress,
    pub spender_address: ContractAddress,
    pub removed: i128,
    pub until: BlockHeight,
}

impl<'a> From<&AllowanceResetEvent<'a>> for StdAllowanceResetEvent {
    fn from(val: &AllowanceResetEvent<'a>) -> Self {
        StdAllowanceResetEvent {
            owner_address: *val.owner,
            spender_address: *val.spender,
            removed: val.removed,
            until: val.until.clone(),
        }
    }
}

impl<'a> fmt::Display for AllowanceResetEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ALLOWANCE_RESET({}, {}, removed:{:?}, until:{:?})",
            hex_encode(self.owner),
            hex_encode(self.spender),
            self.removed,
            *self.until
        )
    }
}

// ----------------------------------------------------------------------

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
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

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
    /// Emit this event.
    pub fn log(&self) {
        log_event(self);
    }

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
    AllowanceChange(AllowanceChangeEvent<'a>),
    Mint(MintEvent<'a>),
    Burn(BurnEvent<'a>),
    AllowanceDelete(AllowanceDeleteEvent<'a>),
    AllowanceReset(AllowanceResetEvent<'a>),
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
            AllowanceChangeEvent::SIGNATURE => {
                Ok(ERC20Event::AllowanceChange(event.try_into()?))
            }
            MintEvent::SIGNATURE => Ok(ERC20Event::Mint(event.try_into()?)),
            BurnEvent::SIGNATURE => Ok(ERC20Event::Burn(event.try_into()?)),
            AllowanceDeleteEvent::SIGNATURE => {
                Ok(ERC20Event::AllowanceDelete(event.try_into()?))
            }
            AllowanceResetEvent::SIGNATURE => {
                Ok(ERC20Event::AllowanceReset(event.try_into()?))
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
            ERC20Event::AllowanceChange(e) => e.fmt(f),
            ERC20Event::Mint(e) => e.fmt(f),
            ERC20Event::Burn(e) => e.fmt(f),
            ERC20Event::AllowanceDelete(e) => e.fmt(f),
            ERC20Event::AllowanceReset(e) => e.fmt(f),
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
    Mint(StdMintEvent),
    Burn(StdBurnEvent),
    AllowanceChange(StdAllowanceChangeEvent),
    AllowanceDelete(StdAllowanceDeleteEvent),
    AllowanceReset(StdAllowanceResetEvent),
}

impl<'a> From<&ERC20Event<'a>> for StdERC20Event {
    fn from(val: &ERC20Event<'a>) -> StdERC20Event {
        match val {
            ERC20Event::Transfer(e) => StdERC20Event::Transfer(e.into()),
            ERC20Event::AllowanceChange(e) => {
                StdERC20Event::AllowanceChange(e.into())
            }
            ERC20Event::Mint(e) => StdERC20Event::Mint(e.into()),
            ERC20Event::Burn(e) => StdERC20Event::Burn(e.into()),
            ERC20Event::AllowanceDelete(e) => {
                StdERC20Event::AllowanceDelete(e.into())
            }
            ERC20Event::AllowanceReset(e) => {
                StdERC20Event::AllowanceReset(e.into())
            }
        }
    }
}

/// Owned representation of a signed allowance adjustment.
#[derive(Clone, Debug, PartialEq, Eq, WriteXDR, ReadXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct StdAllowanceChangeEvent {
    pub owner_address: ContractAddress,
    pub spender_address: ContractAddress,
    pub difference: i128,
}
