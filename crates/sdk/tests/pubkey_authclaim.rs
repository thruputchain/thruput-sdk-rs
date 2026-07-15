use sdk::*;
use groundhog_xdr::auth_claim::*;

const TEST_ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

// Gives some confirmation that the SDK's derivation is as intended.
#[test]
fn pubkey_address_derivation_check() {
    let env = TestEnvironment::no_cost();
    let claim = example_pubkey_claim();
    let expected_address = native_derive_pubkey_contract_address(&claim);

    env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let sdk_derived_address = auth_claim_pubkey_contract_address(&claim);

            assert!(sdk_derived_address.is_some());
            assert_eq!(sdk_derived_address, expected_address);
        })
    });
}

// Verifies that an auth-claim can be attached to a mock transaction via
// `transact_with_auth_claim` and is observed by the contract under test.
#[test]
fn pubkey_auth_claim_is_visible_to_contract() {
    let env = TestEnvironment::no_cost();
    let default_claim = MockTransactionHandler::default_auth_claim();
    let claim = example_pubkey_claim();
    let expected_address = native_derive_pubkey_contract_address(&claim);

    // Check behavior with the default auth-claim
    let claim_for_default = claim.clone();
    env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            // Without an override, the default source-address claim is attached.
            let observed = sdk::get_auth_claim().unwrap();
            assert_eq!(observed, default_claim);
            assert_ne!(observed, claim_for_default);
            // `auth_claim_pubkey_contract_address` only derives an address
            // for pubkey claims.
            assert_eq!(auth_claim_pubkey_contract_address(&observed), None);
        })
    });

    // Check behavior with the supplied example pubkey auth-claim
    let claim_for_custom = claim.clone();
    env.transact_with_auth_claim(claim.clone(), |tx| {
        tx.call(&TEST_ADDR).run(|| {
            let observed = sdk::get_auth_claim().unwrap();
            assert_eq!(observed, claim_for_custom);

            let sdk_derived_address = auth_claim_pubkey_contract_address(&claim);
            assert!(sdk_derived_address.is_some());
            assert_eq!(sdk_derived_address, expected_address);
        })
    });
}

// Verify that we can `transact_with_auth_claim`, `tx.call` with the associated
// address, and then assert that the current address is that address.
#[test]
fn pubkey_auth_claim_maps_to_current_address() {
    let env = TestEnvironment::no_cost();
    let claim = example_pubkey_claim();
    let expected_address = native_derive_pubkey_contract_address(&claim)
        .expect("pubkey claim derives to some address");

    // The positive case
    let claim_for_custom = claim.clone();
    env.transact_with_auth_claim(claim.clone(), |tx| {
        tx.call(&expected_address).run(|| {
            let observed = sdk::get_auth_claim().unwrap();
            assert_eq!(observed, claim_for_custom);

            let sdk_derived_address = auth_claim_pubkey_contract_address(&claim)
                .expect("sdk derives some address from a pubkey claim");

            assert_eq!(sdk_derived_address, expected_address);
            assert_eq!(sdk_derived_address, get_current_address());
        })
    });

    // The negative case (running as TEST_ADDR)
    env.transact_with_auth_claim(claim.clone(), |tx| {
        tx.call(&TEST_ADDR).run(|| {
            let sdk_derived_address = auth_claim_pubkey_contract_address(&claim)
                .expect("sdk derives some address from a pubkey claim");

            assert_ne!(sdk_derived_address, get_current_address());
        })
    });
}

// Make a pubkey auth-claim
fn example_pubkey_claim() -> AuthClaim {
    let ed_pubkey: Ed25519PublicKey = [17; 32].into();
    let pubkey = PublicKey::Ed25519 { ed25519: ed_pubkey };
    let pubkey_preimage = PubkeyAccountPreimage {
        pubkey,
        nonce: 7,
    };
    AuthClaim::kPubkeySignature { pubkey_preimage }
}

// Derive the address with native sha256
fn native_derive_pubkey_contract_address(auth_claim: &AuthClaim) -> Option<ContractAddress> {
    use sha2::{Digest, Sha256};

    match auth_claim {
        AuthClaim::kPubkeySignature { pubkey_preimage } => {
            let preimage_encoded = xdrs::cxs::encode(&pubkey_preimage).unwrap();
            let hashed: [u8; 32] = Sha256::digest(preimage_encoded.as_slice()).into();
            let tag: [u8; 32] = Sha256::digest("PUBKEY_ACCOUNT".as_bytes()).into();
            let hashed: SHA256Hash = hashed.into();
            let tag: SHA256Hash = tag.into();
            let addr: SHA256Hash = tag ^ hashed;
            Some(addr.into())
        }
        _ => None
    }
}
