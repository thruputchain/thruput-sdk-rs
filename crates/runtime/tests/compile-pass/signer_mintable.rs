#![allow(unexpected_cfgs)]

use sdk::mintable::{AuthTokenPreimage, MintableType};
use sdk::signer::Signed;
use sdk::*;

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[contracttype]
pub struct Payment {
    pub memo: u32,
    pub auth: Signed<TransferAuth>,
}

type PaymentPreimage = <Payment as MintableType>::Preimage;
type TransferAuthPreimage = <TransferAuth as MintableType>::Preimage;

fn main() {
    let preimage = PaymentPreimage {
        memo: 42,
        auth: AuthTokenPreimage {
            consumer: ContractAddress::default(),
            data: TransferAuthPreimage { amount: 100 },
        },
    };

    let _node = sdk::mintable::build_auth_token_node::<Payment>(preimage);
}
