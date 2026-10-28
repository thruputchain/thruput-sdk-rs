/// Roundtrip test: prove our Rust `pubkey_account_address` derivation agrees
/// with the C++ validator's `pubkey_auth_address`.
///
/// The C++ host (test-ffi 0.20.1) independently recomputes the address from
/// the auth claim in the transaction and compares it against
/// `tx.invocation.invoked_address`.  If our derivation disagrees by even one
/// byte, the auth check fails.  So a green `pubkey_auth_address_roundtrip`
/// test is a genuine cross-implementation equality check on the derivation.
///
/// `pubkey_auth_address_wrong_address_fails` is the vacuity guard: it proves
/// the positive test would actually catch a wrong address rather than always
/// returning success.
use ed25519_dalek::{Signer, SigningKey};
use thruput_domain_separator::sha2::{
    pubkey_account_address, public_key_hash,
};
use thruput_domain_separator::transaction_signature_preimage;
use thruput_native_exec::test_environment::{
    TestEnvironment, test_network_hash,
};
use thruput_xdr::auth_claim::{
    AuthClaim, AuthVerifyErrorCode, PubkeyAccountPreimage,
};
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::transaction::{
    DecoratedSignature, Invocation, SignedTransaction, Transaction, Uniquifier,
};
use thruput_xdr::types::{
    BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
    PublicKey, Signature as XdrSignature,
};
use xdrs::{BytesM, VecM};

/// Build a `SignedTransaction` whose `invoked_address` is `address`.
///
/// The transaction carries a `kPubkeySignature` claim for `preimage`, attaches
/// `signing_key`'s public key as the claim key, and is signed with
/// `signing_key` over the canonical signature preimage for
/// [`test_network_hash`].
fn make_signed_tx(
    signing_key: &SigningKey,
    preimage: PubkeyAccountPreimage,
    invoked_address: ContractAddress,
) -> SignedTransaction {
    let tx = Transaction {
        invocation: Invocation {
            invoked_address,
            entrypoint: ContractEntrypointID::default(),
            gas_limit: 100_000,
            calldata: BytesM(vec![]),
        },
        uniquifier: Uniquifier {
            block_expiry_height: BlockHeight::new(0),
            nonce: 0,
        },
        contracts_to_deploy: VecM(vec![]),
        auth_claim: AuthClaim::kPubkeySignature {
            pubkey_preimage: preimage,
        },
    };

    // Sign the domain-separated, network-bound preimage (ed25519 hashes
    // internally, so no pre-hashing).  This must match the C++ reference,
    // the node's transaction signature preimage.
    let preimage = transaction_signature_preimage(&test_network_hash(), &tx)
        .expect("failed to encode signature preimage");
    let sig = signing_key.sign(&preimage);

    let decorated_sig = DecoratedSignature {
        public_key_id: 0,
        signature: XdrSignature::Ed25519 {
            ed25519: Ed25519Signature::from(sig.to_bytes()),
        },
    };

    SignedTransaction {
        tx,
        signatures: VecM(vec![decorated_sig]),
        pubkey_claim_key: Some(public_key(signing_key)),
    }
}

fn public_key(signing_key: &SigningKey) -> PublicKey {
    PublicKey::Ed25519 {
        ed25519: Ed25519PublicKey::from(
            signing_key.verifying_key().to_bytes(),
        ),
    }
}

/// The account preimage for `signing_key`'s key at nonce 42.
fn account_preimage(signing_key: &SigningKey) -> PubkeyAccountPreimage {
    PubkeyAccountPreimage {
        pubkey_hash: public_key_hash(&public_key(signing_key)),
        nonce: 42,
    }
}

/// Positive roundtrip: our Rust derivation must agree with the C++ host.
///
/// The C++ host checks `pubkey_auth_address(claim.preimage) == tx.invoked_address`.
/// We set `tx.invoked_address` from `pubkey_account_address(&preimage)`.  If the
/// two implementations disagree, auth fails and the assertion below trips.
#[test]
fn pubkey_auth_address_roundtrip() {
    let signing_key = SigningKey::from_bytes(&[0xABu8; 32]);
    let preimage = account_preimage(&signing_key);

    let address = pubkey_account_address(&preimage);
    let stx = make_signed_tx(&signing_key, preimage, address);

    // None = no registered auth data; the pubkey claim path in test-ffi 0.20.1
    // succeeds exactly when the derived address matches invoked_address.
    let result = TestEnvironment::verify_signed_transaction_auth_static(
        &stx,
        None,
        &test_network_hash(),
    );

    assert_eq!(
        result,
        AuthVerifyErrorCode(0),
        "pubkey auth roundtrip FAILED: C++ host rejected our derived address \
         (auth_error={result:?}). This means Rust pubkey_account_address disagrees \
         with C++ pubkey_auth_address — investigate the derivation, not the assertion."
    );
}

/// Vacuity guard: a WRONG address must make verification fail.
///
/// This proves `pubkey_auth_address_roundtrip` is not a vacuous pass.  If a
/// corrupted address still passes, the test machinery is broken — that is
/// reported as a loud failure here rather than a silent false-positive above.
#[test]
fn pubkey_auth_address_wrong_address_fails() {
    let signing_key = SigningKey::from_bytes(&[0xCDu8; 32]);
    let preimage = account_preimage(&signing_key);

    // Derive the correct address, then flip its first byte to produce a wrong one.
    let correct = pubkey_account_address(&preimage);
    let mut bad_bytes: [u8; 32] = *correct;
    bad_bytes[0] ^= 0xFF;
    let bad_address = ContractAddress::from(bad_bytes);

    let stx = make_signed_tx(&signing_key, preimage, bad_address);
    let result = TestEnvironment::verify_signed_transaction_auth_static(
        &stx,
        None,
        &test_network_hash(),
    );

    assert_ne!(
        result,
        AuthVerifyErrorCode(0),
        "VACUITY FAILURE: a corrupted invoked_address passed auth verification — \
         the positive test proves nothing. Investigate the C++ host logic."
    );
}
