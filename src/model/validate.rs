//! Boundary checks for user-supplied values.
//!
//! Everything that reaches the model through [`Project`](super::project::Project)'s
//! mutation API passes through here, so a value the file format cannot
//! represent (`NaN` serialises to `null` and vanishes on reload) or the
//! renderers cannot lay out (an empty name, a newline in a table cell) is
//! refused at the door instead of being stored and quietly lost or mangled.
//! The CLI reuses [`validate_duration`] as a clap `value_parser` so the
//! error there names the flag; the model-level call is what protects library
//! callers and `import`.

use super::project::{ProjectError, Result};
use super::reference::{RefKind, classify};

/// Check a concept or task name and return it trimmed.
///
/// Leading and trailing whitespace is dropped — it is almost always a shell
/// quoting artefact. What remains must be non-empty and free of control
/// characters: a tab or newline would misalign every listing and split a
/// diagram declaration across lines.
pub fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ProjectError::EmptyName);
    }
    if let Some(c) = trimmed.chars().find(|c| c.is_control()) {
        return Err(ProjectError::ControlCharInName(c));
    }
    Ok(trimmed.to_string())
}

/// Check a task duration in days.
///
/// Must be finite and non-negative. `NaN` and the infinities are rejected
/// because JSON has no representation for them; negative durations because
/// the schedule has no meaning for a task that finishes before it starts.
/// Zero is allowed (a milestone) and `-0.0` is normalised to `0.0` so it
/// never prints as `-0`.
pub fn validate_duration(days: f64) -> Result<f64> {
    if !days.is_finite() || days < 0.0 {
        return Err(ProjectError::InvalidDuration(days));
    }
    Ok(if days == 0.0 { 0.0 } else { days })
}

/// Check a ref — a URI reference — and return it trimmed.
///
/// A ref is stored verbatim, so the rules are only what would make it
/// unusable: it must be non-empty, contain no whitespace or control character
/// (a URI has none, and either would break a `[[…]]` hyperlink in a diagram),
/// and not be a filesystem-absolute path (`/home/…`, `C:\…`), which would not
/// survive a clone — cite a path relative to the project file, or a URL. A
/// fragment alone (`#section`) names no page and is refused too. Nothing is
/// resolved here; see [`crate::refs`] for that.
pub fn validate_ref(uri: &str) -> Result<String> {
    let trimmed = uri.trim();
    let reject = |reason: &'static str| {
        Err(ProjectError::InvalidRef {
            uri: trimmed.to_string(),
            reason,
        })
    };
    if trimmed.is_empty() {
        return reject("must not be empty");
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return reject("must not contain whitespace or control characters");
    }
    if trimmed.starts_with('/') {
        return reject(
            "absolute filesystem paths are not portable; \
             use a path relative to the project file, or a URL",
        );
    }
    if is_drive_path(trimmed) {
        return reject(
            "looks like a Windows drive path; \
             use a path relative to the project file, or a URL",
        );
    }
    if let RefKind::Relative { path: "", .. } = classify(trimmed) {
        return reject("a fragment alone names no page");
    }
    Ok(trimmed.to_string())
}

/// `C:\…` or `C:/…` — a single ASCII letter, a colon, then a separator.
fn is_drive_path(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_are_trimmed_and_otherwise_kept_verbatim() {
        for ok in [
            "a.md",
            "dir/a.md#frag",
            "../x.md",
            "./a:b.md",
            "RND/",
            "https://h/p?q#f",
            "mailto:x@y",
            "file:///etc/hosts",
            "a.md#",
            "a%20b.md",
        ] {
            assert_eq!(validate_ref(&format!("  {ok}\n")).unwrap(), ok, "{ok:?}");
        }
    }

    #[test]
    fn bad_refs_are_rejected_with_the_trimmed_value_in_the_error() {
        for (bad, reason_fragment) in [
            ("", "empty"),
            ("   ", "empty"),
            ("a b.md", "whitespace"),
            ("a\tb.md", "whitespace"),
            ("a\u{7}b.md", "control"),
            ("/abs/path.md", "absolute"),
            ("C:\\x.md", "drive"),
            ("c:/x.md", "drive"),
            ("#only", "fragment"),
        ] {
            match validate_ref(bad) {
                Err(ProjectError::InvalidRef { uri, reason }) => {
                    assert_eq!(uri, bad.trim(), "{bad:?}");
                    assert!(reason.contains(reason_fragment), "{bad:?}: {reason}");
                }
                other => panic!("{bad:?}: expected InvalidRef, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_two_letter_scheme_is_a_url_not_a_drive() {
        // `ab:` cannot be a drive letter, so it is read as a scheme.
        assert_eq!(validate_ref("ab:thing").unwrap(), "ab:thing");
    }

    #[test]
    fn name_is_trimmed() {
        assert_eq!(validate_name("  Design API \n").unwrap(), "Design API");
    }

    #[test]
    fn name_keeps_interior_spacing_and_unicode() {
        assert_eq!(validate_name("a  b — c 日本").unwrap(), "a  b — c 日本");
    }

    #[test]
    fn empty_and_whitespace_only_names_are_rejected() {
        for bad in ["", "   ", "\t\n"] {
            assert!(
                matches!(validate_name(bad), Err(ProjectError::EmptyName)),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn control_characters_inside_a_name_are_rejected() {
        for (bad, c) in [
            ("line one\nline two", '\n'),
            ("tabs\there", '\t'),
            ("nul\0", '\0'),
        ] {
            match validate_name(bad) {
                Err(ProjectError::ControlCharInName(found)) => assert_eq!(found, c),
                other => panic!("{bad:?}: expected ControlCharInName, got {other:?}"),
            }
        }
    }

    #[test]
    fn finite_non_negative_durations_pass() {
        assert_eq!(validate_duration(2.5).unwrap(), 2.5);
        assert_eq!(validate_duration(0.0).unwrap(), 0.0);
    }

    #[test]
    fn negative_zero_is_normalised() {
        let d = validate_duration(-0.0).unwrap();
        assert!(d.is_sign_positive(), "{d}");
    }

    #[test]
    fn non_finite_and_negative_durations_are_rejected() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -5.0, -0.001] {
            match validate_duration(bad) {
                Err(ProjectError::InvalidDuration(d)) => {
                    assert!(d.is_nan() && bad.is_nan() || d == bad, "{bad}");
                }
                other => panic!("{bad}: expected InvalidDuration, got {other:?}"),
            }
        }
    }
}
