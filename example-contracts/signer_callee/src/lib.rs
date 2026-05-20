#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::{AuthTokenPreimage, MintableType};
use sdk::signer::Signed;
use sdk::*;

type TransferAuthPreimage = <TransferAuth as MintableType>::Preimage;

#[contract_type]
pub struct TransferAuth {
    pub amount: u64,
}

#[derive(Default)]
pub struct SignerCallee;

#[contract(storage = false)]
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
        let expiry = sdk::block::get_block_height();
        sdk::signer::mint_auth_token::<TransferAuth>(
            expiry,
            AuthTokenPreimage {
                consumer: *consumer,
                data: TransferAuthPreimage { amount },
            },
        )
    }
}
