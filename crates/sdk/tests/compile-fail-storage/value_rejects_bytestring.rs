use thruput_sdk::storage::*;

fn main() {
    let mut v: Value<Bytestring> = Value::from(Keypath::default());
    v.set(&Bytestring::from(Keypath::default()));
}
