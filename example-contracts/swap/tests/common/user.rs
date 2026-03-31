// Tracking of users for testing

use super::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct User {
    pub addr: &'static ContractAddress,
    pub name: &'static str,
    pub init: (u128, u128), // initial token drop requested
}

impl User {
    pub const fn new(
        addr: &'static ContractAddress,
        name: &'static str,
        init: (u128, u128),
    ) -> Self {
        Self { addr, name, init }
    }
}

hash_ident! { pub static TEST_ADDR: ContractAddress; }
hash_ident! { pub static FACTORY_ADDR: ContractAddress; }
hash_ident! { pub static PRIMER_ADDR: ContractAddress; }
hash_ident! { pub static ALICE_ADDR: ContractAddress; }
hash_ident! { pub static BOB_ADDR: ContractAddress; }
hash_ident! { pub static CAROL_ADDR: ContractAddress; }
hash_ident! { pub static DAVE_ADDR: ContractAddress; }

pub const TEST: User = User::new(&TEST_ADDR, "Test", (0, 0));
pub const FACTORY: User = User::new(&FACTORY_ADDR, "Factory", (0, 0));
pub const PRIMER: User =
    User::new(&PRIMER_ADDR, "Primer", (PRIME_AMT, PRIME_AMT));
pub const ALICE: User = User::new(&ALICE_ADDR, "Alice", (SOME_AMT, SOME_AMT));
pub const BOB: User =
    User::new(&BOB_ADDR, "Bob", (3 * SOME_AMT, 3 * SOME_AMT));
pub const CAROL: User =
    User::new(&CAROL_ADDR, "Carol", (3 * SOME_AMT, 3 * SOME_AMT));
pub const DAVE: User =
    User::new(&DAVE_ADDR, "Dave", (3 * SOME_AMT, 3 * SOME_AMT));

pub const USERS: [User; 7] = [TEST, FACTORY, PRIMER, ALICE, BOB, CAROL, DAVE];

pub const PRIME_AMT: u128 = 10u128.pow(6) * DECIMAL_FACTOR;
pub const SOME_AMT: u128 = undecimalize(1023.4);
pub const UNIT_AMT: u128 = undecimalize(1.0);
