use sdk::prelude::*;

pub use sdk::prelude::{Error, Result};
pub type Amount = u128;

#[contract(client = "client")]
pub trait ERC20 {
    fn initialize(
        maybe_self: Option<&mut Self>,
        contract_owner: &ContractAddress,
    ) -> Result<()>;

    /// Transfer from caller to `receiver` address
    fn transfer(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()>;

    /// Transfer from `owner` to `receiver` address, using allowance
    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()>;

    /// Adjust allowance for `spender` by `amount` (positive or negative)
    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        amount: Amount,
    ) -> Result<()>;

    /// Get balance for given address
    fn get_balance_of(&self, addr: &ContractAddress) -> Result<Amount>;

    /// Get balance for caller
    fn get_balance(&self) -> Result<Amount>;

    /// Create tokens into `receiver` address
    fn mint(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()>;

    /// Destroy tokens from `owner` address
    fn burn(&mut self, owner: &ContractAddress, amount: Amount) -> Result<()>;

    /// Get total supply of outstanding tokens
    fn total_supply(&self) -> Result<Amount>;
}
