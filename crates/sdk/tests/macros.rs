use hex_literal::hex;
use sdk::{hash_to_array, sha256_static, static_key};

// Pin the key derivation: sha256 of the string's value (no quotes).
// echo -n 'method_called' | shasum -a 256
#[test]
fn test_static_key() {
    let key = static_key!("method_called");
    assert_eq!(
        key.as_slice(),
        hex!(
            "f506f3f7c0a444f55bcac69ac62f0572878d982d5498603d95411c19466b4301"
        )
    );
}

// All the string-hashing macros must agree on the same bytes: the
// literal's *value* (quotes stripped, escapes resolved).
#[test]
fn string_hash_macros_agree() {
    assert_eq!(
        sha256_static!("method_called").as_slice(),
        static_key!("method_called").as_slice(),
    );
    assert_eq!(
        sha256_static!("method_called").as_slice(),
        hash_to_array!("method_called"),
    );
    // Raw strings and escaped strings with the same value hash identically.
    assert_eq!(
        static_key!(r"method_called").as_slice(),
        static_key!("method_called").as_slice(),
    );
}
