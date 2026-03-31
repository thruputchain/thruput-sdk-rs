use sdk::storage::*;

fn main() {
    let mut v: Value<NNInt> = Value::new(&Keypath::new());
    v.set(&NNInt::from(Keypath::new()));
}
