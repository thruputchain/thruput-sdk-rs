#[derive(thruput_sdk::Storage)]
pub struct Bad {
    #[slot(1)] #[slot(0)] a: thruput_sdk::storage::Value<u64>,
}
fn main() {}
