#![no_std]
#![no_main]

extern crate groundhog_erc20_controls_if;

sdk::export_interface_spec!(groundhog_erc20_controls_if::ERC20Admin);
sdk::export_interface_spec!(groundhog_erc20_controls_if::ERC20Authorization);
sdk::export_interface_spec!(groundhog_erc20_controls_if::ERC20Clawback);
sdk::export_interface_spec!(groundhog_erc20_controls_if::ERC20Burn);
