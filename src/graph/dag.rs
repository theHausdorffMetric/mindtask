//! Task dependency DAG operations: cycle detection, topological sort, and validation.

use std::collections::HashMap;

use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;

use crate::model::id::TaskId;
use crate::model::task::Task;

/// Build a directed graph from the task list.
/// Returns the graph and a mapping from TaskId to NodeIndex.
pub fn build_task_graph(tasks: &[Task]) -> (DiGraph<TaskId, ()>, HashMap<TaskId, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut id_to_node = HashMap::new();

    for task in tasks {
        let node = graph.add_node(task.id);
        id_to_node.insert(task.id, node);
    }

    for task in tasks {
        if let Some(&task_node) = id_to_node.get(&task.id) {
            for &dep_id in &task.depends_on {
                if let Some(&dep_node) = id_to_node.get(&dep_id) {
                    // Edge from dependency to dependent (dep must finish before task)
                    graph.add_edge(dep_node, task_node, ());
                }
            }
        }
    }

    (graph, id_to_node)
}

/// Check if the task graph has a cycle.
pub fn has_cycle(tasks: &[Task]) -> bool {
    let (graph, _) = build_task_graph(tasks);
    petgraph::algo::is_cyclic_directed(&graph)
}

/// Return tasks in topological order (dependencies first).
pub fn topological_order(tasks: &[Task]) -> std::result::Result<Vec<TaskId>, String> {
    let (graph, _) = build_task_graph(tasks);
    match petgraph::algo::toposort(&graph, None) {
        Ok(sorted) => Ok(sorted.into_iter().map(|n| graph[n]).collect()),
        Err(_) => Err("task dependency graph contains a cycle".to_string()),
    }
}

/// Validate that all concept IDs and all task IDs are unique.
///
/// Normal CLI usage cannot produce duplicates (IDs are monotonically allocated),
/// but a hand-edited or merged project file can. Duplicates are checked first
/// because the rest of validation relies on first-match lookups that are
/// ambiguous when IDs collide.
pub fn validate_unique_ids(
    project: &crate::model::project::Project,
) -> std::result::Result<(), String> {
    let mut seen_concepts = std::collections::HashSet::new();
    for concept in &project.concepts {
        if !seen_concepts.insert(concept.id) {
            return Err(format!("duplicate concept ID {}", concept.id));
        }
    }

    let mut seen_tasks = std::collections::HashSet::new();
    for task in &project.tasks {
        if !seen_tasks.insert(task.id) {
            return Err(format!("duplicate task ID {}", task.id));
        }
    }

    Ok(())
}

/// Validate the task DAG:
/// - All depends_on references point to existing tasks
/// - No cycles
pub fn validate_dag(tasks: &[Task]) -> std::result::Result<(), String> {
    let task_ids: std::collections::HashSet<TaskId> =
        tasks.iter().map(|t| t.id).collect();

    for task in tasks {
        for dep_id in &task.depends_on {
            if !task_ids.contains(dep_id) {
                return Err(format!(
                    "task {} depends on non-existent task {}",
                    task.id, dep_id
                ));
            }
        }
    }

    if has_cycle(tasks) {
        return Err("task dependency graph contains a cycle".to_string());
    }

    Ok(())
}

