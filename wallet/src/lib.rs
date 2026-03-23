mod account;
mod nonce;
mod keychain;
mod vecm;
mod keypair;

pub use account::*;
pub use nonce::*;
pub use keypair::*;
pub use keychain::*;
pub use vecm::*;

use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;

/// Offline state for a user wallet
pub struct Wallet {
    keychain: Keychain,
    tx_nonce_next: UniquifierNonce,
    seed: Seed,
}

impl Wallet {
    pub fn new(seed: Seed) -> Self {
        Wallet {
            keychain: Keychain::new(),
            tx_nonce_next: UniquifierNonce::initial(),
            seed,
        }
    }

    pub fn generate_tx_nonce(&mut self) -> UniquifierNonce {
        let n = self.tx_nonce_next;
        self.tx_nonce_next += 1;
        n
    }

    pub fn create_account(&mut self, account_name: &str) {
        self.keychain.create_account(account_name, &self.seed);
    }

    pub fn get_account(&self, account_name: &str) -> Option<&Account> {
        self.keychain.get_account(account_name)
    }

    /// Generate a signed transaction that will deploy a wallet contract at the
    /// address associated with the named public-key account.
    ///
    pub fn account_deploy_wallet(&mut self, account_name: &str, wallet_contract_hash: &SHA256Hash) -> SignedTransaction {
        let tx_nonce = self.generate_tx_nonce();

        let account = self.get_account(account_name).unwrap();

        account.deploy_wallet(wallet_contract_hash, tx_nonce)
    }
}
