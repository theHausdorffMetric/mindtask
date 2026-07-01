//! The default project-file lookup resolves `.mindtask.json` in the current
//! directory only — it must not walk up into parent directories.

use std::process::Command;

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_mindtask");

#[test]
fn does_not_search_parent_directories() {
    let root = TempDir::new().unwrap();
    std::fs::write(
        root.path().join(".mindtask.json"),
        r#"{"version":1,"concepts":[],"tasks":[]}"#,
    )
    .unwrap();
    let child = root.path().join("sub");
    std::fs::create_dir(&child).unwrap();

    // From the child (which has no file of its own), the parent's project file
    // must NOT be picked up.
    let out = Command::new(BIN)
        .args(["concept", "ls"])
        .current_dir(&child)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "must not find a parent directory's project file"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no .mindtask.json"), "stderr: {stderr}");

    // From the directory that actually holds the file, it resolves fine.
    let out = Command::new(BIN)
        .args(["concept", "ls"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
