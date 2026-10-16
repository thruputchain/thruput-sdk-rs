//! The `pubkey-auth` golden vectors, checked against hogd's verifier.
//!
//! Every signed transaction `thruput-test-vectors pubkey-auth` emits goes
//! through the C++ host's auth check (test-ffi), which re-derives the key
//! hash, the account address and the signature preimage itself and verifies
//! the Ed25519 or ML-DSA-65 signature with hogd's own code. A green run means
//! the vectors other SDKs replay are transactions hogd accepts, not merely
//! ones the Rust SDK agrees with itself about.
//!
//! The rejection tests break one rule of a valid vector each and pin the
//! `AuthVerifyErrorCode` hogd answers with. They are also the vacuity guard:
//! they show the positive test would catch a wrong byte.

use thruput_domain_separator::sha2::{
    pubkey_account_address, public_key_hash,
};
use thruput_native_exec::test_environment::TestEnvironment;
use thruput_test_vectors::pubkey_auth_vectors::{
    PUBKEY_CLAIM_KEY_ID, SigningCase, TestKey, cases, devnode_network_hash,
    transaction_hash,
};
use thruput_xdr::address_auth::{
    AddressAuthData, PublicKeyConfig, PublicKeyConfigOpt,
};
use thruput_xdr::auth_claim::{
    AuthClaim, AuthClaim_source_address_claim, AuthVerifyErrorCode,
    kMaxSigners,
};
use thruput_xdr::transaction::{DecoratedSignature, SignedTransaction};
use thruput_xdr::types::{
    BlockHeight, MlDsa65Signature, NetworkHash, SHA256Hash, Signature,
};
use xdrs::VecM;

const OK: AuthVerifyErrorCode = AuthVerifyErrorCode(0);
const MALFORMED: AuthVerifyErrorCode = AuthVerifyErrorCode::kAuthMalformed;
const BAD_SIGNATURE: AuthVerifyErrorCode =
    AuthVerifyErrorCode::kSignatureVerificationFailed;

/// hogd's verdict on `stx` for an address with no registered auth data, the
/// case every pubkey account is in.
fn verify(
    stx: &SignedTransaction,
    network: &NetworkHash,
) -> AuthVerifyErrorCode {
    TestEnvironment::verify_signed_transaction_auth_static(stx, None, network)
}

fn case(name: &str) -> SigningCase {
    cases()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no vector named {name}"))
}

fn sign(
    key: &TestKey,
    stx: &SignedTransaction,
    network: &NetworkHash,
) -> Vec<DecoratedSignature> {
    let msg = thruput_domain_separator::transaction_signature_preimage(
        network, &stx.tx,
    )
    .unwrap();
    vec![DecoratedSignature {
        public_key_id: PUBKEY_CLAIM_KEY_ID,
        signature: key.sign(&msg),
    }]
}

/// Asserts hogd's verdict, naming the vector and the mutation on failure.
fn expect(
    what: &str,
    stx: &SignedTransaction,
    network: &NetworkHash,
    want: AuthVerifyErrorCode,
) {
    let got = verify(stx, network);
    assert_eq!(
        got, want,
        "{what}: hogd answered {got:?}, expected {want:?}"
    );
}

#[test]
fn every_vector_verifies() {
    for c in cases() {
        expect(c.name, &c.signed, &c.network_hash, OK);
    }
}

/// The re-signed vector carries a different signature over the same
/// transaction, so the transaction hash, which covers only the unsigned
/// transaction, is the same.
#[test]
fn resigning_keeps_the_transaction_hash() {
    let all = cases();
    for c in all.iter().filter(|c| c.resign_of.is_some()) {
        let base = all
            .iter()
            .find(|b| Some(b.name) == c.resign_of)
            .expect("re-signed vector without its base");
        assert_ne!(c.signed.signatures, base.signed.signatures, "{}", c.name);
        assert_eq!(
            transaction_hash(&c.signed.tx),
            transaction_hash(&base.signed.tx),
            "{}",
            c.name
        );
        expect(c.name, &c.signed, &c.network_hash, OK);
    }
}

/// Mutations that apply to both schemes. `other` is another key of the same
/// scheme.
fn reject_mutations(name: &str, other: &str) {
    let c = case(name);
    let other = case(other).key;
    let net = &c.network_hash;
    let what = |m: &str| format!("{name}: {m}");

    let mut s = c.signed.clone();
    s.pubkey_claim_key = None;
    expect(&what("missing pubkey_claim_key"), &s, net, MALFORMED);

    let mut s = c.signed.clone();
    s.pubkey_claim_key = Some(other.public_key());
    expect(&what("pubkey_claim_key of another key"), &s, net, MALFORMED);

    let mut s = c.signed.clone();
    let AuthClaim::kPubkeySignature { pubkey_preimage } = &mut s.tx.auth_claim
    else {
        unreachable!()
    };
    pubkey_preimage.nonce = pubkey_preimage.nonce.wrapping_add(1);
    let moved_address = pubkey_account_address(pubkey_preimage);
    expect(
        &what("claim nonce changed after signing"),
        &s,
        net,
        MALFORMED,
    );

    // The same change made consistently: the claim and the invoked address
    // agree again, but the signature covers the original transaction.
    s.tx.invocation.invoked_address = moved_address;
    expect(
        &what("claim nonce and address changed"),
        &s,
        net,
        BAD_SIGNATURE,
    );

    let mut s = c.signed.clone();
    let wrong = if *net == devnode_network_hash() {
        NetworkHash::new(SHA256Hash::new([0x01; 32]))
    } else {
        devnode_network_hash()
    };
    s.signatures = VecM(sign(&c.key, &s, &wrong));
    expect(&what("signed for another network"), &s, net, BAD_SIGNATURE);

    let mut s = c.signed.clone();
    s.signatures.0.push(s.signatures.0[0].clone());
    expect(&what("two signatures"), &s, net, MALFORMED);

    let mut s = c.signed.clone();
    s.signatures.0[0].public_key_id = 1;
    expect(&what("key id 1"), &s, net, MALFORMED);

    let mut s = c.signed.clone();
    match &mut s.signatures.0[0].signature {
        Signature::Ed25519 { ed25519 } => ed25519[10] ^= 1,
        Signature::MlDsa65 { mldsa65 } => mldsa65[10] ^= 1,
        _ => unreachable!(),
    }
    expect(&what("one signature bit flipped"), &s, net, BAD_SIGNATURE);
}

