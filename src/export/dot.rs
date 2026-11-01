//! Graphviz DOT generation — the concept tree and the task DAG as
//! `digraph`s for `dot -Tsvg`. Graphviz is what a workstation tends to have
//! installed, where PlantUML needs a Java runtime, so these are the diagrams
//! the report recipes embed.
//!
//! Gantt and WBS have no DOT form: Graphviz draws graphs, not charts.

use std::collections::HashSet;
use std::fmt::Write;

use super::one_line;
use super::plantuml::collect_downstream;
use crate::graph::schedule::schedule;
use crate::model::concept::Concept;
use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;
use crate::model::task::{Task, TaskState};

/// Font every node uses; a plain sans-serif that every Graphviz build maps.
const FONT: &str = "Helvetica";

/// Colour of the critical path (tasks and the edges between them).
const CRITICAL: &str = "tomato";

/// Label for a DOT double-quoted string: whitespace collapsed, `\` and `"`
/// escaped. The result still needs its surrounding quotes.
fn quoted(s: &str) -> String {
    one_line(s).replace('\\', "\\\\").replace('"', "\\\"")
}

/// Generate a DOT digraph of the concept tree, left to right.
///
/// If `root_id` is given, only the subtree rooted at that concept is
/// rendered. A single root stands alone; several hang under a synthetic
/// *Project* node, as in the PlantUML mindmap. The caller must ensure the
/// concept exists.
pub fn tree(project: &Project, root_id: Option<ConceptId>) -> String {
    let mut out = String::from("digraph concepts {\n");
    writeln!(out, "  rankdir=LR;").unwrap();
    writeln!(
        out,
        "  node [shape=box, style=rounded, fontname=\"{FONT}\"];"
    )
    .unwrap();
    writeln!(out, "  edge [arrowhead=none];").unwrap();

    let mut seen = HashSet::new();
    match root_id {
        Some(id) => {
            let concept = project.get_concept(id).expect("caller validated existence");
            write_concept(&mut out, project, concept, &mut seen);
        }
        None => {
            let roots = project.roots();
            if roots.len() > 1 {
                writeln!(out, "  project [label=\"Project\", shape=plaintext];").unwrap();
            }
            for root in &roots {
                if roots.len() > 1 {
                    writeln!(out, "  project -> c{};", root.id).unwrap();
                }
                write_concept(&mut out, project, root, &mut seen);
            }
        }
    }

    out.push_str("}\n");
    out
}

/// One concept's node, then its children and the edges to them.
fn write_concept(
    out: &mut String,
    project: &Project,
    concept: &Concept,
    seen: &mut HashSet<ConceptId>,
) {
    if !seen.insert(concept.id) {
        // Cyclic in-memory data cannot occur on validated projects; never loop.
        return;
    }
    writeln!(
        out,
        "  c{} [label=\"{}\"];",
        concept.id,
        quoted(&concept.name)
    )
    .unwrap();
    for child in project.children_of(concept.id) {
        writeln!(out, "  c{} -> c{};", concept.id, child.id).unwrap();
        write_concept(out, project, child, seen);
    }
}

