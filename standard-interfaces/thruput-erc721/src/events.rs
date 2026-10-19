//! ERC721 events, see [EIP-721].
//!
//! - `event Transfer(address indexed from, address indexed to, uint256 indexed tokenId)`
//! - `event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId)`
//! - `event ApprovalForAll(address indexed owner, address indexed operator, bool approved)`
//!
//! The two addresses are topics, as on Ethereum. A Thruput event carries
//! two topics beyond its signature, so the token id (a 32-byte
//! [`TokenId`], not a `uint256`) rides in the payload instead of being
//! the third; filter on the addresses and decode the id.
//! Minting and burning have their own [`MintEvent`] and [`BurnEvent`],
//! where Ethereum overloads `Transfer` with the zero address; a transfer
//! involving the zero address here is exactly that.
//!
//! [EIP-721]: https://eips.ethereum.org/EIPS/eip-721

use crate::TokenId;
use core::fmt;
use thruput_sdk::{
    ContractAddress, EmittedEvent, Event, base58_encode, log_event,
};

fn token_id_payload(
    event: &EmittedEvent<'_>,
) -> Result<TokenId, &'static str> {
    let payload = event
        .payload()
        .ok_or("Malformed event: topics overrun body")?;
    thruput_sdk::internal::decode_xdr(payload)
        .map_err(|_| "Couldn't parse event payload")
}

fn address_topic<'a>(
    event: &EmittedEvent<'a>,
    index: u32,
) -> Result<&'a ContractAddress, &'static str> {
    Ok(event.topic(index).ok_or("Missing topic")?.into())
}

/// Log that `token_id` moved from `from` to `to`.
pub fn log_transfer_event(
    from: &ContractAddress,
    to: &ContractAddress,
    token_id: &TokenId,
) {
    log_event(&TransferEvent::new(from, to, *token_id));
}

/// An event emitted when a token changes owner. Minting and burning are
/// [`MintEvent`] and [`BurnEvent`], not transfers.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct TransferEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    #[topic]
    pub to: &'a ContractAddress,
    pub token_id: TokenId,
}

impl<'a> TransferEvent<'a> {
    pub fn new(
        from: &'a ContractAddress,
        to: &'a ContractAddress,
        token_id: TokenId,
    ) -> Self {
        Self { from, to, token_id }
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

        Ok(TransferEvent::new(
            address_topic(&event, 1)?,
            address_topic(&event, 2)?,
            token_id_payload(&event)?,
        ))
    }
}

impl<'a> fmt::Display for TransferEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TRANSFER({}, {}, token:{})",
            base58_encode(self.from),
            base58_encode(self.to),
            base58_encode(&self.token_id),
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `token_id` was created for `to`.
pub fn log_mint_event(to: &ContractAddress, token_id: &TokenId) {
    log_event(&MintEvent::new(to, *token_id));
}

/// An event emitted when a token is created.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct MintEvent<'a> {
    #[topic]
    pub to: &'a ContractAddress,
    pub token_id: TokenId,
}

impl<'a> MintEvent<'a> {
    pub fn new(to: &'a ContractAddress, token_id: TokenId) -> Self {
        Self { to, token_id }
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

        Ok(MintEvent::new(
            address_topic(&event, 1)?,
            token_id_payload(&event)?,
        ))
    }
}

impl<'a> fmt::Display for MintEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MINT_TO({}, token:{})",
            base58_encode(self.to),
            base58_encode(&self.token_id)
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `token_id` was destroyed, taking it from `from`.
pub fn log_burn_event(from: &ContractAddress, token_id: &TokenId) {
    log_event(&BurnEvent::new(from, *token_id));
}

/// An event emitted when a token is destroyed.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct BurnEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    pub token_id: TokenId,
}

impl<'a> BurnEvent<'a> {
    pub fn new(from: &'a ContractAddress, token_id: TokenId) -> Self {
        Self { from, token_id }
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

        Ok(BurnEvent::new(
            address_topic(&event, 1)?,
            token_id_payload(&event)?,
        ))
    }
}

impl<'a> fmt::Display for BurnEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BURN_FROM({}, token:{})",
            base58_encode(self.from),
            base58_encode(&self.token_id)
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `owner` approved `approved` to transfer `token_id`.
pub fn log_approval_event(
    owner: &ContractAddress,
    approved: &ContractAddress,
    token_id: &TokenId,
) {
    log_event(&ApprovalEvent::new(owner, approved, *token_id));
}

/// An event emitted when the approved address for a token changes.
/// `approved` equal to [`CONTRACT_ADDR_ZERO`](crate::CONTRACT_ADDR_ZERO)
/// clears the approval.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ApprovalEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub approved: &'a ContractAddress,
    pub token_id: TokenId,
}

impl<'a> ApprovalEvent<'a> {
    pub fn new(
        owner: &'a ContractAddress,
        approved: &'a ContractAddress,
        token_id: TokenId,
    ) -> Self {
        Self {
            owner,
            approved,
            token_id,
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

        Ok(ApprovalEvent::new(
            address_topic(&event, 1)?,
            address_topic(&event, 2)?,
            token_id_payload(&event)?,
        ))
    }
}

impl<'a> fmt::Display for ApprovalEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "APPROVE({}, {}, token:{})",
            base58_encode(self.owner),
            base58_encode(self.approved),
            base58_encode(&self.token_id),
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `owner` granted or revoked `operator` over all their tokens.
pub fn log_approval_for_all_event(
    owner: &ContractAddress,
    operator: &ContractAddress,
    approved: bool,
) {
    log_event(&ApprovalForAllEvent::new(owner, operator, approved));
}

/// An event emitted when an operator is enabled or disabled for an owner.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ApprovalForAllEvent<'a> {
    #[topic]
    pub owner: &'a ContractAddress,
    #[topic]
    pub operator: &'a ContractAddress,
    pub approved: bool,
}

impl<'a> ApprovalForAllEvent<'a> {
    pub fn new(
        owner: &'a ContractAddress,
        operator: &'a ContractAddress,
        approved: bool,
    ) -> Self {
        Self {
            owner,
            operator,
            approved,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for ApprovalForAllEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;
        if sig != ApprovalForAllEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let approved: bool = thruput_sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(ApprovalForAllEvent::new(
            address_topic(&event, 1)?,
            address_topic(&event, 2)?,
            approved,
        ))
    }
}

impl<'a> fmt::Display for ApprovalForAllEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "APPROVE_FOR_ALL({}, {}, {})",
            base58_encode(self.owner),
            base58_encode(self.operator),
            self.approved,
        )
    }
}
