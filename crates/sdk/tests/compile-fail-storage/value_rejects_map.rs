use sdk::storage::*;

fn main() {
    let mut v: Value<Map<u32, NNInt>> = Value::new(&Keypath::new());
    v.set(&Map::from(Keypath::new()));
}
