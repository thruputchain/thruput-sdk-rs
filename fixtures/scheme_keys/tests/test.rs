use ml_dsa::{B32, Keypair, MlDsa65};
use scheme_keys::SchemeKeysClient;
use std::sync::{Arc, Mutex};
use thruput_sdk::event::EmittedEvent;
use thruput_sdk::internal::decode_xdr;
use thruput_sdk::*;

static NONCE: SHA256Hash = thruput_sdk::sha256_static!("SCHEME_KEYS_NONCE");

/// Calls into the contract come from here: a contract cannot be invoked from
/// its own frame.
fn caller() -> ContractAddress {
    ContractAddress::named("scheme_keys_caller")
}

const MESSAGE: &[u8] = b"scheme keys example message";

struct Key {
    scheme: CryptographicScheme,
    public: PublicKey,
    signature: Signature,
}

fn ed25519(seed: u8) -> Key {
    use ed25519_dalek::Signer;
    let key = ed25519_dalek::SigningKey::from_bytes(&[seed; 32]);
    Key {
        scheme: CryptographicScheme::Ed25519,
        public: PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from(key.verifying_key().to_bytes()),
        },
        signature: Signature::Ed25519 {
            ed25519: Ed25519Signature::from(key.sign(MESSAGE).to_bytes()),
        },
    }
}

/// Pure ML-DSA-65 with an empty context, as the `VerifySignature` syscall
/// checks it.
fn mldsa65(seed: u8) -> Key {
    let key = ml_dsa::SigningKey::<MlDsa65>::from_seed(&B32::from([seed; 32]));
    let public =
        <[u8; 1952]>::try_from(key.verifying_key().encode().as_slice())
            .unwrap();
    let signature = key
        .expanded_key()
        .sign_internal(&[&[0, 0], MESSAGE], &B32::from([0u8; 32]));
    let signature =
        <[u8; 3309]>::try_from(signature.encode().as_slice()).unwrap();
    Key {
        scheme: CryptographicScheme::MlDsa65,
        public: PublicKey::MlDsa65 {
            mldsa65: MlDsa65PublicKey::from(public).into(),
        },
        signature: Signature::MlDsa65 {
            mldsa65: MlDsa65Signature::from(signature).into(),
        },
    }
}

fn setup() -> (TestEnvironmentHandle, ContractAddress) {
    // An ML-DSA-65 key is 1953 bytes encoded, over the default 1024-byte
    // limit on one storage value.
    let env = TestEnvironment::builder()
        .no_cost()
        .with_max_bytestring_len(4096)
        .build();
    let hash = *scheme_keys::register_mock_contract(&env).as_hash();
    // `Arc<Mutex<_>>` rather than a `Cell`: the body must be `UnwindSafe`.
    let deployed = Arc::new(Mutex::new(None));
    let slot = deployed.clone();
    env.transact(|tx| {
        tx.call(&caller()).run(move || {
            *slot.lock().unwrap() = Some(deploy_contract(&NONCE, &hash));
        })
    })
    .assert_success();
    // A transaction sees state as of the last closed block.
    env.advance_block();
    let contract = deployed.lock().unwrap().take().unwrap();
    (env, contract)
}

/// Store `key`, check the event it emits, and read its id back.
fn store_and_read(
    env: &TestEnvironmentHandle,
    contract: &ContractAddress,
    key: &Key,
) -> SHA256Hash {
    let public = key.public.clone();
    let result = env.transact(|tx| {
        tx.call(&caller())
            .run(|| SchemeKeysClient::new(contract).set_key(public))
    });
    result.assert_success();
    result.assert_event_count(1);

    let event = EmittedEvent::from(&result.events()[0]);
    let key_id = SHA256Hash::from(*event.topic(1).unwrap());
    let scheme: i32 = decode_xdr(event.payload().unwrap()).unwrap();
    assert_eq!(scheme, key.scheme.0);
    env.advance_block();

    let public = key.public.clone();
    env.transact(|tx| {
        tx.call(&caller()).run(|| {
            assert_eq!(key_id, scheme_keys::key_id(&public));
            assert_eq!(SchemeKeysClient::new(contract).key_id(), key_id);
        })
    })
    .assert_success();
    key_id
}

#[test]
fn round_trips_both_schemes() {
    let (env, contract) = setup();
    let classic = ed25519(1);
    let pq = mldsa65(2);

    let classic_id = store_and_read(&env, &contract, &classic);
    let pq_id = store_and_read(&env, &contract, &pq);
    assert_ne!(classic_id, pq_id, "the key id commits to the scheme");
}

#[test]
fn verifies_against_the_stored_key_of_either_scheme() {
    let (env, contract) = setup();
    let classic = ed25519(3);
    let pq = mldsa65(4);

    for (stored, other) in [(&classic, &pq), (&pq, &classic)] {
        store_and_read(&env, &contract, stored);
        let (good, wrong_scheme) =
            (stored.signature.clone(), other.signature.clone());
        env.transact(|tx| {
            tx.call(&caller()).run(|| {
                let client = SchemeKeysClient::new(&contract);
                assert!(client.verify(good.clone(), MESSAGE.to_vec()));
                assert!(!client.verify(good, b"other".to_vec()));
                assert!(!client.verify(wrong_scheme, MESSAGE.to_vec()));
            })
        })
        .assert_success();
    }
}
