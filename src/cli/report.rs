//! The top-level `report`: the concept tree, then every task grouped under
//! the concept it belongs to, in the order the tree was just printed.
//!
//! `task ls` is the flat, ID-ordered list for looking a task up; `report` is
//! the orientation view, so its table follows the shape of the project
//! instead of the order tasks happened to be created in.

use std::collections::{HashMap, HashSet};

use anyhow::Result;

use mindtask::model::id::ConceptId;
use mindtask::model::project::Project;
use mindtask::model::task::Task;

use super::concept;
use super::render::{DescMode, TableGroup, render_table_grouped, resolve_wrap_width};
use super::state_filter::{StateFilter, hidden_footer};
use super::task::{join_ids, task_cells, task_desc_block};

/// Title of the trailing group that holds tasks linked to no concept.
pub(super) const UNLINKED: &str = "(no concept)";

/// Separator between the concepts on a group title's breadcrumb path.
const CRUMB_SEP: &str = " › ";

pub fn run(project: &Project, filter: &StateFilter, desc: DescMode) -> Result<()> {
    concept::tree(project, None, desc)?;
    println!();
    tasks_by_concept(project, filter, desc);
    Ok(())
}

/// The task half of `report`: one table whose rows are grouped by each task's
/// *primary* concept — the first one it lists — with the groups in DFS
/// pre-order of the concept tree, so they follow the tree printed above.
/// Within a group rows ascend by task ID. A concept with no visible task gets
/// no group (the tree already shows it exists); tasks with no concept close
/// the table under [`UNLINKED`]. Every task appears exactly once, so the
/// `--state` footer and the row count stay honest.
pub fn tasks_by_concept(project: &Project, filter: &StateFilter, desc: DescMode) {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return;
    }

    let (shown, hidden): (Vec<&Task>, Vec<&Task>) =
        project.tasks.iter().partition(|t| filter.keeps(t.state));

    if shown.is_empty() {
        println!("No tasks in the selected states.");
    } else {
        let width = resolve_wrap_width(project);
        let groups: Vec<TableGroup> = group_by_primary_concept(project, &shown)
            .into_iter()
            .map(|(title, tasks)| TableGroup {
                title,
                rows: tasks
                    .iter()
                    .map(|&task| {
                        let mut cells = task_cells(task, project);
                        cells.push(join_ids(&task.concepts));
                        cells
                    })
                    .collect(),
                blocks: tasks
                    .iter()
                    .map(|&task| task_desc_block(task, desc, width))
                    .collect(),
            })
            .collect();
        println!(
            "{}",
            render_table_grouped(
                &["ID", "NAME", "STATE", "DUE", "DEPENDS ON", "CONCEPTS"],
                &groups,
                Some(1),
                width,
            )
        );
    }
    if let Some(footer) = hidden_footer(hidden) {
        println!("{footer}");
    }
}

/// Bucket `tasks` by primary concept. Buckets come back in the order the tree
/// prints (see [`Project::dfs_preorder_ids`]), each sorted by task ID and
/// titled with the concept's breadcrumb; a final bucket holds the tasks with
/// no concept. Only non-empty buckets are returned.
fn group_by_primary_concept<'a>(
    project: &Project,
    tasks: &[&'a Task],
) -> Vec<(String, Vec<&'a Task>)> {
    let mut by_concept: HashMap<ConceptId, Vec<&'a Task>> = HashMap::new();
    let mut unlinked: Vec<&'a Task> = Vec::new();
    for &task in tasks {
        match task.concepts.first() {
            Some(&cid) if project.get_concept(cid).is_some() => {
                by_concept.entry(cid).or_default().push(task);
            }
            _ => unlinked.push(task),
        }
    }

    let mut groups = Vec::with_capacity(by_concept.len() + 1);
    for cid in project.dfs_preorder_ids() {
        if let Some(mut bucket) = by_concept.remove(&cid) {
            bucket.sort_by_key(|t| t.id.0);
            groups.push((breadcrumb(project, cid), bucket));
        }
    }
    // A concept unreachable from any root cannot occur on validated data, but
    // if it did its tasks must not vanish from the report: list them as
    // unlinked rather than drop them.
    for bucket in by_concept.into_values() {
        unlinked.extend(bucket);
    }
    if !unlinked.is_empty() {
        unlinked.sort_by_key(|t| t.id.0);
        groups.push((UNLINKED.to_string(), unlinked));
    }
    groups
}

