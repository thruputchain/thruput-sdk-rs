//! gnaw: builds, tests, and checks this repository.
//!
//! Work is a graph of named [tasks](task::Task) in [`tasks::TASKS`], selected
//! by task id, [group](tasks::GROUPS), or glob. [`plan`] resolves a selection
//! and its prerequisites, [`exec`] runs it, and [`report`] renders progress.

pub mod exec;
pub mod plan;
pub mod repo;
pub mod report;
pub mod task;
pub mod tasks;
