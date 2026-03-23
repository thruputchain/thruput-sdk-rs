use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use groundhog_xdr::auth_claim::*;
use groundhog_contract_util::*;
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;
use xdrs::txs;
use sha2::{Digest, Sha256};
use crate::{Keypair, PubkeyAccountNonce, Seed, UniquifierNonce, vecM};

// SDK dependencies
use sdk::sha256_static;

pub struct Account {
    name: String,
    keypair: Keypair,
    /// A nonce to distinguish multiple "pubkey account" addresses using the
    /// same keypair. (For now, we use only one nonce per keypair.)
    nonce: PubkeyAccountNonce,
}

const PUBKEY_ACCOUNT_TAG: SHA256Hash = sha256_static!("PUBKEY_ACCOUNT_TAG");

impl Account {
    pub fn create(name: &str, seed: &Seed) -> Self {
        Account {
            name: name.to_owned(),
            keypair: Keypair::generate(seed),
            nonce: PubkeyAccountNonce::initial(),
        }
    }

    pub fn nonce(&self) -> PubkeyAccountNonce {
        self.nonce
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn pubkey(&self) -> PublicKey {
        self.keypair.public()
    }

    pub fn sign(&self, msg: &impl AsRef<[u8]>) -> Signature {
        self.keypair.sign(msg)
    }

    /// Generate the address associated with this public-key account.
    ///
    pub fn address(&self) -> ContractAddress {
        let pubkey = self.keypair.public();
        let nonce = self.nonce.into();
        let preimage = PubkeyAccountPreimage { pubkey, nonce };
        let preimage_bytes = txs::encode(&preimage).unwrap();

        let digest: [u8; 32] = Sha256::digest(preimage_bytes).into();
        let mut digest: SHA256Hash = digest.into();
        digest ^= &PUBKEY_ACCOUNT_TAG;
        digest.into()
    }

    /// Generate a signed transaction that will deploy a wallet contract at the
    /// address associated with this public-key account.
    ///
    pub fn deploy_wallet(&self, wallet_contract_hash: &SHA256Hash, tx_nonce: UniquifierNonce) -> SignedTransaction {
        //
        // Construct the invocation `Wallet::deploy_in_place(contract_hash: &SHA256Hash)`

        let interface_name = Some("Wallet");
        let version_str = "0.0.0";
        let method_name = "deploy_in_place";
        let entrypoint = contract_entrypoint_id(
            interface_name,
            version_str,
            &NormalizedMethodSignature::new(method_name),
        );
        let method_args = &(wallet_contract_hash,);

        let calldata = txs::encode(method_args).unwrap().try_into().unwrap();

        let invocation = Invocation {
            invoked_address: self.address(),
            entrypoint,
            calldata,
            gas_limit: 0, // TODO
        };

        // Construct the transaction

        let uniquifier = Uniquifier {
            block_expiry_height: 0, /* TODO: recent block height plus a window? */
            nonce: tx_nonce.into(),
        };

        let pubkey_preimage = PubkeyAccountPreimage {
            pubkey: self.pubkey(),
            nonce: self.nonce().into(),
        };

        let pubkey_auth_claim = AuthClaim::kPubkeySignature { pubkey_preimage };

        let tx = Transaction {
            invocation,
            uniquifier,
            contracts_to_deploy: vecM![],
            auth_claims: vecM![pubkey_auth_claim],
            gas_claims: vecM![],
        };

        let tx_bytes = txs::encode(&tx).unwrap();

        let signature = self.sign(&tx_bytes);

        let pubkey_account_sig = DecoratedSignature {
            auth_claim_idx: 0, // the pubkey auth-claim
            public_key_id: 0, // XXX?
            signature,
        };

        let signatures = vecM![pubkey_account_sig];

        let deploy_stx = SignedTransaction { tx, signatures };

        deploy_stx
    }
}

impl PartialEq for Account {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Account {}

impl Hash for Account {
    fn hash<H>(&self, state: &mut H)
    where H: Hasher
    {
        self.name.hash(state);
    }
}

impl Borrow<str> for Account {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}
