//! Export project data as diagrams for external renderers.
//!
//! Supported formats:
//! - [`Format::PlantUml`] — generates PlantUML syntax for tree, DAG, Gantt, and WBS diagrams.
//! - [`Format::Mermaid`] — accepted but not implemented; every call returns
//!   `Err`, so a script never mistakes a placeholder for a diagram.

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
/// or [`TaskId`] for dag diagrams. Gantt diagrams always cover the whole
/// project; supplying a root for them is an error.
pub fn render(
    project: &Project,
    format: Format,
    kind: DiagramKind,
    root: Option<&str>,
) -> Result<String, String> {
    if kind == DiagramKind::Gantt && root.is_some() {
        return Err("gantt diagrams do not take a root ID".to_string());
    }
    match format {
        Format::PlantUml => render_plantuml(project, kind, root),
        Format::Mermaid => render_mermaid(project, kind, root),
    }
}

fn parse_concept_root(project: &Project, root: Option<&str>) -> Result<Option<ConceptId>, String> {
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
        DiagramKind::Wbs => {
            let root_id = parse_concept_root(project, root)?;
            Ok(plantuml::wbs(project, root_id))
        }
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
            mermaid::tree(project, root_id)
        }
        DiagramKind::Dag => {
            let root_id = parse_task_root(project, root)?;
            mermaid::dag(project, root_id)
        }
        DiagramKind::Gantt => mermaid::gantt(project),
        DiagramKind::Wbs => {
            let root_id = parse_concept_root(project, root)?;
            mermaid::wbs(project, root_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> Project {
        let mut p = Project::new();
        p.add_concept("Backend".into(), None, None).unwrap();
        p.add_concept("Frontend".into(), None, None).unwrap();
        p.add_task("Build API".into(), None, None, None);
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        p
    }

    #[test]
    fn gantt_rejects_root() {
        let p = sample_project();
        for format in [Format::PlantUml, Format::Mermaid] {
            let err = render(&p, format, DiagramKind::Gantt, Some("1")).unwrap_err();
            assert!(err.contains("gantt"), "{err}");
        }
    }

    #[test]
    fn wbs_validates_root() {
        let p = sample_project();
        let err = render(&p, Format::PlantUml, DiagramKind::Wbs, Some("999")).unwrap_err();
        assert!(err.contains("not found"), "{err}");
    }

    #[test]
    fn wbs_honors_root() {
        let p = sample_project();
        let out = render(&p, Format::PlantUml, DiagramKind::Wbs, Some("1")).unwrap();
        assert!(out.contains("Backend"), "{out}");
        assert!(out.contains("Build API"), "{out}");
        assert!(!out.contains("Frontend"), "{out}");
    }
}
