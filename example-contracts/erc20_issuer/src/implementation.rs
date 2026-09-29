use crate::common::*;
use crate::*;
use sdk::signer::Signed;
use sdk::storage::Deletable;
use sdk::*;

impl TokenContract {
    fn open_policy() -> Policy {
        Policy {
            auth_required: false,
            auth_revocable: false,
            clawback_enabled: false,
        }
    }
    fn adjust(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        self.settings();
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .try_strict_inc(diff)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }
    fn clear(&mut self, owner: &ContractAddress, spender: &ContractAddress) {
        self.settings();
        let remaining = amount_to_diff(self.allowance(owner, spender));
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .delete_exclusive();
        AllowanceChangeEvent::new(owner, spender, -remaining).log();
    }
}

// Policy extensions can also throw ERC20ControlsError through the standard client's
// unknown-error path, as with ERC20Pausable.
#[contract(version = "0.1.0")]
impl ERC20 for TokenContract {
    fn initialize(&mut self, administrator: &ContractAddress) {
        self.init(
            administrator,
            administrator,
            &Settings {
                name: None,
                symbol: None,
                decimals: None,
                policy: Self::open_policy(),
            },
        );
    }
    fn get_balance_of(&self, owner: &ContractAddress) -> Amount {
        self.settings();
        self.balances.at(owner).get()
    }
    fn get_balance(&self) -> Amount {
        self.get_balance_of(&get_caller())
    }
    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.settings();
        self.allowances
            .at(owner)
            .at(spender)
            .try_get()
            .none_if_missing()
            .unwrap_or(0) as Amount
    }
    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
        exclusive: bool,
    ) {
        self.settings();
        let owner = get_caller();
        let mut allowances = self.allowances.at_mut(&owner);
        let mut allowance = if exclusive {
            allowances.at_mut_exclusive(spender)
        } else {
            allowances.at_mut(spender)
        };
        allowance
            .try_strict_inc(difference)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn clear_allowance(&mut self, spender: &ContractAddress) {
        self.clear(&get_caller(), spender);
    }

    fn clear_balance(&mut self) {
        self.settings();
        let owner = get_caller();
        let mut balance = self.balances.at_mut(&owner);
        ensure(balance.get() == 0, ERC20Error::BalanceNotZero);
        balance.delete_exclusive();
    }

    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount) {
        self.move_tokens(&get_caller(), receiver, amount);
    }
    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self.adjust(owner, &get_caller(), -amount_to_diff(amount));
        self.move_tokens(owner, receiver, amount);
    }
    fn mint(&mut self, receiver: &ContractAddress, amount: Amount) {
        self.mint_tokens(receiver, amount);
    }
    fn burn(&mut self, owner: &ContractAddress, amount: Amount) {
        self.forced_burn(owner, amount);
    }
    fn total_supply(&self) -> Amount {
        self.settings();
        self.supply.get()
    }
}

#[contract(version = "0.1.0")]
impl ERC20Metadata for TokenContract {
    fn initialize(
        &mut self,
        administrator: &ContractAddress,
        name: Option<Name>,
        symbol: Option<Symbol>,
        decimals: Option<u32>,
    ) {
        self.init(
            administrator,
            administrator,
            &Settings {
                name,
                symbol,
                decimals,
                policy: Self::open_policy(),
            },
        );
    }
    fn name(&self) -> Option<Name> {
        self.settings().name
    }
    fn symbol(&self) -> Option<Symbol> {
        self.settings().symbol
    }
    fn decimals(&self) -> Option<u32> {
        self.settings().decimals
    }
}

#[contract(version = "0.1.0")]
impl ERC20Burn for TokenContract {
    fn burn(&mut self, amount: Amount) {
        let owner = get_caller();
        self.require_holder(&owner);
        self.destroy(&owner, amount);
    }
    fn burn_from(&mut self, owner: &ContractAddress, amount: Amount) {
        self.require_holder(owner);
        self.adjust(owner, &get_caller(), -amount_to_diff(amount));
        self.destroy(owner, amount);
    }
}

#[contract(version = "0.1.0")]
impl ERC20Permit for TokenContract {
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        let (grant, owner) = grant.consume();
        ensure(get_caller() == grant.spender, ERC20PermitError::NotSpender);
        ensure(amount <= grant.amount, ERC20PermitError::ExceedsGrant);
        self.move_tokens(&owner, receiver, amount);
    }

    fn increase_allowance_signed(&mut self, grant: Signed<PermitGrant>) {
        let (grant, owner) = grant.consume();
        let difference = amount_to_diff(grant.amount);
        self.adjust(&owner, &grant.spender, difference);
        AllowanceChangeEvent::new(&owner, &grant.spender, difference).log();
    }

    fn clear_allowance_signed(&mut self, grant: Signed<ClearAllowanceGrant>) {
        let (grant, owner) = grant.consume();
        self.clear(&owner, &grant.spender);
    }
}
