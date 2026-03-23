use groundhog_xdr::types::*;
use ed25519_compact::KeyPair as _KeyPair;
use std::ops::Deref;

pub use ed25519_compact::Seed;

#[derive(Clone)]
pub struct Keypair {
    inner: _KeyPair,
}

impl Keypair {
    pub fn generate(seed: &Seed) -> Self {
        Self {
            inner: _KeyPair::from_seed(seed.clone()),
        }
    }

    pub fn public(&self) -> PublicKey {
        let _pk = &self.inner.pk;
        let pk_ed = Ed25519PublicKey::from(_pk.deref().clone());
        let public = PublicKey::Ed25519 { ed25519: pk_ed };

        public
    }

    pub fn secret(&self) -> SecretKey {
        let _sk = &self.inner.sk;
        let sk_ed = Ed25519SecretKey::from(_sk.deref().clone());
        let secret = SecretKey::Ed25519 { ed25519: sk_ed };

        secret
    }

    pub fn sign(&self, msg: &impl AsRef<[u8]>) -> Signature {
        let _noise = None;
        let _sig = self.inner.sk.sign(msg, _noise);
        let bytes: &[u8; 64] = &*_sig;

        // Lame: ed25519_compact doesn't seem to allow moving the bytes out
        let sig_ed = Ed25519Signature::from(bytes.clone());
        let sig = Signature::Ed25519 { ed25519: sig_ed };

        sig
    }
}
