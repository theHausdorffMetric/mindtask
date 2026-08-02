//! `mindtask import` — merge a typed concept-graph JSONL stream into the
//! concept tree (see [`mindtask::import`] for the projection semantics).

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};

use mindtask::import::{ImportReport, Projection, apply, parse_jsonl, project_tree};
use mindtask::model::id::ConceptId;
use mindtask::model::project::Project;

/// Read, project, and merge. Returns `true` when the project was modified
/// (so the caller knows whether to save).
pub fn run(
    proj: &mut Project,
    input: &Path,
    under: ConceptId,
    min_docs: usize,
    reparent: bool,
    dry_run: bool,
) -> Result<bool> {
    let graph = if input.as_os_str() == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading stdin")?;
        parse_jsonl(buf.as_bytes())?
    } else {
        let f = File::open(input).with_context(|| format!("opening {}", input.display()))?;
        parse_jsonl(BufReader::new(f))?
    };

    let projection = project_tree(&graph, min_docs);
    print_projection(&projection);

    if projection.concepts.is_empty() {
        println!("nothing to import (no is-a-connected concepts above --min-docs).");
        return Ok(false);
    }

    // Dry run: rehearse against a copy so the report is the real one.
    if dry_run {
        let mut copy = proj.clone();
        let report = apply(&mut copy, under, &projection, reparent)?;
        print_report(&report);
        println!("\ndry-run: nothing was saved.");
        return Ok(false);
    }

    let report = apply(proj, under, &projection, reparent)?;
    print_report(&report);
    Ok(true)
}

fn print_projection(p: &Projection) {
    let umbrellas = p.concepts.iter().filter(|c| c.umbrella).count();
    println!(
        "projection: {} concepts ({} category umbrellas); {} multi-parent edges dropped, {} cycle edges dropped",
        p.concepts.len(),
        umbrellas,
        p.multi_parent_dropped.len(),
        p.cycle_dropped.len()
    );
    for (child, parent) in &p.multi_parent_dropped {
        println!("  multi-parent: kept best parent of {child:?}, dropped is-a -> {parent:?}");
    }
    for (child, parent) in &p.cycle_dropped {
        println!("  cycle: dropped {child:?} is-a -> {parent:?} ({child:?} becomes a root)");
    }
}

fn print_report(r: &ImportReport) {
    println!(
        "import: {} added, {} kept (already present), {} re-parented, {} drifted",
        r.added.len(),
        r.kept.len(),
        r.reparented.len(),
        r.drift.len()
    );
    for (name, actual, projected) in &r.drift {
        println!(
            "  drift: {name:?} is under {actual:?}, projection says {projected:?} (kept; use --reparent to apply)"
        );
    }
}
