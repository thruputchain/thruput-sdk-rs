use sdk::*;
use sha2::{Digest, Sha256};

contract_test!(test_pubkey_address {
    use crate::types::*;
    use groundhog_xdr::auth_claim::*;

    let ed_pubkey: Ed25519PublicKey = [17; 32].into();
    let pubkey = PublicKey::Ed25519 { ed25519: ed_pubkey };
    let pubkey_preimage = PubkeyAccountPreimage {
        pubkey,
        nonce: 7,
    };

    // Derive with native sha256
    let preimage_encoded = xdrs::cxs::encode(&pubkey_preimage).unwrap();
    let hashed: [u8; 32] = Sha256::digest(preimage_encoded.as_slice()).into();
    let tag: [u8; 32] = Sha256::digest("PUBKEY_ACCOUNT".as_bytes()).into();
    let hashed: SHA256Hash = hashed.into();
    let tag: SHA256Hash = tag.into();
    let addr: SHA256Hash = tag ^ hashed;
    let expected_address: ContractAddress = addr.into();

    // Derive with SDK
    let claim = AuthClaim::kPubkeySignature { pubkey_preimage };
    let derived_address = auth_claim_pubkey_contract_address(&claim);

    assert_eq!(derived_address, Some(expected_address));
});
