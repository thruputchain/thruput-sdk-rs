use sdk::storage::*;

fn main() {
    let mut v: Value<Value<i32>> = Value::from(Keypath::root());
    v.set(&Value::from(Keypath::root()));
}
