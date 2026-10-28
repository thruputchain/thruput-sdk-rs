//! ML-DSA-65 alongside Ed25519: registering keys in an address's auth data,
//! and verifying signatures through the `VerifySignature` syscall.

use ml_dsa::{B32, Keypair, MlDsa65};
use thruput_sdk::crypto::{verify_signature, verify_signature_ed25519};
use thruput_sdk::*;
use thruput_xdr::address_auth::PublicKeyConfig;

fn ed25519_key(seed: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
}

fn mldsa65_key(seed: u8) -> ml_dsa::SigningKey<MlDsa65> {
    ml_dsa::SigningKey::<MlDsa65>::from_seed(&B32::from([seed; 32]))
}

fn ed25519_public(key: &ed25519_dalek::SigningKey) -> Ed25519PublicKey {
    Ed25519PublicKey::from(key.verifying_key().to_bytes())
}

fn ed25519_sign(
    key: &ed25519_dalek::SigningKey,
    msg: &[u8],
) -> Ed25519Signature {
    use ed25519_dalek::Signer;
    Ed25519Signature::from(key.sign(msg).to_bytes())
}

fn mldsa65_public(key: &ml_dsa::SigningKey<MlDsa65>) -> MlDsa65PublicKey {
    MlDsa65PublicKey::from(
        <[u8; 1952]>::try_from(key.verifying_key().encode().as_slice())
            .unwrap(),
    )
}

/// Pure ML-DSA-65 with an empty context: the message is prefixed with
/// `0 || |ctx|`, which is what hogd verifies.
fn mldsa65_sign(
    key: &ml_dsa::SigningKey<MlDsa65>,
    msg: &[u8],
) -> MlDsa65Signature {
    let sig = key
        .expanded_key()
        .sign_internal(&[&[0, 0], msg], &B32::from([0u8; 32]));
    MlDsa65Signature::from(
        <[u8; 3309]>::try_from(sig.encode().as_slice()).unwrap(),
    )
}

fn mldsa65_public_key(key: &ml_dsa::SigningKey<MlDsa65>) -> PublicKey {
    PublicKey::MlDsa65 {
        mldsa65: mldsa65_public(key).into(),
    }
}

fn ed25519_public_key(key: &ed25519_dalek::SigningKey) -> PublicKey {
    PublicKey::Ed25519 {
        ed25519: ed25519_public(key),
    }
}

const MSG: &[u8] = b"signature scheme test message";

#[test]
fn registers_and_removes_mldsa65_and_ed25519_keys() {
    let env = TestEnvironment::no_cost();
    let addr = ContractAddress::named("registered_keys");
    let pq = mldsa65_public_key(&mldsa65_key(7));
    let classic = ed25519_public_key(&ed25519_key(9));

    {
        let (pq, classic) = (pq.clone(), classic.clone());
        env.transact(|tx| {
            tx.call(&addr).run(move || {
                add_address_public_key(0, pq.clone(), 3).unwrap();
                add_address_public_key(1, classic.clone(), 1).unwrap();
                set_address_required_weight(2).unwrap();
            })
        })
        .assert_success();
    }
    // The snapshot is the last closed block's state.
    env.advance_block();

    let auth = env
        .get_snapshot_auth_data(&addr)
        .expect("registration writes auth data");
    assert_eq!(
        *auth.public_keys[0],
        Some(PublicKeyConfig {
            key: pq.clone(),
            max_weight: 3,
        })
    );
    assert_eq!(
        *auth.public_keys[1],
        Some(PublicKeyConfig {
            key: classic.clone(),
            max_weight: 1,
        })
    );
    assert_eq!(auth.required_weight, 2);

    env.transact(|tx| {
        tx.call(&addr).run(|| remove_address_public_key(0).unwrap())
    })
    .assert_success();
    env.advance_block();

    let auth = env.get_snapshot_auth_data(&addr).unwrap();
    assert_eq!(*auth.public_keys[0], None);
    assert_eq!(
        *auth.public_keys[1],
        Some(PublicKeyConfig {
            key: classic,
            max_weight: 1,
        })
    );
}

#[test]
fn verifies_mldsa65_signatures() {
    let env = TestEnvironment::no_cost();
    let key = mldsa65_key(3);
    let pk = mldsa65_public_key(&key);
    let raw = mldsa65_sign(&key, MSG);
    let other_pk = mldsa65_public_key(&mldsa65_key(4));
    let mut tampered = raw.into_inner();
    tampered[10] ^= 1;
    let sig = Signature::MlDsa65 {
        mldsa65: raw.into(),
    };
    let tampered = Signature::MlDsa65 {
        mldsa65: MlDsa65Signature::from(tampered).into(),
    };

    env.transact(|tx| {
        tx.call(&ContractAddress::named("mldsa65_verify"))
            .run(move || {
                assert!(verify_signature(&pk, &sig, MSG));
                assert!(!verify_signature(&pk, &tampered, MSG));
                assert!(!verify_signature(&pk, &sig, b"another message"));
                assert!(!verify_signature(&other_pk, &sig, MSG));
            })
    })
    .assert_success();
}

#[test]
fn verify_signature_uses_the_key_scheme() {
    let env = TestEnvironment::no_cost();
    let ed_key = ed25519_key(5);
    let pq_key = mldsa65_key(6);
    let ed_pk = ed25519_public(&ed_key);
    let ed_sig = ed25519_sign(&ed_key, MSG);
    let ed_pub = ed25519_public_key(&ed_key);
    let ed_signature = Signature::Ed25519 { ed25519: ed_sig };
    let pq_pub = mldsa65_public_key(&pq_key);
    let pq_signature = Signature::MlDsa65 {
        mldsa65: mldsa65_sign(&pq_key, MSG).into(),
    };

    env.transact(|tx| {
        tx.call(&ContractAddress::named("verify_dispatch"))
            .run(move || {
                // Ed25519 is unchanged, called directly or through dispatch.
                assert!(verify_signature_ed25519(&ed_pk, &ed_sig, MSG));
                assert!(!verify_signature_ed25519(&ed_pk, &ed_sig, b"other"));
                assert!(verify_signature(&ed_pub, &ed_signature, MSG));
                assert!(verify_signature(&pq_pub, &pq_signature, MSG));
                assert!(!verify_signature(&pq_pub, &pq_signature, b"other"));
                // A signature of the other scheme never verifies.
                assert!(!verify_signature(&ed_pub, &pq_signature, MSG));
                assert!(!verify_signature(&pq_pub, &ed_signature, MSG));
            })
    })
    .assert_success();
}
