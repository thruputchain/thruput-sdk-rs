#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::SignerPreimage;
use sdk::signer::Signer;
use sdk::signer_mapper::MintableType;
use sdk::*;
use signer_leaf::LeafAuth;
use signer_middle::{MiddleAuth, SignerMiddleClient};

type MiddleAuthPreimage = <MiddleAuth as MintableType>::Preimage;
type LeafAuthPreimage = <LeafAuth as MintableType>::Preimage;

#[derive(Default)]
pub struct SignerMinter;

#[contract(storage = false)]
impl SignerMinter {
    pub fn mint_and_call(
        middle_contract: &ContractAddress,
        leaf_contract: &ContractAddress,
    ) -> u64 {
        let expiry = sdk::block::get_block_height();

        let preimage = SignerPreimage {
            consumer: *middle_contract,
            data: MiddleAuthPreimage {
                label: 1000,
                child: SignerPreimage {
                    consumer: *leaf_contract,
                    data: LeafAuthPreimage { data: 10 },
                },
                extra_children: vec![
                    SignerPreimage {
                        consumer: *leaf_contract,
                        data: LeafAuthPreimage { data: 20 },
                    },
                    SignerPreimage {
                        consumer: *leaf_contract,
                        data: LeafAuthPreimage { data: 30 },
                    },
                ],
            },
        };

        let signer: Signer<MiddleAuth> =
            sdk::signer::mint_auth_token::<MiddleAuth>(expiry, preimage);

        SignerMiddleClient::new(middle_contract)
            .consume_and_forward(signer, leaf_contract)
    }
}
