#![allow(clippy::new_ret_no_self)]

use sdk::*;
use swap::*;

/// A request submitted to the pair contract
#[derive(Debug, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum Request {
    Purchase(PurchaseRequest),
    Redeem(RedeemRequest),
    Swap(SwapRequest),
}

impl Request {
    pub const fn purchase(purchaser: &Address) -> Request {
        Self::Purchase(PurchaseRequest {
            purchaser: *purchaser,
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

#[derive(Debug, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct PurchaseRequest {
    // The would-be purchaser of LP-tokens
    pub purchaser: ContractAddress,
}

#[derive(Debug, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct RedeemRequest {
    // The would-be redeemer of LP-tokens
    pub redeemer: ContractAddress,
}

#[derive(Debug, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct SwapRequest {
    // The designated receiver of the LP-tokens
    pub trader: ContractAddress,

    // Amounts the trader wishes to extract
    pub amounts_out: Amounts,
}
