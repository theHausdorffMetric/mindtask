//! The top-level project container and all mutation operations.
//!
//! [`Project`] owns the concept tree and task list, allocates IDs, and enforces
//! structural invariants (no cycles, no dangling references) on every mutation.

use serde::{Deserialize, Serialize};

use super::concept::Concept;
use super::id::{ConceptId, TaskId};
use super::task::{Task, TaskState};
use jiff::Zoned;

/// Errors from project mutation operations.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// A concept ID was not found in the project.
    #[error("concept {0} not found")]
    ConceptNotFound(ConceptId),
    /// A task ID was not found in the project.
    #[error("task {0} not found")]
    TaskNotFound(TaskId),
    /// Attempted to remove a concept that still has children.
    #[error("cannot remove concept {0}: it has child concepts")]
    ConceptHasChildren(ConceptId),
    /// Attempted to remove a concept that is still referenced by tasks.
    #[error("cannot remove concept {0}: tasks reference it: {1:?}")]
    ConceptReferencedByTasks(ConceptId, Vec<TaskId>),
    /// Moving a concept would create a cycle in the tree.
    #[error("cannot move concept {id} under {new_parent}: would create a cycle")]
    ConceptCycleDetected {
        /// The concept being moved.
        id: ConceptId,
        /// The proposed new parent.
        new_parent: ConceptId,
    },
    /// Adding a dependency would create a cycle in the task DAG.
    #[error("adding dependency {from} -> {to} would create a cycle")]
    DependencyCycle {
        /// The task that would gain the dependency.
        from: TaskId,
        /// The proposed dependency target.
        to: TaskId,
    },
    /// The dependency already exists.
    #[error("duplicate dependency: {0} already depends on {1}")]
    DuplicateDependency(TaskId, TaskId),
    /// Attempted to remove a dependency edge that does not exist.
    #[error("dependency does not exist: {0} does not depend on {1}")]
    DependencyNotFound(TaskId, TaskId),
    /// A task cannot depend on itself.
    #[error("a task cannot depend on itself: {0}")]
    SelfDependency(TaskId),
    /// Attempted to unlink a concept that the task is not linked to.
    #[error("task {0} is not linked to concept {1}")]
    ConceptNotLinked(TaskId, ConceptId),
}

/// Convenience alias for results from project operations.
pub type Result<T> = std::result::Result<T, ProjectError>;

/// Root container holding all concepts, tasks, and project metadata.
///
/// ID counters (`next_concept_id`, `next_task_id`) are not serialized — they
/// are recomputed from the stored data via [`recompute_next_ids`](Self::recompute_next_ids).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Schema version number.
    pub version: u32,
    /// Default IANA timezone for due-date parsing (e.g. `"America/New_York"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// All concepts in the project.
    pub concepts: Vec<Concept>,
    /// All tasks in the project.
    pub tasks: Vec<Task>,
    /// Next available concept ID (not serialized).
    #[serde(skip)]
    pub next_concept_id: u64,
    /// Next available task ID (not serialized).
    #[serde(skip)]
    pub next_task_id: u64,
}

impl Project {
    /// Create an empty project with version 1.
    pub fn new() -> Self {
        Self {
            version: 1,
            timezone: None,
            concepts: Vec::new(),
            tasks: Vec::new(),
            next_concept_id: 1,
            next_task_id: 1,
        }
    }

    /// Return the project timezone name, falling back to "UTC".
    pub fn timezone_or_utc(&self) -> &str {
        self.timezone.as_deref().unwrap_or("UTC")
    }

    /// Recompute next IDs from existing concepts and tasks.
    /// Called after deserialization.
    pub fn recompute_next_ids(&mut self) {
        self.next_concept_id = self
            .concepts
            .iter()
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            + 1;
        self.next_task_id = self
            .tasks
            .iter()
            .map(|t| t.id.0)
            .max()
            .unwrap_or(0)
            + 1;
    }

    /// Allocate and return the next unused [`ConceptId`].
    pub fn allocate_concept_id(&mut self) -> ConceptId {
        let id = ConceptId(self.next_concept_id);
        self.next_concept_id += 1;
        id
    }

    /// Allocate and return the next unused [`TaskId`].
    pub fn allocate_task_id(&mut self) -> TaskId {
        let id = TaskId(self.next_task_id);
        self.next_task_id += 1;
        id
    }

    // --- Concept operations ---

    /// Look up a concept by ID.
    pub fn get_concept(&self, id: ConceptId) -> Option<&Concept> {
        self.concepts.iter().find(|c| c.id == id)
    }

