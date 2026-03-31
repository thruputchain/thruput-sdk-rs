use sdk::storage::*;

fn main() {
    let mut v: Value<Bytestring> = Value::new(&Keypath::new());
    v.set(&Bytestring::from(Keypath::new()));
}
