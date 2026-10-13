#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use signer_callee::{SignerCalleeClient, TransferAuth};
use thruput_sdk::signer::Signed;
use thruput_sdk::*;

pub struct SignerCaller;

#[contract]
impl SignerCaller {
    pub fn forward_signer(
        target: &ContractAddress,
        auth: Signed<TransferAuth>,
    ) -> u32 {
        SignerCalleeClient::new(target).accept_signer(auth)
    }

    pub fn forward_mixed(
        target: &ContractAddress,
        auth: Signed<TransferAuth>,
        extra: u32,
    ) -> u32 {
        SignerCalleeClient::new(target).accept_mixed(10, auth, extra)
    }

    pub fn mint_and_forward(
        target: &ContractAddress,
        amount: u64,
    ) -> Signed<TransferAuth> {
        let address = thruput_sdk::get_current_address();
        SignerCalleeClient::new(target).mint_and_return(&address, amount)
    }

    pub fn consume_minted_token(auth: Signed<TransferAuth>, amount: u64) {
        let value = auth.consume().0;
        assert_eq!(value.amount, amount);
    }
}
