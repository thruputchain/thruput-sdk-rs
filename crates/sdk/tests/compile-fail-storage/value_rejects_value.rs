use thruput_sdk::storage::*;

fn main() {
    let mut v: Value<Value<i32>> = Value::from(Keypath::default());
    v.set(&Value::from(Keypath::default()));
}
