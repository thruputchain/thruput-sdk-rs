//! [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
//! token operations as client code over the Groundhog ERC20 interfaces.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

use groundhog_erc20_controls_if::{ERC20BurnClient, ERC20BurnEntrypoints};
use groundhog_erc20_if::{Amount, ERC20Client, ERC20Entrypoints};
use groundhog_erc20_metadata_if::{
    ERC20MetadataClient, ERC20MetadataEntrypoints, Name, Symbol,
};
use groundhog_erc165_if::{ERC165Entrypoints, probe_interface};
use sdk::{BlockHeight, ContractAddress, ensure, get_block_height, throw};

/// SEP-41 token operations over any token implementing `ERC20`, `ERC20Burn`
/// and `ERC20Metadata`. Tokens implement no SEP-41 interface of their own.
///
/// This is client code: it runs in the holder's contract and calls the token,
/// so the holder is the Groundhog caller. SEP-41's explicit authorizers
/// (`from` on `transfer`, `approve` and `burn`; `spender` on `transfer_from`
/// and `burn_from`) are the caller and are not arguments. Expiry is measured
/// in Groundhog blocks, not Stellar ledgers, and the token emits its native
/// events, not Soroban's.
pub struct SEP41View<'a> {
    token: &'a ContractAddress,
}

impl<'a> SEP41View<'a> {
    pub fn new(token: &'a ContractAddress) -> Self {
        Self { token }
    }

    /// Whether the token reports implementing every interface this view
    /// calls: `Some` of its ERC-165 answer, or `None` if it does not implement
    /// ERC-165 and so cannot say. Other call failures throw.
    ///
    /// Like any ERC-165 claim, this describes the token's code, so treat it
    /// as a dispatch hint rather than an authorization boundary.
    pub fn is_supported(&self) -> Option<bool> {
        if !probe_interface(self.token, ERC165Entrypoints::INTERFACE_ID) {
            return None;
        }
        Some(
            [
                ERC20Entrypoints::INTERFACE_ID,
                ERC20BurnEntrypoints::INTERFACE_ID,
                ERC20MetadataEntrypoints::INTERFACE_ID,
            ]
            .into_iter()
            .all(|id| probe_interface(self.token, id)),
        )
    }

    /// `spender`'s remaining allowance of `from`'s tokens, or zero once expired.
    pub fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> i128 {
        // Implementations bound allowances to `0..=i128::MAX`.
        ERC20Client::new(self.token).allowance(from, spender) as i128
    }

    /// Replace the caller's allowance for `spender` with `amount`, spendable
    /// through block `live_until_ledger` inclusive. A nonzero amount must not
    /// already be expired. Zero revokes the allowance and deletes its storage,
    /// whatever `live_until_ledger` says.
    ///
    /// The reset or delete locks the allowance for the rest of the
    /// transaction, so a concurrent spend conflicts with the replacement
    /// instead of landing beside it. A spend in an earlier block may still
    /// consume the old allowance first.
    pub fn approve(
        &self,
        spender: &ContractAddress,
        amount: i128,
        live_until_ledger: BlockHeight,
    ) {
        Self::amount(amount);
        let token = ERC20Client::new(self.token);
        if amount == 0 {
            token.delete_allowance(spender);
            return;
        }
        ensure(
            get_block_height() <= live_until_ledger,
            SEP41Error::InvalidExpiration,
        );
        token.reset_allowance(spender, live_until_ledger);
        token.adjust_allowance(spender, amount);
    }

    pub fn balance(&self, id: &ContractAddress) -> i128 {
        // Implementations bound balances to `0..=i128::MAX`.
        ERC20Client::new(self.token).get_balance_of(id) as i128
    }

    /// Transfer from the caller to `to`.
    pub fn transfer(&self, to: &ContractAddress, amount: i128) {
        ERC20Client::new(self.token).transfer(to, Self::amount(amount));
    }

    /// Spend the caller's allowance to move `from`'s tokens to `to`.
    pub fn transfer_from(
        &self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        ERC20Client::new(self.token).transfer_from(
            from,
            to,
            Self::amount(amount),
        );
    }

    /// Destroy the caller's tokens.
    pub fn burn(&self, amount: i128) {
        ERC20BurnClient::new(self.token).burn(Self::amount(amount));
    }

    /// Spend the caller's allowance to destroy `from`'s tokens.
    pub fn burn_from(&self, from: &ContractAddress, amount: i128) {
        ERC20BurnClient::new(self.token).burn_from(from, Self::amount(amount));
    }

    /// Throws [`SEP41Error::MetadataUnavailable`] when unset, as do
    /// `name` and `symbol`.
    pub fn decimals(&self) -> u32 {
        ERC20MetadataClient::new(self.token)
            .decimals()
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }

    pub fn name(&self) -> Name {
        ERC20MetadataClient::new(self.token)
            .name()
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }

    pub fn symbol(&self) -> Symbol {
        ERC20MetadataClient::new(self.token)
            .symbol()
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }

    fn amount(amount: i128) -> Amount {
        ensure(amount >= 0, SEP41Error::NegativeAmount);
        amount as Amount
    }
}
