#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;
use sdk::mintable::SignerPreimage;

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
    // Preimage mode: separate preimage struct in companion module
    let preimage = __payment_preimage::Preimage {
        memo: 42,
        auth: SignerPreimage {
            consumer: ContractAddress::default(),
            data: __transferauth_preimage::Preimage { amount: 100 },
        },
    };

    let _node = sdk::mintable::build_auth_token_node::<Payment>(preimage);
}
