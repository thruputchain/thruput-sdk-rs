use sdk::storage::*;

fn main() {
    let mut v: Value<NNInt> = Value::from(Keypath::root());
    v.set(&NNInt::from(Keypath::root()));
}
