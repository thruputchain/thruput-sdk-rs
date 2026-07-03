use sdk::storage::*;

fn main() {
    let mut v: Value<Map<u32, NNInt>> = Value::from(Keypath::default());
    v.set(&Map::from(Keypath::default()));
}
