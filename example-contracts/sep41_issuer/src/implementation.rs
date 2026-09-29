use crate::common::*;
use crate::*;
use sdk::block::get_block_height;
use sdk::storage::{Map, NNInt, Node, Value};
use sdk::*;

pub(crate) type Allowances =
    Map<ContractAddress, Map<ContractAddress, Node<Allowance>>>;

#[sdk::storage]
pub(crate) struct Allowance {
    amount: NNInt,
    expiry: Value<u32>,
}

fn unsigned_amount(amount: i128) -> Amount {
    ensure(amount >= 0, SEP41Error::NegativeAmount);
    amount as Amount
}

fn require_caller(address: &ContractAddress) {
    ensure(get_caller() == *address, SEP41Error::Unauthorized);
}

impl TokenContract {
    fn allowance_is_live(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> bool {
        self.allowances
            .at(from)
            .at(spender)
            .expiry
            .try_get()
            .none_if_missing()
            .is_some_and(|deadline| {
                get_block_height() <= i64::from(deadline).into()
            })
    }

    fn spend_allowance(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
    ) {
        self.settings();
        if amount == 0 {
            return;
        }
        ensure(self.allowance_is_live(from, spender), ERC20Error::TooLarge);
        // Approval exclusively replaces this same key, so a changed deadline
        // cannot race a spend that read the old deadline.
        self.allowances
            .at_mut(from)
            .at_mut(spender)
            .amount
            .try_strict_inc(-amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }
}

#[contract(version = "0.1.0")]
impl SEP41 for TokenContract {
    fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> i128 {
        self.settings();
        if !self.allowance_is_live(from, spender) {
            return 0;
        }
        self.allowances
            .at(from)
            .at(spender)
            .amount
            .try_get()
            .none_if_missing()
            .unwrap_or(0)
    }

    fn approve(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
        live_until_ledger: u32,
    ) {
        self.settings();
        require_caller(from);
        unsigned_amount(amount);
        ensure(
            amount == 0
                || get_block_height() <= i64::from(live_until_ledger).into(),
            SEP41Error::InvalidExpiration,
        );
        let mut allowances = self.allowances.at_mut(from);
        let mut grant = allowances.at_mut_exclusive(spender);
        grant.amount.set(amount);
        grant.expiry.set(&live_until_ledger);
        SEP41ApprovalEvent::new(from, spender, amount, live_until_ledger)
            .log();
    }

    fn balance(&self, id: &ContractAddress) -> i128 {
        self.settings();
        self.balances.at(id).get() as i128
    }

    fn transfer(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        require_caller(from);
        self.move_tokens(from, to, unsigned_amount(amount));
    }

    fn transfer_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    ) {
        require_caller(spender);
        let value = unsigned_amount(amount);
        self.spend_allowance(from, spender, amount);
        self.move_tokens(from, to, value);
    }

    fn burn(&mut self, from: &ContractAddress, amount: i128) {
        require_caller(from);
        let value = unsigned_amount(amount);
        self.require_holder(from);
        self.destroy(from, value);
    }

    fn burn_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        amount: i128,
    ) {
        require_caller(spender);
        let value = unsigned_amount(amount);
        self.require_holder(from);
        self.spend_allowance(from, spender, amount);
        self.destroy(from, value);
    }

    fn decimals(&self) -> u32 {
        self.settings()
            .decimals
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }

    fn name(&self) -> Name {
        self.settings()
            .name
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }

    fn symbol(&self) -> Symbol {
        self.settings()
            .symbol
            .unwrap_or_else(|| throw(SEP41Error::MetadataUnavailable))
    }
}

// Issuance is deliberately outside SEP-41's transfer/allowance interface.
#[contract(client = IssuerClient, throws(SEP41Error, ERC20Error, ERC20ControlsError), emits(MintEvent))]
impl TokenContract {
    pub fn mint(&mut self, to: &ContractAddress, amount: i128) {
        self.mint_tokens(to, unsigned_amount(amount));
    }

    pub fn total_supply(&self) -> i128 {
        self.settings();
        self.supply.get() as i128
    }
}
