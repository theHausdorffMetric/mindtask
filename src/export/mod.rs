//! Export project data as diagrams for external renderers.
//!
//! Supported formats:
//! - [`Format::PlantUml`] — generates PlantUML syntax for tree, DAG, Gantt, and WBS diagrams.
//! - [`Format::Mermaid`] — stubbed for future implementation.

pub mod mermaid;
pub mod plantuml;

use std::fmt;
use std::str::FromStr;

use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;

/// Output format for diagram generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    PlantUml,
    Mermaid,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlantUml => write!(f, "plantuml"),
            Self::Mermaid => write!(f, "mermaid"),
        }
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "plantuml" => Ok(Self::PlantUml),
            "mermaid" => Ok(Self::Mermaid),
            _ => Err(format!(
                "invalid format '{s}': expected 'plantuml' or 'mermaid'"
            )),
        }
    }
}

/// Type of diagram to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    Tree,
    Dag,
    Gantt,
    Wbs,
}

impl fmt::Display for DiagramKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree => write!(f, "tree"),
            Self::Dag => write!(f, "dag"),
            Self::Gantt => write!(f, "gantt"),
            Self::Wbs => write!(f, "wbs"),
        }
    }
}

impl FromStr for DiagramKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tree" => Ok(Self::Tree),
            "dag" => Ok(Self::Dag),
            "gantt" => Ok(Self::Gantt),
            "wbs" => Ok(Self::Wbs),
            _ => Err(format!(
                "invalid diagram kind '{s}': expected 'tree', 'dag', 'gantt', or 'wbs'"
            )),
        }
    }
}

/// Render a diagram from project data.
///
/// `root` is an optional ID string: parsed as [`ConceptId`] for tree/wbs diagrams,
/// or [`TaskId`] for dag/gantt diagrams.
pub fn render(
    project: &Project,
    format: Format,
    kind: DiagramKind,
    root: Option<&str>,
) -> Result<String, String> {
    match format {
        Format::PlantUml => render_plantuml(project, kind, root),
        Format::Mermaid => render_mermaid(project, kind, root),
    }
}

fn parse_concept_root(
    project: &Project,
    root: Option<&str>,
) -> Result<Option<ConceptId>, String> {
    let root_id = root
        .map(|s| s.parse::<ConceptId>())
        .transpose()
        .map_err(|e| e.to_string())?;
    if let Some(id) = root_id
        && project.get_concept(id).is_none()
    {
        return Err(format!("concept {id} not found"));
    }
    Ok(root_id)
}

fn parse_task_root(project: &Project, root: Option<&str>) -> Result<Option<TaskId>, String> {
    let root_id = root
        .map(|s| s.parse::<TaskId>())
        .transpose()
        .map_err(|e| e.to_string())?;
    if let Some(id) = root_id
        && project.get_task(id).is_none()
    {
        return Err(format!("task {id} not found"));
    }
    Ok(root_id)
}

fn render_plantuml(
    project: &Project,
    kind: DiagramKind,
    root: Option<&str>,
) -> Result<String, String> {
    match kind {
        DiagramKind::Tree => {
            let root_id = parse_concept_root(project, root)?;
            Ok(plantuml::tree(project, root_id))
        }
        DiagramKind::Dag => {
            let root_id = parse_task_root(project, root)?;
            Ok(plantuml::dag(project, root_id))
        }
        DiagramKind::Gantt => Ok(plantuml::gantt(project)),
        DiagramKind::Wbs => Ok(plantuml::wbs(project)),
    }
}

fn render_mermaid(
    project: &Project,
    kind: DiagramKind,
    root: Option<&str>,
) -> Result<String, String> {
    match kind {
        DiagramKind::Tree => {
            let root_id = parse_concept_root(project, root)?;
            Ok(mermaid::tree(project, root_id))
        }
        DiagramKind::Dag => {
            let root_id = parse_task_root(project, root)?;
            Ok(mermaid::dag(project, root_id))
        }
        DiagramKind::Gantt => Ok(mermaid::gantt(project)),
        DiagramKind::Wbs => Ok(mermaid::wbs(project)),
    }
}
