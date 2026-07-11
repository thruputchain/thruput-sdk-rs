use sdk::storage::*;

fn main() {
    let mut v: Value<Map<u32, NNInt>> = Value::from_keypath(&Keypath::default());
    v.set(&Map::from_keypath(&Keypath::default()));
}
