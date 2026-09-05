//! A contract that can take custody of ERC721 tokens: it implements the
//! receiver hook, remembers the last delivery, and refuses a delivery whose
//! attached data says so, to show a rejection reaching the transfer's
//! caller.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::{
    Delivery, REJECT_DATA, Receiver, ReceiverClient, ReceiverError,
};
