#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use groundhog_erc20_controls_if::ERC20ControlsError;
use groundhog_erc20_if::ERC20Error;
use groundhog_sep41::{SEP41Error, SEP41View};
use sdk::storage::Value;
use sdk::*;

#[contract_exception]
pub enum ActorError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    DeliberateAbort = 3,
}

#[sdk::storage]
pub struct RuntimeActor {
    owner: Value<ContractAddress>,
    token: Value<ContractAddress>,
}

impl RuntimeActor {
    fn authenticate(&self) {
        ensure(get_caller() == self.owner.get(), ActorError::Unauthorized);
    }
}

#[contract(throws(ActorError, SEP41Error, ERC20Error, ERC20ControlsError))]
impl RuntimeActor {
    pub fn initialize(&mut self, token: &ContractAddress) {
        ensure(
            self.owner.try_get().none_if_missing().is_none(),
            ActorError::AlreadyInitialized,
        );
        self.owner.set_exclusive(&get_caller());
        self.token.set_exclusive(token);
    }

    pub fn owner(&self) -> ContractAddress {
        self.owner.get()
    }

    pub fn approve(
        &self,
        spender: &ContractAddress,
        amount: i128,
        until: BlockHeight,
    ) {
        self.authenticate();
        SEP41View::new(&self.token.get()).approve(
            spender,
            amount,
            Some(until),
        );
    }

    pub fn spend(
        &self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        self.authenticate();
        SEP41View::new(&self.token.get()).transfer_from(from, to, amount);
    }

    pub fn transfer_then_abort(&self, to: &ContractAddress, amount: i128) {
        self.authenticate();
        // The child emits and changes balances before the parent aborts.
        SEP41View::new(&self.token.get()).transfer(to, amount);
        throw(ActorError::DeliberateAbort);
    }
}
