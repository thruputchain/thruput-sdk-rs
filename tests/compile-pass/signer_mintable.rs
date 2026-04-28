#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;
use sdk::mintable::{UnsignedSigner, ForMinting};

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[contracttype]
pub struct Payment {
    pub memo: u32,
    pub auth: Signer<TransferAuth>,
}

fn main() {
    // ForMinting mode: same struct name, different generic parameter
    let unsigned: Payment<ForMinting> = Payment {
        memo: 42,
        auth: UnsignedSigner {
            consumer: ContractAddress::default(),
            data: TransferAuth { amount: 100 },
        },
    };

    let _node = sdk::mintable::build_auth_token_node::<Payment>(unsigned);
}
