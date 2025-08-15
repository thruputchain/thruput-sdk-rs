//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "mock"), no_std)]

use erc20_if::Error as ERC20Error;
use erc20_if::ERC20Client;
use erc20_if::Amount;
use sdk::*;
use sdk::serialize::*;
use thiserror::Error;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Caller {
    token_contract: ContractAddress,
}

#[derive(Error, Debug, PartialEq, Clone)]
#[derive(ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum CallerError {
    #[error("ERC20 contract failed")]
    ERC20Err(#[from] ERC20Error),

    #[error("contract invocation failed")]
    ContractCall(#[from] ClientError<ERC20Error>),
}

pub type CallerResult<T> = Result<T, CallerError>;

#[contract(client = CallerClient)]
impl Caller {
    pub fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract = token_contract.clone();
    }

    pub fn check_balance(&self, account: &ContractAddress) -> CallerResult<Amount> {
        let mut token = ERC20Client::new(&self.token_contract);

        let bal = token.get_balance_of(account)?;

        Ok(bal)
    }
}
