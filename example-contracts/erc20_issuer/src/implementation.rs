use crate::*;
use sdk::signer::Signed;
use sdk::storage::{Balance, Map, NNInt, Value};
use sdk::*;

#[sdk::storage]
pub struct TokenContract {
    settings: Value<Settings>,
    administrator: Value<ContractAddress>,
    mint_authority: Value<ContractAddress>,
    authorizations: Map<ContractAddress, Value<bool>>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, NNInt>>,
    supply: Balance,
}

#[contract_type]
struct Settings {
    pub name: Option<Name>,
    pub symbol: Option<Symbol>,
    pub decimals: Option<u32>,
    pub policy: Policy,
}

impl TokenContract {
    fn set_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        amount: Amount,
    ) {
        self.settings();
        self.allowances
            .at_mut(owner)
            .at_mut_exclusive(spender)
            .set(amount_to_diff(amount));
        ApprovalEvent::new(owner, spender, amount_to_diff(amount)).log();
    }
    fn settings(&self) -> Settings {
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
    fn require_holder(&self, id: &ContractAddress) {
        ensure(self.authorized(id), ERC20ControlsError::HolderNotAuthorized);
    }
    fn init(
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
    fn move_tokens(
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
    fn destroy(&mut self, from: &ContractAddress, amount: Amount) {
        self.debit(from, amount);
        self.supply
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        BurnEvent::new(from, amount_to_diff(amount)).log();
    }
    fn forced_burn(&mut self, from: &ContractAddress, amount: Amount) {
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
    fn approve(&mut self, spender: &ContractAddress, amount: Amount) {
        self.set_allowance(&get_caller(), spender, amount);
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
            new_admin: *new_admin,
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
            new_minter: *new_minter,
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
    fn permit(&mut self, grant: Signed<PermitGrant>) {
        let (grant, owner) = grant.consume();
        self.set_allowance(&owner, &grant.spender, grant.amount);
    }

    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
    ) {
        let (grant, owner) = grant.consume();
        ensure(get_caller() == grant.spender, ERC20PermitError::NotSpender);
        self.move_tokens(&owner, receiver, grant.amount);
    }
}
