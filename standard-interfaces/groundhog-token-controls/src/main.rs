#![no_std]
#![no_main]

extern crate groundhog_token_controls_if;

sdk::export_interface_spec!(groundhog_token_controls_if::TokenAdmin);
sdk::export_interface_spec!(groundhog_token_controls_if::TokenAuthorization);
sdk::export_interface_spec!(groundhog_token_controls_if::TokenClawback);
sdk::export_interface_spec!(groundhog_token_controls_if::TokenBurn);
