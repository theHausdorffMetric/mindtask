//! JSON file storage for projects.
//!
//! Projects are stored as pretty-printed JSON. On load, ID counters are
//! recomputed from the stored data so they don't need to be persisted.

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::model::project::Project;

/// Errors that can occur during project load or save.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The project file does not exist at the given path.
    #[error("project file not found: {0}")]
    NotFound(String),
    /// An I/O error occurred while reading.
    #[error("failed to read project file: {0}")]
    ReadError(#[from] std::io::Error),
    /// An I/O error occurred while writing.
    ///
    /// Separate from [`StoreError::ReadError`] so a failed save doesn't report
    /// itself as a read failure — the atomic-save path has several distinct
    /// write steps (temp file, flush, rename) and a full disk must not surface
    /// as "failed to read project file".
    #[error("failed to write project file: {0}")]
    WriteError(std::io::Error),
    /// The file contents are not valid JSON or don't match the schema.
    #[error("failed to parse project file: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Convenience alias for store results.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Load a project from a JSON file, recomputing ID counters.
pub fn load(path: &Path) -> Result<Project> {
    if !path.exists() {
        return Err(StoreError::NotFound(path.display().to_string()));
    }
    let contents = std::fs::read_to_string(path)?;
    let mut project: Project = serde_json::from_str(&contents)?;
    project.recompute_next_ids();
    Ok(project)
}

/// The directory a temp file for `path` must live in.
///
/// `rename` is only atomic within a single filesystem, so the temp file has to
/// be a sibling of the target. A bare filename has an empty parent, which means
/// the current directory.
fn sibling_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Save a project to a JSON file (pretty-printed), atomically.
///
/// The project file is the single source of truth, so it is never truncated in
/// place: the new contents are written to a temp file alongside it, flushed to
/// disk, and only then renamed over the target. An interrupted, failed, or
/// out-of-space save therefore leaves the previous file intact rather than
/// truncated or half-written.
pub fn save(path: &Path, project: &Project) -> Result<()> {
    let json = serde_json::to_string_pretty(project)?;

    let dir = sibling_dir(path);
    let mut tmp = NamedTempFile::new_in(dir).map_err(StoreError::WriteError)?;
    tmp.write_all(json.as_bytes())
        .map_err(StoreError::WriteError)?;
    tmp.write_all(b"\n").map_err(StoreError::WriteError)?;

    // `NamedTempFile` creates its file 0600. Carry over the mode of the file
    // being replaced so an existing project file keeps its permissions.
    #[cfg(unix)]
    {
        if let Ok(meta) = std::fs::metadata(path) {
            tmp.as_file()
                .set_permissions(meta.permissions())
                .map_err(StoreError::WriteError)?;
        }
    }

    // Flush to disk *before* the rename, so a crash can never publish a temp
    // file whose contents never landed.
    tmp.as_file().sync_all().map_err(StoreError::WriteError)?;

    tmp.persist(path)
        .map_err(|e| StoreError::WriteError(e.error))?;

    // Make the rename itself durable, so a power loss can't undo a save that
    // already reported success.
    #[cfg(unix)]
    {
        std::fs::File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(StoreError::WriteError)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::id::ConceptId;
    use tempfile::NamedTempFile;

    #[test]
    fn save_and_load_roundtrip() {
        let mut project = Project::new();
        project.add_concept("Root".into(), None, None).unwrap();
        project
            .add_concept("Child".into(), Some(ConceptId(1)), Some("desc".into()))
            .unwrap();
        project.add_task("Do stuff".into(), Some("details".into()), Some(2.5), None);

        let file = NamedTempFile::new().unwrap();
        save(file.path(), &project).unwrap();
        let loaded = load(file.path()).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.concepts.len(), 2);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.next_concept_id, 3);
        assert_eq!(loaded.next_task_id, 2);
    }

    #[test]
    fn load_nonexistent_file() {
        let err = load(Path::new("/tmp/nonexistent_mindtask_test.json")).unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[test]
    fn sibling_dir_handles_bare_and_nested_paths() {
        // A bare filename must resolve to the current directory, not `/`.
        assert_eq!(sibling_dir(Path::new("project.json")), Path::new("."));
        assert_eq!(sibling_dir(Path::new("a/b/project.json")), Path::new("a/b"));
        assert_eq!(
            sibling_dir(Path::new("/tmp/project.json")),
            Path::new("/tmp")
        );
    }

    #[test]
    fn save_leaves_no_temp_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");
        let mut project = Project::new();
        project.add_concept("Root".into(), None, None).unwrap();

        save(&path, &project).unwrap();
        save(&path, &project).unwrap();

        let mut entries: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec!["project.json".to_string()],
            "stray temp files"
        );
    }

    #[test]
    fn save_replaces_previous_contents_completely() {
        // The failure mode of a non-truncating in-place write: a shorter save
        // leaving a tail of the longer previous file behind.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");

        let mut big = Project::new();
        for i in 0..50 {
            big.add_concept(format!("Concept number {i}"), None, None)
                .unwrap();
        }
        save(&path, &big).unwrap();
        let big_len = std::fs::metadata(&path).unwrap().len();

        save(&path, &Project::new()).unwrap();

        let small_len = std::fs::metadata(&path).unwrap().len();
        assert!(
            small_len < big_len,
            "file did not shrink: {small_len} vs {big_len}"
        );
        // And it is still valid JSON describing the *new* project.
        assert_eq!(load(&path).unwrap().concepts.len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn save_preserves_the_replaced_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");
        let project = Project::new();

        save(&path, &project).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        // The temp file is created 0600; replacing must not silently tighten
        // the mode of the file the user already had.
        save(&path, &project).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "replace changed the mode to {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_save_leaves_the_previous_file_intact() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");

        let mut original = Project::new();
        original.add_concept("Original".into(), None, None).unwrap();
        save(&path, &original).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();

        // Make the directory unwritable so the temp file cannot be created.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        let mut replacement = Project::new();
        replacement
            .add_concept("Replacement".into(), None, None)
            .unwrap();
        let result = save(&path, &replacement);

        // Root ignores the permission bits; restore them either way so the
        // TempDir can clean itself up.
        let bypassed_by_root = result.is_ok();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        if bypassed_by_root {
            return;
        }

        assert!(
            matches!(result, Err(StoreError::WriteError(_))),
            "expected a write error, got: {result:?}"
        );
        // The point of the exercise: the old file survives a failed save.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        assert!(before.contains("Original"));
        assert!(load(&path).is_ok(), "previous file left unreadable");
    }
}
