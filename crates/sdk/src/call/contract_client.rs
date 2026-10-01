pub trait ContractClient {
    /// The identity of the interface this client calls, at its major
    /// version: the same id as the interface's `Entrypoints::INTERFACE_ID`
    /// and, for a trait-defined interface, the trait's `INTERFACE_ID`.
    const INTERFACE_ID: crate::InterfaceId;

    fn new(address: &crate::types::ContractAddress) -> Self;
}
