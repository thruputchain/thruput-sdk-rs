use super::*;
use std::collections::HashSet;

pub struct Keychain {
    accounts: HashSet<Account>,
}

impl Keychain {
    pub fn new() -> Self {
        Self { accounts: HashSet::new() }
    }

    pub fn create_account(&mut self, name: &str, seed: &Seed) {
        self.accounts.insert(Account::create(name, seed));
    }

    pub fn get_account(&self, name: &str) -> Option<&Account> {
        self.accounts.get(name)
    }
}