    /// Return the direct children of the given concept.
    pub fn children_of(&self, id: ConceptId) -> Vec<&Concept> {
        self.concepts
            .iter()
            .filter(|c| c.parent == Some(id))
            .collect()
    }

    /// Return all root concepts (those with no parent).
    pub fn roots(&self) -> Vec<&Concept> {
        self.concepts.iter().filter(|c| c.parent.is_none()).collect()
    }

    /// Add a new concept. Returns `Err` if the parent ID doesn't exist.
    pub fn add_concept(
        &mut self,
        name: String,
        parent: Option<ConceptId>,
        description: Option<String>,
    ) -> Result<ConceptId> {
        if let Some(pid) = parent
            && self.get_concept(pid).is_none()
        {
            return Err(ProjectError::ConceptNotFound(pid));
        }
        let id = self.allocate_concept_id();
        self.concepts.push(Concept {
            id,
            name,
            description,
            parent,
        });
        Ok(id)
    }

    /// Remove a concept. Returns `Err` if it has children or is referenced by tasks.
    pub fn remove_concept(&mut self, id: ConceptId) -> Result<()> {
        if self.get_concept(id).is_none() {
            return Err(ProjectError::ConceptNotFound(id));
        }

        let children = self.children_of(id);
        if !children.is_empty() {
            return Err(ProjectError::ConceptHasChildren(id));
        }

        let referencing_tasks: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|t| t.concepts.contains(&id))
            .map(|t| t.id)
            .collect();
        if !referencing_tasks.is_empty() {
            return Err(ProjectError::ConceptReferencedByTasks(id, referencing_tasks));
        }

