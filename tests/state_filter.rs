//! Integration tests for the `--state` filter on the task-listing commands.
//!
//! `report`, `task ls`, and `concept report` default to `todo,in_progress`;
//! `--state` overrides that (comma-separated or repeated, `all` = every
//! state), and a footer line accounts for whatever the filter hid.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Three concept areas; six tasks covering every state, with dependency
/// chains that cross the filter: build-core (in_progress) depends on
/// research-notes (done), and old-migration (done) depends on draft-plan
/// (todo) — the latter chain is only reachable through a done task.
const PROJECT: &str = r#"{
  "version": 1,
  "concepts": [
    { "id": 1, "name": "Area" },
    { "id": 2, "name": "Other" },
    { "id": 3, "name": "DoneArea" },
    { "id": 4, "name": "Empty" }
  ],
  "tasks": [
    { "id": 1, "name": "write-spec", "state": "todo", "concepts": [1] },
    { "id": 2, "name": "build-core", "state": "in_progress", "depends_on": [3], "concepts": [1] },
    { "id": 3, "name": "research-notes", "state": "done", "concepts": [2] },
    { "id": 4, "name": "old-migration", "state": "done", "depends_on": [5], "concepts": [1] },
    { "id": 5, "name": "draft-plan", "state": "todo", "concepts": [2] },
    { "id": 6, "name": "shipped-feature", "state": "done", "concepts": [3] }
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

fn stdout(out: &Output) -> String {
    assert!(
        out.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[test]
fn task_ls_default_hides_done() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["task", "ls"]));
    for name in ["write-spec", "build-core", "draft-plan"] {
        assert!(out.contains(name), "missing {name} in:\n{out}");
    }
    for name in ["research-notes", "old-migration", "shipped-feature"] {
        assert!(!out.contains(name), "unexpected {name} in:\n{out}");
    }
    assert!(
        out.contains("(hidden: 3 done — --state all to show)"),
        "missing footer in:\n{out}"
    );
}

#[test]
fn task_ls_state_all_shows_everything() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["task", "ls", "--state", "all"]));
    for name in [
        "write-spec",
        "build-core",
        "research-notes",
        "old-migration",
        "draft-plan",
        "shipped-feature",
    ] {
        assert!(out.contains(name), "missing {name} in:\n{out}");
    }
    assert!(!out.contains("(hidden:"), "unexpected footer in:\n{out}");
}

#[test]
fn task_ls_state_done_inverts_the_filter() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["task", "ls", "--state", "done"]));
    for name in ["research-notes", "old-migration", "shipped-feature"] {
        assert!(out.contains(name), "missing {name} in:\n{out}");
    }
    for name in ["write-spec", "build-core", "draft-plan"] {
        assert!(!out.contains(name), "unexpected {name} in:\n{out}");
    }
    assert!(
        out.contains("(hidden: 2 todo, 1 in_progress — --state all to show)"),
        "missing footer in:\n{out}"
    );
}

#[test]
fn comma_and_repeated_forms_agree() {
    let dir = project_dir(PROJECT);
    let comma = stdout(&run(dir.path(), &["task", "ls", "--state", "todo,done"]));
    let repeated = stdout(&run(
        dir.path(),
        &["task", "ls", "--state", "todo", "--state", "done"],
    ));
    assert_eq!(comma, repeated);
    assert!(comma.contains("shipped-feature"));
    assert!(!comma.contains("build-core"));
}

#[test]
fn invalid_state_errors_and_names_all() {
    let dir = project_dir(PROJECT);
    let out = run(dir.path(), &["task", "ls", "--state", "bogus"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("'todo', 'in_progress', 'done', or 'all'"),
        "unhelpful error:\n{stderr}"
    );
}

#[test]
fn hidden_deps_keep_their_ids_in_depends_on() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["task", "ls"]));
    // build-core depends on hidden done task 3; the ID must stay verbatim.
    let row = out
        .lines()
        .find(|l| l.contains("build-core"))
        .expect("build-core row");
    assert!(row.contains('3'), "dep ID dropped from row: {row}");
}

#[test]
fn report_applies_the_default_filter() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["report"]));
    assert!(out.contains("Area {1}"), "concept tree missing:\n{out}");
    assert!(out.contains("write-spec"));
    assert!(!out.contains("shipped-feature"));
    assert!(out.contains("(hidden: 3 done — --state all to show)"));
}

#[test]
fn concept_report_filters_direct_and_upstream() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["concept", "report", "1"]));
    assert!(out.contains("write-spec"));
    assert!(out.contains("build-core"));
    // Done tasks are hidden, and draft-plan is only reachable through the
    // hidden old-migration, so it must not appear either.
    for name in ["research-notes", "old-migration", "draft-plan"] {
        assert!(!out.contains(name), "unexpected {name} in:\n{out}");
    }
    assert!(
        out.contains("(hidden: 1 todo, 2 done — --state all to show)"),
        "missing footer in:\n{out}"
    );
}

#[test]
fn concept_report_state_all_matches_old_behavior() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(
        dir.path(),
        &["concept", "report", "1", "--state", "all"],
    ));
    for name in [
        "write-spec",
        "build-core",
        "research-notes",
        "old-migration",
        "draft-plan",
    ] {
        assert!(out.contains(name), "missing {name} in:\n{out}");
    }
    assert!(out.contains("(upstream dep)"));
    assert!(out.contains("3 direct + 2 upstream = 5 total tasks"));
    assert!(!out.contains("(hidden:"));
}

#[test]
fn concept_report_all_rows_filtered_out() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["concept", "report", "3"]));
    assert!(out.contains("No tasks in the selected states."));
    assert!(out.contains("(hidden: 1 done — --state all to show)"));
}

#[test]
fn concept_report_without_tasks_is_unchanged() {
    let dir = project_dir(PROJECT);
    let out = stdout(&run(dir.path(), &["concept", "report", "4"]));
    assert!(out.contains("No tasks linked to this concept subtree."));
    assert!(!out.contains("(hidden:"));
}

#[test]
fn task_ls_empty_project_is_unchanged() {
    let dir = project_dir(r#"{ "version": 1, "concepts": [], "tasks": [] }"#);
    let out = stdout(&run(dir.path(), &["task", "ls"]));
    assert!(out.contains("No tasks."));
    assert!(!out.contains("(hidden:"));
}
