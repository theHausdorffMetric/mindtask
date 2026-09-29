//! Integration tests for the grouped task table of the top-level `report`.
//!
//! `report` prints the concept tree and then one task table whose rows are
//! grouped by each task's primary (first-listed) concept, the groups in the
//! order the tree prints, rows by ID within a group. `task ls` stays the flat
//! ID-ordered list. These drive the real binary against a fixture whose tree
//! order deliberately differs from its ID order.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Group-title prefix, as `cli::render::GROUP_MARK`.
const MARK: &str = "── ";

/// Root {1} ─┬─ Beta {3} ── Leaf {4}
///           ├─ Alpha {2}
///           └─ Empty {5}
///
/// Beta precedes Alpha in the array (so tree order ≠ ID order); the task
/// array is scrambled (7 before 3); task 5 lists two concepts; task 6 none;
/// task 8 is done and hidden by default.
const PROJECT: &str = r#"{
  "version": 2,
  "wrap_width": 100,
  "concepts": [
    { "id": 1, "name": "Root" },
    { "id": 3, "name": "Beta", "parent": 1 },
    { "id": 2, "name": "Alpha", "parent": 1 },
    { "id": 4, "name": "Leaf", "parent": 3 },
    { "id": 5, "name": "Empty", "parent": 1 }
  ],
  "tasks": [
    { "id": 1, "name": "alpha-task", "state": "todo", "concepts": [2] },
    { "id": 2, "name": "leaf-task", "state": "todo", "concepts": [4] },
    { "id": 7, "name": "beta-late", "state": "todo", "concepts": [3] },
    { "id": 3, "name": "beta-task", "state": "in_progress", "concepts": [3] },
    { "id": 4, "name": "root-task", "state": "todo", "concepts": [1] },
    { "id": 5, "name": "shared-task", "state": "todo", "concepts": [2, 3],
      "description": "Lives in Alpha, also tagged Beta." },
    { "id": 6, "name": "loose-task", "state": "todo" },
    { "id": 8, "name": "done-task", "state": "done", "concepts": [3] }
  ]
}"#;

/// A two-level tree with names long enough that the breadcrumb overflows a
/// narrow wrap width.
const NARROW: &str = r#"{
  "version": 2,
  "wrap_width": 24,
  "concepts": [
    { "id": 1, "name": "Infrastructure and operations" },
    { "id": 2, "name": "Networking equipment", "parent": 1 }
  ],
  "tasks": [
    { "id": 1, "name": "patch", "state": "todo", "concepts": [2] }
  ]
}"#;

fn project_dir(contents: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".mindtask.json"), contents).unwrap();
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn mindtask")
}

fn stdout(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(
        out.status.success(),
        "`{}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// The task half of a report: everything from the table header on.
fn task_section(out: &str) -> &str {
    let start = out.find("\nID ").expect("no task table header") + 1;
    &out[start..]
}

fn group_titles(section: &str) -> Vec<&str> {
    section.lines().filter(|l| l.starts_with(MARK)).collect()
}

fn line_index(section: &str, needle: &str) -> usize {
    section
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line containing {needle:?} in:\n{section}"))
}

#[test]
fn groups_follow_tree_order_not_id_order() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    assert_eq!(
        group_titles(section),
        [
            "── Root {1}",
            "── Root › Beta {3}",
            "── Root › Beta › Leaf {4}",
            "── Root › Alpha {2}",
            "── (no concept)",
        ],
        "in:\n{section}"
    );
}

#[test]
fn rows_ascend_by_id_within_a_group() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    let beta = line_index(section, "── Root › Beta {3}");
    let leaf = line_index(section, "── Root › Beta › Leaf {4}");
    let t3 = line_index(section, "beta-task");
    let t7 = line_index(section, "beta-late");
    // The array lists 7 before 3; the group must not.
    assert!(beta < t3 && t3 < t7 && t7 < leaf, "in:\n{section}");
}

#[test]
fn each_task_appears_once_under_its_first_concept() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    let rows: Vec<&str> = section
        .lines()
        .filter(|l| l.contains("shared-task"))
        .collect();
    assert_eq!(rows.len(), 1, "in:\n{section}");
    // Under Alpha (its first concept), not Beta — but CONCEPTS still lists both.
    let alpha = line_index(section, "── Root › Alpha {2}");
    let shared = line_index(section, "shared-task");
    let unlinked = line_index(section, "── (no concept)");
    assert!(alpha < shared && shared < unlinked, "in:\n{section}");
    assert!(
        rows[0].ends_with("2, 3"),
        "CONCEPTS cell lost a concept: {}",
        rows[0]
    );
}

