use thruput_sdk::{
    ContractAddress, SHA256Hash, base58, base58_encode, hex_encode,
};

#[test]
fn identifier_text_round_trips_without_changing_xdr_bytes() {
    let bytes = [1; 32];
    let address = ContractAddress::new(bytes);
    let text = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";
    assert_eq!(base58_encode(&address).to_string(), text);
    assert_eq!(base58::decode_fixed::<32>(text).unwrap(), bytes);
    assert_eq!(format!("{address:?}"), text);
    assert_eq!(format!("{:?}", SHA256Hash::new(bytes)), text);
    assert_eq!(
        thruput_sdk::internal::encode_xdr(&address)
            .unwrap()
            .as_slice(),
        bytes
    );
    assert_eq!(hex_encode(&[0, 1, 255]).to_string(), "0001ff");
}
