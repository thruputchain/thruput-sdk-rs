#![no_std]
#![no_main]

extern crate thruput_erc20_controls_if;

thruput_sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Admin);
thruput_sdk::export_interface_spec!(
    thruput_erc20_controls_if::ERC20Authorization
);
thruput_sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Clawback);
thruput_sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Burn);