#[test]
fn concepts_without_visible_tasks_get_no_group() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    // The tree half still shows Empty; the table half must not.
    assert!(out.contains("Empty {5}"), "tree lost the concept:\n{out}");
    let section = task_section(&out);
    assert!(
        group_titles(section).iter().all(|t| !t.contains("Empty")),
        "in:\n{section}"
    );
}

#[test]
fn unlinked_tasks_close_the_table() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    let titles = group_titles(section);
    assert_eq!(titles.last(), Some(&"── (no concept)"), "in:\n{section}");
    let unlinked = line_index(section, "── (no concept)");
    let loose = line_index(section, "loose-task");
    assert!(unlinked < loose, "in:\n{section}");
    // No group title follows the unlinked group's rows.
    assert!(
        section.lines().skip(loose).all(|l| !l.starts_with(MARK)),
        "in:\n{section}"
    );
}

#[test]
fn one_header_row_and_aligned_columns_across_groups() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    let headers: Vec<&str> = section.lines().filter(|l| l.starts_with("ID ")).collect();
    assert_eq!(headers.len(), 1, "in:\n{section}");
    let state_col = headers[0].find("STATE").unwrap();
    // Every task row (they start with a digit) has its STATE cell in the
    // header's STATE column, whichever group it sits in.
    let rows: Vec<&str> = section
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    assert_eq!(rows.len(), 7, "in:\n{section}");
    for row in rows {
        let cell = &row[state_col..];
        assert!(
            cell.starts_with("todo") || cell.starts_with("in_progress"),
            "misaligned row: {row}"
        );
    }
}

#[test]
fn hidden_footer_and_state_filter_still_apply() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report"]);
    assert!(!out.contains("done-task"), "done row leaked:\n{out}");
    assert!(
        out.contains("(hidden: 1 done — --state all to show)"),
        "missing footer in:\n{out}"
    );

    let all = stdout(dir.path(), &["report", "--state", "all"]);
    let section = task_section(&all);
    let beta = line_index(section, "── Root › Beta {3}");
    let done = line_index(section, "done-task");
    let leaf = line_index(section, "── Root › Beta › Leaf {4}");
    assert!(beta < done && done < leaf, "in:\n{section}");
    assert!(!all.contains("(hidden:"), "unexpected footer in:\n{all}");

    let only_done = stdout(dir.path(), &["report", "--state", "done"]);
    let section = task_section(&only_done);
    assert_eq!(
        group_titles(section),
        ["── Root › Beta {3}"],
        "in:\n{section}"
    );
    assert!(only_done.contains("(hidden: 6 todo, 1 in_progress — --state all to show)"));
}

#[test]
fn all_rows_filtered_out_keeps_the_plain_message() {
    let dir = project_dir(NARROW);
    let out = stdout(dir.path(), &["report", "--state", "done"]);
    assert!(
        out.contains("No tasks in the selected states."),
        "in:\n{out}"
    );
    // Tree branches (`├── `) contain the mark mid-line; a title starts with it.
    assert!(
        out.lines().all(|l| !l.starts_with(MARK)),
        "no group should print:\n{out}"
    );
    assert!(out.contains("(hidden: 1 todo — --state all to show)"));
}

#[test]
fn descriptions_sit_beneath_their_grouped_row() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["report", "-d"]);
    let section = task_section(&out);
    let lines: Vec<&str> = section.lines().collect();
    let shared = line_index(section, "shared-task");
    assert!(
        lines[shared + 1].starts_with("     [Lives in Alpha"),
        "block not beneath its row:\n{section}"
    );
    // Grouping is unaffected by -d.
    assert_eq!(group_titles(section).len(), 5, "in:\n{section}");
}

#[test]
fn group_titles_are_truncated_to_the_wrap_width() {
    let dir = project_dir(NARROW);
    let out = stdout(dir.path(), &["report"]);
    let section = task_section(&out);
    let titles = group_titles(section);
    assert_eq!(titles.len(), 1, "in:\n{section}");
    assert_eq!(titles[0].chars().count(), 24, "got: {}", titles[0]);
    assert!(titles[0].ends_with('…'), "got: {}", titles[0]);
}

#[test]
fn task_ls_stays_flat_and_in_array_order() {
    let dir = project_dir(PROJECT);
    let out = stdout(dir.path(), &["task", "ls"]);
    assert!(
        out.lines().all(|l| !l.starts_with(MARK)),
        "task ls grew group titles:\n{out}"
    );
    let alpha = line_index(&out, "alpha-task");
    let late = line_index(&out, "beta-late");
    let beta = line_index(&out, "beta-task");
    assert!(alpha < late && late < beta, "in:\n{out}");
}
