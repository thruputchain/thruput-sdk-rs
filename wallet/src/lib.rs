#![allow(unused)]

mod account_pubkey;
mod keychain;

use sha2::{Sha256, Digest};
use std::collections::HashMap;
use groundhog_xdr::types::*;
use groundhog_xdr::transaction::*;
use groundhog_xdr::contract::*;
// use groundhog_xdr::account::*;
use account_pubkey::*;
use keychain::*;

// Offline state in user wallet app
pub struct Wallet {
    keychain: Keychain,
    nonce_next: ReplayNonce,
}

impl Wallet {
    pub fn create_account(&mut self, name: &str) -> ContractAddress {
        let acct_keypair = Keypair::generate();

        let acct_nonce = self.nonce_next;
        self.nonce_next += 1;

        let acct_address = pubkey_account_address(&acct_keypair.public, acct_nonce);

        self.keychain.insert(name.to_owned(), (acct_keypair, acct_nonce));

        acct_address
    }

    pub fn account_address(&self, name: &str) -> ContractAddress {
        let (keypair, nonce) = self.keychain.get(name).unwrap();

        pubkey_account_address(&keypair.public, *nonce)
    }

    pub fn account_deploy_wallet(&self, name: &str) -> SignedTransaction {
        let acct_address = self.account_address(name);
        make_deploy_wallet_transaction(&acct_address)
    }
}

fn pubkey_account_address(pubkey: &PublicKey, nonce: ReplayNonce) -> ContractAddress {
    let pubkey = pubkey.clone();
    let acct_preimage = PubkeyAccountPreimage { pubkey, nonce, };
    let acct_preimage_bytes = xdrs::serialize_txs::encode(&acct_preimage).unwrap();

    let acct_address = /* PUBKEY_ACCOUNT_TAG ^ */ Sha256::digest(acct_preimage_bytes);
    let acct_address: [u8; 32] = acct_address.into();

    acct_address.into()
}

fn make_deploy_wallet_transaction(acct_address: &ContractAddress) -> SignedTransaction {
    let deploy_invocation = Invocation {
        invoked_address: *acct_address,
        entrypoint: todo!(),
        calldata: todo!() /* (wallet_contract_hash) */,
        gas_limit: todo!(),
    };

    let deploy_uniquifier = Uniquifier {
        block_expiry_height: 0, /* recent block height plus a window? */
        nonce: 0, // the user app keeps track of the last nonce used
    };

    let wallet_contract: Contract = todo!();

    let deploy_tx = Transaction {
        invocation: deploy_invocation,
        uniquifier: deploy_uniquifier,
        contracts_to_deploy: todo!(), // VecM::from([wallet_contract])
        auth_claims: todo!(),
            /*
            AuthClaim::new(kPubkeySignature)
                .pubkey_preimage({ PK, NONCE}),
            }
            */
        gas_claims: todo!(),
    };

    let deploy_stx: SignedTransaction = todo!();

    deploy_stx
}
