use thruput_access_control_if::*;
use thruput_sdk::{ContractAddress, SHA256Hash};

#[test]
fn event_displays_use_base58_addresses_and_role_hashes() {
    let account = ContractAddress::new([1; 32]);
    let role = SHA256Hash::new([0; 32]);
    let next_role = SHA256Hash::new([1; 32]);
    let a = "11111111111111111111111111111111";
    let b = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";
    assert_eq!(
        RoleGrantedEvent::new(&role, &account, account).to_string(),
        format!("ROLE_GRANTED(role:{a}, {b}, by:{b})")
    );
    assert_eq!(
        RoleRevokedEvent::new(&role, &account, account).to_string(),
        format!("ROLE_REVOKED(role:{a}, {b}, by:{b})")
    );
    assert_eq!(
        RoleAdminChangedEvent::new(&role, &role, next_role).to_string(),
        format!("ROLE_ADMIN_CHANGED(role:{a}, {a} -> {b})")
    );
}
