use sdk::storage::*;

fn main() {
    let mut v: Value<Bytestring> = Value::from_keypath(&Keypath::default());
    v.set(&Bytestring::from_keypath(&Keypath::default()));
}
