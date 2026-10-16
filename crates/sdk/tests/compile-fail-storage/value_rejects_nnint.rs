use thruput_sdk::storage::*;

fn main() {
    let mut v: Value<NNInt> = Value::from(Keypath::default());
    v.set(&NNInt::from(Keypath::default()));
}
