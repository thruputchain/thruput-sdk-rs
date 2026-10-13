#[derive(thruput_sdk::Storage)]
pub struct Bad {
    #[slot(3)] a: thruput_sdk::storage::Value<u64>,
    #[slot(3)] b: thruput_sdk::storage::Value<u64>,
}

fn main() {}
