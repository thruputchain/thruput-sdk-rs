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
    fn settings(&self) -> Settings {
        self.settings
            .try_get()
            .none_if_missing()
            .unwrap_or_else(|| throw(TokenError::NotInitialized))
    }
    fn require_admin(&self) {
        ensure(get_caller() == self.admin(), TokenError::Unauthorized);
    }
    fn require_holder(&self, id: &ContractAddress) {
        ensure(self.authorized(id), TokenError::HolderNotAuthorized);
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
            TokenError::InvalidPolicy,
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
        log_approval_event(owner, spender, diff);
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
        log_transfer_event(from, to, amount);
    }
    fn destroy(&mut self, from: &ContractAddress, amount: Amount) {
        self.debit(from, amount);
        self.supply
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        log_burn_event(from, amount);
    }
    fn forced_burn(&mut self, from: &ContractAddress, amount: Amount) {
        ensure(
            get_caller() == self.admin(),
            ERC20Error::MustBeContractOwner,
        );
        ensure(
            self.settings().policy.clawback_enabled,
            TokenError::OperationDisabled,
        );
        self.destroy(from, amount);
        log_event(&ClawbackEvent {
            from,
            amount: amount_to_diff(amount),
        });
    }
}

// Policy extensions can also throw TokenError through the standard client's
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
        self.settings();
        let owner = get_caller();
        self.allowances
            .at_mut(&owner)
            .at_mut_exclusive(spender)
            .set(amount_to_diff(amount));
        log_approve_event(&owner, spender, amount);
    }
    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        self.adjust(&get_caller(), spender, diff);
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
        log_mint_event(receiver, amount);
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
impl TokenAdmin for TokenContract {
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
        self.require_admin();
        let minter = self.minter();
        self.mint_authority.set_exclusive(new_minter);
        log_event(&SetMinterEvent {
            minter: &minter,
            new_minter: *new_minter,
        });
    }
    fn policy(&self) -> Policy {
        self.settings().policy
    }
}

#[contract(version = "0.1.0")]
impl TokenAuthorization for TokenContract {
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
            TokenError::OperationDisabled,
        );
        self.authorizations.at_mut(id).set_exclusive(&authorize);
        log_event(&SetAuthorizedEvent { id, authorize });
    }
}

#[contract(version = "0.1.0")]
impl TokenClawback for TokenContract {
    fn clawback(&mut self, from: &ContractAddress, amount: Amount) {
        self.forced_burn(from, amount);
    }
}

#[contract(version = "0.1.0")]
impl TokenBurn for TokenContract {
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
        self.adjust(&owner, &grant.spender, amount_to_diff(grant.amount));
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
