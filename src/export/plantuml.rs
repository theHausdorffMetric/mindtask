//! PlantUML diagram generation from project data.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

use crate::graph::schedule::{Schedule, ScheduledTask, schedule};
use crate::model::concept::Concept;
use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;
use crate::model::task::{Task, TaskState};

// --- Label sanitising -------------------------------------------------------
//
// Names are user text and go straight into generated diagram syntax, which has
// no escape mechanism for its structural characters. PlantUML would either
// reject the output or — worse — accept a mangled version, so offending
// characters are substituted with visually equivalent safe ones rather than
// escaped.

/// Placeholder for a name that sanitises to nothing.
const UNNAMED: &str = "(unnamed)";

/// Collapse every whitespace run — newlines and tabs included — to a single
/// space, and trim. PlantUML is line-oriented: an embedded newline would split
/// one declaration across two lines, silently producing a different diagram.
fn one_line(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        UNNAMED.to_string()
    } else {
        joined
    }
}

/// Label for a `"…"`-quoted position (component names). A literal `"` closes
/// the string early; PlantUML has no escape for it, so it becomes an apostrophe.
fn quoted_label(s: &str) -> String {
    one_line(s).replace('"', "'")
}

/// Label for a `[…]`-delimited position (Gantt task names). Brackets would
/// unbalance the delimiter, so they become parentheses.
fn bracket_label(s: &str) -> String {
    one_line(s).replace('[', "(").replace(']', ")")
}

/// Generate a PlantUML mindmap of the concept tree.
///
/// If `root_id` is given, only the subtree rooted at that concept is rendered.
/// The caller must ensure the concept exists.
pub fn tree(project: &Project, root_id: Option<ConceptId>) -> String {
    let mut out = String::from("@startmindmap\n");

    match root_id {
        Some(id) => {
            let concept = project.get_concept(id).expect("caller validated existence");
            write_mindmap_node(&mut out, project, concept, 1);
        }
        None => {
            let roots = project.roots();
            if roots.len() == 1 {
                write_mindmap_node(&mut out, project, roots[0], 1);
            } else {
                writeln!(out, "* Project").unwrap();
                for root in roots {
                    write_mindmap_node(&mut out, project, root, 2);
                }
            }
        }
    }

    out.push_str("@endmindmap\n");
    out
}

fn write_mindmap_node(out: &mut String, project: &Project, concept: &Concept, depth: usize) {
    for _ in 0..depth {
        out.push('*');
    }
    writeln!(out, " {}", one_line(&concept.name)).unwrap();

    for child in project.children_of(concept.id) {
        write_mindmap_node(out, project, child, depth + 1);
    }
}

/// Generate a PlantUML component diagram of the task dependency DAG.
///
/// If `root_id` is given, only that task and all downstream dependents are shown.
/// The caller must ensure the task exists.
pub fn dag(project: &Project, root_id: Option<TaskId>) -> String {
    let tasks: Vec<&Task> = match root_id {
        Some(id) => collect_downstream(project, id),
        None => project.tasks.iter().collect(),
    };

    let mut out = String::from("@startuml\n");

    if !tasks.is_empty() {
        writeln!(out, "skinparam component {{").unwrap();
        writeln!(out, "  BackgroundColor<<done>> LightGreen").unwrap();
        writeln!(out, "  BackgroundColor<<in_progress>> Gold").unwrap();
        writeln!(out, "}}").unwrap();
        out.push('\n');
    }

    let task_ids: HashSet<TaskId> = tasks.iter().map(|t| t.id).collect();

    for task in &tasks {
        let stereotype = match task.state {
            TaskState::Done => " <<done>>",
            TaskState::InProgress => " <<in_progress>>",
            TaskState::Todo => "",
        };
        writeln!(
            out,
            "component \"{}\" as t{}{}",
            quoted_label(&task.name),
            task.id,
            stereotype
        )
        .unwrap();
    }

    if !tasks.is_empty() {
        out.push('\n');
    }

    for task in &tasks {
        for &dep in &task.depends_on {
            if task_ids.contains(&dep) {
                writeln!(out, "t{} --> t{}", dep, task.id).unwrap();
            }
        }
    }

    out.push_str("@enduml\n");
    out
}

