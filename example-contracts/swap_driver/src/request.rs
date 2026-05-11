#![allow(clippy::new_ret_no_self)]

use sdk::*;
use swap::*;
use groundhog_erc20_permit_if::*;

/// A request submitted to the pair contract
#[derive(ReadXDR, WriteXDR)]
#[cfg_attr(feature = "dev", derive(Debug))]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum Request {
    Purchase(PurchaseRequest),
    PurchaseWithPermits(PurchaseWithPermitsRequest),
    Redeem(RedeemRequest),
    Swap(SwapRequest),
}

impl Request {
    pub const fn purchase(purchaser: &Address, amounts: Amounts, recipient: &Address) -> Request {
        Self::Purchase(PurchaseRequest {
            purchaser: *purchaser,
            amounts,
            recipient: *recipient,
        })
    }

    pub const fn purchase_with_permits(purchaser: &Address, amounts: Amounts, recipient: &Address, permits: [Permit; 2]) -> Request {
        Self::PurchaseWithPermits(PurchaseWithPermitsRequest {
            purchaser: *purchaser,
            amounts,
            recipient: *recipient,
            permits,
        })
    }

    pub const fn redeem(redeemer: &Address) -> Request {
        Self::Redeem(RedeemRequest {
            redeemer: *redeemer,
        })
    }

    pub const fn swap(trader: &Address, amounts_out: Amounts) -> Request {
        Self::Swap(SwapRequest {
            trader: *trader,
            amounts_out,
        })
    }
}

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct PurchaseRequest {
    // The would-be purchaser of LP-tokens
    pub purchaser: Address,
    pub amounts: Amounts,
    pub recipient: Address,
}

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct PurchaseWithPermitsRequest {
    // The would-be purchaser of LP-tokens
    pub purchaser: Address,
    pub amounts: Amounts,
    pub recipient: Address,
    pub permits: [Permit; 2],
}

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct RedeemRequest {
    // The would-be redeemer of LP-tokens
    pub redeemer: Address,
}

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct SwapRequest {
    // The designated receiver of the LP-tokens
    pub trader: Address,

    // Amounts the trader wishes to extract
    pub amounts_out: Amounts,
}