#[test]
fn ed25519_rejections() {
    reject_mutations("ed25519_rfc8032_test1", "ed25519_seed_11");
    reject_mutations("ed25519_seed_11", "ed25519_rfc8032_test1");
}

#[test]
fn mldsa65_rejections() {
    reject_mutations("mldsa65_seed_00_1f", "mldsa65_seed_ff_e0");
    reject_mutations("mldsa65_seed_ff_e0", "mldsa65_seed_00_1f");
}

/// `pubkey_claim_key` belongs to `kPubkeySignature` claims only. Here the
/// address has auth data registering the signing key, so the claim is
/// otherwise valid (the control shows hogd accepts it without the key), and
/// attaching the key is what hogd rejects.
#[test]
fn pubkey_claim_key_on_source_address_claim() {
    for name in ["ed25519_rfc8032_test1", "mldsa65_seed_00_1f"] {
        let c = case(name);
        let net = &c.network_hash;
        let mut public_keys: [PublicKeyConfigOpt; kMaxSigners as usize] =
            core::array::from_fn(|_| PublicKeyConfigOpt::new(None));
        public_keys[0] = PublicKeyConfigOpt::new(Some(PublicKeyConfig {
            key: c.key.public_key(),
            max_weight: 1,
        }));
        let auth_data = AddressAuthData {
            public_keys,
            required_weight: 1,
            last_written_height: BlockHeight::new(0),
        };

        let mut s = c.signed.clone();
        s.tx.auth_claim = AuthClaim::kSourceAddressAuthData {
            source_address_claim: AuthClaim_source_address_claim {
                allowed_keys: 1,
                required_weight: 1,
            },
        };
        s.pubkey_claim_key = None;
        s.signatures = VecM(sign(&c.key, &s, net));
        let verdict = |s: &SignedTransaction| {
            TestEnvironment::verify_signed_transaction_auth_static(
                s,
                Some(&auth_data),
                net,
            )
        };
        assert_eq!(verdict(&s), OK, "{name}: source-address control");

        s.pubkey_claim_key = Some(c.key.public_key());
        assert_eq!(
            verdict(&s),
            MALFORMED,
            "{name}: pubkey_claim_key on a source-address claim"
        );
    }
}

#[test]
fn mldsa65_signature_rejections() {
    let c = case("mldsa65_seed_00_1f");
    let net = &c.network_hash;
    let msg = thruput_domain_separator::transaction_signature_preimage(
        net,
        &c.signed.tx,
    )
    .unwrap();

    // Pure ML-DSA with a non-empty context string signs different bytes.
    let mut s = c.signed.clone();
    s.signatures.0[0].signature = c.key.sign_with(&msg, b"thruput", [0; 32]);
    expect(
        "ML-DSA-65 signature with a context string",
        &s,
        net,
        BAD_SIGNATURE,
    );

    // Not a well-formed signature encoding at all.
    let mut s = c.signed.clone();
    s.signatures.0[0].signature = Signature::MlDsa65 {
        mldsa65: MlDsa65Signature::from([0xff; 3309]).into(),
    };
    expect("all-0xff ML-DSA-65 signature", &s, net, BAD_SIGNATURE);
}

/// A signature of one scheme for a key of the other is malformed: hogd
/// rejects it before any signature check.
#[test]
fn scheme_mismatch_rejections() {
    let ed = case("ed25519_rfc8032_test1");
    let ml = case("mldsa65_seed_00_1f");

    let mut s = ml.signed.clone();
    s.signatures = VecM(sign(&ed.key, &s, &ml.network_hash));
    let got = verify(&s, &ml.network_hash);
    assert_eq!(got, MALFORMED, "Ed25519 signature for an ML-DSA-65 key");

    let mut s = ed.signed.clone();
    s.signatures = VecM(sign(&ml.key, &s, &ed.network_hash));
    let got = verify(&s, &ed.network_hash);
    assert_eq!(got, MALFORMED, "ML-DSA-65 signature for an Ed25519 key");

    // The claim names the Ed25519 key; the attached, signing key is the
    // ML-DSA-65 one. The hash check rejects it first.
    let mut s = ed.signed.clone();
    s.pubkey_claim_key = Some(ml.key.public_key());
    s.signatures = VecM(sign(&ml.key, &s, &ed.network_hash));
    expect(
        "ML-DSA-65 key for an Ed25519 claim",
        &s,
        &ed.network_hash,
        MALFORMED,
    );
    assert_ne!(
        public_key_hash(&ml.key.public_key()),
        public_key_hash(&ed.key.public_key())
    );
}
