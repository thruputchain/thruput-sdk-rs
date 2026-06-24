#[derive(sdk::Storage)]
pub struct Bad {
    #[slot(3)] a: sdk::storage::Value<u64>,
    #[slot(3)] b: sdk::storage::Value<u64>,
}

fn main() {}