/// Collect a task and all tasks that transitively depend on it.
fn collect_downstream(project: &Project, root_id: TaskId) -> Vec<&Task> {
    // Build reverse adjacency: task -> tasks that depend on it
    let mut dependents: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
    for task in &project.tasks {
        for &dep in &task.depends_on {
            dependents.entry(dep).or_default().push(task.id);
        }
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(root_id);
    queue.push_back(root_id);

    while let Some(id) = queue.pop_front() {
        if let Some(deps) = dependents.get(&id) {
            for &dep_id in deps {
                if visited.insert(dep_id) {
                    queue.push_back(dep_id);
                }
            }
        }
    }

    project
        .tasks
        .iter()
        .filter(|t| visited.contains(&t.id))
        .collect()
}

/// Generate a PlantUML Gantt chart.
///
/// When any task has a dependency, tasks are positioned by the computed CPM
/// schedule (relative days) and the critical path is highlighted. Otherwise it
/// falls back to a due-date chart (start = due − duration; tasks without a due
/// date are skipped).
pub fn gantt(project: &Project) -> String {
    let has_deps = project.tasks.iter().any(|t| !t.depends_on.is_empty());
    if has_deps {
        gantt_scheduled(project)
    } else {
        gantt_due(project)
    }
}

/// Schedule-driven Gantt: relative day positioning from the CPM schedule, with
/// the critical path highlighted (critical tasks take colour priority over state).
fn gantt_scheduled(project: &Project) -> String {
    let sched = match schedule(&project.tasks) {
        Ok(s) => s,
        Err(_) => return gantt_due(project), // cyclic input — pre-validate guards this
    };

    // Declarations (`lasts`/colour) first, then all `starts at` constraints:
    // PlantUML resolves task references line by line, so a constraint naming a
    // task that is only declared further down is an error (forward reference).
    let mut constraints = String::new();
    let mut out = String::from("@startgantt\n");
    for st in &sched.tasks {
        let Some(task) = project.get_task(st.id) else {
            continue;
        };
        let days = st.duration.round().max(0.0) as i64;
        // Declare once with an ID alias, then address the task only by that
        // alias. Names are not unique, so using them as identifiers merged
        // distinct tasks into one — and a dependency between two same-named
        // tasks became a self-referential constraint PlantUML happily drew.
        writeln!(
            out,
            "[{}] as [t{}] lasts {days} days",
            bracket_label(&task.name),
            st.id
        )
        .unwrap();

        if let Some(bind) = binding_predecessor(project, &sched, st) {
            writeln!(constraints, "[t{}] starts at [t{bind}]'s end", st.id).unwrap();
        }

        if st.critical {
            writeln!(out, "[t{}] is colored in Tomato", st.id).unwrap();
        } else {
            match task.state {
                TaskState::Done => {
                    writeln!(out, "[t{}] is colored in LightGreen", st.id).unwrap();
                }
                TaskState::InProgress => {
                    writeln!(out, "[t{}] is colored in Gold", st.id).unwrap();
                }
                TaskState::Todo => {}
            }
        }
    }
    out.push_str(&constraints);
    out.push_str("@endgantt\n");
    out
}

/// The predecessor whose finish sets this task's earliest start (ES = its EF),
/// used as the PlantUML start constraint. None when the task has no dependency.
///
/// Returns the predecessor's ID rather than its name: the constraint addresses
/// tasks by their `t<id>` alias, and names are neither unique nor stable.
fn binding_predecessor(project: &Project, sched: &Schedule, st: &ScheduledTask) -> Option<TaskId> {
    let task = project.get_task(st.id)?;
    task.depends_on
        .iter()
        .filter_map(|d| sched.get(*d).map(|sd| (*d, sd.earliest_finish)))
        .find(|(_, ef)| (ef - st.earliest_start).abs() < 1e-9)
        .map(|(d, _)| d)
}

/// Generate a PlantUML Gantt chart from tasks with due dates.
///
/// Tasks without a due date are skipped.
fn gantt_due(project: &Project) -> String {
    let mut out = String::from("@startgantt\n");

    let tasks_with_due: Vec<&Task> = project.tasks.iter().filter(|t| t.due.is_some()).collect();

    // PlantUML requires a project start date before absolute dates work.
    if let Some(earliest) = tasks_with_due
        .iter()
        .filter_map(|t| {
            let due = t.due.as_ref().unwrap();
            let days = t.duration.unwrap_or(1.0).ceil().max(1.0) as i64;
            due.checked_sub(jiff::Span::new().days(days)).ok()
        })
        .min_by_key(|z| z.timestamp())
    {
        writeln!(out, "Project starts {}", earliest.date()).unwrap();
    }

    for task in &tasks_with_due {
        let due = task.due.as_ref().unwrap();
        let duration_days = task.duration.unwrap_or(1.0).ceil().max(1.0) as i64;
        let span = jiff::Span::new().days(duration_days);
        let start = due.checked_sub(span).unwrap_or_else(|_| due.clone());

        writeln!(
            out,
            "[{}] as [t{}] starts {} and ends {}",
            bracket_label(&task.name),
            task.id,
            start.date(),
            due.date(),
        )
        .unwrap();

        match task.state {
            TaskState::Done => {
                writeln!(out, "[t{}] is colored in LightGreen", task.id).unwrap();
            }
            TaskState::InProgress => {
                writeln!(out, "[t{}] is colored in Gold", task.id).unwrap();
            }
            TaskState::Todo => {}
        }
    }

    out.push_str("@endgantt\n");
    out
}

/// Generate a PlantUML WBS diagram with concepts as structure and tasks as leaves.
///
/// Tasks linked to multiple concepts appear under each. Unlinked tasks are
/// grouped under a separate "Unlinked" node.
///
/// If `root_id` is given, only the subtree rooted at that concept is rendered
/// (the "Unlinked" group is omitted — those tasks belong to no subtree).
/// The caller must ensure the concept exists.
pub fn wbs(project: &Project, root_id: Option<ConceptId>) -> String {
    let mut out = String::from("@startwbs\n");

    // Build map: concept_id -> tasks linked to it
    let mut concept_tasks: HashMap<ConceptId, Vec<&Task>> = HashMap::new();
    let mut linked_task_ids: HashSet<TaskId> = HashSet::new();

    for task in &project.tasks {
        for &cid in &task.concepts {
            concept_tasks.entry(cid).or_default().push(task);
            linked_task_ids.insert(task.id);
        }
    }

    match root_id {
        Some(id) => {
            let concept = project.get_concept(id).expect("caller validated existence");
            write_wbs_concept(&mut out, project, concept, &concept_tasks, 1);
        }
        None => {
            let roots = project.roots();
            let unlinked: Vec<&Task> = project
                .tasks
                .iter()
                .filter(|t| !linked_task_ids.contains(&t.id))
                .collect();

            if roots.len() == 1 && unlinked.is_empty() {
                write_wbs_concept(&mut out, project, roots[0], &concept_tasks, 1);
            } else if !roots.is_empty() || !unlinked.is_empty() {
                writeln!(out, "* Project").unwrap();
                for root in &roots {
                    write_wbs_concept(&mut out, project, root, &concept_tasks, 2);
                }
                if !unlinked.is_empty() {
                    writeln!(out, "** Unlinked").unwrap();
                    for task in &unlinked {
                        writeln!(out, "*** {}", one_line(&task.name)).unwrap();
                    }
                }
            }
        }
    }

    out.push_str("@endwbs\n");
    out
}

fn write_wbs_concept(
    out: &mut String,
    project: &Project,
    concept: &Concept,
    concept_tasks: &HashMap<ConceptId, Vec<&Task>>,
    depth: usize,
) {
    for _ in 0..depth {
        out.push('*');
    }
    writeln!(out, " {}", one_line(&concept.name)).unwrap();

    // Tasks linked to this concept as leaves
    if let Some(tasks) = concept_tasks.get(&concept.id) {
        for task in tasks {
            for _ in 0..=depth {
                out.push('*');
            }
            writeln!(out, " {}", one_line(&task.name)).unwrap();
        }
    }

    for child in project.children_of(concept.id) {
        write_wbs_concept(out, project, child, concept_tasks, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::project::Project;

    fn sample_project() -> Project {
        let mut p = Project::new();
        // Concepts: Backend (1), API (2) under Backend, Frontend (3)
        p.add_concept("Backend".into(), None, None).unwrap();
        p.add_concept("API".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("Frontend".into(), None, None).unwrap();

        // Tasks: Design API (1), Build API (2), Build UI (3)
        p.add_task("Design API".into(), None, None, None).unwrap();
        p.add_task("Build API".into(), None, None, None).unwrap();
        p.add_task("Build UI".into(), None, None, None).unwrap();

        // Build API depends on Design API
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();

        // Link tasks to concepts
        p.link_concept(TaskId(1), ConceptId(2)).unwrap(); // Design API -> API
        p.link_concept(TaskId(2), ConceptId(2)).unwrap(); // Build API -> API
        p.link_concept(TaskId(3), ConceptId(3)).unwrap(); // Build UI -> Frontend

        p
    }

    #[test]
    fn tree_full() {
        let p = sample_project();
        let output = tree(&p, None);
        assert!(output.starts_with("@startmindmap\n"));
        assert!(output.ends_with("@endmindmap\n"));
        assert!(output.contains("* Project\n"));
        assert!(output.contains("** Backend\n"));
        assert!(output.contains("*** API\n"));
        assert!(output.contains("** Frontend\n"));
    }

    #[test]
    fn tree_single_root() {
        let mut p = Project::new();
        p.add_concept("Root".into(), None, None).unwrap();
        p.add_concept("Child".into(), Some(ConceptId(1)), None)
            .unwrap();
        let output = tree(&p, None);
        // Single root: no synthetic "Project" wrapper
        assert!(output.contains("* Root\n"));
        assert!(output.contains("** Child\n"));
        assert!(!output.contains("Project"));
    }

    #[test]
    fn tree_subtree() {
        let p = sample_project();
        let output = tree(&p, Some(ConceptId(1)));
        assert!(output.contains("* Backend\n"));
        assert!(output.contains("** API\n"));
        assert!(!output.contains("Frontend"));
    }

    #[test]
    fn tree_empty_project() {
        let p = Project::new();
        let output = tree(&p, None);
        assert!(output.starts_with("@startmindmap\n"));
        assert!(output.ends_with("@endmindmap\n"));
    }

    #[test]
    fn dag_full() {
        let p = sample_project();
        let output = dag(&p, None);
        assert!(output.starts_with("@startuml\n"));
        assert!(output.ends_with("@enduml\n"));
        assert!(output.contains("component \"Design API\" as t1"));
        assert!(output.contains("component \"Build API\" as t2"));
        assert!(output.contains("component \"Build UI\" as t3"));
        assert!(output.contains("t1 --> t2"));
    }

    #[test]
    fn dag_with_root() {
        let p = sample_project();
        // Root = task 1 (Design API), downstream = task 2 (Build API depends on it)
        let output = dag(&p, Some(TaskId(1)));
        assert!(output.contains("Design API"));
        assert!(output.contains("Build API"));
        assert!(!output.contains("Build UI"));
        assert!(output.contains("t1 --> t2"));
    }

    #[test]
    fn dag_leaf_task() {
        let p = sample_project();
        // Task 3 has no downstream dependents
        let output = dag(&p, Some(TaskId(3)));
        assert!(output.contains("Build UI"));
        assert!(!output.contains("Design API"));
        assert!(!output.contains("Build API"));
    }

    #[test]
    fn dag_colored_states() {
        let mut p = Project::new();
        p.add_task("Done task".into(), None, None, None).unwrap();
        p.add_task("WIP task".into(), None, None, None).unwrap();
        p.add_task("Todo task".into(), None, None, None).unwrap();
        p.set_task_state(TaskId(1), TaskState::Done).unwrap();
        p.set_task_state(TaskId(2), TaskState::InProgress).unwrap();
        let output = dag(&p, None);
        assert!(output.contains("t1 <<done>>"));
        assert!(output.contains("t2 <<in_progress>>"));
        // Todo tasks have no stereotype
        assert!(output.contains("\"Todo task\" as t3\n"));
    }

    #[test]
    fn dag_empty_project() {
        let p = Project::new();
        let output = dag(&p, None);
        assert_eq!(output, "@startuml\n@enduml\n");
    }

    #[test]
    fn gantt_tasks_with_due() {
        let mut p = Project::new();
        p.timezone = Some("UTC".into());
        let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
        p.add_task("Deploy".into(), None, Some(2.0), Some(due))
            .unwrap();
        let output = gantt(&p);
        assert!(output.starts_with("@startgantt\n"));
        assert!(output.ends_with("@endgantt\n"));
        assert!(output.contains("[Deploy] as [t1] starts 2025-03-13 and ends 2025-03-15"));
    }

    #[test]
    fn gantt_default_duration() {
        let mut p = Project::new();
        let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
        p.add_task("Quick task".into(), None, None, Some(due))
            .unwrap();
        let output = gantt(&p);
        assert!(output.contains("and ends 2025-03-15"));
    }

    #[test]
    fn gantt_skips_tasks_without_due() {
        let mut p = Project::new();
        p.add_task("No due".into(), None, None, None).unwrap();
        let output = gantt(&p);
        assert!(!output.contains("No due"));
    }

    #[test]
    fn gantt_colored_states() {
        let mut p = Project::new();
        let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
        p.add_task("Done".into(), None, None, Some(due.clone()))
            .unwrap();
        p.set_task_state(TaskId(1), TaskState::Done).unwrap();
        let output = gantt(&p);
        assert!(output.contains("[Done] as [t1] starts"));
        assert!(output.contains("[t1] is colored in LightGreen"));
    }

    #[test]
    fn gantt_empty() {
        let p = Project::new();
        let output = gantt(&p);
        assert_eq!(output, "@startgantt\n@endgantt\n");
    }

    #[test]
    fn gantt_scheduled_when_dependencies_exist() {
        // A(1) -> B(2): a single chain, so both are critical and relative.
        let mut p = Project::new();
        p.add_task("A".into(), None, Some(1.0), None).unwrap();
        p.add_task("B".into(), None, Some(2.0), None).unwrap();
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        let out = gantt(&p);
        assert!(out.contains("[A] as [t1] lasts 1 days"), "{out}");
        assert!(out.contains("[B] as [t2] lasts 2 days"), "{out}");
        assert!(out.contains("[t2] starts at [t1]'s end"), "{out}");
        assert!(out.contains("[t1] is colored in Tomato"), "{out}");
        assert!(out.contains("[t2] is colored in Tomato"), "{out}");
    }

    #[test]
    fn gantt_scheduled_colors_noncritical_by_state() {
        // A -> Long(5) is the critical path; A -> Short(1) has slack.
        let mut p = Project::new();
        p.add_task("A".into(), None, Some(1.0), None).unwrap(); // 1
        p.add_task("Long".into(), None, Some(5.0), None).unwrap(); // 2 (critical)
        p.add_task("Short".into(), None, Some(1.0), None).unwrap(); // 3 (slack)
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        p.add_dependency(TaskId(3), TaskId(1)).unwrap();
        p.set_task_state(TaskId(3), TaskState::InProgress).unwrap();
        let out = gantt(&p);
        assert!(out.contains("[t2] is colored in Tomato"), "{out}");
        // Off the critical path → keeps its workflow-state colour.
        assert!(out.contains("[t3] is colored in Gold"), "{out}");
    }

    #[test]
    fn gantt_scheduled_constraints_follow_all_declarations() {
        // Task 1 depends on task 2, so its constraint references a task that
        // schedule order declares later. PlantUML rejects forward references:
        // every `starts at` must come after the last `lasts` declaration.
        let mut p = Project::new();
        p.add_task("Late".into(), None, Some(1.0), None).unwrap(); // 1
        p.add_task("Early".into(), None, Some(2.0), None).unwrap(); // 2
        p.add_dependency(TaskId(1), TaskId(2)).unwrap();
        let out = gantt(&p);
        assert!(out.contains("[t1] starts at [t2]'s end"), "{out}");
        let lines: Vec<&str> = out.lines().collect();
        let last_decl = lines.iter().rposition(|l| l.contains("] lasts ")).unwrap();
        let first_constraint = lines
            .iter()
            .position(|l| l.contains("] starts at "))
            .unwrap();
        assert!(
            last_decl < first_constraint,
            "constraint emitted before a declaration:\n{out}"
        );
    }

    #[test]
    fn wbs_full() {
        let p = sample_project();
        let output = wbs(&p, None);
        assert!(output.starts_with("@startwbs\n"));
        assert!(output.ends_with("@endwbs\n"));
        assert!(output.contains("Backend"));
        assert!(output.contains("API"));
        assert!(output.contains("Design API"));
        assert!(output.contains("Build API"));
        assert!(output.contains("Frontend"));
        assert!(output.contains("Build UI"));
    }

    #[test]
    fn wbs_unlinked_tasks() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        p.add_task("Linked".into(), None, None, None).unwrap();
        p.add_task("Unlinked".into(), None, None, None).unwrap();
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        let output = wbs(&p, None);
        assert!(output.contains("Unlinked"));
    }

    #[test]
    fn wbs_empty() {
        let p = Project::new();
        let output = wbs(&p, None);
        assert_eq!(output, "@startwbs\n@endwbs\n");
    }

    #[test]
    fn wbs_subtree() {
        let p = sample_project();
        let output = wbs(&p, Some(ConceptId(1)));
        assert!(output.contains("* Backend\n"));
        assert!(output.contains("** API\n"));
        assert!(output.contains("Design API"));
        assert!(output.contains("Build API"));
        assert!(!output.contains("Frontend"));
        assert!(!output.contains("Build UI"));
        assert!(!output.contains("Project"));
    }

    #[test]
    fn wbs_subtree_omits_unlinked_group() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        p.add_task("Linked".into(), None, None, None).unwrap();
        p.add_task("Floating".into(), None, None, None).unwrap();
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        let output = wbs(&p, Some(ConceptId(1)));
        assert!(output.contains("Linked"));
        assert!(!output.contains("Floating"));
        assert!(!output.contains("Unlinked"));
    }

    #[test]
    fn wbs_task_under_multiple_concepts() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), None, None).unwrap();
        p.add_task("Shared".into(), None, None, None).unwrap();
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        p.link_concept(TaskId(1), ConceptId(2)).unwrap();
        let output = wbs(&p, None);
        // "Shared" appears under both A and B
        let count = output.matches("Shared").count();
        assert_eq!(count, 2);
    }

    // --- Hostile names -----------------------------------------------------
    //
    // Names are user text interpolated into generated syntax. Until 0.9.1 they
    // went in raw, so a quote, a bracket, or a newline produced broken output —
    // and two tasks sharing a name collapsed into one Gantt identifier. These
    // pin the guarantees rather than any particular rendering.

    /// Names that previously broke one or more diagram kinds.
    const HOSTILE: [&str; 6] = [
        r#"He said "hello""#,
        "Fix [urgent] bug",
        "line one\nline two",
        "   ",
        "tabs\tand\nnewlines",
        "trailing bracket ]",
    ];

    fn hostile_project() -> Project {
        let mut p = Project::new();
        p.timezone = Some("UTC".into());
        let root = p.add_concept("Root".into(), None, None).unwrap();
        for (i, name) in HOSTILE.iter().enumerate() {
            let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
            // `add_task` refuses these names since 0.11.0, but a hand-edited
            // file can still carry them, so set them behind the boundary check.
            let id = p
                .add_task(
                    format!("placeholder {i}"),
                    None,
                    Some(i as f64 + 1.0),
                    Some(due),
                )
                .unwrap();
            p.get_task_mut(id).unwrap().name = (*name).to_string();
            p.link_concept(id, root).unwrap();
        }
        p
    }

    /// Every declaration must occupy exactly one line: an embedded newline
    /// would otherwise split it and silently change the diagram.
    fn assert_no_stray_blank_or_split_lines(out: &str, what: &str) {
        for line in out.lines() {
            assert!(
                !line.starts_with("line two"),
                "{what}: a name's newline split a declaration:\n{out}"
            );
        }
    }

    #[test]
    fn hostile_names_keep_component_quotes_balanced() {
        let out = dag(&hostile_project(), None);
        for line in out.lines().filter(|l| l.starts_with("component ")) {
            assert_eq!(
                line.matches('"').count(),
                2,
                "unbalanced quotes in: {line}\n{out}"
            );
        }
        assert_no_stray_blank_or_split_lines(&out, "dag");
    }

    #[test]
    fn hostile_names_keep_gantt_brackets_balanced() {
        for out in [gantt(&hostile_project()), {
            // Also exercise the schedule-driven branch.
            let mut p = hostile_project();
            p.add_dependency(TaskId(2), TaskId(1)).unwrap();
            gantt(&p)
        }] {
            for line in out.lines().filter(|l| l.starts_with('[')) {
                assert_eq!(
                    line.matches('[').count(),
                    line.matches(']').count(),
                    "unbalanced brackets in: {line}\n{out}"
                );
            }
            assert_no_stray_blank_or_split_lines(&out, "gantt");
        }
    }

    #[test]
    fn hostile_names_do_not_break_mindmap_or_wbs_structure() {
        let p = hostile_project();
        for (what, out) in [("tree", tree(&p, None)), ("wbs", wbs(&p, None))] {
            for line in out.lines() {
                if line.starts_with("@") || line.is_empty() {
                    continue;
                }
                assert!(
                    line.starts_with('*'),
                    "{what}: a name leaked a line without a depth marker: {line:?}\n{out}"
                );
            }
        }
    }

    #[test]
    fn a_whitespace_only_name_still_gets_a_label() {
        let mut p = Project::new();
        let id = p.add_task("x".into(), None, Some(1.0), None).unwrap();
        p.get_task_mut(id).unwrap().name = "   ".into(); // bypasses validate_name
        let out = dag(&p, None);
        assert!(out.contains(UNNAMED), "{out}");
    }

    #[test]
    fn duplicate_task_names_stay_distinct_in_the_gantt() {
        // The C9 regression: identifying tasks by name merged these two and
        // emitted `[X] starts at [X]'s end` — a dependency that never existed.
        let mut p = Project::new();
        p.add_task("Same".into(), None, Some(1.0), None).unwrap(); // 1
        p.add_task("Same".into(), None, Some(1.0), None).unwrap(); // 2
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        let out = gantt(&p);

        assert!(out.contains("[Same] as [t1] lasts"), "{out}");
        assert!(out.contains("[Same] as [t2] lasts"), "{out}");
        assert!(out.contains("[t2] starts at [t1]'s end"), "{out}");
        // No constraint may name the same alias on both sides.
        for line in out.lines().filter(|l| l.contains("starts at")) {
            let (lhs, rhs) = line.split_once(" starts at ").unwrap();
            assert_ne!(
                lhs.trim(),
                rhs.trim_end_matches("'s end"),
                "self-dependency: {line}"
            );
        }
    }

    #[test]
    fn label_sanitisers_are_minimal() {
        // Ordinary names must pass through untouched — the fix must not
        // rewrite the 184 real names in a live project.
        for name in ["Plain name", "Fix (10.0.7.0/24) overlap", "a - b — c"] {
            assert_eq!(one_line(name), name);
            assert_eq!(quoted_label(name), name);
            assert_eq!(bracket_label(name), name);
        }
        assert_eq!(one_line("a\n b\tc"), "a b c");
        assert_eq!(quoted_label(r#"say "hi""#), "say 'hi'");
        assert_eq!(bracket_label("[x]"), "(x)");
        assert_eq!(one_line("  "), UNNAMED);
    }
}
