use std::collections::HashMap;
use groundhog_xdr::types::*;

#[derive(Clone)]
pub struct Keypair {
    pub secret: SecretKey,
    pub public: PublicKey,
}

impl Keypair {
    pub fn generate(/* RNG */) -> Self {
        let ed_pk = Ed25519PublicKey::from([0; 32]);
        let public = PublicKey::Ed25519 { ed25519: ed_pk };

        let ed_sk = Ed25519SecretKey::from([0; 64]);
        let secret = SecretKey::Ed25519 { ed25519: ed_sk };

        Self { public, secret }
    }
}

pub type Keychain = HashMap<String, (Keypair, ReplayNonce)>;
