//! Shared terminal-output helpers: width resolution and char-safe text wrapping.
//!
//! These back both the concept-description rendering and the task tables, so the
//! whole CLI agrees on the available width (config `wrap_width` → terminal → 80)
//! and on Unicode-scalar-safe text handling.

use std::str::FromStr;

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

/// Wrap `text` to `width` columns, breaking at whitespace so words stay whole.
///
/// A single token too long to ever fit (a URL, a long path) is hard-split by
/// [`hard_wrap`] rather than overflowing, so no line ever exceeds `width`.
/// Existing newlines are preserved as forced breaks, and because runs of
/// whitespace are consumed as separators no line carries trailing space.
pub fn wrap_words(text: &str, width: usize) -> String {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();

    for src in text.split('\n') {
        // Pre-split over-long tokens so every token below is known to fit.
        let mut tokens: Vec<String> = Vec::new();
        for word in src.split_whitespace() {
            if word.chars().count() > width {
                tokens.extend(hard_wrap(word, width).split('\n').map(str::to_string));
            } else {
                tokens.push(word.to_string());
            }
        }
        if tokens.is_empty() {
            out.push(String::new());
            continue;
        }

        let mut line = String::new();
        let mut line_len = 0usize;
        for token in tokens {
            let tlen = token.chars().count();
            if line_len == 0 {
                line.push_str(&token);
                line_len = tlen;
            } else if line_len + 1 + tlen <= width {
                line.push(' ');
                line.push_str(&token);
                line_len += 1 + tlen;
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(&token);
                line_len = tlen;
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// Columns a description block is indented by when printed beneath a table row.
pub const DESC_INDENT: usize = 5;

/// Columns left for text after `indent` is consumed, floored at
/// [`MIN_WRAP_WIDTH`] so a deep indent never collapses output to nothing.
pub fn avail_width(width: usize, indent: usize) -> usize {
    width.saturating_sub(indent).max(MIN_WRAP_WIDTH)
}

/// Hard-wrap `text` into a block indented by `indent` columns, sized so the
/// indent plus the text still fits `width`. Every line carries the indent, so
/// the block reads as subordinate to whatever printed above it.
pub fn wrap_block(text: &str, indent: usize, width: usize) -> String {
    let pad = " ".repeat(indent);
    wrap_words(text, avail_width(width, indent))
        .lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// How much of an entity's description a listing shows.
///
/// Descriptions in real projects run to paragraphs, so a listing needs two
/// densities: a one-line lede you can scan across a hundred rows, and the full
/// text when you are actually reading one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DescMode {
    /// The flag was absent — print no descriptions at all.
    #[default]
    None,
    /// A single line, truncated with `…` to the available width.
    Short,
    /// The complete description, wrapped across as many lines as it needs.
    Full,
}

impl FromStr for DescMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "short" => Ok(Self::Short),
            "full" => Ok(Self::Full),
            _ => Err(format!(
                "invalid description mode '{s}': expected 'short' or 'full'"
            )),
        }
    }
}

/// Render `desc` as bracketed text fitted to `avail` columns, unindented.
///
/// This is the shared core of description rendering. Callers whose indent is
/// consumed by something already on the line — the concept tree, where
/// termtree draws the branch prefix — use this directly; callers that must pad
/// each line themselves use [`desc_block`].
pub fn desc_text(desc: &str, mode: DescMode, avail: usize) -> Option<String> {
    match mode {
        DescMode::None => None,
        // Truncate the text, not the rendered result, so the brackets stay
        // balanced and the `…` sits inside them.
        DescMode::Short => Some(format!(
            "[{}]",
            truncate(desc, avail.saturating_sub(2).max(1))
        )),
        DescMode::Full => Some(wrap_words(&format!("[{desc}]"), avail)),
    }
}

/// [`desc_text`], indented by `indent` columns on every line, to sit beneath a
/// table row. Returns `None` when nothing should print.
pub fn desc_block(desc: &str, mode: DescMode, indent: usize, width: usize) -> Option<String> {
    let pad = " ".repeat(indent);
    desc_text(desc, mode, avail_width(width, indent)).map(|text| {
        text.lines()
            .map(|line| format!("{pad}{line}"))
            .collect::<Vec<_>>()
            .join("\n")
    })
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

/// Columns between adjacent table columns.
const GUTTER: usize = 2;

/// Natural width per column (the widest of the header and any cell), with the
/// `flex` column shrunk toward [`MIN_WRAP_WIDTH`] when the table would exceed
/// `budget`. Over-long cells in the shrunk column are truncated on render.
fn column_widths(
    headers: &[&str],
    rows: &[Vec<String>],
    flex: Option<usize>,
    budget: usize,
) -> Vec<usize> {
    let ncols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| col_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(ncols) {
            widths[i] = widths[i].max(col_width(cell));
        }
    }

    if let Some(f) = flex.filter(|&f| f < ncols) {
        let sep = GUTTER * ncols.saturating_sub(1);
        let total: usize = widths.iter().sum::<usize>() + sep;
        if total > budget {
            let shrunk = widths[f].saturating_sub(total - budget);
            widths[f] = shrunk.max(MIN_WRAP_WIDTH).min(widths[f]);
        }
    }
    widths
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
    render_table_with_blocks(headers, rows, &[], flex, budget)
}

/// [`render_table`], with an optional pre-rendered text block printed beneath
/// each row (`blocks[i]` belongs to `rows[i]`; a shorter slice simply annotates
/// fewer rows). Blocks arrive already indented and wrapped — see [`desc_block`]
/// — so column alignment is unaffected: every row stays exactly one line.
pub fn render_table_with_blocks(
    headers: &[&str],
    rows: &[Vec<String>],
    blocks: &[Option<String>],
    flex: Option<usize>,
    budget: usize,
) -> String {
    let widths = column_widths(headers, rows, flex, budget);
    let gutter = " ".repeat(GUTTER);
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
    lines.push(render_row(&header_cells, &widths, &gutter));
    for (i, row) in rows.iter().enumerate() {
        lines.push(render_row(row, &widths, &gutter));
        if let Some(Some(block)) = blocks.get(i) {
            lines.push(block.clone());
        }
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

    #[test]
    fn wrap_words_breaks_on_whitespace() {
        assert_eq!(wrap_words("alpha beta gamma", 11), "alpha beta\ngamma");
    }

    #[test]
    fn wrap_words_never_leaves_trailing_space() {
        let wrapped = wrap_words("aaa bbb ccc ddd", 7);
        assert!(
            wrapped.lines().all(|l| l == l.trim_end()),
            "got: {wrapped:?}"
        );
    }

    #[test]
    fn wrap_words_hard_splits_a_token_too_long_to_fit() {
        // No break opportunity inside the token: split it rather than overflow.
        assert_eq!(wrap_words("ab abcdefghij", 4), "ab\nabcd\nefgh\nij");
    }

    #[test]
    fn wrap_words_preserves_blank_lines() {
        assert_eq!(wrap_words("a\n\nb", 80), "a\n\nb");
    }

    #[test]
    fn wrap_words_never_exceeds_width() {
        let text = "short 10.0.7.200/24 (ip neigh FAILED, no reply) supercalifragilistic";
        for w in 4..40 {
            assert!(
                wrap_words(text, w).lines().all(|l| l.chars().count() <= w),
                "width {w} overflowed"
            );
        }
    }

    #[test]
    fn desc_text_none_renders_nothing() {
        assert_eq!(desc_text("anything", DescMode::None, 40), None);
    }

    #[test]
    fn desc_short_is_one_line_with_balanced_brackets() {
        let out = desc_text(
            "a description far longer than the budget",
            DescMode::Short,
            20,
        )
        .unwrap();
        assert_eq!(out.lines().count(), 1);
        assert!(out.starts_with('[') && out.ends_with(']'), "got: {out}");
        assert!(out.chars().count() <= 20, "got: {out}");
    }

    #[test]
    fn desc_full_wraps_and_keeps_the_whole_text() {
        let out = desc_text("alpha beta gamma delta", DescMode::Full, 12).unwrap();
        assert!(out.lines().count() > 1, "expected a wrapped block: {out}");
        assert!(out.replace('\n', " ").contains("delta"));
    }

    #[test]
    fn desc_block_indents_every_line_and_fits_the_width() {
        let out = desc_block("alpha beta gamma delta epsilon", DescMode::Full, 5, 24).unwrap();
        assert!(out.lines().all(|l| l.starts_with("     ")), "got: {out}");
        assert!(out.lines().all(|l| l.chars().count() <= 24), "got: {out}");
    }

    #[test]
    fn desc_mode_parses_known_values_and_rejects_others() {
        assert_eq!("short".parse::<DescMode>(), Ok(DescMode::Short));
        assert_eq!("full".parse::<DescMode>(), Ok(DescMode::Full));
        assert!("nope".parse::<DescMode>().is_err());
    }

    #[test]
    fn table_blocks_print_beneath_their_row() {
        let rows = vec![
            vec!["1".to_string(), "first".to_string()],
            vec!["2".to_string(), "second".to_string()],
        ];
        let blocks = vec![None, Some("     [note]".to_string())];
        let out = render_table_with_blocks(&["ID", "NAME"], &rows, &blocks, Some(1), 200);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "1   first");
        assert_eq!(lines[2], "2   second");
        assert_eq!(lines[3], "     [note]");
    }

    #[test]
    fn table_blocks_do_not_disturb_column_widths() {
        let rows = vec![vec!["1".to_string(), "x".to_string()]];
        let with = render_table_with_blocks(
            &["ID", "NAME"],
            &rows,
            &[Some("     block".to_string())],
            Some(1),
            200,
        );
        let without = render_table(&["ID", "NAME"], &rows, Some(1), 200);
        assert_eq!(with.lines().next(), without.lines().next());
        assert_eq!(with.lines().nth(1), without.lines().nth(1));
    }

    #[test]
    fn table_tolerates_a_blocks_slice_shorter_than_the_rows() {
        let rows = vec![
            vec!["1".to_string(), "a".to_string()],
            vec!["2".to_string(), "b".to_string()],
        ];
        let out = render_table_with_blocks(&["ID", "NAME"], &rows, &[], Some(1), 200);
        assert_eq!(out.lines().count(), 3);
    }
}
