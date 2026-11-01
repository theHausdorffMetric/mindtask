//! Integration tests for `--format` on the listing commands: `report`,
//! `task ls`, `concept tree`, `schedule`. The text form is covered by the
//! other binary tests; these check that the JSON form carries the same data
//! in the documented shape, and that text stays the default.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Root {1} ─┬─ Beta {3} ── Leaf {4}
///           └─ Alpha {2}
///
/// Task 8 is done (hidden by default); task 2 depends on 1; durations on 1
/// and 2 so the schedule has a critical path; task 1 carries a ref and a due
/// date in New York time while the project is on Zurich time.
const PROJECT: &str = r#"{
  "version": 2,
  "timezone": "Europe/Zurich",
  "wrap_width": 100,
  "concepts": [
    { "id": 1, "name": "Root", "description": "The top." },
    { "id": 3, "name": "Beta", "parent": 1 },
    { "id": 2, "name": "Alpha", "parent": 1, "refs": ["notes/alpha.md"] },
    { "id": 4, "name": "Leaf", "parent": 3 }
  ],
  "tasks": [
    { "id": 1, "name": "design", "state": "done", "duration": 2,
      "due": "2025-04-01T09:00:00-04:00[America/New_York]",
      "concepts": [2], "refs": ["notes/design.md#plan"] },
    { "id": 2, "name": "build | ship", "state": "in_progress", "duration": 3,
      "depends_on": [1], "concepts": [4, 2],
      "description": "Two lines.\nSecond one." },
    { "id": 3, "name": "loose", "state": "todo" },
    { "id": 8, "name": "old", "state": "done", "concepts": [3] }
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

fn json(dir: &Path, args: &[&str]) -> Value {
    let out = stdout(dir, args);
    assert!(out.ends_with('\n'), "JSON output ends with a newline");
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("not JSON ({e}):\n{out}"))
}

