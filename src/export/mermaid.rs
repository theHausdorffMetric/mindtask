//! Mermaid diagram generation — not yet implemented.
//!
//! Every entry point returns `Err` rather than a placeholder string. Returning
//! `Ok("… not yet implemented")` gave callers a zero exit status and a file
//! full of prose, which a script cannot distinguish from a real diagram.

use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;

/// The error every entry point returns until Mermaid support lands.
fn unimplemented(kind: &str) -> Result<String, String> {
    Err(format!(
        "mermaid {kind} export is not implemented yet; use 'plantuml' instead"
    ))
}

/// Generate a Mermaid mindmap of the concept tree (unimplemented).
pub fn tree(_project: &Project, _root_id: Option<ConceptId>) -> Result<String, String> {
    unimplemented("tree")
}

/// Generate a Mermaid flowchart of the task dependency DAG (unimplemented).
pub fn dag(_project: &Project, _root_id: Option<TaskId>) -> Result<String, String> {
    unimplemented("dag")
}

/// Generate a Mermaid Gantt chart (unimplemented).
pub fn gantt(_project: &Project) -> Result<String, String> {
    unimplemented("gantt")
}

/// Generate a Mermaid WBS diagram (unimplemented).
pub fn wbs(_project: &Project, _root_id: Option<ConceptId>) -> Result<String, String> {
    unimplemented("wbs")
}
