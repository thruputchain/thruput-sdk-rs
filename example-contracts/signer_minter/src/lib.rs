#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::{ForMinting, UnsignedSigner};
use sdk::signer::Signer;
use sdk::*;
use signer_leaf::LeafAuth;
use signer_middle::{MiddleAuth, SignerMiddleClient};

#[derive(Default)]
pub struct SignerMinter;

#[contract(storage = false)]
impl SignerMinter {
    pub fn mint_and_call(
        middle_contract: &ContractAddress,
        leaf_contract: &ContractAddress,
    ) -> u64 {
        let expiry = sdk::block::get_block_height();

        let unsigned = UnsignedSigner {
            consumer: middle_contract.clone(),
            data: MiddleAuth::<ForMinting> {
                label: 1000,
                child: UnsignedSigner {
                    consumer: leaf_contract.clone(),
                    data: LeafAuth { data: 10 },
                },
                extra_children: vec![
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data: LeafAuth { data: 20 },
                    },
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data: LeafAuth { data: 30 },
                    },
                ],
            },
        };

        let signer: Signer<MiddleAuth> =
            sdk::signer::mint_auth_token::<MiddleAuth>(expiry, unsigned);

        SignerMiddleClient::new(middle_contract)
            .consume_and_forward(signer, leaf_contract)
    }
}
