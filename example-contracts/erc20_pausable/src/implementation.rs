//! A pausable ERC20 with metadata and role-based access control.
//!
//! The token core follows the reference `erc20` example. On top of it:
//!
//! - [`ERC20Pausable`]: holders of [`PAUSER_ROLE`] may pause and unpause;
//!   while paused, every balance-changing operation throws
//!   [`ERC20PausableError::EnforcedPause`].
//! - [`AccessControl`]: implemented by embedding
//!   [`AccessControlState`] from the `groundhog-access-control`
//!   component crate and delegating the trait methods to it.
//!   `initialize` grants [`DEFAULT_ADMIN_ROLE`] and [`PAUSER_ROLE`] to
//!   the administrator; the component's flat default administers every
//!   role with [`DEFAULT_ADMIN_ROLE`].
//! - [`ERC165`]: `impl_erc165!()`; `supports_interface` reads the
//!   linker-collected interface table, so it reports exactly the impl
//!   blocks in this file.

use crate::interface::*;
use groundhog_access_control::AccessControlState;
use sdk::storage::{Balance, Deletable, Map, Value};
use sdk::*;

type Allowance = sdk::storage::NNInt;

hash_ident! { pub static PAUSER_ROLE: SHA256Hash; }

#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct PausableTokenContract {
    config: Value<Config>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: Balance,
    paused: Value<bool>,
    access: AccessControlState,
}

#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Config {
    pub administrator: ContractAddress,
    pub name: Option<Name>,
    pub symbol: Option<Symbol>,
    pub decimals: Option<u32>,
}

impl PausableTokenContract {
    fn _init(&mut self, config: &Config) {
        if self.config.try_get().none_if_missing().is_some() {
            throw(ERC20Error::AlreadyInitialized);
        }

        self.config.set(config);

        // The administrator starts with both roles; the role wiring can be
        // changed afterwards through the AccessControl surface.
        self.access
            .grant_role_unchecked(DEFAULT_ADMIN_ROLE, &config.administrator);
        self.access
            .grant_role_unchecked(&PAUSER_ROLE, &config.administrator);
    }

    fn _administrator(&self) -> ContractAddress {
        self.config.get().administrator
    }

    fn _paused(&self) -> bool {
        self.paused.try_get().none_if_missing().unwrap_or(false)
    }

    fn _require_not_paused(&self) {
        ensure(!self._paused(), ERC20PausableError::EnforcedPause);
    }

    // These internal token functions do _not_ check authorization or the
    // pause state.

    fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
        self.balances.at(owner).get()
    }

    fn _lazy_increase_balance(
        &mut self,
        owner: &ContractAddress,
        amount: Amount,
    ) {
        self.balances
            .at_mut(owner)
            .try_lazy_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _decrease_balance(&mut self, owner: &ContractAddress, amount: Amount) {
        self.balances
            .at_mut(owner)
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.allowances
            .at(owner)
            .at(spender)
            .try_get()
            .none_if_missing()
            .unwrap_or(0) as Amount
    }

    // Adjust allowance for owner and spender, from zero if empty.
    fn _spend_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let mut owner_allowances = self.allowances.at_mut(owner);
        let mut allowance = owner_allowances.at_mut(spender);
        allowance
            .try_strict_inc(diff)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _increase_total_supply(&mut self, amount: Amount) {
        self.total_supply
            .try_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _total_supply(&self) -> Amount {
        self.total_supply.get()
    }
}

// The ERC20 token core. Balance-changing operations additionally require
// the token to be unpaused (allowance adjustments and reads stay
// available), matching OpenZeppelin's ERC20Pausable.
#[contract]
impl ERC20 for PausableTokenContract {
    fn initialize(&mut self, administrator: &ContractAddress) {
        self._init(&Config {
            administrator: *administrator,
            name: None,
            symbol: None,
            decimals: None,
        });
    }

    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount) {
        check_amount(amount);
        self._require_not_paused();
        let owner = get_caller();

        self._decrease_balance(&owner, amount);
        self._lazy_increase_balance(receiver, amount);

        TransferEvent::new(&owner, receiver, amount).log();
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self._require_not_paused();
        let spender = get_caller();

        let diff = check_amount(amount);
        self._spend_allowance(owner, &spender, -diff);
        self._decrease_balance(owner, amount);
        self._lazy_increase_balance(receiver, amount);

        TransferEvent::new(owner, receiver, amount).log();
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
    ) {
        let owner = get_caller();
        let mut allowances = self.allowances.at_mut(&owner);
        let mut allowance = allowances.at_mut(spender);
        allowance
            .try_strict_inc(difference)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn delete_allowance(&mut self, spender: &ContractAddress) {
        let owner = get_caller();
        let remaining = self.allowance(&owner, spender);
        self.allowances
            .at_mut(&owner)
            .at_mut(spender)
            .delete_exclusive();
        AllowanceDeleteEvent::new(&owner, spender, remaining).log();
    }

    fn delete_balance(&mut self) {
        let owner = get_caller();
        let mut balance = self.balances.at_mut(&owner);
        ensure(balance.get() == 0, ERC20Error::BalanceNotZero);
        balance.delete_exclusive();
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self._allowance(owner, spender)
    }

    fn get_balance_of(&self, owner: &ContractAddress) -> Amount {
        self._balance_amount_of(owner)
    }

    fn get_balance(&self) -> Amount {
        let owner = get_caller();
        self._balance_amount_of(&owner)
    }

    fn mint(&mut self, receiver: &ContractAddress, amount: Amount) {
        check_amount(amount);
        self._require_not_paused();
        let caller = get_caller();
        ensure(
            caller == self._administrator(),
            ERC20Error::MustBeContractOwner,
        );

        // Establish the checked supply bound first. Once it holds, the lazy
        // receiver increase cannot overflow an individual balance.
        self._increase_total_supply(amount);
        self._lazy_increase_balance(receiver, amount);

        MintEvent::new(receiver, amount).log();
    }

    fn total_supply(&self) -> Amount {
        self._total_supply()
    }
}

#[contract]
impl ERC20Metadata for PausableTokenContract {
    fn initialize(
        &mut self,
        administrator: &ContractAddress,
        name: Option<Name>,
        symbol: Option<Symbol>,
        decimals: Option<u32>,
    ) {
        self._init(&Config {
            administrator: *administrator,
            name,
            symbol,
            decimals,
        });
    }

    fn name(&self) -> Option<Name> {
        self.config.get().name
    }

    fn symbol(&self) -> Option<Symbol> {
        self.config.get().symbol
    }

    fn decimals(&self) -> Option<u32> {
        self.config.get().decimals
    }
}

#[contract]
impl ERC20Pausable for PausableTokenContract {
    fn pause(&mut self) {
        let caller = get_caller();
        ensure(
            self.access.has_role(&PAUSER_ROLE, &caller),
            ERC20PausableError::MustBePauser,
        );
        ensure(!self._paused(), ERC20PausableError::EnforcedPause);

        self.paused.set(&true);
        PausedEvent::new(caller).log();
    }

    fn unpause(&mut self) {
        let caller = get_caller();
        ensure(
            self.access.has_role(&PAUSER_ROLE, &caller),
            ERC20PausableError::MustBePauser,
        );
        ensure(self._paused(), ERC20PausableError::ExpectedPause);

        self.paused.set(&false);
        UnpausedEvent::new(caller).log();
    }

    fn paused(&self) -> bool {
        self._paused()
    }
}

// Pure delegation into the embedded component: the storage layout, the
// admin checks, and the event emission all live in
// `groundhog-access-control`.
#[contract]
impl AccessControl for PausableTokenContract {
    fn has_role(&self, role: &Role, account: &ContractAddress) -> bool {
        self.access.has_role(role, account)
    }

    fn get_role_admin(&self, role: &Role) -> Role {
        self.access.role_admin(role)
    }

    fn grant_role(&mut self, role: &Role, account: &ContractAddress) {
        self.access.grant_role(role, account);
    }

    fn revoke_role(&mut self, role: &Role, account: &ContractAddress) {
        self.access.revoke_role(role, account);
    }

    fn renounce_role(&mut self, role: &Role) {
        self.access.renounce_role(role);
    }
}

// ERC-165 interface detection: `supports_interface` reads the interface
// table the impl blocks above leave behind, so there is no list here to
// keep in sync, and it needs no storage, so it lives on the macro's own
// stateless carrier rather than on the token.
groundhog_erc165_if::impl_erc165!();
