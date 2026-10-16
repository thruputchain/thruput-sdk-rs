//! Vectors for signing a transaction from a standalone-pubkey account
//! (`kPubkeySignature`), for Ed25519 and ML-DSA-65 keys.
//!
//! Every stage is emitted -- key, key hash, account address, unsigned
//! transaction, signature preimage, transaction hash, signature, signed
//! transaction -- so a client that drifts fails at the stage that drifted.
//! Signatures are deterministic (Ed25519 always is; ML-DSA-65 uses FIPS 204's
//! deterministic variant, `rnd` = 0), so a client signing the same
//! transaction with the same seed must reproduce them byte for byte. The one
//! exception is the re-signing vector, which signs with a fixed non-zero
//! `rnd` to show that a second, different signature over the same transaction
//! leaves the transaction hash unchanged.
//!
//! [`cases`] is public so the test-ffi tests in this crate can check that
//! hogd's verifier accepts exactly these signed transactions.

use ed25519_dalek::Signer as _;
use ml_dsa::signature::Keypair as _;
use ml_dsa::{B32, MlDsa65};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thruput_domain_separator::sha2::{
    pubkey_account_address, public_key_hash,
};
use thruput_domain_separator::{TRANSACTION, transaction_signature_preimage};
use thruput_xdr::auth_claim::{AuthClaim, PubkeyAccountPreimage};
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::transaction::{
    DecoratedSignature, Invocation, SignedTransaction, Transaction, Uniquifier,
};
use thruput_xdr::types::{
    BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
    MlDsa65PublicKey, MlDsa65Signature, NetworkHash, PublicKey, SHA256Hash,
    Signature,
};
use xdrs::{BytesM, VecM};

use crate::hex;

/// The key id a `kPubkeySignature` claim's single signature carries
/// (`kDefaultNewAddressKeyID`).
pub const PUBKEY_CLAIM_KEY_ID: u32 = 0;

/// The default wallet's `execute` entrypoint: the first 8 bytes of
/// `sha256("DefaultWallet::0.1::execute")`. Auth does not depend on what the
/// transaction invokes; this just makes the vectors look like the transactions
/// a wallet sends.
const DEFAULT_WALLET_EXECUTE: [u8; 8] =
    [0xc3, 0xed, 0xb1, 0x1f, 0xdd, 0x6d, 0x5d, 0x58];

/// A signing key of either scheme, derived from a 32-byte seed.
pub enum TestKey {
    /// RFC 8032 key from its 32-byte secret seed.
    Ed25519(Box<ed25519_dalek::SigningKey>),
    /// FIPS 204 key from its key-generation seed `ξ`.
    MlDsa65(Box<ml_dsa::SigningKey<MlDsa65>>),
}

impl TestKey {
    pub fn ed25519(seed: [u8; 32]) -> Self {
        Self::Ed25519(Box::new(ed25519_dalek::SigningKey::from_bytes(&seed)))
    }

    pub fn mldsa65(xi: [u8; 32]) -> Self {
        Self::MlDsa65(Box::new(ml_dsa::SigningKey::<MlDsa65>::from_seed(
            &B32::from(xi),
        )))
    }

    pub fn scheme_name(&self) -> &'static str {
        match self {
            Self::Ed25519(_) => "Ed25519",
            Self::MlDsa65(_) => "MlDsa65",
        }
    }

    pub fn public_key(&self) -> PublicKey {
        match self {
            Self::Ed25519(k) => PublicKey::Ed25519 {
                ed25519: Ed25519PublicKey::from(k.verifying_key().to_bytes()),
            },
            Self::MlDsa65(k) => PublicKey::MlDsa65 {
                mldsa65: MlDsa65PublicKey::from(
                    <[u8; 1952]>::try_from(
                        k.verifying_key().encode().as_slice(),
                    )
                    .unwrap(),
                )
                .into(),
            },
        }
    }

    /// Sign `msg`: Ed25519, or deterministic pure ML-DSA-65 with an empty
    /// context.
    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.sign_with(msg, b"", [0u8; 32])
    }

    /// Sign `msg` with a given ML-DSA context and `rnd` (FIPS 204
    /// Algorithm 2). Ed25519 ignores both. Transactions are signed with an
    /// empty context; the parameter exists so tests can show other contexts
    /// are rejected.
    pub fn sign_with(
        &self,
        msg: &[u8],
        ctx: &[u8],
        rnd: [u8; 32],
    ) -> Signature {
        match self {
            Self::Ed25519(k) => Signature::Ed25519 {
                ed25519: Ed25519Signature::from(k.sign(msg).to_bytes()),
            },
            Self::MlDsa65(k) => {
                let ctx_len =
                    u8::try_from(ctx.len()).expect("context too long");
                // M' = 0 || |ctx| || ctx || M: pure ML-DSA's message prefix.
                let sig = k.expanded_key().sign_internal(
                    &[&[0, ctx_len], ctx, msg],
                    &B32::from(rnd),
                );
                Signature::MlDsa65 {
                    mldsa65: MlDsa65Signature::from(
                        <[u8; 3309]>::try_from(sig.encode().as_slice())
                            .unwrap(),
                    )
                    .into(),
                }
            }
        }
    }
}