        self.concepts.retain(|c| c.id != id);
        Ok(())
    }

    /// Edit a concept's name and/or description.
    pub fn edit_concept(
        &mut self,
        id: ConceptId,
        name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<()> {
        let concept = self
            .concepts
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ProjectError::ConceptNotFound(id))?;
        if let Some(n) = name {
            concept.name = n;
        }
        if let Some(d) = description {
            concept.description = d;
        }
        Ok(())
    }

    /// Re-parent a concept. Returns `Err` if the move would create a cycle.
    pub fn move_concept(&mut self, id: ConceptId, new_parent: Option<ConceptId>) -> Result<()> {
        if self.get_concept(id).is_none() {
            return Err(ProjectError::ConceptNotFound(id));
        }

        if let Some(pid) = new_parent {
            if self.get_concept(pid).is_none() {
                return Err(ProjectError::ConceptNotFound(pid));
            }
            if pid == id {
                return Err(ProjectError::ConceptCycleDetected {
                    id,
                    new_parent: pid,
                });
            }
            if crate::graph::tree::is_ancestor(self, id, pid) {
                return Err(ProjectError::ConceptCycleDetected {
                    id,
                    new_parent: pid,
                });
            }
        }

        let concept = self.concepts.iter_mut().find(|c| c.id == id).unwrap();
        concept.parent = new_parent;
        Ok(())
    }

    // --- Task operations ---

    /// Look up a task by ID.
    pub fn get_task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Look up a task by ID (mutable).
    pub fn get_task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Edit a task's name, description, and/or duration.
    pub fn edit_task(
        &mut self,
        id: TaskId,
        name: Option<String>,
        description: Option<Option<String>>,
        duration: Option<Option<f64>>,
    ) -> Result<()> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(ProjectError::TaskNotFound(id))?;
        if let Some(n) = name {
            task.name = n;
        }
        if let Some(d) = description {
            task.description = d;
        }
        if let Some(d) = duration {
            task.duration = d;
        }
        Ok(())
    }

    /// Add a new task with state `Todo` and no dependencies or concepts.
    pub fn add_task(
        &mut self,
        name: String,
        description: Option<String>,
        duration: Option<f64>,
        due: Option<Zoned>,
    ) -> TaskId {
        let id = self.allocate_task_id();
        self.tasks.push(Task {
            id,
            name,
            description,
            duration,
            state: TaskState::default(),
            due,
            depends_on: Vec::new(),
            concepts: Vec::new(),
        });
        id
    }

    /// Remove a task and clean up any references to it in other tasks' dependency lists.
    pub fn remove_task(&mut self, id: TaskId) -> Result<()> {
        if self.get_task(id).is_none() {
            return Err(ProjectError::TaskNotFound(id));
        }
        self.tasks.retain(|t| t.id != id);
        // Clean up references from other tasks
        for task in &mut self.tasks {
            task.depends_on.retain(|&dep| dep != id);
        }
        Ok(())
    }

    /// Add a dependency edge. Returns `Err` if it would create a cycle, is a duplicate, or is a self-loop.
    pub fn add_dependency(&mut self, task_id: TaskId, depends_on_id: TaskId) -> Result<()> {
        if task_id == depends_on_id {
            return Err(ProjectError::SelfDependency(task_id));
        }
        if self.get_task(task_id).is_none() {
            return Err(ProjectError::TaskNotFound(task_id));
        }
        if self.get_task(depends_on_id).is_none() {
            return Err(ProjectError::TaskNotFound(depends_on_id));
        }

        let task = self.get_task(task_id).unwrap();
        if task.depends_on.contains(&depends_on_id) {
            return Err(ProjectError::DuplicateDependency(task_id, depends_on_id));
        }

        // Temporarily add the dependency to check for cycles
        let task = self.get_task_mut(task_id).unwrap();
        task.depends_on.push(depends_on_id);

        if crate::graph::dag::has_cycle(&self.tasks) {
            // Roll back
            let task = self.get_task_mut(task_id).unwrap();
            task.depends_on.pop();
            return Err(ProjectError::DependencyCycle {
                from: task_id,
                to: depends_on_id,
            });
        }

        Ok(())
    }

    /// Remove a dependency edge. Returns `Err` if the edge does not exist.
    pub fn remove_dependency(&mut self, task_id: TaskId, depends_on_id: TaskId) -> Result<()> {
        let task = self
            .get_task_mut(task_id)
            .ok_or(ProjectError::TaskNotFound(task_id))?;
        if !task.depends_on.contains(&depends_on_id) {
            return Err(ProjectError::DependencyNotFound(task_id, depends_on_id));
        }
        task.depends_on.retain(|&d| d != depends_on_id);
        Ok(())
    }

    /// Tag a task with a concept. Idempotent — linking twice is not an error.
    pub fn link_concept(&mut self, task_id: TaskId, concept_id: ConceptId) -> Result<()> {
        if self.get_concept(concept_id).is_none() {
            return Err(ProjectError::ConceptNotFound(concept_id));
        }
        let task = self
            .get_task_mut(task_id)
            .ok_or(ProjectError::TaskNotFound(task_id))?;
        if !task.concepts.contains(&concept_id) {
            task.concepts.push(concept_id);
        }
        Ok(())
    }

    /// Remove a concept tag from a task. Returns `Err` if the task is not
    /// linked to the given concept.
    pub fn unlink_concept(&mut self, task_id: TaskId, concept_id: ConceptId) -> Result<()> {
        let task = self
            .get_task_mut(task_id)
            .ok_or(ProjectError::TaskNotFound(task_id))?;
        if !task.concepts.contains(&concept_id) {
            return Err(ProjectError::ConceptNotLinked(task_id, concept_id));
        }
        task.concepts.retain(|&c| c != concept_id);
        Ok(())
    }

    /// Update a task's workflow state.
    pub fn set_task_state(&mut self, id: TaskId, state: TaskState) -> Result<()> {
        let task = self
            .get_task_mut(id)
            .ok_or(ProjectError::TaskNotFound(id))?;
        task.state = state;
        Ok(())
    }

    /// Set or clear a task's due date.
    pub fn set_task_due(&mut self, id: TaskId, due: Option<Zoned>) -> Result<()> {
        let task = self
            .get_task_mut(id)
            .ok_or(ProjectError::TaskNotFound(id))?;
        task.due = due;
        Ok(())
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project() {
        let p = Project::new();
        assert_eq!(p.version, 1);
        assert!(p.concepts.is_empty());
        assert!(p.tasks.is_empty());
        assert_eq!(p.next_concept_id, 1);
        assert_eq!(p.next_task_id, 1);
    }

    #[test]
    fn allocate_ids() {
        let mut p = Project::new();
        assert_eq!(p.allocate_concept_id(), ConceptId(1));
        assert_eq!(p.allocate_concept_id(), ConceptId(2));
        assert_eq!(p.allocate_task_id(), TaskId(1));
        assert_eq!(p.allocate_task_id(), TaskId(2));
    }

    #[test]
    fn project_serde_roundtrip() {
        let mut p = Project::new();
        p.add_concept("Root".into(), None, None).unwrap();
        p.add_concept("Child".into(), Some(ConceptId(1)), Some("desc".into()))
            .unwrap();
        p.add_task("Do thing".into(), None, Some(1.5), None);

        let json = serde_json::to_string_pretty(&p).unwrap();
        let mut parsed: Project = serde_json::from_str(&json).unwrap();
        parsed.recompute_next_ids();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.concepts.len(), 2);
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.next_concept_id, 3);
        assert_eq!(parsed.next_task_id, 2);
    }

    // --- Concept CRUD tests ---

    #[test]
    fn add_concept_with_invalid_parent() {
        let mut p = Project::new();
        let err = p
            .add_concept("Orphan".into(), Some(ConceptId(99)), None)
            .unwrap_err();
        assert!(matches!(err, ProjectError::ConceptNotFound(_)));
    }

    #[test]
    fn remove_concept_with_children_fails() {
        let mut p = Project::new();
        p.add_concept("Parent".into(), None, None).unwrap();
        p.add_concept("Child".into(), Some(ConceptId(1)), None)
            .unwrap();
        let err = p.remove_concept(ConceptId(1)).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptHasChildren(_)));
    }

    #[test]
    fn remove_concept_referenced_by_task_fails() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        let tid = p.add_task("Work".into(), None, None, None);
        p.link_concept(tid, ConceptId(1)).unwrap();
        let err = p.remove_concept(ConceptId(1)).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptReferencedByTasks(_, _)));
    }

    #[test]
    fn remove_leaf_concept_succeeds() {
        let mut p = Project::new();
        p.add_concept("Leaf".into(), None, None).unwrap();
        p.remove_concept(ConceptId(1)).unwrap();
        assert!(p.concepts.is_empty());
    }

    #[test]
    fn move_concept_prevents_self_parent() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        let err = p.move_concept(ConceptId(1), Some(ConceptId(1))).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptCycleDetected { .. }));
    }

    #[test]
    fn move_concept_prevents_cycle() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("C".into(), Some(ConceptId(2)), None).unwrap();
        // Moving A under C would create A->B->C->A cycle
        let err = p.move_concept(ConceptId(1), Some(ConceptId(3))).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptCycleDetected { .. }));
    }

    #[test]
    fn move_concept_succeeds() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), None, None).unwrap();
        p.add_concept("C".into(), Some(ConceptId(1)), None).unwrap();
        // Move C from under A to under B
        p.move_concept(ConceptId(3), Some(ConceptId(2))).unwrap();
        assert_eq!(p.get_concept(ConceptId(3)).unwrap().parent, Some(ConceptId(2)));
    }

    #[test]
    fn children_of_and_roots() {
        let mut p = Project::new();
        p.add_concept("R1".into(), None, None).unwrap();
        p.add_concept("R2".into(), None, None).unwrap();
        p.add_concept("C1".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("C2".into(), Some(ConceptId(1)), None).unwrap();

        assert_eq!(p.roots().len(), 2);
        assert_eq!(p.children_of(ConceptId(1)).len(), 2);
        assert_eq!(p.children_of(ConceptId(2)).len(), 0);
    }

    // --- Task CRUD tests ---

    #[test]
    fn add_and_remove_task() {
        let mut p = Project::new();
        let id = p.add_task("Test".into(), None, None, None);
        assert_eq!(id, TaskId(1));
        assert_eq!(p.tasks.len(), 1);
        p.remove_task(id).unwrap();
        assert!(p.tasks.is_empty());
    }

    #[test]
    fn remove_task_cleans_up_deps() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let t2 = p.add_task("B".into(), None, None, None);
        p.add_dependency(t2, t1).unwrap();
        assert_eq!(p.get_task(t2).unwrap().depends_on.len(), 1);
        p.remove_task(t1).unwrap();
        assert!(p.get_task(t2).unwrap().depends_on.is_empty());
    }

    #[test]
    fn self_dependency_fails() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let err = p.add_dependency(t1, t1).unwrap_err();
        assert!(matches!(err, ProjectError::SelfDependency(_)));
    }

    #[test]
    fn remove_dependency_missing_edge_fails() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let t2 = p.add_task("B".into(), None, None, None);
        // No edge between them yet.
        let err = p.remove_dependency(t2, t1).unwrap_err();
        assert!(matches!(err, ProjectError::DependencyNotFound(_, _)));
    }

    #[test]
    fn remove_dependency_task_not_found_fails() {
        let mut p = Project::new();
        let err = p.remove_dependency(TaskId(99), TaskId(1)).unwrap_err();
        assert!(matches!(err, ProjectError::TaskNotFound(_)));
    }

    #[test]
    fn remove_dependency_succeeds() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let t2 = p.add_task("B".into(), None, None, None);
        p.add_dependency(t2, t1).unwrap();
        p.remove_dependency(t2, t1).unwrap();
        assert!(p.get_task(t2).unwrap().depends_on.is_empty());
    }

    #[test]
    fn duplicate_dependency_fails() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let t2 = p.add_task("B".into(), None, None, None);
        p.add_dependency(t2, t1).unwrap();
        let err = p.add_dependency(t2, t1).unwrap_err();
        assert!(matches!(err, ProjectError::DuplicateDependency(_, _)));
    }

    #[test]
    fn cycle_detection() {
        let mut p = Project::new();
        let t1 = p.add_task("A".into(), None, None, None);
        let t2 = p.add_task("B".into(), None, None, None);
        let t3 = p.add_task("C".into(), None, None, None);
        p.add_dependency(t2, t1).unwrap(); // B depends on A
        p.add_dependency(t3, t2).unwrap(); // C depends on B
        let err = p.add_dependency(t1, t3).unwrap_err(); // A depends on C -> cycle!
        assert!(matches!(err, ProjectError::DependencyCycle { .. }));
        // Verify t1 was not modified
        assert!(p.get_task(t1).unwrap().depends_on.is_empty());
    }

    #[test]
    fn link_unlink_concept() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        let t1 = p.add_task("Work".into(), None, None, None);
        p.link_concept(t1, ConceptId(1)).unwrap();
        assert_eq!(p.get_task(t1).unwrap().concepts.len(), 1);
        // Idempotent
        p.link_concept(t1, ConceptId(1)).unwrap();
        assert_eq!(p.get_task(t1).unwrap().concepts.len(), 1);
        // Unlink
        p.unlink_concept(t1, ConceptId(1)).unwrap();
        assert!(p.get_task(t1).unwrap().concepts.is_empty());
    }

    #[test]
    fn unlink_concept_not_linked_fails() {
        let mut p = Project::new();
        p.add_concept("Topic".into(), None, None).unwrap();
        let t1 = p.add_task("Work".into(), None, None, None);
        // Never linked: unlinking should error rather than silently succeed.
        let err = p.unlink_concept(t1, ConceptId(1)).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptNotLinked(_, _)));
    }

    #[test]
    fn unlink_concept_task_not_found_fails() {
        let mut p = Project::new();
        let err = p.unlink_concept(TaskId(99), ConceptId(1)).unwrap_err();
        assert!(matches!(err, ProjectError::TaskNotFound(_)));
    }

    #[test]
    fn edit_concept_name_and_description() {
        let mut p = Project::new();
        p.add_concept("Old".into(), None, None).unwrap();
        p.edit_concept(ConceptId(1), Some("New".into()), Some(Some("desc".into())))
            .unwrap();
        let c = p.get_concept(ConceptId(1)).unwrap();
        assert_eq!(c.name, "New");
        assert_eq!(c.description.as_deref(), Some("desc"));
    }

    #[test]
    fn edit_concept_clear_description() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, Some("old desc".into())).unwrap();
        p.edit_concept(ConceptId(1), None, Some(None)).unwrap();
        assert!(p.get_concept(ConceptId(1)).unwrap().description.is_none());
    }

    #[test]
    fn edit_concept_not_found() {
        let mut p = Project::new();
        let err = p.edit_concept(ConceptId(99), Some("X".into()), None).unwrap_err();
        assert!(matches!(err, ProjectError::ConceptNotFound(_)));
    }

    #[test]
    fn edit_task_name_and_description() {
        let mut p = Project::new();
        let id = p.add_task("Old".into(), None, Some(1.0), None);
        p.edit_task(id, Some("New".into()), Some(Some("desc".into())), None)
            .unwrap();
        let t = p.get_task(id).unwrap();
        assert_eq!(t.name, "New");
        assert_eq!(t.description.as_deref(), Some("desc"));
        assert_eq!(t.duration, Some(1.0)); // unchanged
    }

    #[test]
    fn edit_task_clear_description_and_duration() {
        let mut p = Project::new();
        let id = p.add_task("T".into(), Some("desc".into()), Some(2.0), None);
        p.edit_task(id, None, Some(None), Some(None)).unwrap();
        let t = p.get_task(id).unwrap();
        assert!(t.description.is_none());
        assert!(t.duration.is_none());
    }

    #[test]
    fn edit_task_not_found() {
        let mut p = Project::new();
        let err = p
            .edit_task(TaskId(99), Some("X".into()), None, None)
            .unwrap_err();
        assert!(matches!(err, ProjectError::TaskNotFound(_)));
    }

    #[test]
    fn set_task_state() {
        let mut p = Project::new();
        let t1 = p.add_task("Work".into(), None, None, None);
        assert_eq!(p.get_task(t1).unwrap().state, TaskState::Todo);
        p.set_task_state(t1, TaskState::InProgress).unwrap();
        assert_eq!(p.get_task(t1).unwrap().state, TaskState::InProgress);
        p.set_task_state(t1, TaskState::Done).unwrap();
        assert_eq!(p.get_task(t1).unwrap().state, TaskState::Done);
    }
}
