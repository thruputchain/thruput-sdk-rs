#![allow(unexpected_cfgs)]

use thruput_sdk::mintable::{AuthTokenPreimage, MintableType};
use thruput_sdk::signer::Signed;
use thruput_sdk::*;

#[contract_type]
pub struct TransferAuth {
    pub amount: u64,
}

#[contract_type]
pub struct Payment {
    pub memo: u32,
    pub auth: Signed<TransferAuth>,
}

type PaymentPreimage = <Payment as MintableType>::Preimage;
type TransferAuthPreimage = <TransferAuth as MintableType>::Preimage;

fn main() {
    let preimage = AuthTokenPreimage {
        consumer: ContractAddress::default(),
        data: PaymentPreimage {
            memo: 42,
            auth: AuthTokenPreimage {
                consumer: ContractAddress::default(),
                data: TransferAuthPreimage { amount: 100 },
            },
        },
    };

    let _node =
        thruput_sdk::mintable::build_auth_token_node::<Payment>(preimage);
}