/// Full project validation: unique IDs + tree + DAG + cross-references.
pub fn validate_project(project: &crate::model::project::Project) -> std::result::Result<(), String> {
    validate_unique_ids(project)?;
    crate::graph::tree::validate_tree(project)?;
    validate_dag(&project.tasks)?;

    // Validate concept references from tasks
    for task in &project.tasks {
        for concept_id in &task.concepts {
            if project.get_concept(*concept_id).is_none() {
                return Err(format!(
                    "task {} references non-existent concept {}",
                    task.id, concept_id
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::id::{ConceptId, TaskId};
    use crate::model::project::Project;

    #[test]
    fn topological_order_basic() {
        let mut p = Project::new();
        p.add_task("A".into(), None, None, None);
        p.add_task("B".into(), None, None, None);
        p.add_task("C".into(), None, None, None);
        p.add_dependency(TaskId(2), TaskId(1)).unwrap(); // B depends on A
        p.add_dependency(TaskId(3), TaskId(2)).unwrap(); // C depends on B

        let order = topological_order(&p.tasks).unwrap();
        let pos_a = order.iter().position(|&id| id == TaskId(1)).unwrap();
        let pos_b = order.iter().position(|&id| id == TaskId(2)).unwrap();
        let pos_c = order.iter().position(|&id| id == TaskId(3)).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn validate_project_valid() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        let t1 = p.add_task("A".into(), None, None, None);
        p.add_task("B".into(), None, None, None);
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        p.link_concept(t1, ConceptId(1)).unwrap();
        assert!(validate_project(&p).is_ok());
    }

    #[test]
    fn validate_dag_invalid_ref() {
        let mut p = Project::new();
        p.add_task("A".into(), None, None, None);
        // Manually add a bad dependency
        p.tasks[0].depends_on.push(TaskId(99));
        assert!(validate_dag(&p.tasks).is_err());
    }

    #[test]
    fn validate_project_bad_concept_ref() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        // Manually add bad concept reference
        p.get_task_mut(t1).unwrap().concepts.push(ConceptId(99));
        assert!(validate_project(&p).is_err());
    }

    #[test]
    fn validate_detects_duplicate_concept_id() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        // Inject a second concept sharing the same ID (simulates a hand-edited file).
        p.concepts.push(crate::model::concept::Concept {
            id: ConceptId(1),
            name: "Dup".into(),
            description: None,
            parent: None,
        });
        let err = validate_project(&p).unwrap_err();
        assert!(err.contains("duplicate concept ID 1"), "got: {err}");
    }

    #[test]
    fn validate_detects_duplicate_task_id() {
        let mut p = Project::new();
        p.add_task("A".into(), None, None, None);
        p.tasks.push(crate::model::task::Task {
            id: TaskId(1),
            name: "Dup".into(),
            description: None,
            duration: None,
            state: crate::model::task::TaskState::Todo,
            due: None,
            depends_on: vec![],
            concepts: vec![],
        });
        let err = validate_project(&p).unwrap_err();
        assert!(err.contains("duplicate task ID 1"), "got: {err}");
    }

    #[test]
    fn validate_unique_ids_accepts_clean_project() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), None, None).unwrap();
        p.add_task("T".into(), None, None, None);
        assert!(validate_unique_ids(&p).is_ok());
    }

    #[test]
    fn integration_complex_project() {
        let mut p = Project::new();
        // Build a concept tree
        p.add_concept("Project".into(), None, None).unwrap();
        p.add_concept("Backend".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("Frontend".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("API".into(), Some(ConceptId(2)), None)
            .unwrap();
        p.add_concept("Database".into(), Some(ConceptId(2)), None)
            .unwrap();

        // Build tasks
        p.add_task("Design API".into(), None, Some(2.0), None);
        p.add_task("Implement API".into(), None, Some(5.0), None);
        p.add_task("Design DB".into(), None, Some(1.0), None);
        p.add_task("Implement DB".into(), None, Some(3.0), None);
        p.add_task("Frontend prototype".into(), None, Some(4.0), None);

        // Dependencies
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        p.add_dependency(TaskId(4), TaskId(3)).unwrap();
        p.add_dependency(TaskId(2), TaskId(4)).unwrap();

        // Link tasks to concepts
        p.link_concept(TaskId(1), ConceptId(4)).unwrap();
        p.link_concept(TaskId(2), ConceptId(4)).unwrap();
        p.link_concept(TaskId(3), ConceptId(5)).unwrap();
        p.link_concept(TaskId(4), ConceptId(5)).unwrap();
        p.link_concept(TaskId(5), ConceptId(3)).unwrap();

        // Validate
        assert!(validate_project(&p).is_ok());

        // Check topological order
        let order = topological_order(&p.tasks).unwrap();
        assert_eq!(order.len(), 5);

        // Serialize and deserialize
        let json = serde_json::to_string_pretty(&p).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert!(validate_project(&loaded).is_ok());
    }
}
