//! [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
//! token operations as client code over the Groundhog ERC20 interfaces.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

use groundhog_erc20_controls_if::{ERC20BurnClient, ERC20BurnEntrypoints};
use groundhog_erc20_if::{
    Amount, ERC20AllowanceExpiryClient, ERC20AllowanceExpiryEntrypoints,
    ERC20Client, ERC20Entrypoints, check_amount,
};
use groundhog_erc20_metadata_if::{
    ERC20MetadataClient, ERC20MetadataEntrypoints, Name, Symbol,
};
use groundhog_erc165_if::{ERC165Entrypoints, InterfaceId, probe_interface};
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
        self.claims(&[
            ERC20Entrypoints::INTERFACE_ID,
            ERC20BurnEntrypoints::INTERFACE_ID,
            ERC20MetadataEntrypoints::INTERFACE_ID,
        ])
    }

    /// Whether the token reports implementing [`ERC20AllowanceExpiry`], which
    /// `approve` needs for a deadline. Answers like [`Self::is_supported`].
    ///
    /// [`ERC20AllowanceExpiry`]: groundhog_erc20_if::ERC20AllowanceExpiry
    pub fn supports_expiry(&self) -> Option<bool> {
        self.claims(&[ERC20AllowanceExpiryEntrypoints::INTERFACE_ID])
    }

    /// `spender`'s remaining allowance of `from`'s tokens, or zero once expired.
    pub fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        ERC20Client::new(self.token).allowance(from, spender)
    }

    /// Replace the caller's allowance for `spender` with `amount`, spendable
    /// through block `live_until_ledger` inclusive, or with no expiry for
    /// `None`. A deadline needs a token implementing [`ERC20AllowanceExpiry`]
    /// and must not already have passed. Zero revokes the allowance and deletes
    /// its storage, whatever `live_until_ledger` says.
    ///
    /// The reset or delete locks the allowance for the rest of the
    /// transaction, so a concurrent spend conflicts with the replacement
    /// instead of landing beside it. A spend in an earlier block may still
    /// consume the old allowance first.
    ///
    /// [`ERC20AllowanceExpiry`]: groundhog_erc20_if::ERC20AllowanceExpiry
    pub fn approve(
        &self,
        spender: &ContractAddress,
        amount: Amount,
        live_until_ledger: Option<BlockHeight>,
    ) {
        check_amount(amount);
        let token = ERC20Client::new(self.token);
        match live_until_ledger {
            Some(until) if amount != 0 => {
                ensure(
                    get_block_height() <= until,
                    SEP41Error::InvalidExpiration,
                );
                ERC20AllowanceExpiryClient::new(self.token)
                    .reset_allowance(spender, until);
            }
            _ => token.delete_allowance(spender),
        }
        if amount != 0 {
            token.adjust_allowance(spender, amount);
        }
    }

    pub fn balance(&self, id: &ContractAddress) -> Amount {
        ERC20Client::new(self.token).get_balance_of(id)
    }

    /// Transfer from the caller to `to`.
    pub fn transfer(&self, to: &ContractAddress, amount: Amount) {
        ERC20Client::new(self.token).transfer(to, amount);
    }

    /// Spend the caller's allowance to move `from`'s tokens to `to`.
    pub fn transfer_from(
        &self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: Amount,
    ) {
        ERC20Client::new(self.token).transfer_from(from, to, amount);
    }

    /// Destroy the caller's tokens.
    pub fn burn(&self, amount: Amount) {
        ERC20BurnClient::new(self.token).burn(amount);
    }

    /// Spend the caller's allowance to destroy `from`'s tokens.
    pub fn burn_from(&self, from: &ContractAddress, amount: Amount) {
        ERC20BurnClient::new(self.token).burn_from(from, amount);
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

    // `None` when the token has no ERC-165; otherwise whether it claims all.
    fn claims(&self, interfaces: &[InterfaceId]) -> Option<bool> {
        if !probe_interface(self.token, ERC165Entrypoints::INTERFACE_ID) {
            return None;
        }
        Some(interfaces.iter().all(|id| probe_interface(self.token, *id)))
    }
}
