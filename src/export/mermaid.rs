//! Mermaid diagram generation (not yet implemented).

use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;

/// Generate a Mermaid mindmap of the concept tree (stub).
pub fn tree(_project: &Project, _root_id: Option<ConceptId>) -> String {
    "Mermaid export not yet implemented\n".into()
}

/// Generate a Mermaid flowchart of the task dependency DAG (stub).
pub fn dag(_project: &Project, _root_id: Option<TaskId>) -> String {
    "Mermaid export not yet implemented\n".into()
}

/// Generate a Mermaid Gantt chart from tasks with due dates (stub).
pub fn gantt(_project: &Project) -> String {
    "Mermaid export not yet implemented\n".into()
}

/// Generate a Mermaid WBS diagram with concepts and tasks (stub).
pub fn wbs(_project: &Project) -> String {
    "Mermaid export not yet implemented\n".into()
}
