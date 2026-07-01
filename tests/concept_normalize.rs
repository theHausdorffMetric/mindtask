//! Integration tests for `mindtask concept normalize`.
//!
//! These drive the real binary against a deliberately non-canonical
//! `.mindtask.json` (gappy IDs, a subtree stored out of tree order, a task with
//! concept links) and assert the on-disk result via the public loader.

use std::path::Path;
use std::process::{Command, Output};

use mindtask::model::id::ConceptId;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Tree: 5(root) → { 2 → {9}, 7 }, stored as [5, 2, 7, 9] so DFS pre-order
/// ([5, 2, 9, 7]) differs from the array. Task 1 links concepts 9 and 5.
const SCRAMBLED: &str = r#"{
  "version": 1,
  "concepts": [
    { "id": 5, "name": "c5" },
    { "id": 2, "name": "c2", "parent": 5 },
    { "id": 7, "name": "c7", "parent": 5 },
    { "id": 9, "name": "c9", "parent": 2 }
  ],
  "tasks": [
    { "id": 1, "name": "t", "concepts": [9, 5] }
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
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn normalize_renumbers_file_to_dfs_order() {
    let dir = project_dir(SCRAMBLED);
    let out = run(dir.path(), &["concept", "normalize"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let proj = mindtask::store::json::load(&dir.path().join(".mindtask.json")).unwrap();
    let concepts: Vec<(u64, &str, Option<u64>)> = proj
        .concepts
        .iter()
        .map(|c| (c.id.0, c.name.as_str(), c.parent.map(|p| p.0)))
        .collect();
    // IDs 1..4 in DFS pre-order; parents remapped in step.
    assert_eq!(
        concepts,
        vec![
            (1, "c5", None),
            (2, "c2", Some(1)),
            (3, "c9", Some(2)),
            (4, "c7", Some(1)),
        ]
    );
    // Task links remapped 9→3, 5→1 (order preserved).
    assert_eq!(proj.tasks[0].concepts, vec![ConceptId(3), ConceptId(1)]);

    // The rewritten file passes validation.
    assert!(run(dir.path(), &["validate"]).status.success());
}

#[test]
fn normalize_dry_run_leaves_file_untouched() {
    let dir = project_dir(SCRAMBLED);
    let path = dir.path().join(".mindtask.json");
    let before = std::fs::read_to_string(&path).unwrap();

    let out = run(dir.path(), &["concept", "normalize", "--dry-run"]);
    assert!(out.status.success());
    assert_eq!(
        before,
        std::fs::read_to_string(&path).unwrap(),
        "dry-run must not modify the file"
    );

    let s = stdout(&out);
    assert!(s.contains("dry run"), "stdout: {s}");
    assert!(s.contains("5 -> 1"), "expected remap line, stdout: {s}");
}

#[test]
fn normalize_is_noop_on_canonical_file() {
    // A project built via the normal API is already canonical.
    let dir = project_dir(r#"{"version":1,"concepts":[],"tasks":[]}"#);
    assert!(
        run(dir.path(), &["concept", "add", "root"])
            .status
            .success()
    );
    assert!(
        run(dir.path(), &["concept", "add", "child", "--parent", "1"])
            .status
            .success()
    );
    let path = dir.path().join(".mindtask.json");
    let before = std::fs::read_to_string(&path).unwrap();

    let out = run(dir.path(), &["concept", "normalize"]);
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("Already normalized"),
        "stdout: {}",
        stdout(&out)
    );
    assert_eq!(before, std::fs::read_to_string(&path).unwrap());
}