#[test]
fn report_json_carries_tree_groups_and_hidden() {
    let dir = project_dir(PROJECT);
    let v = json(dir.path(), &["report", "--format", "json"]);

    assert_eq!(v["timezone"], "Europe/Zurich");
    assert_eq!(v["states"], serde_json::json!(["todo", "in_progress"]));

    // The tree nests and keeps sibling order (Beta before Alpha, as stored).
    let root = &v["concepts"][0];
    assert_eq!(root["name"], "Root");
    assert_eq!(root["description"], "The top.");
    assert_eq!(root["children"][0]["name"], "Beta");
    assert_eq!(root["children"][0]["children"][0]["id"], 4);
    assert_eq!(
        root["children"][1]["refs"],
        serde_json::json!(["notes/alpha.md"])
    );
    assert!(root.get("refs").is_none(), "empty refs are omitted");

    // Groups: task 2's primary concept is Leaf {4}; task 3 is unlinked.
    let groups = v["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["concept"], 4);
    assert_eq!(groups[0]["title"], "Root › Beta › Leaf {4}");
    let t2 = &groups[0]["tasks"][0];
    assert_eq!(t2["id"], 2);
    assert_eq!(t2["name"], "build | ship");
    assert_eq!(t2["state"], "in_progress");
    assert_eq!(t2["duration"], 3.0);
    assert_eq!(t2["depends_on"], serde_json::json!([1]));
    assert_eq!(t2["concepts"], serde_json::json!([4, 2]));
    assert_eq!(t2["description"], "Two lines.\nSecond one.");
    assert!(groups[1]["concept"].is_null());
    assert_eq!(groups[1]["title"], "(no concept)");
    assert_eq!(groups[1]["tasks"][0]["id"], 3);

    // Hidden: the two done tasks.
    assert_eq!(
        v["hidden"],
        serde_json::json!({"todo": 0, "in_progress": 0, "done": 2})
    );
}

#[test]
fn report_json_ignores_description_mode_and_honours_state() {
    let dir = project_dir(PROJECT);
    let plain = json(
        dir.path(),
        &["report", "--format", "json", "--state", "all"],
    );
    let short = json(
        dir.path(),
        &["report", "--format", "json", "--state", "all", "-d=short"],
    );
    assert_eq!(plain, short, "-d does not change the JSON");
    assert_eq!(
        plain["states"],
        serde_json::json!(["todo", "in_progress", "done"])
    );
    assert_eq!(plain["hidden"]["done"], 0);
    let ids: Vec<u64> = plain["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["tasks"].as_array().unwrap())
        .map(|t| t["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [8, 2, 1, 3], "tree order: Beta, Leaf, Alpha, unlinked");
}

#[test]
fn due_dates_are_converted_to_the_project_timezone() {
    let dir = project_dir(PROJECT);
    let v = json(
        dir.path(),
        &["task", "ls", "--format", "json", "--state", "done"],
    );
    let design = &v["tasks"][0];
    assert_eq!(design["id"], 1);
    assert_eq!(
        design["due"], "2025-04-01T15:00:00+02:00[Europe/Zurich]",
        "09:00 New York is 15:00 Zurich"
    );
    assert_eq!(design["refs"], serde_json::json!(["notes/design.md#plan"]));
}

#[test]
fn task_ls_json_is_flat_and_counts_hidden() {
    let dir = project_dir(PROJECT);
    let v = json(dir.path(), &["task", "ls", "--format", "json"]);
    let ids: Vec<u64> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [2, 3]);
    assert_eq!(v["hidden"]["done"], 2);
    assert!(v.get("groups").is_none());
}

#[test]
fn concept_tree_json_takes_a_root_and_rejects_unknown() {
    let dir = project_dir(PROJECT);
    let v = json(dir.path(), &["concept", "tree", "3", "--format", "json"]);
    assert_eq!(v["concepts"].as_array().unwrap().len(), 1);
    assert_eq!(v["concepts"][0]["name"], "Beta");
    assert_eq!(v["concepts"][0]["children"][0]["name"], "Leaf");

    let out = run(dir.path(), &["concept", "tree", "42", "--format", "json"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("concept 42 not found"));
}

#[test]
fn schedule_json_has_rows_and_critical_path() {
    let dir = project_dir(PROJECT);
    let v = json(dir.path(), &["schedule", "--format", "json"]);
    assert_eq!(v["duration"], 5.0);
    assert_eq!(v["critical_path"], serde_json::json!([1, 2]));
    let rows = v["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let build = rows.iter().find(|r| r["id"] == 2).unwrap();
    assert_eq!(build["earliest_start"], 2.0);
    assert_eq!(build["earliest_finish"], 5.0);
    assert_eq!(build["slack"], 0.0);
    assert_eq!(build["critical"], true);

    let narrowed = json(dir.path(), &["schedule", "--critical", "--format", "json"]);
    assert_eq!(narrowed["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(narrowed["critical_path"], serde_json::json!([1, 2]));
}

#[test]
fn json_is_structured_where_text_prints_a_sentence() {
    let empty = project_dir(r#"{ "version": 2, "concepts": [], "tasks": [] }"#);
    assert_eq!(stdout(empty.path(), &["task", "ls"]), "No tasks.\n");
    let v = json(empty.path(), &["task", "ls", "--format", "json"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    let v = json(empty.path(), &["schedule", "--format", "json"]);
    assert_eq!(v["duration"], 0.0);
    assert_eq!(v["tasks"], serde_json::json!([]));
    let v = json(empty.path(), &["report", "--format", "json"]);
    assert_eq!(v["concepts"], serde_json::json!([]));
    assert_eq!(v["groups"], serde_json::json!([]));
}

#[test]
fn text_is_the_default_and_format_is_validated() {
    let dir = project_dir(PROJECT);
    let default = stdout(dir.path(), &["task", "ls"]);
    let explicit = stdout(dir.path(), &["task", "ls", "--format", "text"]);
    assert_eq!(default, explicit);
    assert!(default.starts_with("ID "), "a text table:\n{default}");

    let out = run(dir.path(), &["task", "ls", "--format", "yaml"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("yaml"), "{err}");
    assert!(err.contains("json"), "{err}");
}
