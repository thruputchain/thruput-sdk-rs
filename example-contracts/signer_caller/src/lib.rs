#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;
use sdk::signer::Signer;
use signer_callee::{SignerCalleeClient, TransferAuth};

#[derive(Default)]
pub struct SignerCaller;

#[contract(storage = false)]
impl SignerCaller {
    pub fn forward_signer(
        target: &ContractAddress,
        auth: Signer<TransferAuth>,
    ) -> u32 {
        SignerCalleeClient::new(target).accept_signer(auth)
    }

    pub fn forward_mixed(
        target: &ContractAddress,
        auth: Signer<TransferAuth>,
        extra: u32,
    ) -> u32 {
        SignerCalleeClient::new(target).accept_mixed(10, auth, extra)
    }
}
