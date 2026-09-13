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

#[cfg(test)]
mod tests {
    use super::*;

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
