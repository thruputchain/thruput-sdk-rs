use groundhog_sep41_if::*;
use sdk::storage::{Balance, Map, Value};
use sdk::*;

#[sdk::storage]
pub struct TokenContract {
    settings: Value<Settings>,
    administrator: Value<ContractAddress>,
    mint_authority: Value<ContractAddress>,
    authorizations: Map<ContractAddress, Value<bool>>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Value<Allowance>>>,
    supply: Balance,
}

#[contract_type]
struct Settings {
    pub name: Name,
    pub symbol: Symbol,
    pub decimals: u32,
    pub policy: Policy,
}

#[contract_type]
struct Allowance {
    pub amount: i128,
    pub live_until_ledger: u32,
}

impl TokenContract {
    fn settings(&self) -> Settings {
        self.settings
            .try_get()
            .none_if_missing()
            .unwrap_or_else(|| throw(TokenError::NotInitialized))
    }

    fn require_caller(address: &ContractAddress) {
        ensure(get_caller() == *address, TokenError::Unauthorized);
    }

    fn amount(amount: i128) -> u128 {
        ensure(amount >= 0, TokenError::NegativeAmount);
        amount as u128
    }

    fn require_holder(&self, id: &ContractAddress) {
        ensure(self.authorized(id), TokenError::HolderNotAuthorized);
    }

    fn current_height() -> i64 {
        *sdk::block::get_block_height()
    }

    fn allowance_record(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> Allowance {
        self.allowances
            .at(from)
            .at(spender)
            .try_get()
            .none_if_missing()
            .unwrap_or(Allowance {
                amount: 0,
                live_until_ledger: 0,
            })
    }

    fn effective_allowance(allowance: &Allowance) -> i128 {
        if Self::current_height() > i64::from(allowance.live_until_ledger) {
            0
        } else {
            allowance.amount
        }
    }

    fn spend_allowance(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
    ) {
        let mut allowance = self.allowance_record(from, spender);
        let remaining = Self::effective_allowance(&allowance);
        ensure(remaining >= amount, TokenError::InsufficientAllowance);
        if amount == 0 {
            return;
        }
        allowance.amount = remaining - amount;
        // Equal concurrent writes must not merge: two spends could otherwise
        // debit balances twice while recording only one allowance reduction.
        self.allowances
            .at_mut(from)
            .at_mut(spender)
            .set_exclusive(&allowance);
    }

    fn balance_error(error: LibraryExn) -> ! {
        match error {
            LibraryExn::NonNegativeIntUnderflow => {
                throw(TokenError::InsufficientBalance)
            }
            LibraryExn::NonNegativeIntOverflow => throw(TokenError::Overflow),
            other => other.re_throw(),
        }
    }

    fn debit(&mut self, from: &ContractAddress, amount: u128) {
        self.balances
            .at_mut(from)
            .try_decrease(amount)
            .unwrap_or_else(|e| Self::balance_error(e));
    }

    fn credit(&mut self, to: &ContractAddress, amount: u128) {
        // Supply is bounded before minting; transfers conserve it. A holder's
        // balance therefore cannot overflow if the committed supply is valid.
        self.balances
            .at_mut(to)
            .try_lazy_increase(amount)
            .unwrap_or_else(|e| Self::balance_error(e));
    }

    fn destroy(&mut self, from: &ContractAddress, amount: u128) {
        self.debit(from, amount);
        self.supply
            .try_decrease(amount)
            .unwrap_or_else(|e| Self::balance_error(e));
    }
}

#[contract(version = "0.1.0")]
impl Token for TokenContract {
    fn balance(&self, id: &ContractAddress) -> i128 {
        self.settings();
        self.balances.at(id).get() as i128
    }

    fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> i128 {
        self.settings();
        Self::effective_allowance(&self.allowance_record(from, spender))
    }

    fn approve(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
        live_until_ledger: u32,
    ) {
        self.settings();
        Self::require_caller(from);
        Self::amount(amount);
        ensure(
            amount == 0
                || i64::from(live_until_ledger) >= Self::current_height(),
            TokenError::InvalidExpiry,
        );
        self.allowances.at_mut(from).at_mut(spender).set_exclusive(
            &Allowance {
                amount,
                live_until_ledger,
            },
        );
        log_event(&ApproveEvent {
            from,
            spender,
            amount,
            live_until_ledger,
        });
    }

