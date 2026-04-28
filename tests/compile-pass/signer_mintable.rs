#![allow(unexpected_cfgs)]

use sdk::mintable::SignerPreimage;
use sdk::signer::Signer;
use sdk::signer_mapper::MintableType;
use sdk::*;

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[contracttype]
pub struct Payment {
    pub memo: u32,
    pub auth: Signer<TransferAuth>,
}

type PaymentPreimage = <Payment as MintableType>::Preimage;
type TransferAuthPreimage = <TransferAuth as MintableType>::Preimage;

fn main() {
    let preimage = PaymentPreimage {
        memo: 42,
        auth: SignerPreimage {
            consumer: ContractAddress::default(),
            data: TransferAuthPreimage { amount: 100 },
        },
    };

    let _node = sdk::mintable::build_auth_token_node::<Payment>(preimage);
}
