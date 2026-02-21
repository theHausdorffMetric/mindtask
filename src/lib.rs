//! A task and concept management library.
//!
//! Mindtask organizes work as **tasks** grouped under a tree of **concepts**
//! (categories/topics). Tasks can declare dependencies on other tasks, forming
//! a DAG that is validated for cycles.
//!
//! # Modules
//!
//! - [`model`] — Core data types: [`Project`](model::project::Project),
//!   [`Concept`](model::concept::Concept), [`Task`](model::task::Task), and their IDs.
//! - [`graph`] — Structural validation: cycle detection for the concept tree
//!   and the task dependency DAG.
//! - [`store`] — JSON file persistence (load/save).

pub mod export;
pub mod graph;
pub mod model;
pub mod store;
