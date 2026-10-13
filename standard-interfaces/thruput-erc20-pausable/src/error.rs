#[thruput_sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20PausableError {
    #[cfg_attr(feature = "dev", error("Caller must be a pauser"))]
    MustBePauser = 1,

    #[cfg_attr(feature = "dev", error("Operation unavailable while paused"))]
    EnforcedPause = 2,

    #[cfg_attr(feature = "dev", error("Operation requires a paused token"))]
    ExpectedPause = 3,
}
