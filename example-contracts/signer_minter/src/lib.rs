#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::SignerPreimage;
use sdk::signer::Signer;
use sdk::*;
use signer_leaf::__leafauth_preimage;
use signer_middle::{MiddleAuth, SignerMiddleClient};
use signer_middle::__middleauth_preimage;

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
            consumer: middle_contract.clone(),
            data: __middleauth_preimage::Preimage {
                label: 1000,
                child: SignerPreimage {
                    consumer: leaf_contract.clone(),
                    data: __leafauth_preimage::Preimage { data: 10 },
                },
                extra_children: vec![
                    SignerPreimage {
                        consumer: leaf_contract.clone(),
                        data: __leafauth_preimage::Preimage { data: 20 },
                    },
                    SignerPreimage {
                        consumer: leaf_contract.clone(),
                        data: __leafauth_preimage::Preimage { data: 30 },
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
