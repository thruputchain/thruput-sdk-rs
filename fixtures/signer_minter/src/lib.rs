#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

thruput_sdk::declare_mock_contract!();

use alloc::vec;
use signer_leaf::LeafAuth;
use signer_middle::{MiddleAuth, SignerMiddleClient};
use thruput_sdk::mintable::AuthTokenPreimage;
use thruput_sdk::mintable::MintableType;
use thruput_sdk::signer::Signed;
use thruput_sdk::*;

type MiddleAuthPreimage = <MiddleAuth as MintableType>::Preimage;
type LeafAuthPreimage = <LeafAuth as MintableType>::Preimage;

pub struct SignerMinter;

#[contract]
impl SignerMinter {
    pub fn mint_and_call(
        middle_contract: &ContractAddress,
        leaf_contract: &ContractAddress,
    ) -> u64 {
        let expiry = thruput_sdk::block::get_block_height();

        let preimage = AuthTokenPreimage {
            consumer: *middle_contract,
            data: MiddleAuthPreimage {
                label: 1000,
                child: AuthTokenPreimage {
                    consumer: *leaf_contract,
                    data: LeafAuthPreimage { data: 10 },
                },
                extra_children: vec![
                    AuthTokenPreimage {
                        consumer: *leaf_contract,
                        data: LeafAuthPreimage { data: 20 },
                    },
                    AuthTokenPreimage {
                        consumer: *leaf_contract,
                        data: LeafAuthPreimage { data: 30 },
                    },
                ],
            },
        };

        let token: Signed<MiddleAuth> = thruput_sdk::signer::mint_auth_token::<
            MiddleAuth,
        >(expiry, preimage);

        SignerMiddleClient::new(middle_contract)
            .consume_and_forward(token, leaf_contract)
    }
}
