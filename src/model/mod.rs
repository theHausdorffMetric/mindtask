//! Data model for projects, concepts, and tasks.
//!
//! A [`Project`](project::Project) owns a list of [`Concept`](concept::Concept)s
//! (organized as a tree) and a list of [`Task`](task::Task)s (linked by dependencies
//! into a DAG). Typed IDs ([`ConceptId`](id::ConceptId), [`TaskId`](id::TaskId))
//! ensure concepts and tasks are never confused.

pub mod concept;
pub mod id;
pub mod project;
pub mod task;
