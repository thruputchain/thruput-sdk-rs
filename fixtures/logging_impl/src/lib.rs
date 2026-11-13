//! Contract for gnaw's `fixtures:logging` check. It implements
//! [`logging_if::Logged`] and calls it through the generated client, so a
//! `logging` build covers both the logged entrypoint and the logged client.
//! See `logging_if` for the bugs the check guards.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use logging_if::{Logged, LoggedClient, Note};
use thruput_sdk::*;

pub struct LoggingImpl;

#[contract]
impl Logged for LoggingImpl {
    fn record(note: Note) -> u32 {
        note.tag
    }
}

/// Forwards to another `Logged` contract through its client.
#[contract]
impl LoggingImpl {
    pub fn forward(target: ContractAddress, tag: u32) -> u32 {
        LoggedClient::new(&target).record(Note { tag })
    }
}
