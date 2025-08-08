use sdk::macros::static_key;
use hex_literal::hex;

// Simple test to make sure the macro matches what is happening on the hogd side
#[test]
fn test_static_key() {
    let key = static_key!("method_called");
    assert_eq!(key.as_slice(), hex!("8f27a663356f199348b94c38d663df64077ca24f27c3db9d30eced7774c9b24e"));
}
