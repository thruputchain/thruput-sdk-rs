use sdk::storage::*;

fn main() {
    let mut v: Value<Value<i32>> = Value::new(&Keypath::new());
    v.set(&Value::from(Keypath::new()));
}
