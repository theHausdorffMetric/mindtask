//! Markdown renderers (GitHub-flavoured) over the view models in
//! [`mindtask::report`] — the second output format the binary owns, beside
//! the terminal tables. Each command's output is a standalone document with
//! its own H1, so a page written beside the project file reads on its own
//! and pandoc can turn it into HTML as is.
//!
//! Cells escape `|` and turn newlines into `<br>`; nothing else is escaped —
//! names carry no control characters (validated), and the odd `*` or `_`
//! rendering as emphasis is the usual Markdown trade-off.

use mindtask::model::project::Project;
use mindtask::report::{ConceptNode, Report, ScheduleView, TaskListing, TaskRow};

use super::render::{DescMode, resolve_wrap_width, truncate};
use super::schedule::fmt_days;
use super::state_filter::hidden_footer;
use super::task::{join_ids, row_cells};

/// One table cell: trimmed, `|` escaped, newline runs as `<br>`.
pub(super) fn cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_break = false;
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if pending_break {
            out.push_str("<br>");
        }
        out.push_str(&line.replace('|', "\\|"));
        pending_break = true;
    }
    out
}

/// `[uri](uri)` for each ref, comma-separated; empty when there are none.
fn ref_links(refs: &[String]) -> String {
    refs.iter()
        .map(|r| format!("[{0}]({0})", cell(r)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The description as the `-d` mode asks for it: the lede truncated to
/// `width` for `short`, the whole text for `full`, nothing otherwise.
fn description(desc: Option<&str>, mode: DescMode, width: usize) -> Option<String> {
    let desc = desc?;
    match mode {
        DescMode::None => None,
        DescMode::Short => Some(cell(&truncate(desc, width))),
        DescMode::Full => Some(cell(desc)),
    }
}

/// A GFM table: header, alignment row (`numeric` columns right-aligned),
/// rows. No trailing newline.
fn table(headers: &[&str], numeric: &[usize], rows: &[Vec<String>]) -> String {
    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(format!("| {} |", headers.join(" | ")));
    let align: Vec<&str> = (0..headers.len())
        .map(|i| if numeric.contains(&i) { "---:" } else { "---" })
        .collect();
    lines.push(format!("|{}|", align.join("|")));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| cell(c)).collect();
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    lines.join("\n")
}

/// The concept tree as a nested bullet list, two spaces per level.
fn tree_list(out: &mut String, nodes: &[ConceptNode], depth: usize, mode: DescMode, width: usize) {
    for node in nodes {
        let indent = "  ".repeat(depth);
        out.push_str(&format!("{indent}- {} {{{}}}", cell(&node.name), node.id));
        if let Some(desc) = description(node.description.as_deref(), mode, width) {
            out.push_str(" — ");
            out.push_str(&desc);
        }
        if !node.refs.is_empty() {
            out.push_str(" · ");
            out.push_str(&ref_links(&node.refs));
        }
        out.push('\n');
        tree_list(out, &node.children, depth + 1, mode, width);
    }
}

/// `# Concepts` and the tree — the `concept tree` document.
pub fn concept_tree(nodes: &[ConceptNode], mode: DescMode, width: usize) -> String {
    let mut out = String::from("# Concepts\n\n");
    if nodes.is_empty() {
        out.push_str("_No concepts._\n");
    } else {
        tree_list(&mut out, nodes, 0, mode, width);
    }
    out
}

/// Which optional columns a task table carries.
struct TaskColumns {
    description: bool,
    refs: bool,
}

impl TaskColumns {
    fn for_rows<'a>(rows: impl Iterator<Item = &'a TaskRow>, mode: DescMode) -> Self {
        Self {
            description: mode != DescMode::None,
            refs: rows.into_iter().any(|r| !r.refs.is_empty()),
        }
    }

    fn headers(&self) -> Vec<&'static str> {
        let mut h = vec!["ID", "Name", "State", "Due", "Depends on", "Concepts"];
        if self.description {
            h.push("Description");
        }
        if self.refs {
            h.push("Refs");
        }
        h
    }

    fn cells(&self, row: &TaskRow, mode: DescMode, width: usize) -> Vec<String> {
        let mut cells = row_cells(row);
        cells.push(join_ids(&row.concepts));
        if self.description {
            cells.push(description(row.description.as_deref(), mode, width).unwrap_or_default());
        }
        if self.refs {
            cells.push(ref_links(&row.refs));
        }
        cells
    }
}

/// One task table over `rows`.
fn task_table(rows: &[TaskRow], cols: &TaskColumns, mode: DescMode, width: usize) -> String {
    let body: Vec<Vec<String>> = rows.iter().map(|r| cols.cells(r, mode, width)).collect();
    table(&cols.headers(), &[0], &body)
}

fn footer(out: &mut String, hidden: &mindtask::report::Hidden) {
    if let Some(f) = hidden_footer(hidden) {
        out.push_str(&format!("\n_{f}_\n"));
    }
}

