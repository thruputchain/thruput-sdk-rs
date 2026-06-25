#[derive(sdk::Storage)]
pub struct Bad {
    #[slot(1)] #[slot(0)] a: sdk::storage::Value<u64>,
}
fn main() {}
