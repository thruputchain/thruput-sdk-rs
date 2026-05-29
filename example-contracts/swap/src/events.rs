use crate::interfaces::iswapv2_pair::{Amount, Amounts};
use sdk::*;

#[derive(Event, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MintEvent<'a> {
    /// The contract that called `mint`
    #[topic]
    pub sender: &'a ContractAddress,

    /// The amount of LP tokens minted
    pub amount_minted: Amount,
}

impl<'a> MintEvent<'a> {
    pub fn new(sender: &'a ContractAddress, amount_minted: Amount) -> Self {
        Self {
            sender,
            amount_minted,
        }
    }
}

#[derive(Event, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BurnEvent<'a> {
    /// The contract that called `burn`
    #[topic]
    pub sender: &'a ContractAddress,

    /// The amounts of reserve tokens redeemed
    pub amounts_redeemed: Amounts,

    /// The recipient of the redeemed tokens
    #[topic]
    pub recipient: &'a ContractAddress,
}

impl<'a> BurnEvent<'a> {
    pub fn new(
        sender: &'a ContractAddress,
        amounts_redeemed: Amounts,
        recipient: &'a ContractAddress,
    ) -> Self {
        Self {
            sender,
            amounts_redeemed,
            recipient,
        }
    }
}

#[derive(Event, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SwapEvent<'a> {
    /// The contract that called `swap`
    #[topic]
    pub sender: &'a ContractAddress,

    /// The amounts paid into the swap pair
    pub amounts_in: Amounts,

    /// The amounts returned from the swap pair
    pub amounts_out: Amounts,

    /// The recipient of the swap proceeds
    #[topic]
    pub recipient: &'a ContractAddress,
}

impl<'a> SwapEvent<'a> {
    pub fn new(
        sender: &'a ContractAddress,
        amounts_in: Amounts,
        amounts_out: Amounts,
        recipient: &'a ContractAddress,
    ) -> Self {
        Self {
            sender,
            amounts_in,
            amounts_out,
            recipient,
        }
    }
}

#[derive(Event, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncEvent {
    /// The token-reserve amounts as of the moment of the `sync` call
    pub reserves: Amounts,
}

impl SyncEvent {
    pub fn new(reserves: Amounts) -> Self {
        Self { reserves }
    }
}