/// `Root › … › Name {id}`: the concept's ancestor path, root first, so a
/// group title reads unambiguously even when leaf names repeat across the
/// tree. Stops at a repeated ID so cyclic in-memory data cannot hang it.
fn breadcrumb(project: &Project, id: ConceptId) -> String {
    let mut names: Vec<&str> = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = Some(id);
    while let Some(cid) = cur {
        if !seen.insert(cid) {
            break;
        }
        let Some(concept) = project.get_concept(cid) else {
            break;
        };
        names.push(concept.name.as_str());
        cur = concept.parent;
    }
    names.reverse();
    format!("{} {{{id}}}", names.join(CRUMB_SEP))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mindtask::model::id::TaskId;
    use mindtask::model::task::TaskState;

    /// Root {1} ─┬─ Beta {3} ── Leaf {4}
    ///           ├─ Alpha {2}
    ///           └─ Empty {5}
    ///
    /// Beta precedes Alpha in the array, so tree order differs from ID order.
    fn project() -> Project {
        let mut p = Project::new();
        p.add_concept("Root".into(), None, None).unwrap(); // 1
        p.add_concept("Alpha".into(), Some(ConceptId(1)), None)
            .unwrap(); // 2
        p.add_concept("Beta".into(), Some(ConceptId(1)), None)
            .unwrap(); // 3
        p.add_concept("Leaf".into(), Some(ConceptId(3)), None)
            .unwrap(); // 4
        p.add_concept("Empty".into(), Some(ConceptId(1)), None)
            .unwrap(); // 5
        p.move_concept_positioned(
            ConceptId(3),
            mindtask::model::project::Placement::Before(ConceptId(2)),
        )
        .unwrap();
        p
    }

    fn task(id: u64, concepts: &[u64]) -> Task {
        Task {
            id: TaskId(id),
            name: format!("t{id}"),
            description: None,
            duration: None,
            state: TaskState::Todo,
            due: None,
            depends_on: vec![],
            concepts: concepts.iter().map(|&c| ConceptId(c)).collect(),
            refs: vec![],
        }
    }

    fn ids(bucket: &[&Task]) -> Vec<u64> {
        bucket.iter().map(|t| t.id.0).collect()
    }

    #[test]
    fn breadcrumb_walks_root_first() {
        let p = project();
        assert_eq!(breadcrumb(&p, ConceptId(4)), "Root › Beta › Leaf {4}");
        assert_eq!(breadcrumb(&p, ConceptId(1)), "Root {1}");
    }

    #[test]
    fn groups_follow_tree_order_and_ids_within() {
        let p = project();
        // Array order deliberately scrambled: grouping must sort by ID.
        let tasks = [
            task(7, &[3]),
            task(1, &[2]),
            task(3, &[3]),
            task(2, &[4]),
            task(4, &[1]),
        ];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        let titles: Vec<&str> = groups.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Root {1}",
                "Root › Beta {3}",
                "Root › Beta › Leaf {4}",
                "Root › Alpha {2}"
            ]
        );
        assert_eq!(ids(&groups[1].1), [3, 7]);
    }

    #[test]
    fn multi_concept_task_lands_under_its_first_concept_once() {
        let p = project();
        let tasks = [task(5, &[2, 3])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "Root › Alpha {2}");
    }

    #[test]
    fn unlinked_tasks_close_the_list_and_empty_concepts_are_skipped() {
        let p = project();
        let tasks = [task(9, &[]), task(6, &[]), task(1, &[2])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        let titles: Vec<&str> = groups.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Root › Alpha {2}", UNLINKED]);
        assert_eq!(ids(&groups[1].1), [6, 9]);
        assert!(titles.iter().all(|t| !t.contains("Empty")));
    }

    #[test]
    fn task_on_unknown_concept_is_listed_not_dropped() {
        let p = project();
        let tasks = [task(1, &[42])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, UNLINKED);
        assert_eq!(ids(&groups[0].1), [1]);
    }
}
