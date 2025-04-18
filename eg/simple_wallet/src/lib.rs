#![no_std]
#![feature(maybe_uninit_slice)]

use erc20::client::ERC20Client;
use erc20::interface::{Amount, ERC20};
use sdk::storage_prelude::*;
use sdk::bytestring::bytestring_exists;

// TODO: Replay prevention?

#[derive(Storage, Debug, Serialize)]
struct Wallet {
    token_addr: ContractAddress,
    pk: Ed25519PublicKey,
}

type AuthMessage = [u8; 80];

#[cfg(feature = "guest")]
// Contract methods (below) exposed with C ABI for running as guest
pub use exports::*;

#[contract(exports = "exports")]
impl Wallet {
    fn token(&self) -> ERC20Client {
        ERC20Client::new(&self.token_addr)
    }

    pub fn initialize(
        _maybe_self: Option<&mut Self>,
        pk: &Ed25519PublicKey,
        token_addr: &ContractAddress,
    ) {
        let keypath = Keypath::root();
        let key = keypath.generate_key();

        // Make sure we haven't already been initialized
        if bytestring_exists(&key).unwrap() {
            panic!("Contract already initialized");
        }

        let value = Wallet {
            pk: *pk,
            token_addr: *token_addr,
        };

        // Store ourselves
        store(&key, &value).unwrap();
    }

    pub fn get_balance(&self) -> Amount {
        self.token().get_balance().unwrap()
    }

    pub fn test_expect_balance(&self, expect: Amount) {
        let bal = self.token().get_balance().unwrap();
        assert_eq!(bal, expect);
    }

    fn compute_auth_message(
        &self,
        to_addr: &ContractAddress,
        amount: Amount,
    ) -> AuthMessage {
        // TODO(geoff) this will eventually need a nonce and a domain separator(?)

        let self_addr = get_current_address().unwrap();

        let input = (&self_addr, to_addr, amount);
        let buf = sdk::serialize::to_bytes(&input).unwrap();

        let bytes: [u8; 80] = buf.try_into().unwrap();

        bytes
    }

    fn check_payment_authorization(
        &self,
        to_addr: &ContractAddress,
        amount: Amount,
        signature: &Ed25519Signature,
    ) {
        // TODO some kind of replay mechanism
        let auth_message = self.compute_auth_message(to_addr, amount);

        sdk::crypto::verify_signature(
            &PublicKey::Ed25519 { ed25519: self.pk },
            &Signature::Ed25519 {
                ed25519: *signature,
            },
            &auth_message,
        )
        .unwrap();
    }

    pub fn send_payment(
        &self,
        to_addr: &ContractAddress,
        amount: Amount,
        signature: &Ed25519Signature,
    ) {
        self.check_payment_authorization(to_addr, amount, signature);

        self.token().transfer(to_addr, amount).unwrap();
    }

    /*
    pub fn try_send_payment(&self,
        to_addr: &ContractAddress,
        amount: Amount,
        signature: &Ed25519Signature)
    {
        self.check_payment_authorization(to_addr, amount, signature);

        self.token().set_try_invoke().transfer(&to_addr, amount);
    }
    */

    pub fn mint(&self, to_addr: &ContractAddress, amount: Amount) {
        self.token().mint(to_addr, amount).unwrap();
    }
}
