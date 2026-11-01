//! Integration tests for `--format md`: each listing is a standalone
//! GitHub-flavoured Markdown document — H1, nested list for the tree, one
//! GFM table per group, cells escaped, the `--state` footer in italics.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// Same fixture as tests/output_formats.rs: task 2's name carries a pipe,
/// its description two lines; task 1 has a ref and a due date.
const PROJECT: &str = r#"{
  "version": 2,
  "timezone": "Europe/Zurich",
  "wrap_width": 40,
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
      "description": "Two lines.\nSecond one, which is rather long for a lede." },
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

#[test]
fn report_md_is_a_document_with_tree_and_grouped_tables() {
    let dir = project_dir(PROJECT);
    let md = stdout(dir.path(), &["report", "--format", "md"]);
    assert!(
        md.starts_with("# Project report\n\n## Concepts\n\n- Root {1}\n"),
        "{md}"
    );
    assert!(
        md.contains(
            "  - Beta {3}\n    - Leaf {4}\n  - Alpha {2} · [notes/alpha.md](notes/alpha.md)\n"
        ),
        "{md}"
    );
    assert!(md.contains("\n## Tasks\n\n### Root › Beta › Leaf {4}\n\n| ID | Name | State | Due | Depends on | Concepts |\n|---:|---|---|---|---|---|\n| 2 | build \\| ship | in_progress | - | 1 | 4, 2 |\n"), "{md}");
    assert!(md.contains("\n### (no concept)\n\n| ID | Name |"), "{md}");
    assert!(
        md.ends_with("\n_(hidden: 2 done — --state all to show)_\n"),
        "{md}"
    );
    // No description column without -d; the done task's ref is hidden with it.
    assert!(!md.contains("Description"), "{md}");
    assert!(!md.contains("| Refs |"), "{md}");
}

#[test]
fn descriptions_and_refs_become_columns() {
    let dir = project_dir(PROJECT);
    let md = stdout(
        dir.path(),
        &["report", "--format", "md", "--state", "all", "-d"],
    );
    assert!(md.contains("- Root {1} — The top.\n"), "{md}");
    assert!(
        md.contains("| ID | Name | State | Due | Depends on | Concepts | Description | Refs |\n"),
        "{md}"
    );
    assert!(
        md.contains("| Two lines.<br>Second one, which is rather long for a lede. |  |\n"),
        "{md}"
    );
    assert!(md.contains("| 1 | design | done | 2025-04-01 15:00 | - | 2 |  | [notes/design.md#plan](notes/design.md#plan) |\n"), "{md}");
    assert!(!md.contains("hidden:"), "{md}");

    let short = stdout(dir.path(), &["task", "ls", "--format", "md", "-d=short"]);
    // wrap_width 40 → the lede is cut at 40 scalars with an ellipsis.
    assert!(
        short.contains("| Two lines.<br>Second one, which is rather … |"),
        "{short}"
    );
}

#[test]
fn markdown_alias_and_other_documents() {
    let dir = project_dir(PROJECT);
    let md = stdout(dir.path(), &["task", "ls", "--format", "markdown"]);
    assert!(md.starts_with("# Tasks\n\n| ID | Name |"), "{md}");
    assert!(
        md.ends_with("_(hidden: 2 done — --state all to show)_\n"),
        "{md}"
    );

    let tree = stdout(dir.path(), &["concept", "tree", "3", "--format", "md"]);
    assert_eq!(tree, "# Concepts\n\n- Beta {3}\n  - Leaf {4}\n");

    let sched = stdout(dir.path(), &["schedule", "--format", "md"]);
    assert!(sched.starts_with("# Schedule\n\n| ID | Name | Duration | Start | Finish | Slack | Critical |\n|---:|---|---:|---:|---:|---:|---|\n"), "{sched}");
    assert!(
        sched.contains("| 2 | build \\| ship | 3 | 2 | 5 | 0 | ✓ |\n"),
        "{sched}"
    );
    assert!(
        sched.contains(
            "\n**Project duration:** 5 days\n\n**Critical path:** design → build \\| ship\n"
        ),
        "{sched}"
    );
}

#[test]
fn empty_cases_are_italic_sentences() {
    let empty = project_dir(r#"{ "version": 2, "concepts": [], "tasks": [] }"#);
    assert_eq!(
        stdout(empty.path(), &["report", "--format", "md"]),
        "# Project report\n\n## Concepts\n\n_No concepts._\n\n## Tasks\n\n_No tasks._\n"
    );
    assert_eq!(
        stdout(empty.path(), &["task", "ls", "--format", "md"]),
        "# Tasks\n\n_No tasks._\n"
    );
    assert_eq!(
        stdout(empty.path(), &["schedule", "--format", "md"]),
        "# Schedule\n\n_No tasks._\n"
    );

    let dir = project_dir(PROJECT);
    let md = stdout(
        dir.path(),
        &["task", "ls", "--format", "md", "--state", "todo"],
    );
    assert!(md.contains("| 3 | loose |"), "{md}");
    let none = stdout(
        dir.path(),
        &[
            "report",
            "--format",
            "md",
            "--state",
            "todo",
            "-f",
            ".mindtask.json",
        ],
    );
    assert!(none.contains("### (no concept)"), "{none}");
}
