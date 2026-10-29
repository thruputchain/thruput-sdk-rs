//! Scheme-tagged keys as ordinary contract values: a [`PublicKey`] taken as
//! an argument, kept in storage and hashed, and a [`Signature`] verified
//! against it, whether the key is Ed25519 or ML-DSA-65.
//!
//! An ML-DSA-65 key is 1953 bytes encoded, over the chain-default 1024-byte
//! limits on an event, a frame's return buffer and a storage value. So the
//! contract names keys by [`key_id`] in its event and its getter, and storing
//! one needs a chain whose `max_bytestring_len` admits it. Calldata (64 KiB
//! by default) carries one as an argument without help.

extern crate alloc;

use alloc::vec::Vec;
use thruput_sdk::crypto::verify_signature;
use thruput_sdk::storage::Value;
use thruput_sdk::*;

/// Domain tag for [`key_id`].
const KEY_ID_DOMAIN: &str = "scheme-keys-example-key-id";

/// A domain-tagged hash of `key`, scheme included.
pub fn key_id(key: &PublicKey) -> SHA256Hash {
    (KEY_ID_DOMAIN, key).hash()
}

#[derive(Event)]
pub struct KeyStored {
    #[topic]
    pub key_id: SHA256Hash,
    /// The key's `CryptographicScheme`.
    pub scheme: i32,
}

#[contract_exception]
pub enum KeyError {
    NoKey = 1,
}

#[thruput_sdk::storage]
pub struct SchemeKeys {
    key: Value<PublicKey>,
}

impl SchemeKeys {
    fn stored_key(&self) -> PublicKey {
        self.key.try_get().none_if_missing().unwrap_or_else(|| {
            throw(KeyError::NoKey);
        })
    }
}

#[contract(
    version = "0.1.0",
    throws = KeyError,
    emits(KeyStored),
    client = SchemeKeysClient
)]
impl SchemeKeys {
    #[contract(effect = "stateChanging")]
    pub fn set_key(&mut self, key: PublicKey) {
        log_event(&KeyStored {
            key_id: key_id(&key),
            scheme: match key {
                PublicKey::Ed25519 { .. } => CryptographicScheme::Ed25519.0,
                PublicKey::MlDsa65 { .. } => CryptographicScheme::MlDsa65.0,
                _ => {
                    unreachable!("the key decoded as a scheme this SDK knows")
                }
            },
        });
        self.key.set(&key);
    }

    /// The [`key_id`] of the stored key.
    #[contract(effect = "readOnly")]
    pub fn key_id(&self) -> SHA256Hash {
        key_id(&self.stored_key())
    }

    /// Whether `signature` over `message` verifies under the stored key.
    #[contract(effect = "readOnly")]
    pub fn verify(&self, signature: Signature, message: Vec<u8>) -> bool {
        verify_signature(&self.stored_key(), &signature, &message)
    }
}
