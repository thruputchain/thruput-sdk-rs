#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::UnsignedSigner;
use sdk::signer::Signer;
use sdk::*;
use signer_middle::{MiddleAuth, SignerMiddleClient};

use signer_leaf::__groundhog_mintable_leaf_auth::LeafAuthUnsigned;
use signer_middle::__groundhog_mintable_middle_auth::MiddleAuthUnsigned;

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
            data: MiddleAuthUnsigned {
                label: 1000,
                child: UnsignedSigner {
                    consumer: leaf_contract.clone(),
                    data: LeafAuthUnsigned {
                        data: 10,
                    },
                },
                extra_children: vec![
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data:
                            LeafAuthUnsigned {
                                data: 20,
                            },
                    },
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data:
                            LeafAuthUnsigned {
                                data: 30,
                            },
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