/// `# Project report` — the tree, then one table per concept group.
pub fn report(project: &Project, report: &Report, mode: DescMode) -> String {
    let width = resolve_wrap_width(project);
    let mut out = String::from("# Project report\n\n## Concepts\n\n");
    if report.concepts.is_empty() {
        out.push_str("_No concepts._\n");
    } else {
        tree_list(&mut out, &report.concepts, 0, mode, width);
    }
    out.push_str("\n## Tasks\n\n");
    if project.tasks.is_empty() {
        out.push_str("_No tasks._\n");
        return out;
    }
    if report.groups.is_empty() {
        out.push_str("_No tasks in the selected states._\n");
    } else {
        let cols = TaskColumns::for_rows(report.tasks(), mode);
        for (i, group) in report.groups.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&format!("### {}\n\n", cell(&group.title)));
            out.push_str(&task_table(&group.tasks, &cols, mode, width));
            out.push('\n');
        }
    }
    footer(&mut out, &report.hidden);
    out
}

/// `# Tasks` — the flat table of `task ls`.
pub fn task_listing(project: &Project, listing: &TaskListing, mode: DescMode) -> String {
    let width = resolve_wrap_width(project);
    let mut out = String::from("# Tasks\n\n");
    if project.tasks.is_empty() {
        out.push_str("_No tasks._\n");
        return out;
    }
    if listing.tasks.is_empty() {
        out.push_str("_No tasks in the selected states._\n");
    } else {
        let cols = TaskColumns::for_rows(listing.tasks.iter(), mode);
        out.push_str(&task_table(&listing.tasks, &cols, mode, width));
        out.push('\n');
    }
    footer(&mut out, &listing.hidden);
    out
}

/// `# Schedule` — the CPM table, project duration, critical path.
pub fn schedule(project: &Project, view: &ScheduleView) -> String {
    let mut out = String::from("# Schedule\n\n");
    if project.tasks.is_empty() {
        out.push_str("_No tasks._\n");
        return out;
    }
    if !view.has_durations() {
        out.push_str(
            "_No task durations set — nothing to schedule. \
             Add estimates with `mindtask task edit <ID> --duration <DAYS>`._\n",
        );
        return out;
    }
    let rows: Vec<Vec<String>> = view
        .tasks
        .iter()
        .map(|r| {
            vec![
                r.id.to_string(),
                r.name.clone(),
                fmt_days(r.duration),
                fmt_days(r.earliest_start),
                fmt_days(r.earliest_finish),
                fmt_days(r.slack),
                if r.critical {
                    "✓".to_string()
                } else {
                    String::new()
                },
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "ID", "Name", "Duration", "Start", "Finish", "Slack", "Critical",
        ],
        &[0, 2, 3, 4, 5],
        &rows,
    ));
    out.push_str(&format!(
        "\n\n**Project duration:** {} days\n",
        fmt_days(view.duration)
    ));
    if !view.critical_path.is_empty() {
        let names: Vec<String> = view
            .critical_path
            .iter()
            .filter_map(|id| project.get_task(*id).map(|t| cell(&t.name)))
            .collect();
        out.push_str(&format!("\n**Critical path:** {}\n", names.join(" → ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mindtask::model::id::{ConceptId, TaskId};
    use mindtask::model::task::TaskState;

    #[test]
    fn cell_escapes_pipes_and_breaks_lines() {
        assert_eq!(cell("a | b"), "a \\| b");
        assert_eq!(cell("one\ntwo\n\nthree "), "one<br>two<br>three");
        assert_eq!(cell(""), "");
    }

    #[test]
    fn table_aligns_numeric_columns_right() {
        let t = table(&["ID", "Name"], &[0], &[vec!["1".into(), "x|y".into()]]);
        assert_eq!(t, "| ID | Name |\n|---:|---|\n| 1 | x\\|y |");
    }

    #[test]
    fn tree_list_nests_and_annotates() {
        let nodes = vec![ConceptNode {
            id: ConceptId(1),
            name: "Root".into(),
            description: Some("Top\nlevel".into()),
            refs: vec!["a.md".into()],
            children: vec![ConceptNode {
                id: ConceptId(2),
                name: "Child".into(),
                description: None,
                refs: vec![],
                children: vec![],
            }],
        }];
        let md = concept_tree(&nodes, DescMode::Full, 80);
        assert_eq!(
            md,
            "# Concepts\n\n- Root {1} — Top<br>level · [a.md](a.md)\n  - Child {2}\n"
        );
        let short = concept_tree(&nodes, DescMode::Short, 4);
        assert!(short.contains("— Top…"), "{short}");
        let none = concept_tree(&nodes, DescMode::None, 80);
        assert!(!none.contains("—"), "{none}");
    }

    #[test]
    fn task_columns_appear_only_when_needed() {
        let row = TaskRow {
            id: TaskId(1),
            name: "n".into(),
            state: TaskState::Todo,
            description: Some("d".into()),
            duration: None,
            due: None,
            depends_on: vec![],
            concepts: vec![],
            refs: vec![],
        };
        let plain = TaskColumns::for_rows([&row].into_iter(), DescMode::None);
        assert_eq!(plain.headers().len(), 6);
        assert_eq!(plain.cells(&row, DescMode::None, 80).len(), 6);
        let with_desc = TaskColumns::for_rows([&row].into_iter(), DescMode::Full);
        assert_eq!(with_desc.headers().last(), Some(&"Description"));
        let with_refs = TaskRow {
            refs: vec!["x.md".into()],
            ..row.clone()
        };
        let cols = TaskColumns::for_rows([&with_refs].into_iter(), DescMode::Short);
        assert_eq!(cols.headers().len(), 8);
        assert_eq!(
            cols.cells(&with_refs, DescMode::Short, 80)[7],
            "[x.md](x.md)"
        );
    }
}