/// `sha256("LOCAL_UNIT_TEST")`, the network `thruput-native-exec`'s
/// `TestEnvironment` runs on.
pub fn test_network_hash() -> NetworkHash {
    NetworkHash::new(SHA256Hash::new(
        Sha256::digest(b"LOCAL_UNIT_TEST").into(),
    ))
}

/// hogd devnode's network hash: all zero.
pub fn devnode_network_hash() -> NetworkHash {
    NetworkHash::new(SHA256Hash::new([0u8; 32]))
}

/// A default-wallet transaction from `key`'s pubkey account at
/// `account_nonce`, with a `kPubkeySignature` claim naming the key by hash.
pub fn pubkey_account_transaction(
    key: &TestKey,
    account_nonce: u64,
    uniquifier_nonce: u64,
) -> Transaction {
    let preimage = PubkeyAccountPreimage {
        pubkey_hash: public_key_hash(&key.public_key()),
        nonce: account_nonce,
    };
    Transaction {
        invocation: Invocation {
            invoked_address: pubkey_account_address(&preimage),
            entrypoint: ContractEntrypointID::from(DEFAULT_WALLET_EXECUTE),
            gas_limit: 10_000_000,
            // Opaque to auth; a CXS-encoded empty call list.
            calldata: BytesM(vec![0]),
        },
        uniquifier: Uniquifier {
            block_expiry_height: BlockHeight::new(1000),
            nonce: uniquifier_nonce,
        },
        contracts_to_deploy: VecM(vec![]),
        auth_claim: AuthClaim::kPubkeySignature {
            pubkey_preimage: preimage,
        },
    }
}

/// `tx` signed by `key` for `network`, with the key attached as
/// `pubkey_claim_key`.
pub fn sign_transaction(
    key: &TestKey,
    tx: Transaction,
    network: &NetworkHash,
    rnd: [u8; 32],
) -> SignedTransaction {
    let msg = transaction_signature_preimage(network, &tx).unwrap();
    SignedTransaction {
        signatures: VecM(vec![DecoratedSignature {
            public_key_id: PUBKEY_CLAIM_KEY_ID,
            signature: key.sign_with(&msg, b"", rnd),
        }]),
        pubkey_claim_key: Some(key.public_key()),
        tx,
    }
}

/// The transaction hash: `sha256(xdr(Transaction))`, covering neither the
/// signatures nor `pubkey_claim_key`.
pub fn transaction_hash(tx: &Transaction) -> [u8; 32] {
    Sha256::digest(xdrs::txs::encode(tx).unwrap()).into()
}

pub struct SigningCase {
    pub name: &'static str,
    pub seed: [u8; 32],
    pub key: TestKey,
    pub account_nonce: u64,
    pub network_name: &'static str,
    pub network_hash: NetworkHash,
    /// `None` for a deterministic signature, else the ML-DSA `rnd` used.
    pub rnd: Option<[u8; 32]>,
    /// The vector this one re-signs, if any.
    pub resign_of: Option<&'static str>,
    pub signed: SignedTransaction,
}

/// RFC 8032 section 7.1, TEST 1.
const ED25519_RFC8032_TEST1: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4,
    0x92, 0xec, 0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19,
    0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

struct Spec {
    name: &'static str,
    seed: [u8; 32],
    mldsa: bool,
    account_nonce: u64,
    uniquifier_nonce: u64,
    network_name: &'static str,
    rnd: Option<[u8; 32]>,
    resign_of: Option<&'static str>,
}

