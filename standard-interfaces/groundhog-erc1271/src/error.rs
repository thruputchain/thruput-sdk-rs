#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC1271Error {
    #[cfg_attr(feature = "dev", error("Invalid signature"))]
    InvalidSignature = 1,
}