/// Generate a DOT digraph of the task dependency DAG, dependencies on the
/// left. Done tasks are greyed, in-progress ones gold; when the project has
/// durations, the critical path — its tasks and the edges between them — is
/// drawn bold in tomato.
///
/// If `root_id` is given, only that task and all downstream dependents are
/// shown. The caller must ensure the task exists.
pub fn dag(project: &Project, root_id: Option<TaskId>) -> String {
    let tasks: Vec<&Task> = match root_id {
        Some(id) => collect_downstream(project, id),
        None => project.tasks.iter().collect(),
    };
    let task_ids: HashSet<TaskId> = tasks.iter().map(|t| t.id).collect();

    // Critical tasks, when there is a schedule worth showing. A cycle cannot
    // occur on validated data; an all-zero schedule marks everything critical,
    // which would say nothing, so it is skipped too.
    let critical: HashSet<TaskId> = match schedule(&project.tasks) {
        Ok(s) if s.duration.abs() >= 1e-9 => s.critical_path().iter().copied().collect(),
        _ => HashSet::new(),
    };

    let mut out = String::from("digraph tasks {\n");
    writeln!(out, "  rankdir=LR;").unwrap();
    writeln!(out, "  node [shape=box, fontname=\"{FONT}\"];").unwrap();

    for task in &tasks {
        let mut attrs = vec![format!(
            "label=\"{}\\n{{{}}}\"",
            quoted(&task.name),
            task.id
        )];
        match task.state {
            TaskState::Done => {
                attrs.push("style=filled".into());
                attrs.push("fillcolor=lightgrey".into());
                attrs.push("fontcolor=gray40".into());
            }
            TaskState::InProgress => {
                attrs.push("style=filled".into());
                attrs.push("fillcolor=gold".into());
            }
            TaskState::Todo => {}
        }
        if critical.contains(&task.id) {
            attrs.push(format!("color={CRITICAL}"));
            attrs.push("penwidth=2".into());
        }
        writeln!(out, "  t{} [{}];", task.id, attrs.join(", ")).unwrap();
    }

    for task in &tasks {
        for &dep in &task.depends_on {
            if !task_ids.contains(&dep) {
                continue;
            }
            if critical.contains(&dep) && critical.contains(&task.id) {
                writeln!(
                    out,
                    "  t{} -> t{} [color={CRITICAL}, penwidth=2];",
                    dep, task.id
                )
                .unwrap();
            } else {
                writeln!(out, "  t{} -> t{};", dep, task.id).unwrap();
            }
        }
    }

    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        let mut p = Project::new();
        p.add_concept("Backend".into(), None, None).unwrap(); // 1
        p.add_concept("API \"v2\"".into(), Some(ConceptId(1)), None)
            .unwrap(); // 2
        p.add_concept("Frontend".into(), None, None).unwrap(); // 3
        p.add_task("Design".into(), None, Some(2.0), None).unwrap(); // 1
        p.add_task("Build  API".into(), None, Some(3.0), None)
            .unwrap(); // 2
        p.add_task("Docs".into(), None, Some(1.0), None).unwrap(); // 3
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        p.add_dependency(TaskId(3), TaskId(1)).unwrap();
        p.set_task_state(TaskId(1), TaskState::Done).unwrap();
        p.set_task_state(TaskId(2), TaskState::InProgress).unwrap();
        p
    }

    #[test]
    fn quoted_escapes_for_dot_strings() {
        assert_eq!(quoted("a \"b\" \\ c\nd"), "a \\\"b\\\" \\\\ c d");
        assert_eq!(quoted("  "), "(unnamed)");
    }

    #[test]
    fn tree_hangs_several_roots_under_project() {
        let out = tree(&project(), None);
        assert!(
            out.starts_with("digraph concepts {\n  rankdir=LR;\n"),
            "{out}"
        );
        assert!(
            out.contains("project [label=\"Project\", shape=plaintext];"),
            "{out}"
        );
        assert!(out.contains("  project -> c1;\n"), "{out}");
        assert!(out.contains("  c1 [label=\"Backend\"];\n"), "{out}");
        assert!(
            out.contains("  c1 -> c2;\n  c2 [label=\"API \\\"v2\\\"\"];\n"),
            "{out}"
        );
        assert!(out.contains("  project -> c3;\n"), "{out}");
        assert!(out.ends_with("}\n"), "{out}");
    }

    #[test]
    fn tree_subtree_stands_alone() {
        let out = tree(&project(), Some(ConceptId(1)));
        assert!(!out.contains("project"), "{out}");
        assert!(out.contains("c1 -> c2"), "{out}");
        assert!(!out.contains("Frontend"), "{out}");
        let single = tree(&Project::new(), None);
        assert_eq!(single.matches('\n').count(), 5, "{single}");
    }

    #[test]
    fn dag_styles_states_and_critical_path() {
        let out = dag(&project(), None);
        // 1 → 2 (5 days) is the critical chain; 1 → 3 (3 days) is not.
        assert!(out.contains("  t1 [label=\"Design\\n{1}\", style=filled, fillcolor=lightgrey, fontcolor=gray40, color=tomato, penwidth=2];\n"), "{out}");
        assert!(out.contains("  t2 [label=\"Build API\\n{2}\", style=filled, fillcolor=gold, color=tomato, penwidth=2];\n"), "{out}");
        assert!(out.contains("  t3 [label=\"Docs\\n{3}\"];\n"), "{out}");
        assert!(
            out.contains("  t1 -> t2 [color=tomato, penwidth=2];\n"),
            "{out}"
        );
        assert!(out.contains("  t1 -> t3;\n"), "{out}");
    }

    #[test]
    fn dag_root_keeps_downstream_only_and_skips_outside_edges() {
        let mut p = project();
        p.add_task("Deploy".into(), None, None, None).unwrap(); // 4
        p.add_dependency(TaskId(4), TaskId(3)).unwrap();
        let out = dag(&p, Some(TaskId(3)));
        assert!(out.contains("t3 ["), "{out}");
        assert!(out.contains("t4 ["), "{out}");
        assert!(!out.contains("t1"), "{out}");
        assert!(out.contains("  t3 -> t4;\n"), "{out}");
    }

    #[test]
    fn dag_without_durations_marks_nothing_critical() {
        let mut p = Project::new();
        p.add_task("a".into(), None, None, None).unwrap();
        p.add_task("b".into(), None, None, None).unwrap();
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        let out = dag(&p, None);
        assert!(!out.contains("tomato"), "{out}");
        assert!(out.contains("  t1 -> t2;\n"), "{out}");
    }
}
