//! Integration tests for the load-time validation guard.
//!
//! These drive the real `mindtask` binary against hand-written (malformed)
//! `.mindtask.json` files to confirm that operational commands refuse to run on
//! invalid data, that `validate` reports the specific problem, and—critically—
//! that a cyclic concept tree terminates instead of hanging.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Write `contents` to `.mindtask.json` in a fresh temp dir and return the dir.
fn project_dir(contents: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".mindtask.json"), contents).unwrap();
    dir
}

/// Run the binary with `args` in `dir`.
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn mindtask")
}

const DUP_CONCEPT: &str = r#"{
  "version": 1,
  "concepts": [
    { "id": 1, "name": "A" },
    { "id": 1, "name": "B" }
  ],
  "tasks": []
}"#;

const CYCLIC_TREE: &str = r#"{
  "version": 1,
  "concepts": [
    { "id": 1, "name": "A", "parent": 2 },
    { "id": 2, "name": "B", "parent": 1 }
  ],
  "tasks": []
}"#;

const DANGLING_DEP: &str = r#"{
  "version": 1,
  "concepts": [],
  "tasks": [
    { "id": 1, "name": "T", "depends_on": [99] }
  ]
}"#;

#[test]
fn operational_command_rejects_duplicate_concept_id() {
    let dir = project_dir(DUP_CONCEPT);
    let out = run(dir.path(), &["concept", "ls"]);
    assert!(!out.status.success(), "expected failure on duplicate IDs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("duplicate concept ID 1"), "stderr: {stderr}");
}

#[test]
fn validate_reports_duplicate_concept_id() {
    let dir = project_dir(DUP_CONCEPT);
    let out = run(dir.path(), &["validate"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("duplicate concept ID 1"), "stderr: {stderr}");
}

#[test]
fn cyclic_tree_terminates_and_is_rejected() {
    // The key property: this must not hang. The test harness imposes its own
    // timeout, but pre-guard this would loop forever in is_ancestor/build_tree.
    let dir = project_dir(CYCLIC_TREE);
    let out = run(dir.path(), &["concept", "tree"]);
    assert!(!out.status.success(), "expected failure on cyclic tree");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cycle"), "stderr: {stderr}");
}

#[test]
fn dangling_dependency_is_rejected() {
    let dir = project_dir(DANGLING_DEP);
    let out = run(dir.path(), &["task", "ls"]);
    assert!(!out.status.success(), "expected failure on dangling dep");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("non-existent task 99"), "stderr: {stderr}");
}

#[test]
fn task_add_with_concept_links_at_creation() {
    let dir = project_dir(
        r#"{
          "version": 1,
          "concepts": [{ "id": 1, "name": "A" }, { "id": 2, "name": "B" }],
          "tasks": []
        }"#,
    );
    let out = run(
        dir.path(),
        &["task", "add", "Work", "--concept", "1", "--concept", "2"],
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    // The new task should be linked to both concepts on disk.
    let show = run(dir.path(), &["task", "show", "1"]);
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(stdout.contains("1 (A)"), "stdout: {stdout}");
    assert!(stdout.contains("2 (B)"), "stdout: {stdout}");
}

#[test]
fn task_add_with_unknown_concept_is_rejected_and_creates_nothing() {
    let dir = project_dir(
        r#"{
          "version": 1,
          "concepts": [{ "id": 1, "name": "A" }],
          "tasks": []
        }"#,
    );
    let out = run(dir.path(), &["task", "add", "Work", "--concept", "99"]);
    assert!(!out.status.success(), "expected failure on unknown concept");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("concept 99 not found"), "stderr: {stderr}");

    // No task should have been persisted.
    let list = run(dir.path(), &["task", "ls"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("No tasks."), "stdout: {stdout}");
}