    fn transfer(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        Self::require_caller(from);
        let units = Self::amount(amount);
        self.require_holder(from);
        self.require_holder(to);
        self.debit(from, units);
        self.credit(to, units);
        log_event(&TransferEvent { from, to, amount });
    }

    fn transfer_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        Self::require_caller(spender);
        let units = Self::amount(amount);
        self.require_holder(from);
        self.require_holder(to);
        self.spend_allowance(from, spender, amount);
        self.debit(from, units);
        self.credit(to, units);
        log_event(&TransferEvent { from, to, amount });
    }

    fn burn(&mut self, from: &ContractAddress, amount: i128) {
        Self::require_caller(from);
        let units = Self::amount(amount);
        self.require_holder(from);
        self.destroy(from, units);
        log_event(&BurnEvent { from, amount });
    }

    fn burn_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        amount: i128,
    ) {
        Self::require_caller(spender);
        let units = Self::amount(amount);
        self.require_holder(from);
        self.spend_allowance(from, spender, amount);
        self.destroy(from, units);
        log_event(&BurnEvent { from, amount });
    }

    fn name(&self) -> Name {
        self.settings().name
    }
    fn symbol(&self) -> Symbol {
        self.settings().symbol
    }
    fn decimals(&self) -> u32 {
        self.settings().decimals
    }
}

#[contract(version = "0.1.0")]
impl TokenAdmin for TokenContract {
    fn initialize(
        &mut self,
        minter: &ContractAddress,
        name: Name,
        symbol: Symbol,
        decimals: u32,
        policy: Policy,
    ) {
        ensure(
            self.settings.try_get().none_if_missing().is_none(),
            TokenError::AlreadyInitialized,
        );
        ensure(
            !policy.clawback_enabled || policy.auth_revocable,
            TokenError::InvalidPolicy,
        );
        self.settings.set_exclusive(&Settings {
            name,
            symbol,
            decimals,
            policy,
        });
        self.administrator.set_exclusive(&get_caller());
        self.mint_authority.set_exclusive(minter);
    }

    fn admin(&self) -> ContractAddress {
        self.settings();
        self.administrator.get()
    }

    fn set_admin(&mut self, new_admin: &ContractAddress) {
        let admin = self.admin();
        Self::require_caller(&admin);
        self.administrator.set_exclusive(new_admin);
        log_event(&SetAdminEvent {
            admin: &admin,
            new_admin: *new_admin,
        });
    }

    fn minter(&self) -> ContractAddress {
        self.settings();
        self.mint_authority.get()
    }

    fn set_minter(&mut self, new_minter: &ContractAddress) {
        Self::require_caller(&self.admin());
        let minter = self.minter();
        self.mint_authority.set_exclusive(new_minter);
        log_event(&SetMinterEvent {
            minter: &minter,
            new_minter: *new_minter,
        });
    }

    fn mint(&mut self, to: &ContractAddress, amount: i128) {
        Self::require_caller(&self.minter());
        let units = Self::amount(amount);
        self.require_holder(to);
        self.supply
            .try_increase(units)
            .unwrap_or_else(|e| Self::balance_error(e));
        self.credit(to, units);
        log_event(&MintEvent { to, amount });
    }

    fn authorized(&self, id: &ContractAddress) -> bool {
        let policy = self.settings().policy;
        self.authorizations
            .at(id)
            .try_get()
            .none_if_missing()
            .unwrap_or(!policy.auth_required)
    }

    fn set_authorized(&mut self, id: &ContractAddress, authorize: bool) {
        Self::require_caller(&self.admin());
        ensure(
            authorize || self.settings().policy.auth_revocable,
            TokenError::OperationDisabled,
        );
        self.authorizations.at_mut(id).set_exclusive(&authorize);
        log_event(&SetAuthorizedEvent { id, authorize });
    }

    fn clawback(&mut self, from: &ContractAddress, amount: i128) {
        Self::require_caller(&self.admin());
        let units = Self::amount(amount);
        ensure(
            self.settings().policy.clawback_enabled,
            TokenError::OperationDisabled,
        );
        self.destroy(from, units);
        log_event(&ClawbackEvent { from, amount });
    }

    fn total_supply(&self) -> i128 {
        self.settings();
        self.supply.get() as i128
    }
}
