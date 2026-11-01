//! View models behind the listings: the data `report`, `task ls`,
//! `concept tree`, and `schedule` show, independent of how it is rendered
//! (terminal table, Markdown, JSON).
//!
//! A renderer consumes one of the three documents — [`Report`],
//! [`TaskListing`], [`ScheduleView`] — or the bare tree from
//! [`concept_tree`]. Every document serialises with serde, so the JSON
//! output of the CLI is exactly these types; their field names are part of
//! the CLI's contract and change only with a release note.

use std::collections::{HashMap, HashSet};

use jiff::Zoned;
use serde::Serialize;

use crate::graph::schedule::{Schedule, schedule};
use crate::model::concept::Concept;
use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;
use crate::model::task::{Task, TaskState};

/// Title of the trailing group that holds tasks linked to no concept.
pub const UNLINKED: &str = "(no concept)";

/// Separator between the concepts on a group title's breadcrumb path.
pub const CRUMB_SEP: &str = " › ";

/// One concept with its subtree, in sibling order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptNode {
    pub id: ConceptId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
    pub children: Vec<ConceptNode>,
}

impl ConceptNode {
    fn build(project: &Project, concept: &Concept, seen: &mut HashSet<ConceptId>) -> Self {
        let children = if seen.insert(concept.id) {
            project
                .children_of(concept.id)
                .into_iter()
                .map(|c| Self::build(project, c, seen))
                .collect()
        } else {
            // A cycle cannot occur on validated data; never recurse forever on
            // unvalidated in-memory data either.
            Vec::new()
        };
        Self {
            id: concept.id,
            name: concept.name.clone(),
            description: concept.description.clone(),
            refs: concept.refs.clone(),
            children,
        }
    }
}

/// The concept tree as nested nodes: every root in order, or only the
/// subtree under `root`. `None` when `root` names no concept.
pub fn concept_tree(project: &Project, root: Option<ConceptId>) -> Option<Vec<ConceptNode>> {
    let mut seen = HashSet::new();
    match root {
        Some(id) => {
            let concept = project.get_concept(id)?;
            Some(vec![ConceptNode::build(project, concept, &mut seen)])
        }
        None => Some(
            project
                .roots()
                .into_iter()
                .map(|c| ConceptNode::build(project, c, &mut seen))
                .collect(),
        ),
    }
}

/// One task as every listing shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskRow {
    pub id: TaskId,
    pub name: String,
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Estimated duration in days.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Due date, converted to the project timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<Zoned>,
    pub depends_on: Vec<TaskId>,
    pub concepts: Vec<ConceptId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
}

impl TaskRow {
    pub fn from_task(task: &Task, project: &Project) -> Self {
        Self {
            id: task.id,
            name: task.name.clone(),
            state: task.state,
            description: task.description.clone(),
            duration: task.duration,
            due: task.due.as_ref().map(|d| in_project_tz(d, project)),
            depends_on: task.depends_on.clone(),
            concepts: task.concepts.clone(),
            refs: task.refs.clone(),
        }
    }
}

/// `due` in the project's timezone (unchanged when the name is unknown).
fn in_project_tz(due: &Zoned, project: &Project) -> Zoned {
    match jiff::tz::TimeZone::get(project.timezone_or_utc()) {
        Ok(tz) => due.with_time_zone(tz),
        Err(_) => due.clone(),
    }
}

/// How many tasks the `--state` filter removed, per state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Hidden {
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
}

impl Hidden {
    pub fn count<'a, I>(tasks: I) -> Self
    where
        I: IntoIterator<Item = &'a Task>,
    {
        let mut h = Self::default();
        for task in tasks {
            match task.state {
                TaskState::Todo => h.todo += 1,
                TaskState::InProgress => h.in_progress += 1,
                TaskState::Done => h.done += 1,
            }
        }
        h
    }

    pub fn total(&self) -> usize {
        self.todo + self.in_progress + self.done
    }
}

/// One titled run of rows in the grouped task table of `report`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskGroup {
    /// The group's concept; `None` for the trailing [`UNLINKED`] group.
    pub concept: Option<ConceptId>,
    /// `Root › … › Name {id}`, or [`UNLINKED`].
    pub title: String,
    pub tasks: Vec<TaskRow>,
}

/// The whole-project report: the concept tree, then every visible task
/// grouped under its primary concept in tree order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// The states the listing shows (the resolved `--state` filter).
    pub states: Vec<TaskState>,
    pub concepts: Vec<ConceptNode>,
    pub groups: Vec<TaskGroup>,
    pub hidden: Hidden,
}

