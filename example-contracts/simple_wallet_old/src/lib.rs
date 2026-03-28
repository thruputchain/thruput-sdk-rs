#![cfg_attr(not(feature = "dev"), no_std)]

use groundhog_erc20_if::{Amount, ERC20Client};
use sdk::*;

pub use exports::*;

#[storage]
struct Wallet {
    token_addr: ContractAddress,
    pk: Ed25519PublicKey,
}

type AuthMessage = [u8; 80];

// TODO: Replay prevention?

#[contract(client = WalletClient)]
impl Wallet {
    fn token(&self) -> ERC20Client {
        ERC20Client::new(&self.token_addr)
    }

    pub fn initialize(
        &mut self,
        pk: &Ed25519PublicKey,
        token_addr: &ContractAddress,
    ) {
        self.pk = *pk;
        self.token_addr = *token_addr;
    }

    pub fn get_balance(&self) -> Amount {
        self.token().get_balance()
    }

    pub fn test_expect_balance(&self, expect: Amount) {
        let bal = self.token().get_balance();
        assert_eq!(bal, expect);
    }

    fn compute_auth_message(
        &self,
        to_addr: &ContractAddress,
        amount: Amount,
    ) -> AuthMessage {
        // TODO(geoff) this will eventually need a nonce and a domain separator(?)

        let self_addr = get_current_address();

        let input = (&self_addr, to_addr, amount);
        let buf = sdk::__xdrs::txs::encode(&input).unwrap();

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

        sdk::crypto::verify_signature_ed25519(
            &self.pk,
            signature,
            &auth_message,
        );
    }

    pub fn send_payment(
        &self,
        to_addr: &ContractAddress,
        amount: Amount,
        signature: &Ed25519Signature,
    ) {
        self.check_payment_authorization(to_addr, amount, signature);

        self.token().transfer(to_addr, amount);
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
        self.token().mint(to_addr, amount);
    }
}
