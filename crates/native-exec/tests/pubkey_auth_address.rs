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
use groundhog_domain_separator::sha2::pubkey_account_address;
use groundhog_native_exec::test_environment::TestEnvironment;
use groundhog_xdr::auth_claim::{
    AuthClaim, AuthVerifyErrorCode, PubkeyAccountPreimage,
};
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::transaction::{
    DecoratedSignature, Invocation, SignedTransaction, Transaction, Uniquifier,
};
use groundhog_xdr::types::{
    BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
    PublicKey, Signature as XdrSignature,
};
use xdrs::{BytesM, VecM, txs};

/// Build a `SignedTransaction` whose `invoked_address` is `address`.
///
/// The transaction carries a `kPubkeySignature` claim for `preimage` and is
/// signed with `signing_key` over the plain XDR encoding of the `Transaction`
/// (matching the C++ `make_signature` helper: `ed25519_sign(sk, serialize(tx))`).
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

    // Sign the plain XDR-serialized Transaction (not CXS; ed25519 hashes
    // internally, so no pre-hashing).  This matches the C++ reference.
    let tx_bytes = txs::encode(&tx).expect("failed to encode transaction");
    let sig = signing_key.sign(&tx_bytes);

    let decorated_sig = DecoratedSignature {
        public_key_id: 0,
        signature: XdrSignature::Ed25519 {
            ed25519: Ed25519Signature::from(sig.to_bytes()),
        },
    };

    SignedTransaction {
        tx,
        signatures: VecM(vec![decorated_sig]),
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
    let pk_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();

    let preimage = PubkeyAccountPreimage {
        pubkey: PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from(pk_bytes),
        },
        nonce: 42,
    };

    let address = pubkey_account_address(&preimage);
    let stx = make_signed_tx(&signing_key, preimage, address);

    // None = no registered auth data; the pubkey claim path in test-ffi 0.20.1
    // succeeds exactly when the derived address matches invoked_address.
    let result =
        TestEnvironment::verify_signed_transaction_auth_static(&stx, None);

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
    let pk_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();

    let preimage = PubkeyAccountPreimage {
        pubkey: PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from(pk_bytes),
        },
        nonce: 42,
    };

    // Derive the correct address, then flip its first byte to produce a wrong one.
    let correct = pubkey_account_address(&preimage);
    let mut bad_bytes: [u8; 32] = *correct;
    bad_bytes[0] ^= 0xFF;
    let bad_address = ContractAddress::from(bad_bytes);

    let stx = make_signed_tx(&signing_key, preimage, bad_address);
    let result =
        TestEnvironment::verify_signed_transaction_auth_static(&stx, None);

    assert_ne!(
        result,
        AuthVerifyErrorCode(0),
        "VACUITY FAILURE: a corrupted invoked_address passed auth verification — \
         the positive test proves nothing. Investigate the C++ host logic."
    );
}
