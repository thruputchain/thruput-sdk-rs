use sdk::*;

pub struct EventShakingSubsetContract;

/// Implements only the `Adopted` trait — NOT `Orphan`.  The presence of this
/// impl anchors the `Adopted` interface in the spec and causes `AdoptedEvent`
/// to be retained.  The absence of an `Orphan` impl means `OrphanEvent` and
/// its `OrphanPayload` typedef are shaken from the final spec.
#[contract]
impl event_shaking_if::Adopted for EventShakingSubsetContract {
    fn adopted_ping() -> u32 {
        1
    }
}
