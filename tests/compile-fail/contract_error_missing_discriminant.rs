use sdk::contract_error;

#[contract_error]
enum Error {
    One = 1,
    Two,
}

fn main() {}
