use sdk::storage::*;

fn main() {
    let mut v: Value<Bytestring> = Value::from(Keypath::root());
    v.set(&Bytestring::from(Keypath::root()));
}
