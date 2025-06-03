use ::serde::{Serialize, Deserialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
struct Data {
    num: i32,
    nums: Vec<u64>,
    bytes: [u8; 32],
}

use ::groundhog_xdr::types::*;
use ::groundhog_xdr::transaction::*;
use ::groundhog_xdr::{CBOR, to_cbor};
use ::groundhog_xdr::cbor;

fn mk_data() -> SignedTransaction {
    let invocation = Invocation {
        invoked_address: Default::default(),
        entrypoint: Default::default(),
        gas_limit: Default::default(),
        calldata: Default::default(),
    };

    let tx = Transaction {
        id: Default::default(),
        created: Default::default(),
        invocation,
        contracts_to_deploy: Default::default(),
    };

    let stx = SignedTransaction {
        signature: Default::default(),
        tx,
    };

    stx
}

fn print_bytes(buf: &[u8]) {
    print!("[");
    for (i, b) in buf.iter().enumerate() {
        let c = Into::<char>::into(*b);
        if c.is_ascii_graphic() {
            print!("{}", c);
        } else {
            print!("{}", '.');
        }

        if i > 0 && i % 40 == 0 {
            print!("\n ");
        }
    }
    println!("]");
}

fn print_encoding(kind: &str, buf: &[u8]) {
    println!("\n{} ENCODE ({} bytes):", kind, buf.len());
    print_bytes(buf);
}


#[test]
fn test_encodings() {
    bcs_test();
    cbor_test();
    cbor_value_test();
}

fn cbor_value_test() {
    let data = mk_data();

    let cbor_value = data.to_cbor();
    let mut bytes: Vec<u8> = vec![];
    ::ciborium::into_writer(&cbor_value, &mut bytes).unwrap();
    print_encoding("CBOR custom", &bytes);

    let val: cbor::Value = ::ciborium::from_reader(bytes.as_slice()).unwrap();
    let data2 = CBOR::from_cbor(&val);
    assert_eq!(data, data2);
}


fn cbor_test() {
    let data = mk_data();

    let mut bytes: Vec<u8> = vec![];
    ::ciborium::into_writer(&data, &mut bytes).unwrap();
    print_encoding("CBOR", &bytes);

    let data2 = ::ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(data, data2);
}

fn bcs_test() {
    let data = mk_data();

    let bytes: Vec<u8> = ::bcs::to_bytes(&data).unwrap();
    print_encoding("BCS", &bytes);

    let data2 = ::bcs::from_bytes(bytes.as_slice()).unwrap();
    assert_eq!(data, data2);
}
