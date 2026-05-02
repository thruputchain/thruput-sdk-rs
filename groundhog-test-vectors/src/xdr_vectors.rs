use groundhog_xdr::auth_claim::AuthClaim;
use groundhog_xdr::auth_claim::AuthClaim_source_address_claim;
use serde_json::{Value, json};
use xdrs::WriteXDR;
use xdrs::txs;
use xdrs::{BytesM, StringM, VecM};

use groundhog_xdr::address_auth::PublicKeyConfig;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::transaction::{Invocation, Transaction, Uniquifier};
use groundhog_xdr::types::{
    ContractAddress, Ed25519PublicKey, PublicKey, SHA256Hash, StorageKeyType,
};

/// Encode a value to XDR and return lowercase hex string.
fn xdr_hex<T: WriteXDR>(value: &T) -> String {
    let bytes = txs::encode(value).expect("XDR encoding failed");
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn primitive_vectors() -> Vec<Value> {
    vec![
        // u32
        json!({ "name": "u32_zero", "type": "u32", "value": "0", "expected_hex": xdr_hex(&0u32) }),
        json!({ "name": "u32_one", "type": "u32", "value": "1", "expected_hex": xdr_hex(&1u32) }),
        json!({ "name": "u32_max", "type": "u32", "value": u32::MAX.to_string(), "expected_hex": xdr_hex(&u32::MAX) }),
        json!({ "name": "u32_mid", "type": "u32", "value": "12345", "expected_hex": xdr_hex(&12345u32) }),
        // i32
        json!({ "name": "i32_zero", "type": "i32", "value": "0", "expected_hex": xdr_hex(&0i32) }),
        json!({ "name": "i32_positive", "type": "i32", "value": "42", "expected_hex": xdr_hex(&42i32) }),
        json!({ "name": "i32_negative", "type": "i32", "value": "-1", "expected_hex": xdr_hex(&(-1i32)) }),
        json!({ "name": "i32_min", "type": "i32", "value": i32::MIN.to_string(), "expected_hex": xdr_hex(&i32::MIN) }),
        json!({ "name": "i32_max", "type": "i32", "value": i32::MAX.to_string(), "expected_hex": xdr_hex(&i32::MAX) }),
        // u64
        json!({ "name": "u64_zero", "type": "u64", "value": "0", "expected_hex": xdr_hex(&0u64) }),
        json!({ "name": "u64_max", "type": "u64", "value": u64::MAX.to_string(), "expected_hex": xdr_hex(&u64::MAX) }),
        json!({ "name": "u64_mid", "type": "u64", "value": "1000000000000", "expected_hex": xdr_hex(&1_000_000_000_000u64) }),
        // i64
        json!({ "name": "i64_zero", "type": "i64", "value": "0", "expected_hex": xdr_hex(&0i64) }),
        json!({ "name": "i64_negative", "type": "i64", "value": "-1", "expected_hex": xdr_hex(&(-1i64)) }),
        json!({ "name": "i64_min", "type": "i64", "value": i64::MIN.to_string(), "expected_hex": xdr_hex(&i64::MIN) }),
        json!({ "name": "i64_max", "type": "i64", "value": i64::MAX.to_string(), "expected_hex": xdr_hex(&i64::MAX) }),
        // u128
        json!({ "name": "u128_zero", "type": "u128", "value": "0", "expected_hex": xdr_hex(&0u128) }),
        json!({ "name": "u128_max", "type": "u128", "value": u128::MAX.to_string(), "expected_hex": xdr_hex(&u128::MAX) }),
        json!({ "name": "u128_large", "type": "u128", "value": "340282366920938463463374607431768211454", "expected_hex": xdr_hex(&(u128::MAX - 1)) }),
        // i128
        json!({ "name": "i128_zero", "type": "i128", "value": "0", "expected_hex": xdr_hex(&0i128) }),
        json!({ "name": "i128_negative", "type": "i128", "value": "-1", "expected_hex": xdr_hex(&(-1i128)) }),
        json!({ "name": "i128_min", "type": "i128", "value": i128::MIN.to_string(), "expected_hex": xdr_hex(&i128::MIN) }),
        json!({ "name": "i128_max", "type": "i128", "value": i128::MAX.to_string(), "expected_hex": xdr_hex(&i128::MAX) }),
        // bool
        json!({ "name": "bool_true", "type": "bool", "value": true, "expected_hex": xdr_hex(&true) }),
        json!({ "name": "bool_false", "type": "bool", "value": false, "expected_hex": xdr_hex(&false) }),
    ]
}

fn collection_vectors() -> Vec<Value> {
    vec![
        // BytesM — empty
        json!({
            "name": "bytes_empty",
            "type": { "bytes": "1024" },
            "value": "",
            "expected_hex": xdr_hex(&BytesM::<1024>::default()),
        }),
        // BytesM — 1 byte (exercises 3 bytes padding)
        json!({
            "name": "bytes_1",
            "type": { "bytes": "1024" },
            "value": "ab",
            "expected_hex": xdr_hex(&BytesM::<1024>::try_from(vec![0xab]).unwrap()),
        }),
        // BytesM — 2 bytes (exercises 2 bytes padding)
        json!({
            "name": "bytes_2",
            "type": { "bytes": "1024" },
            "value": "abcd",
            "expected_hex": xdr_hex(&BytesM::<1024>::try_from(vec![0xab, 0xcd]).unwrap()),
        }),
        // BytesM — 3 bytes (exercises 1 byte padding)
        json!({
            "name": "bytes_3",
            "type": { "bytes": "1024" },
            "value": "abcdef",
            "expected_hex": xdr_hex(&BytesM::<1024>::try_from(vec![0xab, 0xcd, 0xef]).unwrap()),
        }),
        // BytesM — 4 bytes (no padding needed)
        json!({
            "name": "bytes_4",
            "type": { "bytes": "1024" },
            "value": "abcdef01",
            "expected_hex": xdr_hex(&BytesM::<1024>::try_from(vec![0xab, 0xcd, 0xef, 0x01]).unwrap()),
        }),
        // BytesM — 5 bytes (exercises 3 bytes padding again)
        json!({
            "name": "bytes_5",
            "type": { "bytes": "1024" },
            "value": "abcdef0123",
            "expected_hex": xdr_hex(&BytesM::<1024>::try_from(vec![0xab, 0xcd, 0xef, 0x01, 0x23]).unwrap()),
        }),
        // StringM — empty
        json!({
            "name": "string_empty",
            "type": { "string": "256" },
            "value": "",
            "expected_hex": xdr_hex(&StringM::<256>::default()),
        }),
        // StringM — short
        json!({
            "name": "string_hello",
            "type": { "string": "256" },
            "value": "hello",
            "expected_hex": xdr_hex(&StringM::<256>::try_from("hello".to_string()).unwrap()),
        }),
        // StringM — 4 bytes (no padding)
        json!({
            "name": "string_4chars",
            "type": { "string": "256" },
            "value": "abcd",
            "expected_hex": xdr_hex(&StringM::<256>::try_from("abcd".to_string()).unwrap()),
        }),
        // VecM<u32> — empty
        json!({
            "name": "vec_u32_empty",
            "type": { "vec": "u32" },
            "value": [],
            "expected_hex": xdr_hex(&VecM::<u32>::default()),
        }),
        // VecM<u32> — single element
        json!({
            "name": "vec_u32_single",
            "type": { "vec": "u32" },
            "value": ["42"],
            "expected_hex": xdr_hex(&VecM::<u32>::try_from(vec![42u32]).unwrap()),
        }),
        // VecM<u32> — multiple elements
        json!({
            "name": "vec_u32_three",
            "type": { "vec": "u32" },
            "value": ["1", "2", "3"],
            "expected_hex": xdr_hex(&VecM::<u32>::try_from(vec![1u32, 2, 3]).unwrap()),
        }),
        // VecM<u64> — to test different element sizes
        json!({
            "name": "vec_u64_two",
            "type": { "vec": "u64" },
            "value": ["100", "200"],
            "expected_hex": xdr_hex(&VecM::<u64>::try_from(vec![100u64, 200]).unwrap()),
        }),
        // Option<u32> — None
        json!({
            "name": "option_u32_none",
            "type": { "option": "u32" },
            "value": null,
            "expected_hex": xdr_hex(&Option::<u32>::None),
        }),
        // Option<u32> — Some
        json!({
            "name": "option_u32_some",
            "type": { "option": "u32" },
            "value": "42",
            "expected_hex": xdr_hex(&Some(42u32)),
        }),
        // Option<i64> — None
        json!({
            "name": "option_i64_none",
            "type": { "option": "i64" },
            "value": null,
            "expected_hex": xdr_hex(&Option::<i64>::None),
        }),
        // Option<i64> — Some negative
        json!({
            "name": "option_i64_some_neg",
            "type": { "option": "i64" },
            "value": "-999",
            "expected_hex": xdr_hex(&Some(-999i64)),
        }),
    ]
}

/// Hex-encode a byte slice.
fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn protocol_fixed_vectors() -> Vec<Value> {
    let zeros_32 = [0u8; 32];
    let ones_32 = [0xffu8; 32];
    let sample_32: [u8; 32] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba,
        0x98, 0x76, 0x54, 0x32, 0x10, 0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a,
        0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
    ];

    vec![
        // ContractAddress
        json!({
            "name": "contract_address_zeros",
            "type": "ContractAddress",
            "value": hex_bytes(&zeros_32),
            "expected_hex": xdr_hex(&ContractAddress::from(zeros_32)),
        }),
        json!({
            "name": "contract_address_ones",
            "type": "ContractAddress",
            "value": hex_bytes(&ones_32),
            "expected_hex": xdr_hex(&ContractAddress::from(ones_32)),
        }),
        json!({
            "name": "contract_address_sample",
            "type": "ContractAddress",
            "value": hex_bytes(&sample_32),
            "expected_hex": xdr_hex(&ContractAddress::from(sample_32)),
        }),
        // SHA256Hash
        json!({
            "name": "sha256_hash_zeros",
            "type": "SHA256Hash",
            "value": hex_bytes(&zeros_32),
            "expected_hex": xdr_hex(&SHA256Hash::from(zeros_32)),
        }),
        json!({
            "name": "sha256_hash_sample",
            "type": "SHA256Hash",
            "value": hex_bytes(&sample_32),
            "expected_hex": xdr_hex(&SHA256Hash::from(sample_32)),
        }),
        // Ed25519PublicKey
        json!({
            "name": "ed25519_pubkey_zeros",
            "type": "Ed25519PublicKey",
            "value": hex_bytes(&zeros_32),
            "expected_hex": xdr_hex(&Ed25519PublicKey::from(zeros_32)),
        }),
        json!({
            "name": "ed25519_pubkey_sample",
            "type": "Ed25519PublicKey",
            "value": hex_bytes(&sample_32),
            "expected_hex": xdr_hex(&Ed25519PublicKey::from(sample_32)),
        }),
        // ContractEntrypointID
        json!({
            "name": "entrypoint_id_zeros",
            "type": "ContractEntrypointID",
            "value": hex_bytes(&[0u8; 8]),
            "expected_hex": xdr_hex(&ContractEntrypointID::from([0u8; 8])),
        }),
        json!({
            "name": "entrypoint_id_sample",
            "type": "ContractEntrypointID",
            "value": hex_bytes(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]),
            "expected_hex": xdr_hex(&ContractEntrypointID::from([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef])),
        }),
    ]
}

