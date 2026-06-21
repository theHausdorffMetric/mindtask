//! Shared terminal-output helpers: width resolution and char-safe text wrapping.
//!
//! These back both the concept-description rendering and the task tables, so the
//! whole CLI agrees on the available width (config `wrap_width` → terminal → 80)
//! and on Unicode-scalar-safe text handling.

use mindtask::model::project::Project;

/// Default wrap width when no config is set and the terminal size is unknown
/// (e.g. output is piped).
pub const DEFAULT_WRAP_WIDTH: usize = 80;

/// Minimum columns a wrap/format target is clamped to, so a tiny configured
/// width never collapses output to nothing.
pub const MIN_WRAP_WIDTH: usize = 8;

/// Resolve the column width for rendering. The project's configured `wrap_width`
/// takes precedence (clamped to [`MIN_WRAP_WIDTH`]); otherwise the detected
/// terminal width is used, falling back to [`DEFAULT_WRAP_WIDTH`] when neither
/// is available (e.g. piped output).
pub fn resolve_wrap_width(project: &Project) -> usize {
    if let Some(w) = project.wrap_width {
        return (w as usize).max(MIN_WRAP_WIDTH);
    }
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| w as usize)
        .unwrap_or(DEFAULT_WRAP_WIDTH)
}

/// Hard-wrap `text` to `width` columns, breaking at the column limit even
/// mid-word. Existing newlines are preserved as forced breaks. Operates on
/// Unicode scalar values, not bytes, so multibyte text stays valid.
pub fn hard_wrap(text: &str, width: usize) -> String {
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    for src in text.split('\n') {
        let chars: Vec<char> = src.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        } else {
            for chunk in chars.chunks(width) {
                lines.push(chunk.iter().collect());
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_wrap_short_text_unchanged() {
        assert_eq!(hard_wrap("hello", 80), "hello");
    }

    #[test]
    fn hard_wrap_breaks_at_column_limit_mid_word() {
        // No whitespace to break on: must split exactly at the width.
        assert_eq!(hard_wrap("abcdefghij", 4), "abcd\nefgh\nij");
    }

    #[test]
    fn hard_wrap_preserves_existing_newlines() {
        assert_eq!(hard_wrap("ab\ncd", 80), "ab\ncd");
    }

    #[test]
    fn hard_wrap_preserves_blank_lines() {
        assert_eq!(hard_wrap("a\n\nb", 80), "a\n\nb");
    }

    #[test]
    fn hard_wrap_wraps_each_source_line_independently() {
        assert_eq!(hard_wrap("abcd\nefgh", 2), "ab\ncd\nef\ngh");
    }

    #[test]
    fn hard_wrap_is_char_boundary_safe() {
        // Each `é` is multibyte; wrapping at 2 chars must not split a scalar.
        let wrapped = hard_wrap("ééé", 2);
        assert_eq!(wrapped, "éé\né");
        // Round-trips as valid UTF-8 with the expected char counts per line.
        let counts: Vec<usize> = wrapped.lines().map(|l| l.chars().count()).collect();
        assert_eq!(counts, vec![2, 1]);
    }

    #[test]
    fn resolve_wrap_width_prefers_config() {
        let mut p = Project::new();
        p.wrap_width = Some(33);
        assert_eq!(resolve_wrap_width(&p), 33);
    }

    #[test]
    fn resolve_wrap_width_clamps_config_to_minimum() {
        let mut p = Project::new();
        p.wrap_width = Some(1);
        assert_eq!(resolve_wrap_width(&p), MIN_WRAP_WIDTH);
    }
}
