//! Refs on the filesystem: where a relative ref points, and whether it exists.
//!
//! The pure side (classification, fragments) is [`crate::model::reference`];
//! this module is the only place a ref meets the filesystem. It never reads a
//! target — a ref is a citation, and the one question asked is whether the
//! cited file (or directory) is there. Absolute refs (those with a scheme) are
//! never fetched.

use std::path::{Path, PathBuf};

use crate::model::project::Project;
use crate::model::reference::{RefKind, RefOwner, classify};

/// The directory relative refs resolve against: the project file's own
/// directory, so resolution never depends on the working directory and `-f`
/// from elsewhere still works. A bare filename has an empty parent, which
/// means the current directory.
///
/// The store uses the same directory for its atomic-save temp file, since
/// `rename` is only atomic within a single filesystem.
pub fn base_dir(project_file: &Path) -> &Path {
    match project_file.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Where a relative ref's path part points: `base.join(path)`, left
/// uncanonicalised so the result reads like the ref did.
pub fn resolve(base: &Path, path: &str) -> PathBuf {
    base.join(path)
}

/// What checking one ref found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefStatus {
    /// Relative, and the path part exists under the base directory.
    Ok,
    /// Relative, and the path part does not exist under the base directory.
    Missing,
    /// Absolute (has a scheme); not checked.
    External,
}

impl std::fmt::Display for RefStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Via `f.pad` so width flags are honored in tables.
        f.pad(match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::External => "external",
        })
    }
}

/// Check one ref against `base`. A fragment is ignored; a directory counts as
/// existing (an OKF bundle root is a legitimate citation); symlinks are
/// followed.
pub fn status(base: &Path, uri: &str) -> RefStatus {
    match classify(uri) {
        RefKind::Absolute { .. } => RefStatus::External,
        RefKind::Relative { path, .. } => {
            if resolve(base, path).exists() {
                RefStatus::Ok
            } else {
                RefStatus::Missing
            }
        }
    }
}

/// A relative ref whose target is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenRef {
    /// The task or concept holding the ref.
    pub owner: RefOwner,
    /// The ref as stored.
    pub uri: String,
    /// Where it was looked for.
    pub resolved: PathBuf,
}

/// Every relative ref in the project whose path part does not exist under
/// `base`, in project order (concepts first, then tasks).
pub fn check(project: &Project, base: &Path) -> Vec<BrokenRef> {
    project
        .refs()
        .filter_map(|(owner, uri)| match classify(uri) {
            RefKind::Absolute { .. } => None,
            RefKind::Relative { path, .. } => {
                let resolved = resolve(base, path);
                (!resolved.exists()).then(|| BrokenRef {
                    owner,
                    uri: uri.to_string(),
                    resolved,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::id::{ConceptId, TaskId};

    #[test]
    fn base_dir_handles_bare_and_nested_paths() {
        // A bare filename must resolve to the current directory, not `/`.
        assert_eq!(base_dir(Path::new("project.json")), Path::new("."));
        assert_eq!(base_dir(Path::new("a/b/project.json")), Path::new("a/b"));
        assert_eq!(base_dir(Path::new("/tmp/project.json")), Path::new("/tmp"));
    }

    fn bundle() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("wiki/sub")).unwrap();
        std::fs::write(dir.path().join("wiki/page.md"), "# page\n").unwrap();
        dir
    }

    #[test]
    fn status_of_each_kind() {
        let dir = bundle();
        let base = dir.path();
        assert_eq!(status(base, "wiki/page.md"), RefStatus::Ok);
        assert_eq!(status(base, "wiki/page.md#heading"), RefStatus::Ok);
        assert_eq!(status(base, "wiki/sub"), RefStatus::Ok);
        assert_eq!(status(base, "wiki/missing.md"), RefStatus::Missing);
        assert_eq!(status(base, "https://example.org/x"), RefStatus::External);
    }

    #[test]
    fn check_lists_only_missing_relative_refs_in_project_order() {
        let dir = bundle();
        let mut p = Project::new();
        let cid = p.add_concept("C".into(), None, None).unwrap();
        let tid = p.add_task("T".into(), None, None, None).unwrap();
        p.add_ref(RefOwner::Task(tid), "wiki/page.md").unwrap();
        p.add_ref(RefOwner::Task(tid), "wiki/gone.md#sec").unwrap();
        p.add_ref(RefOwner::Concept(cid), "https://example.org/")
            .unwrap();
        p.add_ref(RefOwner::Concept(cid), "nowhere.md").unwrap();

        let broken = check(&p, dir.path());
        assert_eq!(
            broken,
            vec![
                BrokenRef {
                    owner: RefOwner::Concept(ConceptId(1)),
                    uri: "nowhere.md".into(),
                    resolved: dir.path().join("nowhere.md"),
                },
                BrokenRef {
                    owner: RefOwner::Task(TaskId(1)),
                    uri: "wiki/gone.md#sec".into(),
                    resolved: dir.path().join("wiki/gone.md"),
                },
            ]
        );
    }

    #[test]
    fn check_is_empty_for_a_project_without_refs() {
        let dir = bundle();
        let mut p = Project::new();
        p.add_task("T".into(), None, None, None).unwrap();
        assert!(check(&p, dir.path()).is_empty());
    }
}
