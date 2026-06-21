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

/// Truncate `s` to at most `max` columns (Unicode scalar values), appending `…`
/// when shortened. `max == 0` yields an empty string.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out: String = chars[..max - 1].iter().collect();
    out.push('…');
    out
}

/// Width of `s` in columns (Unicode scalar values — display width is not yet
/// accounted for, matching [`hard_wrap`]).
fn col_width(s: &str) -> usize {
    s.chars().count()
}

/// Render a left-aligned text table. Each column is sized to its widest cell or
/// header. If the total width exceeds `budget`, the `flex` column (when given)
/// is shrunk toward [`MIN_WRAP_WIDTH`] and its over-long cells truncated with
/// `…`. Columns are separated by two spaces and the last column is not padded,
/// so no line carries trailing whitespace. Returns the table without a trailing
/// newline; callers `println!` it.
pub fn render_table(
    headers: &[&str],
    rows: &[Vec<String>],
    flex: Option<usize>,
    budget: usize,
) -> String {
    const GUTTER: usize = 2;
    let ncols = headers.len();

    // Natural width per column: the widest of the header and any cell.
    let mut widths: Vec<usize> = headers.iter().map(|h| col_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(ncols) {
            widths[i] = widths[i].max(col_width(cell));
        }
    }

    // Terminal-fit: if the table is too wide, shrink the flexible column toward
    // the minimum (its over-long cells are then truncated on render).
    if let Some(f) = flex.filter(|&f| f < ncols) {
        let sep = GUTTER * ncols.saturating_sub(1);
        let total: usize = widths.iter().sum::<usize>() + sep;
        if total > budget {
            let shrunk = widths[f].saturating_sub(total - budget);
            widths[f] = shrunk.max(MIN_WRAP_WIDTH).min(widths[f]);
        }
    }

    let gutter = " ".repeat(GUTTER);
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
    lines.push(render_row(&header_cells, &widths, &gutter));
    for row in rows {
        lines.push(render_row(row, &widths, &gutter));
    }
    lines.join("\n")
}

/// Render one row: each cell truncated to its column width, left-padded except
/// the last column. Trailing whitespace is trimmed (e.g. an empty last cell).
fn render_row(cells: &[String], widths: &[usize], gutter: &str) -> String {
    let last = widths.len().saturating_sub(1);
    let parts: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let cell = truncate(cells.get(i).map(String::as_str).unwrap_or(""), w);
            if i == last {
                cell
            } else {
                format!("{cell:<w$}")
            }
        })
        .collect();
    parts.join(gutter).trim_end().to_string()
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

    #[test]
    fn truncate_keeps_short_and_exact() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_adds_ellipsis_and_is_char_safe() {
        assert_eq!(truncate("abcdef", 4), "abc…"); // 3 chars + …
        assert_eq!(truncate("abc", 1), "…");
        assert_eq!(truncate("abc", 0), "");
        // `…` counts as one char; result never exceeds `max`.
        assert_eq!(truncate("ééééé", 3).chars().count(), 3);
    }

    #[test]
    fn render_table_sizes_columns_to_content() {
        let rows = vec![
            vec!["1".to_string(), "short".to_string()],
            vec!["20".to_string(), "longer name".to_string()],
        ];
        let out = render_table(&["ID", "NAME"], &rows, Some(1), 200);
        let lines: Vec<&str> = out.lines().collect();
        // ID width = max("ID"=2, "1", "20") = 2; two-space gutter.
        assert_eq!(lines[0], "ID  NAME");
        assert_eq!(lines[1], "1   short");
        assert_eq!(lines[2], "20  longer name");
    }

    #[test]
    fn render_table_shrinks_flex_column_to_budget() {
        let rows = vec![vec!["1".to_string(), "abcdefghijklmnop".to_string()]];
        // Natural total (2 + 16 + 2 = 20) exceeds budget; NAME shrinks, floored
        // at MIN_WRAP_WIDTH (8), and its cell is truncated to fit.
        let out = render_table(&["ID", "NAME"], &rows, Some(1), 10);
        let name = out.lines().nth(1).unwrap();
        assert!(name.ends_with("abcdefg…"), "got: {name}");
    }

    #[test]
    fn render_table_no_trailing_whitespace_on_empty_last_cell() {
        let rows = vec![vec!["x".to_string(), String::new()]];
        let out = render_table(&["A", "B"], &rows, None, 200);
        let row = out.lines().nth(1).unwrap();
        assert_eq!(row, "x"); // padding + empty last cell trimmed away
    }
}
