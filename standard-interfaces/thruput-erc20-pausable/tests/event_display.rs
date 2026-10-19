use thruput_erc20_pausable_if::{PausedEvent, UnpausedEvent};
use thruput_sdk::ContractAddress;

#[test]
fn event_displays_use_base58_addresses() {
    let account = ContractAddress::new([1; 32]);
    let address = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";
    assert_eq!(
        PausedEvent::new(account).to_string(),
        format!("PAUSED({address})")
    );
    assert_eq!(
        UnpausedEvent::new(account).to_string(),
        format!("UNPAUSED({address})")
    );
}
