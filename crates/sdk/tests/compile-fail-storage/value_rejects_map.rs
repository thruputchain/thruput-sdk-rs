use sdk::storage::*;

fn main() {
    let mut v: Value<Map<u32, NNInt>> = Value::from(Keypath::root());
    v.set(&Map::from(Keypath::root()));
}
