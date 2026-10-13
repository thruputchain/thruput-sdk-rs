#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct Wallet {
    balance: u64,
}

#[contract]
impl Wallet {
    pub fn deposit(&mut self, amount: u64) {
        self.balance += amount;
    }
}

fn main() {}
