//! PlantUML diagram generation from project data.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

use crate::model::concept::Concept;
use crate::model::id::{ConceptId, TaskId};
use crate::model::project::Project;
use crate::model::task::{Task, TaskState};

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
    writeln!(out, " {}", concept.name).unwrap();

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
            task.name, task.id, stereotype
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

/// Generate a PlantUML Gantt chart from tasks with due dates.
///
/// Tasks without a due date are skipped.
pub fn gantt(project: &Project) -> String {
    let mut out = String::from("@startgantt\n");

    let tasks_with_due: Vec<&Task> = project.tasks.iter().filter(|t| t.due.is_some()).collect();

    // PlantUML requires a project start date before absolute dates work.
    if let Some(earliest) = tasks_with_due.iter().filter_map(|t| {
        let due = t.due.as_ref().unwrap();
        let days = t.duration.unwrap_or(1.0).ceil().max(1.0) as i64;
        due.checked_sub(jiff::Span::new().days(days)).ok()
    }).min_by_key(|z| z.timestamp()) {
        writeln!(out, "Project starts {}", earliest.date()).unwrap();
    }

    for task in &tasks_with_due {
        let due = task.due.as_ref().unwrap();
        let duration_days = task.duration.unwrap_or(1.0).ceil().max(1.0) as i64;
        let span = jiff::Span::new().days(duration_days);
        let start = due.checked_sub(span).unwrap_or_else(|_| due.clone());

        writeln!(
            out,
            "[{}] starts {} and ends {}",
            task.name,
            start.date(),
            due.date(),
        )
        .unwrap();

        match task.state {
            TaskState::Done => {
                writeln!(out, "[{}] is colored in LightGreen", task.name).unwrap();
            }
            TaskState::InProgress => {
                writeln!(out, "[{}] is colored in Gold", task.name).unwrap();
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
pub fn wbs(project: &Project) -> String {
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
                writeln!(out, "*** {}", task.name).unwrap();
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
    writeln!(out, " {}", concept.name).unwrap();

    // Tasks linked to this concept as leaves
    if let Some(tasks) = concept_tasks.get(&concept.id) {
        for task in tasks {
            for _ in 0..=depth {
                out.push('*');
            }
            writeln!(out, " {}", task.name).unwrap();
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
        p.add_task("Design API".into(), None, None, None);
        p.add_task("Build API".into(), None, None, None);
        p.add_task("Build UI".into(), None, None, None);

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
        p.add_task("Done task".into(), None, None, None);
        p.add_task("WIP task".into(), None, None, None);
        p.add_task("Todo task".into(), None, None, None);
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
        p.add_task("Deploy".into(), None, Some(2.0), Some(due));
        let output = gantt(&p);
        assert!(output.starts_with("@startgantt\n"));
        assert!(output.ends_with("@endgantt\n"));
        assert!(output.contains("[Deploy] starts 2025-03-13 and ends 2025-03-15"));
    }

    #[test]
    fn gantt_default_duration() {
        let mut p = Project::new();
        let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
        p.add_task("Quick task".into(), None, None, Some(due));
        let output = gantt(&p);
        assert!(output.contains("and ends 2025-03-15"));
    }

    #[test]
    fn gantt_skips_tasks_without_due() {
        let mut p = Project::new();
        p.add_task("No due".into(), None, None, None);
        let output = gantt(&p);
        assert!(!output.contains("No due"));
    }

    #[test]
    fn gantt_colored_states() {
        let mut p = Project::new();
        let due = crate::model::task::parse_due("2025-03-15", "UTC").unwrap();
        p.add_task("Done".into(), None, None, Some(due.clone()));
        p.set_task_state(TaskId(1), TaskState::Done).unwrap();
        let output = gantt(&p);
        assert!(output.contains("[Done] is colored in LightGreen"));
    }

    #[test]
    fn gantt_empty() {
        let p = Project::new();
        let output = gantt(&p);
        assert_eq!(output, "@startgantt\n@endgantt\n");
    }

    #[test]
    fn wbs_full() {
        let p = sample_project();
        let output = wbs(&p);
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
        p.add_task("Linked".into(), None, None, None);
        p.add_task("Unlinked".into(), None, None, None);
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        let output = wbs(&p);
        assert!(output.contains("Unlinked"));
    }

    #[test]
    fn wbs_empty() {
        let p = Project::new();
        let output = wbs(&p);
        assert_eq!(output, "@startwbs\n@endwbs\n");
    }

    #[test]
    fn wbs_task_under_multiple_concepts() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), None, None).unwrap();
        p.add_task("Shared".into(), None, None, None);
        p.link_concept(TaskId(1), ConceptId(1)).unwrap();
        p.link_concept(TaskId(1), ConceptId(2)).unwrap();
        let output = wbs(&p);
        // "Shared" appears under both A and B
        let count = output.matches("Shared").count();
        assert_eq!(count, 2);
    }
}
