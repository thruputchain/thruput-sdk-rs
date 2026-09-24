#![no_std]
#![no_main]

extern crate groundhog_sep41_if;

// Retain both interfaces in standalone spec extraction.
sdk::export_interface_spec!(groundhog_sep41_if::Token);
sdk::export_interface_spec!(groundhog_sep41_if::TokenAdmin);
