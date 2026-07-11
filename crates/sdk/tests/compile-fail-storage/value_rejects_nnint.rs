use sdk::storage::*;

fn main() {
    let mut v: Value<NNInt> = Value::from_keypath(&Keypath::default());
    v.set(&NNInt::from_keypath(&Keypath::default()));
}
