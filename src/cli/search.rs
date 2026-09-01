use mindtask::model::project::Project;

use super::render::{
    DESC_INDENT, avail_width, render_table_with_blocks, resolve_wrap_width, truncate,
};
use super::task::{join_ids, task_cells};

/// Characters of context shown on each side of a hit in a snippet.
const SNIPPET_CONTEXT: usize = 40;

fn matches(haystack: &str, query: &str) -> bool {
    haystack.to_lowercase().contains(query)
}

/// Lowercase `s`, returning the lowercased text alongside — for each of its
/// chars — the index of the original char it came from.
///
/// `char::to_lowercase` can yield several chars for one input (e.g. `İ`), so
/// positions in the lowercased string do not line up with the original and the
/// mapping has to be recorded rather than assumed.
fn lower_with_map(s: &str) -> (String, Vec<usize>) {
    let mut lowered = String::with_capacity(s.len());
    let mut origin = Vec::new();
    for (i, ch) in s.chars().enumerate() {
        for lc in ch.to_lowercase() {
            lowered.push(lc);
            origin.push(i);
        }
    }
    (lowered, origin)
}

/// A one-line excerpt of `text` around its first match of the already-lowercased
/// `query`, with `…` marking each trimmed end. `None` when `query` is absent.
fn snippet(text: &str, query: &str, avail: usize) -> Option<String> {
    let (lowered, origin) = lower_with_map(text);
    let byte_pos = lowered.find(query)?;

    // Byte offset in the lowercased text -> char offset -> original char offset.
    let chars: Vec<char> = text.chars().collect();
    let lower_start = lowered[..byte_pos].chars().count();
    let lower_end = lower_start + query.chars().count();
    let start = origin.get(lower_start).copied().unwrap_or(0);
    let end = origin
        .get(lower_end.saturating_sub(1))
        .map(|&i| i + 1)
        .unwrap_or(chars.len())
        .min(chars.len());

    let from = start.saturating_sub(SNIPPET_CONTEXT);
    let to = (end + SNIPPET_CONTEXT).min(chars.len());

    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.extend(&chars[from..to]);
    if to < chars.len() {
        out.push('…');
    }
    // Descriptions are free text; keep the excerpt to a single table-friendly line.
    Some(truncate(&out.replace('\n', " "), avail))
}

/// The indented excerpt shown beneath a row whose *description* matched, or
/// `None` when it did not (a name match is already visible in the row itself).
fn snippet_block(
    description: Option<&str>,
    query: &str,
    search_description: bool,
    width: usize,
) -> Option<String> {
    if !search_description {
        return None;
    }
    let avail = avail_width(width, DESC_INDENT);
    let text = snippet(description?, query, avail)?;
    Some(format!("{}{}", " ".repeat(DESC_INDENT), text))
}

pub fn search(project: &Project, query: &str, search_description: bool) {
    let query = query.to_lowercase();
    let width = resolve_wrap_width(project);

    let matching_concepts: Vec<_> = project
        .concepts
        .iter()
        .filter(|c| {
            matches(&c.name, &query)
                || (search_description
                    && c.description.as_deref().is_some_and(|d| matches(d, &query)))
        })
        .collect();

    let matching_tasks: Vec<_> = project
        .tasks
        .iter()
        .filter(|t| {
            matches(&t.name, &query)
                || (search_description
                    && t.description.as_deref().is_some_and(|d| matches(d, &query)))
        })
        .collect();

    if matching_concepts.is_empty() && matching_tasks.is_empty() {
        println!("No matches found.");
        return;
    }

    if !matching_concepts.is_empty() {
        let rows: Vec<Vec<String>> = matching_concepts
            .iter()
            .map(|concept| {
                let parent = concept
                    .parent
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());
                vec![concept.id.to_string(), concept.name.clone(), parent]
            })
            .collect();
        let blocks: Vec<Option<String>> = matching_concepts
            .iter()
            .map(|c| snippet_block(c.description.as_deref(), &query, search_description, width))
            .collect();
        println!(
            "{}",
            render_table_with_blocks(&["ID", "NAME", "PARENT"], &rows, &blocks, Some(1), width)
        );
    }

    if !matching_tasks.is_empty() {
        if !matching_concepts.is_empty() {
            println!();
        }
        let rows: Vec<Vec<String>> = matching_tasks
            .iter()
            .map(|&task| {
                let mut cells = task_cells(task, project);
                cells.push(join_ids(&task.concepts));
                cells
            })
            .collect();
        let blocks: Vec<Option<String>> = matching_tasks
            .iter()
            .map(|t| snippet_block(t.description.as_deref(), &query, search_description, width))
            .collect();
        println!(
            "{}",
            render_table_with_blocks(
                &["ID", "NAME", "STATE", "DUE", "DEPENDS ON", "CONCEPTS"],
                &rows,
                &blocks,
                Some(1),
                width,
            )
        );
    }
}
