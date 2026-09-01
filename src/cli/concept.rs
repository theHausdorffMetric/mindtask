use std::collections::{HashSet, VecDeque};

use anyhow::{Context, Result};
use termtree::Tree;

use mindtask::model::concept::Concept;
use mindtask::model::id::{ConceptId, TaskId};
use mindtask::model::project::{Placement, Project};

use super::render::{
    DescMode, avail_width, desc_text, render_table, render_table_with_blocks, resolve_wrap_width,
};
use super::state_filter::{StateFilter, hidden_footer};
use super::task::{task_cells, task_desc_block};

pub fn add(
    project: &mut Project,
    name: String,
    parent: Option<ConceptId>,
    description: Option<String>,
) -> Result<()> {
    let id = project
        .add_concept(name.clone(), parent, description)
        .context("failed to add concept")?;
    println!("Added concept {} \"{}\"", id, name);
    Ok(())
}

pub fn remove(project: &mut Project, id: ConceptId) -> Result<()> {
    let name = project
        .get_concept(id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    project
        .remove_concept(id)
        .context("failed to remove concept")?;
    println!("Removed concept {} \"{}\"", id, name);
    Ok(())
}

pub fn edit(
    project: &mut Project,
    id: ConceptId,
    name: Option<String>,
    description: Option<String>,
    clear_description: bool,
) -> Result<()> {
    let desc = if clear_description {
        Some(None)
    } else {
        description.map(Some)
    };

    if name.is_none() && desc.is_none() {
        anyhow::bail!(
            "nothing to edit: provide --name and/or --description (or --clear-description)"
        );
    }

    project
        .edit_concept(id, name, desc)
        .context("failed to edit concept")?;
    println!("Updated concept {}", id);
    Ok(())
}

pub fn mv(
    project: &mut Project,
    id: ConceptId,
    parent: Option<String>,
    before: Option<ConceptId>,
    after: Option<ConceptId>,
) -> Result<()> {
    // --before/--after position the concept among siblings, taking the parent
    // from the anchor. They're mutually exclusive with --parent (enforced by
    // clap), so at most one of these three is set.
    if let Some(sib) = before {
        project
            .move_concept_positioned(id, Placement::Before(sib))
            .context("failed to move concept")?;
        println!("Moved concept {id} before {sib}");
        return Ok(());
    }
    if let Some(sib) = after {
        project
            .move_concept_positioned(id, Placement::After(sib))
            .context("failed to move concept")?;
        println!("Moved concept {id} after {sib}");
        return Ok(());
    }

    let parent =
        parent.ok_or_else(|| anyhow::anyhow!("provide one of --parent, --before, or --after"))?;
    let new_parent = if parent == "root" {
        None
    } else {
        Some(
            parent
                .parse::<ConceptId>()
                .context("invalid parent ID (use a concept ID like '1' or 'root')")?,
        )
    };

    project
        .move_concept(id, new_parent)
        .context("failed to move concept")?;

    match new_parent {
        Some(pid) => println!("Moved concept {} under {}", id, pid),
        None => println!("Moved concept {} to root", id),
    }
    Ok(())
}

/// Renumber concept IDs to `1..n` in DFS pre-order — the order `tree` prints —
/// rewriting parent links and task→concept links. With `dry_run`, print the
/// planned remap and leave the file untouched.
pub fn normalize(project: &mut Project, dry_run: bool) -> Result<()> {
    let plan = project
        .plan_normalization()
        .context("failed to plan concept renumbering")?;

    if dry_run {
        // Whether applying changes anything can't be read from the ID map alone
        // (the array may need reordering even when IDs are unchanged), so try it
        // on a throwaway clone and compare.
        let mut trial = project.clone();
        trial.apply_normalization(&plan);
        if trial.concepts == project.concepts {
            println!("Already normalized; nothing to do.");
            return Ok(());
        }
        println!("Planned renumbering (dry run — no changes written):");
        for (old, new) in plan.pairs.iter().filter(|(o, n)| o != n) {
            let name = project
                .get_concept(*old)
                .map(|c| c.name.as_str())
                .unwrap_or("???");
            println!("  {old} -> {new}  {name}");
        }
        return Ok(());
    }

    let before = project.concepts.clone();
    project.apply_normalization(&plan);
    if project.concepts == before {
        println!("Already normalized; nothing to do.");
    } else {
        println!(
            "Normalized {} concept(s) into tree order.",
            project.concepts.len()
        );
    }
    Ok(())
}

pub fn list(project: &Project) {
    if project.concepts.is_empty() {
        println!("No concepts.");
        return;
    }

    let rows: Vec<Vec<String>> = project
        .concepts
        .iter()
        .map(|concept| {
            let parent = match concept.parent {
                Some(pid) => {
                    let name = project
                        .get_concept(pid)
                        .map(|c| c.name.as_str())
                        .unwrap_or("???");
                    format!("{name} {{{pid}}}")
                }
                None => "-".to_string(),
            };
            vec![concept.id.to_string(), concept.name.clone(), parent]
        })
        .collect();
    println!(
        "{}",
        render_table(
            &["ID", "NAME", "PARENT"],
            &rows,
            Some(1),
            resolve_wrap_width(project)
        )
    );
}

pub fn show(project: &Project, id: ConceptId) -> Result<()> {
    let concept = project
        .get_concept(id)
        .ok_or_else(|| anyhow::anyhow!("concept {} not found", id))?;

    println!("ID:          {}", concept.id);
    println!("Name:        {}", concept.name);
    if let Some(desc) = &concept.description {
        println!("Description: {}", desc);
    }
    match concept.parent {
        Some(pid) => {
            let parent_name = project
                .get_concept(pid)
                .map(|c| c.name.as_str())
                .unwrap_or("???");
            println!("Parent:      {} {{{}}}", parent_name, pid);
        }
        None => println!("Parent:      (root)"),
    }

    let children = project.children_of(id);
    if !children.is_empty() {
        let child_strs: Vec<String> = children
            .iter()
            .map(|c| format!("{} {{{}}}", c.name, c.id))
            .collect();
        println!("Children:    {}", child_strs.join(", "));
    }

    let linked_tasks: Vec<_> = project
        .tasks
        .iter()
        .filter(|t| t.concepts.contains(&id))
        .collect();
    if !linked_tasks.is_empty() {
        let task_strs: Vec<String> = linked_tasks
            .iter()
            .map(|t| format!("{} {{{}}}", t.name, t.id))
            .collect();
        println!("Tasks:       {}", task_strs.join(", "));
    }

    Ok(())
}

pub fn tree(project: &Project, root_id: Option<ConceptId>, show_desc: DescMode) -> Result<()> {
    if project.concepts.is_empty() {
        println!("No concepts.");
        return Ok(());
    }

    let width = resolve_wrap_width(project);
    let trees: Vec<Tree<String>> = match root_id {
        Some(id) => {
            let concept = project
                .get_concept(id)
                .ok_or_else(|| anyhow::anyhow!("concept {} not found", id))?;
            vec![build_tree(project, concept, show_desc, width, 0)]
        }
        None => project
            .roots()
            .into_iter()
            .map(|c| build_tree(project, c, show_desc, width, 0))
            .collect(),
    };

    for t in &trees {
        print!("{t}");
    }
    Ok(())
}

/// Columns consumed by each level of tree-branch indentation.
const INDENT_PER_DEPTH: usize = 4;

fn build_tree(
    project: &Project,
    concept: &Concept,
    show_desc: DescMode,
    width: usize,
    depth: usize,
) -> Tree<String> {
    // Render the description on the line(s) below the concept; termtree's
    // multiline mode indents continuation lines to line up with the branch. The
    // branch prefix already consumes `depth * INDENT_PER_DEPTH` columns, so fit
    // the description to whatever width remains and add no padding of our own.
    let rendered = concept.description.as_deref().and_then(|desc| {
        desc_text(
            desc,
            show_desc,
            avail_width(width, depth * INDENT_PER_DEPTH),
        )
    });
    let mut node = match rendered {
        Some(text) => {
            Tree::new(format!("{} {{{}}}\n{text}", concept.name, concept.id)).with_multiline(true)
        }
        None => Tree::new(format!("{} {{{}}}", concept.name, concept.id)),
    };
    for child in project.children_of(concept.id) {
        node.push(build_tree(project, child, show_desc, width, depth + 1));
    }
    node
}

/// Collect all concept IDs in the subtree rooted at `root_id` (inclusive).
fn collect_subtree_ids(project: &Project, root_id: ConceptId) -> HashSet<ConceptId> {
    let mut ids = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(root_id);
    while let Some(id) = queue.pop_front() {
        ids.insert(id);
        for child in project.children_of(id) {
            queue.push_back(child.id);
        }
    }
    ids
}

/// Walk `depends_on` transitively to find all upstream prerequisites.
fn collect_upstream_tasks(project: &Project, seed: &HashSet<TaskId>) -> HashSet<TaskId> {
    let mut all = seed.clone();
    let mut queue: VecDeque<TaskId> = seed.iter().copied().collect();
    while let Some(tid) = queue.pop_front() {
        if let Some(task) = project.get_task(tid) {
            for &dep_id in &task.depends_on {
                if all.insert(dep_id) {
                    queue.push_back(dep_id);
                }
            }
        }
    }
    all
}

/// Format a short due date for report columns.
pub fn report(
    project: &Project,
    id: ConceptId,
    show_desc: DescMode,
    filter: &StateFilter,
) -> Result<()> {
    let root = project
        .get_concept(id)
        .ok_or_else(|| anyhow::anyhow!("concept {} not found", id))?;

    // 1. Print concept subtree
    println!("=== Concept Subtree ===");
    let width = resolve_wrap_width(project);
    let t = build_tree(project, root, show_desc, width, 0);
    print!("{t}");

    // 2. Collect all concept IDs in subtree
    let subtree_ids = collect_subtree_ids(project, id);

    // 3. Find tasks directly linked to any concept in the subtree
    let direct_task_ids: HashSet<TaskId> = project
        .tasks
        .iter()
        .filter(|t| t.concepts.iter().any(|c| subtree_ids.contains(c)))
        .map(|t| t.id)
        .collect();

    if direct_task_ids.is_empty() {
        println!("\nNo tasks linked to this concept subtree.");
        return Ok(());
    }

    // 4. Walk transitive dependencies upstream, seeded from the *visible*
    // direct tasks — a filtered-out direct task shouldn't pull its
    // dependency chain into the report. Upstream tasks then face the same
    // state filter as everything else.
    let visible_direct_ids: HashSet<TaskId> = direct_task_ids
        .iter()
        .copied()
        .filter(|tid| {
            project
                .get_task(*tid)
                .is_some_and(|t| filter.keeps(t.state))
        })
        .collect();
    let visible_ids: HashSet<TaskId> = collect_upstream_tasks(project, &visible_direct_ids)
        .into_iter()
        .filter(|tid| {
            project
                .get_task(*tid)
                .is_some_and(|t| filter.keeps(t.state))
        })
        .collect();
    // What `--state all` would show, so the hidden footer stays honest.
    let all_task_ids = collect_upstream_tasks(project, &direct_task_ids);

    // 5. Collect and sort tasks (direct first, then upstream-only)
    let mut tasks: Vec<_> = project
        .tasks
        .iter()
        .filter(|t| visible_ids.contains(&t.id))
        .collect();
    // Preserve project ordering (which is insertion order)
    tasks.sort_by_key(|t| {
        // Direct tasks sort before upstream-only
        if direct_task_ids.contains(&t.id) {
            0
        } else {
            1
        }
    });

    println!("\n=== Tasks ===");
    if tasks.is_empty() {
        println!("No tasks in the selected states.");
    } else {
        let rows: Vec<Vec<String>> = tasks
            .iter()
            .map(|&task| {
                let mut cells = task_cells(task, project);
                let marker = if direct_task_ids.contains(&task.id) {
                    String::new()
                } else {
                    "(upstream dep)".to_string()
                };
                cells.push(marker);
                cells
            })
            .collect();
        let blocks: Vec<Option<String>> = tasks
            .iter()
            .map(|&task| task_desc_block(task, show_desc, width))
            .collect();
        println!(
            "{}",
            render_table_with_blocks(
                &["ID", "NAME", "STATE", "DUE", "DEPENDS ON", ""],
                &rows,
                &blocks,
                Some(1),
                width,
            )
        );
    }

    let shown_direct = tasks
        .iter()
        .filter(|t| direct_task_ids.contains(&t.id))
        .count();
    let shown_upstream = tasks.len() - shown_direct;
    if shown_upstream > 0 {
        println!(
            "\n{} direct + {} upstream = {} total tasks",
            shown_direct,
            shown_upstream,
            tasks.len()
        );
    }

    let hidden = project
        .tasks
        .iter()
        .filter(|t| all_task_ids.contains(&t.id) && !visible_ids.contains(&t.id));
    if let Some(footer) = hidden_footer(hidden) {
        println!("{footer}");
    }

    Ok(())
}
