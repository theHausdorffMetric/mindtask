//! Graph validation for the concept tree and task dependency DAG.
//!
//! The [`tree`] module validates the parent-child concept hierarchy, while
//! [`dag`] handles task dependency cycle detection, topological ordering,
//! and full project validation.

pub mod dag;
pub mod tree;
