//! gnaw: builds, tests, and checks this repository.
//!
//! Work is a graph of named [tasks](task::Task) in [`tasks::TASKS`], selected
//! by task id, [group](tasks::GROUPS), or glob. [`plan`] resolves a selection
//! and its prerequisites, [`exec`] runs it, and [`report`] renders progress.
//!
//! [`commands`] work on named [contracts](contract::Contract) directly:
//! build, test, lint, and inspect one while developing it.

pub mod commands;
pub mod contract;
pub mod elf;
pub mod exec;
pub mod plan;
pub mod repo;
pub mod report;
pub mod rv32;
pub mod specs;
pub mod task;
pub mod tasks;
