use sdk::*;

#[derive(Event)]
struct TooManyTopics<'a> {
    #[topic]
    a: &'a ContractAddress,
    #[topic]
    b: &'a ContractAddress,
    #[topic]
    c: &'a ContractAddress,
    value: u32,
}

fn main() {}
