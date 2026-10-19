use thruput_erc721_if::*;
use thruput_sdk::{ContractAddress, SHA256Hash};

#[test]
fn event_displays_use_base58_addresses_and_token_hashes() {
    let zero = ContractAddress::new([0; 32]);
    let other = ContractAddress::new([1; 32]);
    let token = SHA256Hash::new([1; 32]);
    let a = "11111111111111111111111111111111";
    let b = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";
    assert_eq!(
        TransferEvent::new(&zero, &other, token).to_string(),
        format!("TRANSFER({a}, {b}, token:{b})")
    );
    assert_eq!(
        MintEvent::new(&other, token).to_string(),
        format!("MINT_TO({b}, token:{b})")
    );
    assert_eq!(
        BurnEvent::new(&other, token).to_string(),
        format!("BURN_FROM({b}, token:{b})")
    );
    assert_eq!(
        ApprovalEvent::new(&zero, &other, token).to_string(),
        format!("APPROVE({a}, {b}, token:{b})")
    );
    assert_eq!(
        ApprovalForAllEvent::new(&zero, &other, true).to_string(),
        format!("APPROVE_FOR_ALL({a}, {b}, true)")
    );
}
