#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use groundhog_erc1271_if::ERC1271Client;
use sdk::ContractAddress;
use sdk::contract;
use sdk::signer::{AssumeTokens, Signed};

pub struct Caller;

#[contract]
impl Caller {
    /// Call the wallet to authorize, expect exactly one u64-typed token back.
    pub fn authorize_single(
        wallet: &ContractAddress,
        message: &[u8],
        signature: &[u8],
    ) -> Signed<u64> {
        let tokens =
            ERC1271Client::new(wallet).is_valid_signature(message, signature);
        let (token,): (Signed<u64>,) = AssumeTokens::assume_tokens(tokens);
        token
    }

    /// Call the wallet, expect two u64-typed tokens back.
    pub fn authorize_pair(
        wallet: &ContractAddress,
        message: &[u8],
        signature: &[u8],
    ) -> u64 {
        let tokens =
            ERC1271Client::new(wallet).is_valid_signature(message, signature);
        let (a, b): (Signed<u64>, Signed<u64>) =
            AssumeTokens::assume_tokens(tokens);
        let (val_a, minter_a): (u64, ContractAddress) = a.consume();
        let (val_b, minter_b): (u64, ContractAddress) = b.consume();
        assert_eq!(*wallet, minter_a);
        assert_eq!(*wallet, minter_b);
        val_a + val_b
    }

    /// Call the wallet, expect no tokens (authorization only).
    pub fn authorize_none(
        wallet: &ContractAddress,
        message: &[u8],
        signature: &[u8],
    ) {
        let tokens =
            ERC1271Client::new(wallet).is_valid_signature(message, signature);
        let () = AssumeTokens::assume_tokens(tokens);
    }

    /// Consume a Signed<u64> token and return its inner value.
    /// Used in tests to consume tokens that are returned across contract boundaries.
    pub fn consume_token(token: Signed<u64>) -> u64 {
        token.consume().0
    }
}
