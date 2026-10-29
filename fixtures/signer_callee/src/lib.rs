#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::mintable::{AuthTokenPreimage, MintableType};
use thruput_sdk::signer::Signed;
use thruput_sdk::*;

type TransferAuthPreimage = <TransferAuth as MintableType>::Preimage;

#[contract_type]
pub struct TransferAuth {
    pub amount: u64,
}

pub struct SignerCallee;

#[contract]
impl SignerCallee {
    pub fn accept_signer(auth: Signed<TransferAuth>) -> u32 {
        auth.handle()
    }

    pub fn accept_mixed(x: u32, auth: Signed<TransferAuth>, y: u32) -> u32 {
        x + y + auth.handle()
    }

    pub fn mint_and_return(
        consumer: &ContractAddress,
        amount: u64,
    ) -> Signed<TransferAuth> {
        let expiry = thruput_sdk::block::get_block_height();
        thruput_sdk::signer::mint_auth_token::<TransferAuth>(
            expiry,
            AuthTokenPreimage {
                consumer: *consumer,
                data: TransferAuthPreimage { amount },
            },
        )
    }
}