impl Report {
    /// Build the report showing tasks in `states`.
    pub fn build(project: &Project, states: &[TaskState]) -> Self {
        let (shown, hidden) = split_by_state(project, states);
        let groups = group_by_primary_concept(project, &shown)
            .into_iter()
            .map(|(concept, title, tasks)| TaskGroup {
                concept,
                title,
                tasks: tasks
                    .iter()
                    .map(|&t| TaskRow::from_task(t, project))
                    .collect(),
            })
            .collect();
        Self {
            timezone: project.timezone.clone(),
            states: states.to_vec(),
            concepts: concept_tree(project, None).unwrap_or_default(),
            groups,
            hidden: Hidden::count(hidden),
        }
    }

    /// Every visible task, in group order.
    pub fn tasks(&self) -> impl Iterator<Item = &TaskRow> {
        self.groups.iter().flat_map(|g| g.tasks.iter())
    }
}

/// The flat, ID-ordered listing of `task ls`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskListing {
    pub states: Vec<TaskState>,
    pub tasks: Vec<TaskRow>,
    pub hidden: Hidden,
}

impl TaskListing {
    pub fn build(project: &Project, states: &[TaskState]) -> Self {
        let (shown, hidden) = split_by_state(project, states);
        Self {
            states: states.to_vec(),
            tasks: shown
                .iter()
                .map(|&t| TaskRow::from_task(t, project))
                .collect(),
            hidden: Hidden::count(hidden),
        }
    }
}

/// Partition the project's tasks into those in `states` and the rest, both
/// in project order.
fn split_by_state<'a>(
    project: &'a Project,
    states: &[TaskState],
) -> (Vec<&'a Task>, Vec<&'a Task>) {
    project
        .tasks
        .iter()
        .partition(|t| states.contains(&t.state))
}

/// Bucket `tasks` by primary concept — the first one each lists. Buckets come
/// back in the order the tree prints (see [`Project::dfs_preorder_ids`]),
/// each sorted by task ID and titled with the concept's breadcrumb; a final
/// bucket holds the tasks with no concept. Only non-empty buckets are
/// returned, so every task appears exactly once.
pub fn group_by_primary_concept<'a>(
    project: &Project,
    tasks: &[&'a Task],
) -> Vec<(Option<ConceptId>, String, Vec<&'a Task>)> {
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
            groups.push((Some(cid), breadcrumb(project, cid), bucket));
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
        groups.push((None, UNLINKED.to_string(), unlinked));
    }
    groups
}

/// `Root › … › Name {id}`: the concept's ancestor path, root first, so a
/// group title reads unambiguously even when leaf names repeat across the
/// tree. Stops at a repeated ID so cyclic in-memory data cannot hang it.
pub fn breadcrumb(project: &Project, id: ConceptId) -> String {
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

/// One task's row in the schedule. Times are day offsets from project start.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduleRow {
    pub id: TaskId,
    pub name: String,
    pub duration: f64,
    pub earliest_start: f64,
    pub earliest_finish: f64,
    pub latest_start: f64,
    pub latest_finish: f64,
    pub slack: f64,
    pub critical: bool,
}

/// The CPM schedule as `schedule` shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduleView {
    /// Project length in days; `0` when no task carries a duration.
    pub duration: f64,
    /// Critical tasks in topological order — always the full path, even
    /// when `tasks` was narrowed to the critical ones.
    pub critical_path: Vec<TaskId>,
    /// Rows in project order; only the critical ones when `critical_only`.
    pub tasks: Vec<ScheduleRow>,
}

