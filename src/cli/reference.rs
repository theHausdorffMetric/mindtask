//! `mindtask ref …`: attach, detach, list, and rename refs — the citations
//! from tasks and concepts to pages elsewhere (OKF bundle paths or URLs).

use std::path::Path;

use anyhow::{Context, Result};

use mindtask::model::project::Project;
use mindtask::model::reference::{RefOwner, path_part};
use mindtask::refs::{RefStatus, status};

use super::render::{render_table, resolve_wrap_width};

/// The label column width shared by `task show` / `concept show`.
const LABEL_WIDTH: usize = 13;

/// Print a `Refs:` block for `show`, one ref per line — URIs are long, and a
/// comma-joined line would wrap badly. Prints nothing when there are none.
pub(super) fn print_refs(refs: &[String]) {
    let mut label = "Refs:";
    for r in refs {
        println!("{label:<LABEL_WIDTH$}{r}");
        label = "";
    }
}

/// Attach a ref. Returns `true` (the project was modified) on success.
pub fn add(project: &mut Project, owner: RefOwner, uri: &str) -> Result<bool> {
    project.add_ref(owner, uri).context("failed to add ref")?;
    // Echo the stored value: the model trims it.
    println!("Added ref '{}' to {owner}", uri.trim());
    Ok(true)
}

/// Detach a ref. Returns `true` (the project was modified) on success.
pub fn rm(project: &mut Project, owner: RefOwner, uri: &str) -> Result<bool> {
    project
        .remove_ref(owner, uri)
        .context("failed to remove ref")?;
    println!("Removed ref '{}' from {owner}", uri.trim());
    Ok(true)
}

/// The citation table: every ref with its owner and status. With
/// `broken_only`, just the relative refs whose target is missing.
pub fn ls(project: &Project, base: &Path, broken_only: bool) {
    let rows: Vec<Vec<String>> = project
        .refs()
        .filter_map(|(owner, uri)| {
            let st = status(base, uri);
            if broken_only && st != RefStatus::Missing {
                return None;
            }
            let (kind, id, name) = match owner {
                RefOwner::Task(id) => (
                    "task",
                    id.to_string(),
                    project.get_task(id).map(|t| t.name.clone()),
                ),
                RefOwner::Concept(id) => (
                    "concept",
                    id.to_string(),
                    project.get_concept(id).map(|c| c.name.clone()),
                ),
            };
            Some(vec![
                kind.to_string(),
                id,
                name.unwrap_or_default(),
                uri.to_string(),
                st.to_string(),
            ])
        })
        .collect();

    if rows.is_empty() {
        println!(
            "{}",
            if broken_only {
                "No broken refs."
            } else {
                "No refs."
            }
        );
        return;
    }
    println!(
        "{}",
        render_table(
            &["KIND", "ID", "NAME", "REF", "STATUS"],
            &rows,
            Some(2),
            resolve_wrap_width(project),
        )
    );
}

/// Rewrite every ref whose path part is `old` to `new`, keeping fragments —
/// the remedy for a cited page being renamed. Lists what changes; with
/// `dry_run` the project is left unsaved (the caller checks the return value).
/// Returns `true` when refs were rewritten and the project should be saved.
pub fn mv(project: &mut Project, old: &str, new: &str, dry_run: bool) -> Result<bool> {
    let old = old.trim();
    let affected: Vec<(RefOwner, String)> = project
        .refs()
        .filter(|(_, r)| path_part(r) == old)
        .map(|(owner, r)| (owner, r.to_string()))
        .collect();

    let n = project
        .rename_ref(old, new)
        .context("failed to rename ref")?;
    if n == 0 {
        println!("No refs with path '{old}'.");
        return Ok(false);
    }

    let verb = if dry_run { "Would rewrite" } else { "Rewrote" };
    println!("{verb} {n} ref(s): '{old}' -> '{}'", new.trim());
    let width = affected
        .iter()
        .map(|(o, _)| o.to_string().len())
        .max()
        .unwrap_or(0);
    for (owner, before) in &affected {
        let after = match before.split_once('#') {
            Some((_, fragment)) => format!("{}#{fragment}", new.trim()),
            None => new.trim().to_string(),
        };
        println!("  {:<width$}  {before} -> {after}", owner.to_string());
    }
    Ok(!dry_run)
}
