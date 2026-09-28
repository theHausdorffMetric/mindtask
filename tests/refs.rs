//! Integration tests for refs — citations from tasks and concepts to pages.
//!
//! These drive the real `mindtask` binary in a temp directory holding a small
//! fake bundle, and pin the contract the docs describe: refs are attached at
//! creation or with `ref add`, `validate` fails (non-zero) on a missing
//! target but operational commands still run, `ref ls --broken` and `ref mv`
//! find and fix the damage, and a relative ref resolves against the project
//! file's directory rather than the working directory.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// A fresh project directory with `wiki/page.md` beside `.mindtask.json`.
fn bundle_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("wiki")).unwrap();
    std::fs::write(dir.path().join("wiki/page.md"), "# page\n").unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
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

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn project_json(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".mindtask.json")).unwrap()
}

#[test]
fn refs_attach_at_creation_and_show_prints_them_one_per_line() {
    let dir = bundle_project();
    let out = run(
        dir.path(),
        &[
            "task",
            "add",
            "Design",
            "--ref",
            "wiki/page.md#auth",
            "--ref",
            "https://example.org/spec",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        "Added task 1 \"Design\" (refs: wiki/page.md#auth, https://example.org/spec)"
    );

    let show = stdout(&run(dir.path(), &["task", "show", "1"]));
    assert!(
        show.contains("Refs:        wiki/page.md#auth\n             https://example.org/spec\n"),
        "{show}"
    );
    assert!(project_json(dir.path()).contains("\"refs\""));

    let out = run(dir.path(), &["concept", "add", "Backend", "--ref", "wiki/"]);
    assert_eq!(
        stdout(&out).trim(),
        "Added concept 1 \"Backend\" (refs: wiki/)"
    );
    let show = stdout(&run(dir.path(), &["concept", "show", "1"]));
    assert!(show.contains("Refs:        wiki/\n"), "{show}");
}

#[test]
fn a_duplicate_ref_is_refused_and_leaves_the_file_untouched() {
    let dir = bundle_project();
    run(dir.path(), &["task", "add", "T", "--ref", "wiki/page.md"]);
    let before = project_json(dir.path());

    let out = run(dir.path(), &["ref", "add", "--task", "1", " wiki/page.md "]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("task 1 already has ref 'wiki/page.md'"),
        "{}",
        stderr(&out)
    );
    assert_eq!(project_json(dir.path()), before);
}

#[test]
fn an_invalid_dash_dash_ref_creates_nothing() {
    let dir = bundle_project();
    let out = run(dir.path(), &["task", "add", "T", "--ref", "a b.md"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("whitespace"), "{}", stderr(&out));
    let ls = stdout(&run(dir.path(), &["task", "ls", "--state", "all"]));
    assert!(ls.contains("No tasks."), "{ls}");
}

#[test]
fn ref_add_requires_exactly_one_owner_flag() {
    let dir = bundle_project();
    run(dir.path(), &["task", "add", "T"]);
    run(dir.path(), &["concept", "add", "C"]);
    assert!(!run(dir.path(), &["ref", "add", "x.md"]).status.success());
    assert!(
        !run(
            dir.path(),
            &["ref", "add", "--task", "1", "--concept", "1", "x.md"]
        )
        .status
        .success()
    );
}

#[test]
fn validate_fails_on_a_missing_target_and_ref_mv_repairs_it() {
    let dir = bundle_project();
    run(
        dir.path(),
        &[
            "task",
            "add",
            "T",
            "--ref",
            "wiki/page.md#sec",
            "--ref",
            "https://example.org/",
        ],
    );
    let ok = run(dir.path(), &["validate"]);
    assert!(ok.status.success());
    assert_eq!(stdout(&ok).trim(), "Project is valid.");

    std::fs::remove_file(dir.path().join("wiki/page.md")).unwrap();

    let bad = run(dir.path(), &["validate"]);
    assert!(!bad.status.success());
    let text = stdout(&bad);
    assert!(text.contains("Structure is valid."), "{text}");
    assert!(text.contains("1 broken ref(s):"), "{text}");
    assert!(text.contains("task 1"), "{text}");
    assert!(text.contains("wiki/page.md#sec"), "{text}");
    assert!(
        stderr(&bad).contains("Validation failed: 1 broken ref(s)"),
        "{}",
        stderr(&bad)
    );

    // Operational commands are not blocked by a broken ref.
    assert!(
        run(dir.path(), &["task", "state", "1", "done"])
            .status
            .success()
    );

    let broken = stdout(&run(dir.path(), &["ref", "ls", "--broken"]));
    let rows: Vec<&str> = broken.lines().skip(1).collect();
    assert_eq!(rows.len(), 1, "{broken}");
    assert!(rows[0].contains("wiki/page.md#sec") && rows[0].contains("missing"));

    // Dry run reports but does not save.
    std::fs::write(dir.path().join("wiki/moved.md"), "# moved\n").unwrap();
    let before = project_json(dir.path());
    let dry = run(
        dir.path(),
        &["ref", "mv", "wiki/page.md", "wiki/moved.md", "--dry-run"],
    );
    assert!(
        stdout(&dry).contains("Would rewrite 1 ref(s)"),
        "{}",
        stdout(&dry)
    );
    assert_eq!(project_json(dir.path()), before);

    let mv = run(dir.path(), &["ref", "mv", "wiki/page.md", "wiki/moved.md"]);
    assert!(mv.status.success());
    let text = stdout(&mv);
    assert!(text.contains("Rewrote 1 ref(s)"), "{text}");
    assert!(
        text.contains("wiki/page.md#sec -> wiki/moved.md#sec"),
        "{text}"
    );
    assert!(project_json(dir.path()).contains("wiki/moved.md#sec"));
    assert!(run(dir.path(), &["validate"]).status.success());

    let all = stdout(&run(dir.path(), &["ref", "ls"]));
    assert!(all.contains("ok"), "{all}");
    assert!(all.contains("external"), "{all}");
    assert!(!all.contains("missing"), "{all}");
}

#[test]
fn ref_mv_with_no_match_changes_nothing() {
    let dir = bundle_project();
    run(dir.path(), &["task", "add", "T", "--ref", "wiki/page.md"]);
    let before = project_json(dir.path());
    let out = run(dir.path(), &["ref", "mv", "zzz.md", "yyy.md"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("No refs with path 'zzz.md'"));
    assert_eq!(project_json(dir.path()), before);
}

#[test]
fn ref_rm_removes_by_exact_string_and_refuses_a_second_time() {
    let dir = bundle_project();
    run(dir.path(), &["task", "add", "T", "--ref", "wiki/page.md"]);
    let out = run(dir.path(), &["ref", "rm", "--task", "1", "wiki/page.md"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out).trim(),
        "Removed ref 'wiki/page.md' from task 1"
    );
    assert!(!project_json(dir.path()).contains("\"refs\""));

    let again = run(dir.path(), &["ref", "rm", "--task", "1", "wiki/page.md"]);
    assert!(!again.status.success());
    assert!(
        stderr(&again).contains("task 1 has no ref 'wiki/page.md'"),
        "{}",
        stderr(&again)
    );
}

#[test]
fn search_matches_refs_and_shows_the_hit_beneath_the_row() {
    let dir = bundle_project();
    run(
        dir.path(),
        &["task", "add", "Unrelated name", "--ref", "wiki/page.md"],
    );
    let out = stdout(&run(dir.path(), &["search", "page"]));
    assert!(out.contains("Unrelated name"), "{out}");
    assert!(out.contains("\n     wiki/page.md"), "{out}");
    assert!(stdout(&run(dir.path(), &["search", "zzz"])).contains("No matches"));
}

#[test]
fn relative_refs_resolve_against_the_project_file_not_the_cwd() {
    let project = bundle_project();
    run(
        project.path(),
        &["task", "add", "T", "--ref", "wiki/page.md"],
    );

    // A different directory that does NOT contain wiki/page.md.
    let elsewhere = TempDir::new().unwrap();
    let file = project.path().join(".mindtask.json");
    let out = run(
        elsewhere.path(),
        &["-f", file.to_str().unwrap(), "validate"],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "Project is valid.");

    let ls = stdout(&run(
        elsewhere.path(),
        &["-f", file.to_str().unwrap(), "ref", "ls"],
    ));
    assert!(ls.contains("ok"), "{ls}");
}

#[test]
fn a_version_one_file_loads_and_is_marked_version_two_on_save() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(".mindtask.json"),
        r#"{"version":1,"concepts":[],"tasks":[{"id":1,"name":"T"}]}"#,
    )
    .unwrap();
    assert!(run(dir.path(), &["validate"]).status.success());
    // A read-only command leaves the file alone.
    assert!(project_json(dir.path()).contains("\"version\":1"));
    assert!(
        run(dir.path(), &["task", "state", "1", "done"])
            .status
            .success()
    );
    assert!(project_json(dir.path()).contains("\"version\": 2"));
}

#[test]
fn a_newer_format_version_is_refused_with_an_upgrade_hint() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(".mindtask.json"),
        r#"{"version":99,"concepts":[],"tasks":[]}"#,
    )
    .unwrap();
    for args in [&["validate"][..], &["task", "ls"][..]] {
        let out = run(dir.path(), args);
        assert!(!out.status.success(), "{args:?}");
        assert!(
            stderr(&out).contains("format version 99") && stderr(&out).contains("upgrade mindtask"),
            "{args:?}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn an_unknown_field_is_refused_instead_of_dropped() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(".mindtask.json"),
        r#"{"version":2,"concepts":[],"tasks":[{"id":1,"name":"T","future":1}]}"#,
    )
    .unwrap();
    let out = run(dir.path(), &["task", "state", "1", "done"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("unknown field `future`"),
        "{}",
        stderr(&out)
    );
    assert!(project_json(dir.path()).contains("\"future\":1"));
}