#[test]
fn report_shows_concept_tree_then_task_list() {
    let dir = project_dir(
        r#"{
          "version": 1,
          "concepts": [
            { "id": 1, "name": "Backend" },
            { "id": 2, "name": "API", "parent": 1 },
            { "id": 3, "name": "Frontend" }
          ],
          "tasks": [
            { "id": 1, "name": "Design API", "concepts": [2] },
            { "id": 2, "name": "Build API", "depends_on": [1], "concepts": [2] },
            { "id": 3, "name": "Build UI", "concepts": [3] }
          ]
        }"#,
    );
    let out = run(dir.path(), &["report"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Concept tree section (termtree renders the hierarchy with IDs).
    assert!(stdout.contains("Backend {1}"), "stdout: {stdout}");
    assert!(stdout.contains("API {2}"), "stdout: {stdout}");
    assert!(stdout.contains("Frontend {3}"), "stdout: {stdout}");

    // Task list section follows, with the table header and all tasks.
    assert!(stdout.contains("DEPENDS ON"), "stdout: {stdout}");
    assert!(stdout.contains("Design API"), "stdout: {stdout}");
    assert!(stdout.contains("Build UI"), "stdout: {stdout}");

    // Tree must come before the task table.
    let tree_pos = stdout.find("Backend {1}").unwrap();
    let list_pos = stdout.find("DEPENDS ON").unwrap();
    assert!(tree_pos < list_pos, "tree should precede task list:\n{stdout}");
}

#[test]
fn concept_tree_descriptions_flag_shows_descriptions_indented() {
    let dir = project_dir(
        r#"{
          "version": 1,
          "concepts": [
            { "id": 1, "name": "Backend", "description": "Core services" },
            { "id": 2, "name": "API", "parent": 1, "description": "HTTP layer" },
            { "id": 3, "name": "Database", "parent": 1 }
          ],
          "tasks": []
        }"#,
    );

    // Default tree output omits descriptions entirely.
    let plain = run(dir.path(), &["concept", "tree"]);
    assert!(plain.status.success(), "stderr: {}", String::from_utf8_lossy(&plain.stderr));
    let plain_out = String::from_utf8_lossy(&plain.stdout);
    assert!(plain_out.contains("Backend {1}"), "stdout: {plain_out}");
    assert!(!plain_out.contains("Core services"), "default tree leaked description: {plain_out}");

    // With -d, the description rides on the line below its concept, indented to
    // line up with the branch (termtree's multiline skip glyphs).
    let desc = run(dir.path(), &["concept", "tree", "-d"]);
    assert!(desc.status.success(), "stderr: {}", String::from_utf8_lossy(&desc.stderr));
    let desc_out = String::from_utf8_lossy(&desc.stdout);
    assert!(desc_out.contains("[Core services]"), "stdout: {desc_out}");
    // API is a non-last child, so its description line is prefixed with the
    // vertical-continuation glyph.
    assert!(desc_out.contains("\u{2502}   [HTTP layer]"), "stdout: {desc_out}");
    // A concept without a description gets no extra line.
    assert!(!desc_out.contains("Database {3}\n["), "stdout: {desc_out}");
}

#[test]
fn file_flag_selects_an_explicit_project_path() {
    // The project lives under a name that the upward `.mindtask.json` search
    // would never find; `--file` must still load it.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("custom.json");
    std::fs::write(
        &path,
        r#"{
          "version": 1,
          "concepts": [ { "id": 1, "name": "Solo" } ],
          "tasks": []
        }"#,
    )
    .unwrap();

    // Run from an unrelated cwd so discovery can't accidentally succeed.
    let other = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["--file", path.to_str().unwrap(), "concept", "ls"])
        .current_dir(other.path())
        .output()
        .expect("failed to spawn mindtask");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Solo"), "stdout: {}", String::from_utf8_lossy(&out.stdout));

    // A non-existent override path is a clean error, not a fallback search.
    let missing = run(other.path(), &["--file", "/no/such/file.json", "concept", "ls"]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("project file not found"),
        "stderr: {}",
        String::from_utf8_lossy(&missing.stderr)
    );
}

#[test]
fn report_on_empty_project_succeeds() {
    let dir = project_dir(r#"{ "version": 1, "concepts": [], "tasks": [] }"#);
    let out = run(dir.path(), &["report"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No concepts."), "stdout: {stdout}");
    assert!(stdout.contains("No tasks."), "stdout: {stdout}");
}

#[test]
fn invalid_timezone_is_rejected() {
    let dir = project_dir(
        r#"{ "version": 1, "timezone": "Bogus/Zone", "concepts": [], "tasks": [] }"#,
    );
    // validate reports it...
    let out = run(dir.path(), &["validate"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid timezone 'Bogus/Zone'"), "stderr: {stderr}");

    // ...and the load-time guard blocks operational commands too.
    let out = run(dir.path(), &["task", "ls"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid timezone 'Bogus/Zone'"), "stderr: {stderr}");
}

#[test]
fn valid_project_loads_and_lists() {
    let dir = project_dir(
        r#"{
          "version": 1,
          "concepts": [{ "id": 1, "name": "A" }],
          "tasks": [{ "id": 1, "name": "T", "concepts": [1] }]
        }"#,
    );
    let out = run(dir.path(), &["validate"]);
    assert!(out.status.success(), "valid project should pass validate");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("valid"), "stdout: {stdout}");
}
