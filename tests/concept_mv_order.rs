//! Integration tests for `mindtask concept mv --before/--after` sibling
//! positioning, driving the real binary and inspecting results via the loader.

use std::path::Path;
use std::process::{Command, Output};

use mindtask::model::id::ConceptId;
use mindtask::model::project::Project;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn mindtask")
}

fn ok(dir: &Path, args: &[&str]) {
    let out = run(dir, args);
    assert!(
        out.status.success(),
        "`{args:?}` failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Fresh project: root 1 → {a=2, b=3, c=4}, plus second root root2=5.
fn built_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(".mindtask.json"),
        r#"{"version":1,"concepts":[],"tasks":[]}"#,
    )
    .unwrap();
    ok(dir.path(), &["concept", "add", "root"]);
    ok(dir.path(), &["concept", "add", "a", "--parent", "1"]);
    ok(dir.path(), &["concept", "add", "b", "--parent", "1"]);
    ok(dir.path(), &["concept", "add", "c", "--parent", "1"]);
    ok(dir.path(), &["concept", "add", "root2"]);
    dir
}

fn load(dir: &Path) -> Project {
    mindtask::store::json::load(&dir.join(".mindtask.json")).unwrap()
}

fn child_order(p: &Project, parent: u64) -> Vec<u64> {
    p.children_of(ConceptId(parent))
        .iter()
        .map(|c| c.id.0)
        .collect()
}

#[test]
fn mv_before_reorders_and_persists() {
    let dir = built_project();
    ok(dir.path(), &["concept", "mv", "4", "--before", "2"]);
    assert_eq!(child_order(&load(dir.path()), 1), vec![4, 2, 3]);
}

#[test]
fn mv_after_reorders_and_persists() {
    let dir = built_project();
    ok(dir.path(), &["concept", "mv", "2", "--after", "3"]);
    assert_eq!(child_order(&load(dir.path()), 1), vec![3, 2, 4]);
}

#[test]
fn mv_before_across_parents_reparents() {
    let dir = built_project();
    // Move a(2) before root2(5): 2 becomes a root, ordered before 5.
    ok(dir.path(), &["concept", "mv", "2", "--before", "5"]);
    let p = load(dir.path());
    assert_eq!(p.get_concept(ConceptId(2)).unwrap().parent, None);
    let roots: Vec<u64> = p.roots().iter().map(|c| c.id.0).collect();
    assert_eq!(roots, vec![1, 2, 5]);
    assert_eq!(child_order(&p, 1), vec![3, 4]);
}

#[test]
fn mv_parent_still_works() {
    // Backward compatibility: the original --parent form is unchanged.
    let dir = built_project();
    ok(dir.path(), &["concept", "mv", "2", "--parent", "root"]);
    assert_eq!(
        load(dir.path()).get_concept(ConceptId(2)).unwrap().parent,
        None
    );
}

#[test]
fn mv_rejects_bad_placements() {
    let dir = built_project();
    // relative to self
    assert!(
        !run(dir.path(), &["concept", "mv", "2", "--before", "2"])
            .status
            .success()
    );
    // missing anchor
    assert!(
        !run(dir.path(), &["concept", "mv", "2", "--after", "99"])
            .status
            .success()
    );
    // conflicting flags (clap)
    assert!(
        !run(
            dir.path(),
            &["concept", "mv", "2", "--before", "3", "--after", "4"]
        )
        .status
        .success()
    );
    // no destination at all
    assert!(!run(dir.path(), &["concept", "mv", "2"]).status.success());
}

#[test]
fn mv_rejects_cycle() {
    let dir = built_project();
    ok(dir.path(), &["concept", "add", "gc", "--parent", "2"]); // 6, grandchild
    // Placing root(1) beside 6 would reparent 1 under 2 — a cycle.
    assert!(
        !run(dir.path(), &["concept", "mv", "1", "--before", "6"])
            .status
            .success()
    );
}
