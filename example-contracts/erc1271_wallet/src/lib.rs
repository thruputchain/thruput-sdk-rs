#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

sdk::declare_mock_contract!();

use alloc::vec::Vec;
use groundhog_erc1271_if::{ERC1271, ERC1271Error};
use sdk::contract;
use sdk::mintable::AuthTokenPreimage;
use sdk::signer::{Opaque, Signed};

#[derive(Default)]
pub struct Wallet;

#[contract(storage = false, version = "0.1.0")]
impl ERC1271 for Wallet {
    fn is_valid_signature(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> Vec<Signed<Opaque>> {
        if message != signature {
            sdk::throw(ERC1271Error::InvalidSignature);
        }

        // Mint one opaque auth token per 8-byte chunk of the message.
        // Each chunk becomes a u64 payload in its own token node.
        message
            .chunks(8)
            .map(|chunk| {
                let mut payload_bytes = [0u8; 8];
                payload_bytes[..chunk.len()].copy_from_slice(chunk);
                let value = u64::from_le_bytes(payload_bytes);

                let consumer = sdk::get_caller();
                let preimage = AuthTokenPreimage {
                    consumer,
                    data: value,
                };
                let mut node =
                    sdk::mintable::build_auth_token_node::<u64>(preimage.data);
                node.consumer_address = preimage.consumer;
                let encoded = sdk::mintable::encode_auth_token_node(&node);

                let expiry = sdk::block::get_block_height();
                sdk::signer::mint_opaque_auth_token(expiry, &encoded)
            })
            .collect()
    }
}
