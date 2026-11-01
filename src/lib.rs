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
//! - [`export`] — Diagram generation for visualization: PlantUML for every
//!   diagram, Graphviz DOT for the tree and the DAG (Mermaid is not
//!   implemented and returns an error).
//! - [`import`] — Concept-graph JSONL ingestion: project an `is-a` DAG onto
//!   the strict tree and merge it under a target concept.
//! - [`report`] — View models behind the listings (`report`, `task ls`,
//!   `concept tree`, `schedule`): what each shows, independent of rendering;
//!   also the shape of the CLI's JSON output.

pub mod export;
pub mod graph;
pub mod import;
pub mod model;
pub mod refs;
pub mod report;
pub mod store;
