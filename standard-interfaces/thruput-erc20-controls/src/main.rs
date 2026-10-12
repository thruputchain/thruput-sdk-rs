#![no_std]
#![no_main]

extern crate thruput_erc20_controls_if;

sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Admin);
sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Authorization);
sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Clawback);
sdk::export_interface_spec!(thruput_erc20_controls_if::ERC20Burn);
