use sdk::storage::*;

fn main() {
    let mut v: Value<Value<i32>> = Value::from_keypath(&Keypath::default());
    v.set(&Value::from_keypath(&Keypath::default()));
}