pub fn cases() -> Vec<SigningCase> {
    let xi_up: [u8; 32] = core::array::from_fn(|i| i as u8);
    let xi_down: [u8; 32] = core::array::from_fn(|i| 0xff - i as u8);
    let specs = [
        Spec {
            name: "ed25519_rfc8032_test1",
            seed: ED25519_RFC8032_TEST1,
            mldsa: false,
            account_nonce: 0,
            uniquifier_nonce: 1,
            network_name: "test",
            rnd: None,
            resign_of: None,
        },
        Spec {
            name: "ed25519_seed_11",
            seed: [0x11; 32],
            mldsa: false,
            account_nonce: 7,
            uniquifier_nonce: 2,
            network_name: "devnode",
            rnd: None,
            resign_of: None,
        },
        Spec {
            name: "mldsa65_seed_00_1f",
            seed: xi_up,
            mldsa: true,
            account_nonce: 0,
            uniquifier_nonce: 3,
            network_name: "test",
            rnd: None,
            resign_of: None,
        },
        Spec {
            name: "mldsa65_seed_ff_e0",
            seed: xi_down,
            mldsa: true,
            account_nonce: u64::MAX,
            uniquifier_nonce: 4,
            network_name: "devnode",
            rnd: None,
            resign_of: None,
        },
        // The transaction of mldsa65_seed_00_1f, signed again with a
        // non-zero rnd: a different valid signature, the same hash.
        Spec {
            name: "mldsa65_seed_00_1f_resigned",
            seed: xi_up,
            mldsa: true,
            account_nonce: 0,
            uniquifier_nonce: 3,
            network_name: "test",
            rnd: Some([0x01; 32]),
            resign_of: Some("mldsa65_seed_00_1f"),
        },
    ];
    specs
        .into_iter()
        .map(|s| {
            let key = if s.mldsa {
                TestKey::mldsa65(s.seed)
            } else {
                TestKey::ed25519(s.seed)
            };
            let network_hash = match s.network_name {
                "test" => test_network_hash(),
                _ => devnode_network_hash(),
            };
            let tx = pubkey_account_transaction(
                &key,
                s.account_nonce,
                s.uniquifier_nonce,
            );
            let signed = sign_transaction(
                &key,
                tx,
                &network_hash,
                s.rnd.unwrap_or([0; 32]),
            );
            SigningCase {
                name: s.name,
                seed: s.seed,
                key,
                account_nonce: s.account_nonce,
                network_name: s.network_name,
                network_hash,
                rnd: s.rnd,
                resign_of: s.resign_of,
                signed,
            }
        })
        .collect()
}

fn signature_bytes(sig: &Signature) -> &[u8] {
    match sig {
        Signature::Ed25519 { ed25519 } => ed25519.as_ref(),
        Signature::MlDsa65 { mldsa65 } => mldsa65.as_ref().as_ref(),
        _ => unreachable!("no vectors for this scheme"),
    }
}

fn key_bytes(key: &PublicKey) -> &[u8] {
    match key {
        PublicKey::Ed25519 { ed25519 } => ed25519.as_ref(),
        PublicKey::MlDsa65 { mldsa65 } => mldsa65.as_ref().as_ref(),
        _ => unreachable!("no vectors for this scheme"),
    }
}

fn address_bytes(address: &ContractAddress) -> &[u8] {
    address.as_ref()
}

pub fn generate() -> Value {
    let vectors: Vec<Value> = cases()
        .iter()
        .map(|c| {
            let tx = &c.signed.tx;
            let public_key = c.key.public_key();
            let msg = transaction_signature_preimage(&c.network_hash, tx).unwrap();
            json!({
                "name": c.name,
                "scheme": c.key.scheme_name(),
                "seed_hex": hex(&c.seed),
                "signature_rnd_hex": c.rnd.map(|r| hex(&r)),
                "resign_of": c.resign_of,
                "public_key_hex": hex(key_bytes(&public_key)),
                "public_key_cxs_hex": hex(&xdrs::cxs::encode(&public_key).unwrap()),
                "public_key_hash_hex": hex(public_key_hash(&public_key).as_ref()),
                "account_nonce": c.account_nonce.to_string(),
                "address_hex": hex(address_bytes(&tx.invocation.invoked_address)),
                "network": c.network_name,
                "network_hash_hex": hex(c.network_hash.as_slice()),
                "transaction": {
                    "invoked_address_hex": hex(address_bytes(&tx.invocation.invoked_address)),
                    "entrypoint_hex": hex(tx.invocation.entrypoint.as_ref()),
                    "gas_limit": tx.invocation.gas_limit.to_string(),
                    "calldata_hex": hex(&tx.invocation.calldata.0),
                    "block_expiry_height": (*tx.uniquifier.block_expiry_height).to_string(),
                    "uniquifier_nonce": tx.uniquifier.nonce.to_string(),
                },
                "transaction_xdr_hex": hex(&xdrs::txs::encode(tx).unwrap()),
                "signing_preimage_hex": hex(&msg),
                "transaction_hash_hex": hex(&transaction_hash(tx)),
                "signature_hex": hex(signature_bytes(&c.signed.signatures.0[0].signature)),
                "signed_transaction_xdr_hex": hex(&xdrs::txs::encode(&c.signed).unwrap()),
            })
        })
        .collect();

    json!({
        "version": 1,
        "category": "pubkey_auth",
        "transaction_domain_separator_cxs_hex": hex(TRANSACTION.as_bytes()),
        "pubkey_claim_key_id": PUBKEY_CLAIM_KEY_ID,
        "vectors": vectors,
    })
}

pub fn print() {
    println!("{}", serde_json::to_string_pretty(&generate()).unwrap());
}
