//! Mermaid diagram generation (not yet implemented).

use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;

pub fn tree(_project: &Project, _root_id: Option<ConceptId>) -> String {
    "Mermaid export not yet implemented\n".into()
}

pub fn dag(_project: &Project, _root_id: Option<TaskId>) -> String {
    "Mermaid export not yet implemented\n".into()
}

pub fn gantt(_project: &Project) -> String {
    "Mermaid export not yet implemented\n".into()
}

pub fn wbs(_project: &Project) -> String {
    "Mermaid export not yet implemented\n".into()
}