impl ScheduleView {
    /// Compute the schedule. `Err` only when the dependencies contain a
    /// cycle, which a validated project never does.
    pub fn build(project: &Project, critical_only: bool) -> Result<Self, String> {
        let sched: Schedule = schedule(&project.tasks)?;
        let tasks = sched
            .tasks
            .iter()
            .filter(|st| !critical_only || st.critical)
            .map(|st| ScheduleRow {
                id: st.id,
                name: project
                    .get_task(st.id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                duration: st.duration,
                earliest_start: st.earliest_start,
                earliest_finish: st.earliest_finish,
                latest_start: st.latest_start,
                latest_finish: st.latest_finish,
                slack: st.slack,
                critical: st.critical,
            })
            .collect();
        Ok(Self {
            duration: sched.duration,
            critical_path: sched.critical_path().to_vec(),
            tasks,
        })
    }

    /// Whether any task carries a duration — the text renderer prints a hint
    /// instead of an all-zero table when none does.
    pub fn has_durations(&self) -> bool {
        self.duration.abs() >= 1e-9
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::project::Placement;

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
        p.move_concept_positioned(ConceptId(3), Placement::Before(ConceptId(2)))
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
        let titles: Vec<&str> = groups.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Root {1}",
                "Root › Beta {3}",
                "Root › Beta › Leaf {4}",
                "Root › Alpha {2}"
            ]
        );
        assert_eq!(groups[1].0, Some(ConceptId(3)));
        assert_eq!(ids(&groups[1].2), [3, 7]);
    }

    #[test]
    fn multi_concept_task_lands_under_its_first_concept_once() {
        let p = project();
        let tasks = [task(5, &[2, 3])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1, "Root › Alpha {2}");
    }

    #[test]
    fn unlinked_tasks_close_the_list_and_empty_concepts_are_skipped() {
        let p = project();
        let tasks = [task(9, &[]), task(6, &[]), task(1, &[2])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        let titles: Vec<&str> = groups.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Root › Alpha {2}", UNLINKED]);
        assert_eq!(groups[1].0, None);
        assert_eq!(ids(&groups[1].2), [6, 9]);
        assert!(titles.iter().all(|t| !t.contains("Empty")));
    }

    #[test]
    fn task_on_unknown_concept_is_listed_not_dropped() {
        let p = project();
        let tasks = [task(1, &[42])];
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = group_by_primary_concept(&p, &refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1, UNLINKED);
        assert_eq!(ids(&groups[0].2), [1]);
    }

    #[test]
    fn concept_tree_follows_sibling_order_and_nests() {
        let p = project();
        let tree = concept_tree(&p, None).unwrap();
        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        let names: Vec<&str> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Beta", "Alpha", "Empty"]);
        assert_eq!(root.children[0].children[0].name, "Leaf");
        assert!(concept_tree(&p, Some(ConceptId(42))).is_none());
        assert_eq!(
            concept_tree(&p, Some(ConceptId(3))).unwrap()[0].id,
            ConceptId(3)
        );
    }

    #[test]
    fn report_counts_hidden_and_records_states() {
        let mut p = project();
        p.add_task("a".into(), None, None, None).unwrap();
        p.add_task("b".into(), None, None, None).unwrap();
        p.set_task_state(TaskId(2), TaskState::Done).unwrap();
        let r = Report::build(&p, &[TaskState::Todo, TaskState::InProgress]);
        assert_eq!(r.states, [TaskState::Todo, TaskState::InProgress]);
        assert_eq!(r.tasks().count(), 1);
        assert_eq!(
            r.hidden,
            Hidden {
                todo: 0,
                in_progress: 0,
                done: 1
            }
        );
        assert_eq!(r.groups[0].title, UNLINKED);
        let listing = TaskListing::build(&p, &[TaskState::Done]);
        assert_eq!(listing.tasks.len(), 1);
        assert_eq!(listing.hidden.total(), 1);
    }

    #[test]
    fn task_row_due_is_in_project_timezone() {
        let mut p = Project::new();
        p.timezone = Some("Europe/Zurich".into());
        let due =
            crate::model::task::parse_due("2025-04-01T09:00[America/New_York]", "UTC").unwrap();
        p.add_task("a".into(), None, None, Some(due)).unwrap();
        let row = TaskRow::from_task(&p.tasks[0], &p);
        let due = row.due.unwrap();
        assert_eq!(due.time_zone().iana_name(), Some("Europe/Zurich"));
        assert_eq!(due.hour(), 15);
    }

    #[test]
    fn schedule_view_keeps_full_critical_path_when_narrowed() {
        let mut p = Project::new();
        p.add_task("a".into(), None, Some(2.0), None).unwrap();
        p.add_task("b".into(), None, Some(1.0), None).unwrap();
        p.add_task("c".into(), None, Some(0.5), None).unwrap();
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        let v = ScheduleView::build(&p, true).unwrap();
        assert!(v.has_durations());
        assert_eq!(v.duration, 3.0);
        assert_eq!(v.critical_path, [TaskId(1), TaskId(2)]);
        assert_eq!(v.tasks.len(), 2);
        let empty = ScheduleView::build(&Project::new(), false).unwrap();
        assert!(!empty.has_durations());
        assert!(empty.tasks.is_empty());
    }

    #[test]
    fn json_shape_is_stable() {
        let mut p = project();
        p.add_task("a".into(), Some("why".into()), Some(1.5), None)
            .unwrap();
        p.link_concept(TaskId(1), ConceptId(4)).unwrap();
        let r = Report::build(&p, &[TaskState::Todo]);
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["states"], serde_json::json!(["todo"]));
        assert_eq!(v["concepts"][0]["children"][0]["name"], "Beta");
        assert_eq!(v["groups"][0]["concept"], 4);
        assert_eq!(v["groups"][0]["title"], "Root › Beta › Leaf {4}");
        assert_eq!(v["groups"][0]["tasks"][0]["duration"], 1.5);
        assert_eq!(v["groups"][0]["tasks"][0]["description"], "why");
        assert_eq!(v["hidden"]["done"], 0);
        assert!(v["groups"][0]["tasks"][0].get("due").is_none());
    }
}
