pub trait ContractClient {
    fn new(address: &crate::types::ContractAddress) -> Self;
}
