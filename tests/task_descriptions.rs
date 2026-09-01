//! Integration tests for description output on task listings.
//!
//! Task descriptions are long-form prose, so they render as an indented block
//! beneath a row rather than as a table column. These drive the real binary to
//! pin the three things that matter: `-d` is off by default, one flag means the
//! same thing on every listing that shows tasks, and `=short` collapses each
//! description to a single line.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

/// A description long enough to wrap at any sane terminal width.
const LONG: &str = "RESOLVED after the second fix: the bridge no longer answers \
                    ARP from the LAN, verified with a flushed neighbour cache \
                    and a temporary address on the same subnet.";

fn project_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let contents = format!(
        r#"{{
          "version": 1,
          "wrap_width": 80,
          "concepts": [
            {{ "id": 1, "name": "Infra", "description": "Machines and networks" }}
          ],
          "tasks": [
            {{ "id": 1, "name": "Fix the bridge", "state": "todo",
               "description": {LONG:?}, "concepts": [1] }},
            {{ "id": 2, "name": "No description here", "state": "todo" }}
          ]
        }}"#
    );
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

fn stdout_of(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(
        out.status.success(),
        "`{}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn task_ls_omits_descriptions_by_default() {
    let dir = project_dir();
    let out = stdout_of(dir.path(), &["task", "ls"]);
    assert!(out.contains("Fix the bridge"), "stdout: {out}");
    assert!(
        !out.contains("RESOLVED"),
        "default listing leaked a description: {out}"
    );
}

#[test]
fn task_ls_description_flag_prints_an_indented_block() {
    let dir = project_dir();
    let out = stdout_of(dir.path(), &["task", "ls", "-d"]);
    assert!(out.contains("[RESOLVED"), "stdout: {out}");
    // The block is subordinate to its row: indented, and never a table column.
    let block: Vec<&str> = out.lines().filter(|l| l.starts_with("     [")).collect();
    assert_eq!(block.len(), 1, "expected one block opener: {out}");
    // A task without a description contributes no block at all.
    assert!(out.contains("No description here"), "stdout: {out}");
    assert_eq!(
        out.lines()
            .filter(|l| l.trim_start().starts_with('['))
            .count(),
        1,
        "only the described task should get a block: {out}"
    );
}

#[test]
fn short_mode_collapses_each_description_to_one_line() {
    let dir = project_dir();
    let full = stdout_of(dir.path(), &["task", "ls", "-d"]);
    let short = stdout_of(dir.path(), &["task", "ls", "-d=short"]);

    let full_block = full.lines().filter(|l| l.starts_with("     ")).count();
    let short_block = short.lines().filter(|l| l.starts_with("     ")).count();
    assert!(
        full_block > 1,
        "the long description should wrap over several lines: {full}"
    );
    assert_eq!(short_block, 1, "short mode must be one line: {short}");
    assert!(short.contains('…'), "short mode should elide: {short}");
}

#[test]
fn every_task_listing_honors_the_same_flag() {
    let dir = project_dir();
    // `task ls`, the task half of `report`, and `concept report` all render
    // tasks through the same helper, so `-d` must reach all three.
    for args in [
        vec!["task", "ls", "-d=short"],
        vec!["report", "-d=short"],
        vec!["concept", "report", "1", "-d=short"],
    ] {
        let out = stdout_of(dir.path(), &args);
        assert!(
            out.contains("[RESOLVED"),
            "`{}` showed no task description: {out}",
            args.join(" ")
        );
    }
}

#[test]
fn no_output_line_exceeds_the_configured_wrap_width() {
    let dir = project_dir();
    // wrap_width is pinned to 80 in the fixture, so this is deterministic
    // regardless of the terminal the tests run in.
    for args in [
        vec!["task", "ls", "-d"],
        vec!["task", "ls", "-d=short"],
        vec!["task", "show", "1"],
        vec!["report", "-d"],
    ] {
        let out = stdout_of(dir.path(), &args);
        for line in out.lines() {
            assert!(
                line.chars().count() <= 80,
                "`{}` emitted a {}-column line: {line}",
                args.join(" "),
                line.chars().count()
            );
            assert_eq!(line, line.trim_end(), "trailing whitespace: {line:?}");
        }
    }
}

#[test]
fn task_show_wraps_long_descriptions_under_the_label() {
    let dir = project_dir();
    let out = stdout_of(dir.path(), &["task", "show", "1"]);
    let lines: Vec<&str> = out.lines().collect();
    let idx = lines
        .iter()
        .position(|l| l.starts_with("Description: "))
        .expect("no Description line");
    // The description wraps, and its continuation lines align under the value
    // column rather than starting at column zero.
    assert!(
        lines[idx + 1].starts_with("             "),
        "continuation not aligned: {out}"
    );
    // Wrapping is word-aware: no continuation line starts mid-word.
    assert!(
        !lines[idx + 1].trim_start().is_empty(),
        "empty continuation: {out}"
    );
}

#[test]
fn search_shows_the_excerpt_that_matched() {
    let dir = project_dir();
    // Without -d the term only lives in the description, so nothing matches.
    let plain = stdout_of(dir.path(), &["search", "neighbour"]);
    assert!(plain.contains("No matches"), "stdout: {plain}");

    // With -d the task is found *and* the excerpt says why.
    let found = stdout_of(dir.path(), &["search", "neighbour", "-d"]);
    assert!(found.contains("Fix the bridge"), "stdout: {found}");
    assert!(found.contains("neighbour"), "no excerpt shown: {found}");
    assert!(found.contains('…'), "excerpt should be elided: {found}");
}
