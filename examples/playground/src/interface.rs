use sdk::prelude::*;

pub type Amount = i128;

#[contract(client = "client")]
pub trait ERC20 {
    fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()>;

    fn hello(&mut self, buf: &[u8]) -> Result<u32>;

    // We need to patch the generated client code to make this work
    // fn associated(n: u32) -> Result<u32>;

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

    fn add_frot(&mut self, amount: Amount) -> Result<()>;
    fn get_frot(&self) -> Result<Amount>;
}
