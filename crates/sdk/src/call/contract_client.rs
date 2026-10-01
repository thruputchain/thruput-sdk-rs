pub trait ContractClient {
    /// The id of the interface this client calls, matching that interface's
    /// `INTERFACE_ID`.
    const INTERFACE_ID: crate::InterfaceId;

    fn new(address: &crate::types::ContractAddress) -> Self;
}