fn protocol_enum_vectors() -> Vec<Value> {
    vec![
        json!({
            "name": "storage_key_type_contract_data",
            "type": "StorageKeyType",
            "value": { "variant": "ContractData" },
            "expected_hex": xdr_hex(&StorageKeyType::ContractData),
        }),
        json!({
            "name": "storage_key_type_free_native_token",
            "type": "StorageKeyType",
            "value": { "variant": "FreeNativeTokenBalance" },
            "expected_hex": xdr_hex(&StorageKeyType::FreeNativeTokenBalance),
        }),
        json!({
            "name": "storage_key_type_address_auth_data",
            "type": "StorageKeyType",
            "value": { "variant": "AuthData" },
            "expected_hex": xdr_hex(&StorageKeyType::AuthData),
        }),
        json!({
            "name": "storage_key_type_address_metadata",
            "type": "StorageKeyType",
            "value": { "variant": "Metadata" },
            "expected_hex": xdr_hex(&StorageKeyType::Metadata),
        }),
        json!({
            "name": "storage_key_type_replay_cache",
            "type": "StorageKeyType",
            "value": { "variant": "ReplayCache" },
            "expected_hex": xdr_hex(&StorageKeyType::ReplayCache),
        }),
    ]
}

fn protocol_struct_vectors() -> Vec<Value> {
    let sample_key = Ed25519PublicKey::from([
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba,
        0x98, 0x76, 0x54, 0x32, 0x10, 0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a,
        0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
    ]);
    let sample_address = ContractAddress::from([
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44,
        0x55, 0x66, 0x77, 0x88, 0x99, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    ]);
    let sample_entrypoint = ContractEntrypointID::from([
        0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe,
    ]);

    // PublicKeyConfig
    let pubkey_config = PublicKeyConfig {
        key: PublicKey::Ed25519 {
            ed25519: sample_key,
        },
        max_weight: 10,
    };

    let auth_claim: AuthClaim = AuthClaim::kSourceAddressAuthData { 
        source_address_claim: AuthClaim_source_address_claim {
            allowed_keys: 0x0000_00FF,
            required_weight: 5,
        } 
    };

    // Transaction with minimal fields
    let tx = Transaction {
        invocation: Invocation {
            invoked_address: sample_address,
            entrypoint: sample_entrypoint,
            gas_limit: 1_000_000,
            calldata: Default::default(),
        },
        uniquifier: Uniquifier {
            block_expiry_height: 100.into(),
            nonce: 42,
        },
        contracts_to_deploy: Default::default(),
        auth_claim: auth_claim,
    };

    vec![
        json!({
            "name": "public_key_config",
            "type": "PublicKeyConfig",
            "value": {
                "key": {
                    "variant": "Ed25519",
                    "value": {
                        "ed25519": hex_bytes(&*sample_key),
                    },
                },
                "max_weight": "10",
            },
            "expected_hex": xdr_hex(&pubkey_config),
        }),
        json!({
            "name": "transaction_minimal",
            "type": "Transaction",
            "value": {
                "invocation": {
                    "invoked_address": hex_bytes(&*sample_address),
                    "entrypoint": hex_bytes(&*sample_entrypoint),
                    "gas_limit": "1000000",
                    "calldata": "",
                },
                "uniquifier": {
                    "block_expiry_height": "100",
                    "nonce": "42",
                },
                "contracts_to_deploy": [],
                "auth_claim": {
                    "variant": "kSourceAddressAuthData",
                    "value": {
                        "allowed_keys": "255",
                        "required_weight": "5",
                    },
                },
            },
            "expected_hex": xdr_hex(&tx),
        }),
    ]
}

pub fn generate() -> Value {
    let mut vectors = Vec::new();
    vectors.extend(primitive_vectors());
    vectors.extend(collection_vectors());
    vectors.extend(protocol_fixed_vectors());
    vectors.extend(protocol_enum_vectors());
    vectors.extend(protocol_struct_vectors());

    json!({
        "version": 1,
        "category": "xdr_serialization",
        "vectors": vectors,
    })
}

pub fn print() {
    println!("{}", serde_json::to_string_pretty(&generate()).unwrap());
}
