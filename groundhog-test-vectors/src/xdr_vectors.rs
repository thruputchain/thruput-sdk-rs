use serde_json::{json, Value};
use xdrs::txs;
use xdrs::WriteXDR;
use xdrs::{BytesM, VecM, StringM};

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

pub fn print() {
    let mut vectors = Vec::new();
    vectors.extend(primitive_vectors());
    vectors.extend(collection_vectors());

    let output = json!({
        "version": 1,
        "category": "xdr_serialization",
        "vectors": vectors,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
