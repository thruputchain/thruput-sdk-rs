use thruput_erc20_if::*;
use thruput_sdk::ContractAddress;

#[test]
fn event_displays_use_base58_addresses() {
    let zero = ContractAddress::new([0; 32]);
    let other = ContractAddress::new([1; 32]);
    let a = "11111111111111111111111111111111";
    let b = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";
    assert_eq!(
        TransferEvent::new(&zero, &other, 42).to_string(),
        format!("TRANSFER({a}, {b}, amount:42)")
    );
    assert_eq!(
        AllowanceChangeEvent::new(&zero, &other, -7).to_string(),
        format!("ALLOWANCE_CHANGE({a}, {b}, difference:-7)")
    );
    assert_eq!(
        AllowanceDeleteEvent::new(&zero, &other, 7).to_string(),
        format!("ALLOWANCE_DELETE({a}, {b}, removed:7)")
    );
    assert_eq!(
        AllowanceResetEvent::new(&zero, &other, 7, 9.into()).to_string(),
        format!("ALLOWANCE_RESET({a}, {b}, removed:7, until:9)")
    );
    assert_eq!(
        MintEvent::new(&other, 42).to_string(),
        format!("MINT_TO({b}, amount:42)")
    );
    assert_eq!(
        BurnEvent::new(&other, 42).to_string(),
        format!("BURN_FROM({b}, amount:42)")
    );
}
