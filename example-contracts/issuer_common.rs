// Shared source for the native and SEP-41 issuer profiles. Each supplies its
// own Allowances storage and spending entrypoints. Deployments do not share state.
use crate::*;
use sdk::storage::{Balance, Map, Value};
use sdk::*;

#[sdk::storage]
pub struct TokenContract {
    pub(crate) settings: Value<Settings>,
    pub(crate) administrator: Value<ContractAddress>,
    pub(crate) mint_authority: Value<ContractAddress>,
    pub(crate) authorizations: Map<ContractAddress, Value<bool>>,
    pub(crate) balances: Map<ContractAddress, Balance>,
    pub(crate) allowances: Allowances,
    pub(crate) supply: Balance,
}

#[contract_type]
pub(crate) struct Settings {
    pub name: Option<Name>,
    pub symbol: Option<Symbol>,
    pub decimals: Option<u32>,
    pub policy: Policy,
}

impl TokenContract {
    pub(crate) fn settings(&self) -> Settings {
        self.settings
            .try_get()
            .none_if_missing()
            .unwrap_or_else(|| throw(ERC20ControlsError::NotInitialized))
    }
    fn require_admin(&self) {
        ensure(
            get_caller() == self.admin(),
            ERC20ControlsError::Unauthorized,
        );
    }
    pub(crate) fn require_holder(&self, id: &ContractAddress) {
        ensure(self.authorized(id), ERC20ControlsError::HolderNotAuthorized);
    }
    pub(crate) fn init(
        &mut self,
        administrator: &ContractAddress,
        minter: &ContractAddress,
        settings: &Settings,
    ) {
        ensure(
            self.settings.try_get().none_if_missing().is_none(),
            ERC20Error::AlreadyInitialized,
        );
        ensure(
            !settings.policy.clawback_enabled
                || settings.policy.auth_revocable,
            ERC20ControlsError::InvalidPolicy,
        );
        self.settings.set_exclusive(settings);
        self.administrator.set_exclusive(administrator);
        self.mint_authority.set_exclusive(minter);
    }
    fn debit(&mut self, from: &ContractAddress, amount: Amount) {
        amount_to_diff(amount);
        self.balances
            .at_mut(from)
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }
    fn credit(&mut self, to: &ContractAddress, amount: Amount) {
        // Checked supply and conservation bound each holder's balance.
        self.balances
            .at_mut(to)
            .try_lazy_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }
    pub(crate) fn move_tokens(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: Amount,
    ) {
        self.require_holder(from);
        self.require_holder(to);
        self.debit(from, amount);
        self.credit(to, amount);
        TransferEvent::new(from, to, amount_to_diff(amount)).log();
    }
    pub(crate) fn destroy(&mut self, from: &ContractAddress, amount: Amount) {
        self.debit(from, amount);
        self.supply
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        BurnEvent::new(from, amount_to_diff(amount)).log();
    }
    pub(crate) fn forced_burn(
        &mut self,
        from: &ContractAddress,
        amount: Amount,
    ) {
        ensure(
            get_caller() == self.admin(),
            ERC20Error::MustBeContractOwner,
        );
        ensure(
            self.settings().policy.clawback_enabled,
            ERC20ControlsError::OperationDisabled,
        );
        self.destroy(from, amount);
        ClawbackEvent {
            from,
            amount: amount_to_diff(amount),
        }
        .log();
    }
    pub(crate) fn mint_tokens(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        ensure(
            get_caller() == self.minter(),
            ERC20Error::MustBeContractOwner,
        );
        self.require_holder(receiver);
        amount_to_diff(amount);
        self.supply
            .try_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        self.credit(receiver, amount);
        MintEvent::new(receiver, amount_to_diff(amount)).log();
    }
}

#[contract(version = "0.1.0")]
impl ERC20Admin for TokenContract {
    fn initialize(
        &mut self,
        minter: &ContractAddress,
        name: Name,
        symbol: Symbol,
        decimals: u32,
        policy: Policy,
    ) {
        self.init(
            &get_caller(),
            minter,
            &Settings {
                name: Some(name),
                symbol: Some(symbol),
                decimals: Some(decimals),
                policy,
            },
        );
    }
    fn admin(&self) -> ContractAddress {
        self.settings();
        self.administrator.get()
    }
    fn set_admin(&mut self, new_admin: &ContractAddress) {
        self.require_admin();
        let admin = self.admin();
        self.administrator.set_exclusive(new_admin);
        SetAdminEvent {
            admin: &admin,
            new_admin,
        }
        .log();
    }
    fn minter(&self) -> ContractAddress {
        self.settings();
        self.mint_authority.get()
    }
    fn set_minter(&mut self, new_minter: &ContractAddress) {
        self.require_admin();
        let minter = self.minter();
        self.mint_authority.set_exclusive(new_minter);
        SetMinterEvent {
            minter: &minter,
            new_minter,
        }
        .log();
    }
    fn policy(&self) -> Policy {
        self.settings().policy
    }
}

#[contract(version = "0.1.0")]
impl ERC20Authorization for TokenContract {
    fn authorized(&self, id: &ContractAddress) -> bool {
        let policy = self.settings().policy;
        self.authorizations
            .at(id)
            .try_get()
            .none_if_missing()
            .unwrap_or(!policy.auth_required)
    }
    fn set_authorized(&mut self, id: &ContractAddress, authorize: bool) {
        self.require_admin();
        ensure(
            authorize || self.settings().policy.auth_revocable,
            ERC20ControlsError::OperationDisabled,
        );
        self.authorizations.at_mut(id).set_exclusive(&authorize);
        SetAuthorizedEvent { id, authorize }.log();
    }
}

#[contract(version = "0.1.0")]
impl ERC20Clawback for TokenContract {
    fn clawback(&mut self, from: &ContractAddress, amount: Amount) {
        self.forced_burn(from, amount);
    }
}
