//! References: URI citations from tasks and concepts to pages elsewhere.
//!
//! A ref is a URI reference (RFC 3986 § 4.1) stored verbatim. Two kinds are
//! told apart by the presence of a scheme: a **relative** ref such as
//! `knowledge/llm-wiki.md#ingest` names a file relative to the project file's
//! directory (the OKF-bundle case), and an **absolute** ref such as
//! `https://…` is kept for syntax only and never fetched.
//!
//! This module is the pure side — classification and fragment handling — so
//! the model can reason about a ref without I/O. Resolving a relative ref and
//! checking that its target exists is [`crate::refs`].

use std::fmt;

use super::id::{ConceptId, TaskId};

/// Which entity holds a ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefOwner {
    /// A task, by ID.
    Task(TaskId),
    /// A concept, by ID.
    Concept(ConceptId),
}

impl fmt::Display for RefOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Task(id) => write!(f, "task {id}"),
            Self::Concept(id) => write!(f, "concept {id}"),
        }
    }
}

/// A ref, classified by whether it carries a scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind<'a> {
    /// `scheme:…` — a URL or URN. Checked for syntax only, never fetched.
    Absolute {
        /// The scheme, without the trailing colon.
        scheme: &'a str,
    },
    /// No scheme: a path relative to the project file's directory, plus an
    /// optional fragment (whatever follows the first `#`).
    Relative {
        /// The path part, which is what gets resolved and checked.
        path: &'a str,
        /// The fragment, kept verbatim and never verified.
        fragment: Option<&'a str>,
    },
}

/// The scheme of `uri`, if it has one.
///
/// RFC 3986: `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )` followed by `:`.
/// One extra rule: the scheme must be at least two characters, so a Windows
/// drive path (`C:\…`) is not read as a URL with scheme `c`.
pub fn scheme(uri: &str) -> Option<&str> {
    let colon = uri.find(':')?;
    let candidate = &uri[..colon];
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    if candidate.len() < 2 {
        return None;
    }
    Some(candidate)
}

/// Classify a ref. Relative refs are split at the first `#`.
pub fn classify(uri: &str) -> RefKind<'_> {
    if let Some(scheme) = scheme(uri) {
        return RefKind::Absolute { scheme };
    }
    match uri.split_once('#') {
        Some((path, fragment)) => RefKind::Relative {
            path,
            fragment: Some(fragment),
        },
        None => RefKind::Relative {
            path: uri,
            fragment: None,
        },
    }
}

/// Everything before the first `#` — the part a rename rewrites. For a ref
/// without a fragment this is the whole string.
pub fn path_part(uri: &str) -> &str {
    uri.split_once('#').map_or(uri, |(path, _)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_display_names_the_kind_and_id() {
        assert_eq!(RefOwner::Task(TaskId(8)).to_string(), "task 8");
        assert_eq!(RefOwner::Concept(ConceptId(12)).to_string(), "concept 12");
    }

    #[test]
    fn schemes_are_detected() {
        assert_eq!(scheme("https://h/p"), Some("https"));
        assert_eq!(scheme("mailto:x@y"), Some("mailto"));
        assert_eq!(scheme("file:///etc/hosts"), Some("file"));
        assert_eq!(scheme("git+ssh://h/r"), Some("git+ssh"));
        assert_eq!(scheme("x-y.z:thing"), Some("x-y.z"));
    }

    #[test]
    fn a_colon_later_in_a_path_is_not_a_scheme() {
        assert_eq!(scheme("dir/a:b.md"), None);
        assert_eq!(scheme("./a:b.md"), None);
        assert_eq!(scheme("#frag:x"), None);
        assert_eq!(scheme("1abc:x"), None);
        assert_eq!(scheme("a b:x"), None);
    }

    #[test]
    fn a_single_letter_before_the_colon_is_a_drive_not_a_scheme() {
        assert_eq!(scheme("C:\\Users\\x.md"), None);
        assert_eq!(scheme("c:x"), None);
    }

    #[test]
    fn classify_splits_relative_refs_at_the_first_hash() {
        assert_eq!(
            classify("a/b.md#x#y"),
            RefKind::Relative {
                path: "a/b.md",
                fragment: Some("x#y")
            }
        );
        assert_eq!(
            classify("a.md#"),
            RefKind::Relative {
                path: "a.md",
                fragment: Some("")
            }
        );
        assert_eq!(
            classify("a.md"),
            RefKind::Relative {
                path: "a.md",
                fragment: None
            }
        );
        assert_eq!(
            classify("#only"),
            RefKind::Relative {
                path: "",
                fragment: Some("only")
            }
        );
    }

    #[test]
    fn classify_keeps_absolute_refs_whole() {
        assert_eq!(
            classify("https://h/p?q#f"),
            RefKind::Absolute { scheme: "https" }
        );
    }

    #[test]
    fn path_part_drops_the_fragment_only() {
        assert_eq!(path_part("a.md#x"), "a.md");
        assert_eq!(path_part("a.md"), "a.md");
        assert_eq!(path_part("https://h/p#f"), "https://h/p");
        assert_eq!(path_part("#x"), "");
    }
}
