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
    /// Some concepts are unreachable from any root (a cycle or orphaned parent
    /// chain), so a canonical renumbering cannot cover them.
    #[error("cannot normalize: {0} concept(s) unreachable from any root")]
    UnreachableConcepts(usize),
    /// Attempted to position a concept relative to itself.
    #[error("cannot position concept {0} relative to itself")]
    SelfPlacement(ConceptId),
}

/// Where to place a concept among its target parent's children when moving it.
/// The target parent is inferred from the anchor sibling, so the moved concept
/// always lands in the same sibling group as the anchor.
#[derive(Debug, Clone, Copy)]
pub enum Placement {
    /// Immediately before the anchor sibling.
    Before(ConceptId),
    /// Immediately after the anchor sibling.
    After(ConceptId),
}

impl Placement {
    /// The anchor sibling the placement is relative to.
    fn anchor(&self) -> ConceptId {
        match self {
            Placement::Before(a) | Placement::After(a) => *a,
        }
    }
}

/// A planned renumbering of concept IDs: `(old_id, new_id)` pairs ordered by the
/// new ID (`1..=n`, i.e. DFS pre-order). Produced by
/// [`Project::plan_normalization`] and consumed by
/// [`Project::apply_normalization`]; keeping the two apart lets `--dry-run`
/// inspect the plan without mutating anything.
#[derive(Debug, Clone)]
pub struct Renumbering {
    /// `(old_id, new_id)` for every concept, in new-ID order.
    pub pairs: Vec<(ConceptId, ConceptId)>,
}

impl Renumbering {
    /// True when every concept already holds its canonical ID (the ID mapping is
    /// the identity). Note this does not by itself imply the file is unchanged —
    /// the array may still need reordering — so callers detecting "no-op" should
    /// compare the resulting concepts, not rely on this alone.
    pub fn is_identity(&self) -> bool {
        self.pairs.iter().all(|(old, new)| old == new)
    }
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
    /// Column width used to wrap concept descriptions in tree output. When
    /// unset, the renderer detects the terminal width (falling back to 80).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_width: Option<u32>,
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
            wrap_width: None,
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
        self.next_concept_id = self.concepts.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
        self.next_task_id = self.tasks.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
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
        self.concepts
            .iter()
            .filter(|c| c.parent.is_none())
            .collect()
    }

    /// Concept IDs in DFS pre-order: roots in array order, each immediately
    /// followed by its subtree (children in array order), recursively.
    ///
    /// This is the canonical concept ordering used by `normalize`: it matches
    /// the order `mindtask tree` prints. Over a well-formed tree the result
    /// covers every concept exactly once; the `visited` guard keeps it
    /// terminating even on cyclic in-memory data (well-formed data has none,
    /// but callers may hold unvalidated input).
    pub fn dfs_preorder_ids(&self) -> Vec<ConceptId> {
        let mut out = Vec::with_capacity(self.concepts.len());
        let mut visited = std::collections::HashSet::new();
        // Seed the stack with roots reversed so they pop in array order.
        let mut stack: Vec<ConceptId> = self.roots().iter().rev().map(|c| c.id).collect();
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            out.push(id);
            // Push children reversed so they pop in array order.
            for child in self.children_of(id).iter().rev() {
                stack.push(child.id);
            }
        }
        out
    }

    /// Plan a canonical renumbering: assign IDs `1..=n` in DFS pre-order.
    ///
    /// Pure — does not mutate. Errors only when some concepts are unreachable
    /// from any root (a cycle or orphaned parent chain), which validated data
    /// never has; this defensive guard stops [`apply_normalization`] from
    /// silently dropping concepts if handed a malformed tree.
    pub fn plan_normalization(&self) -> Result<Renumbering> {
        let order = self.dfs_preorder_ids();
        if order.len() != self.concepts.len() {
            return Err(ProjectError::UnreachableConcepts(
                self.concepts.len() - order.len(),
            ));
        }
        let pairs = order
            .into_iter()
            .enumerate()
            .map(|(i, old)| (old, ConceptId(i as u64 + 1)))
            .collect();
        Ok(Renumbering { pairs })
    }

    /// Apply a [`Renumbering`]: reorder the `concepts` vector into the plan's
    /// (DFS pre-order) order, relabel each concept's ID, and rewrite every
    /// reference to a concept ID — `concept.parent` and each task's `concepts`
    /// links. Task dependencies (task→task) are untouched. ID counters are
    /// recomputed afterward.
    pub fn apply_normalization(&mut self, plan: &Renumbering) {
        let map: std::collections::HashMap<ConceptId, ConceptId> =
            plan.pairs.iter().copied().collect();
        self.concepts = plan
            .pairs
            .iter()
            .map(|(old, new)| {
                let c = self
                    .get_concept(*old)
                    .expect("renumbering plan references an existing concept");
                Concept {
                    id: *new,
                    name: c.name.clone(),
                    description: c.description.clone(),
                    parent: c.parent.map(|p| map[&p]),
                }
            })
            .collect();
        for task in &mut self.tasks {
            for cid in &mut task.concepts {
                if let Some(new) = map.get(cid) {
                    *cid = *new;
                }
            }
        }
        self.recompute_next_ids();
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
            return Err(ProjectError::ConceptReferencedByTasks(
                id,
                referencing_tasks,
            ));
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

    /// Re-parent a concept *and* position it among its new siblings by moving
    /// its element within the `concepts` vector (sibling order is array order).
    ///
    /// The new parent is taken from the anchor sibling, so the moved concept
    /// joins the anchor's sibling group. Returns `Err` if `id` or the anchor is
    /// missing, if the anchor is `id` itself, or if the move would create a
    /// cycle (same guard as [`move_concept`]).
    pub fn move_concept_positioned(&mut self, id: ConceptId, placement: Placement) -> Result<()> {
        let anchor = placement.anchor();
        if self.get_concept(id).is_none() {
            return Err(ProjectError::ConceptNotFound(id));
        }
        if anchor == id {
            return Err(ProjectError::SelfPlacement(id));
        }
        let new_parent = self
            .get_concept(anchor)
            .ok_or(ProjectError::ConceptNotFound(anchor))?
            .parent;

        // Same cycle guard as move_concept: the new parent can't be the concept
        // itself or one of its descendants.
        if let Some(pid) = new_parent
            && (pid == id || crate::graph::tree::is_ancestor(self, id, pid))
        {
            return Err(ProjectError::ConceptCycleDetected {
                id,
                new_parent: pid,
            });
        }

        // Pull the moving element out, re-parent it, and reinsert it relative to
        // the anchor's position in the now-shortened vector.
        let idx = self.concepts.iter().position(|c| c.id == id).unwrap();
        let mut elem = self.concepts.remove(idx);
        elem.parent = new_parent;
        let anchor_idx = self.concepts.iter().position(|c| c.id == anchor).unwrap();
        let insert_at = match placement {
            Placement::Before(_) => anchor_idx,
            Placement::After(_) => anchor_idx + 1,
        };
        self.concepts.insert(insert_at, elem);
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
        let err = p
            .move_concept(ConceptId(1), Some(ConceptId(1)))
            .unwrap_err();
        assert!(matches!(err, ProjectError::ConceptCycleDetected { .. }));
    }

    #[test]
    fn move_concept_prevents_cycle() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("C".into(), Some(ConceptId(2)), None).unwrap();
        // Moving A under C would create A->B->C->A cycle
        let err = p
            .move_concept(ConceptId(1), Some(ConceptId(3)))
            .unwrap_err();
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
        assert_eq!(
            p.get_concept(ConceptId(3)).unwrap().parent,
            Some(ConceptId(2))
        );
    }

    #[test]
    fn children_of_and_roots() {
        let mut p = Project::new();
        p.add_concept("R1".into(), None, None).unwrap();
        p.add_concept("R2".into(), None, None).unwrap();
        p.add_concept("C1".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("C2".into(), Some(ConceptId(1)), None)
            .unwrap();

        assert_eq!(p.roots().len(), 2);
        assert_eq!(p.children_of(ConceptId(1)).len(), 2);
        assert_eq!(p.children_of(ConceptId(2)).len(), 0);
    }

    // --- DFS pre-order tests ---

    /// Collect concept IDs as plain u64s for terse assertions.
    fn ids(p: &Project) -> Vec<u64> {
        p.dfs_preorder_ids().iter().map(|c| c.0).collect()
    }

    #[test]
    fn dfs_preorder_single_chain() {
        let mut p = Project::new();
        p.add_concept("root".into(), None, None).unwrap(); // 1
        p.add_concept("child".into(), Some(ConceptId(1)), None)
            .unwrap(); // 2
        p.add_concept("grandchild".into(), Some(ConceptId(2)), None)
            .unwrap(); // 3
        assert_eq!(ids(&p), vec![1, 2, 3]);
    }

    #[test]
    fn dfs_preorder_subtree_before_next_root() {
        let mut p = Project::new();
        p.add_concept("R1".into(), None, None).unwrap(); // 1
        p.add_concept("R2".into(), None, None).unwrap(); // 2
        p.add_concept("R1a".into(), Some(ConceptId(1)), None)
            .unwrap(); // 3
        p.add_concept("R1b".into(), Some(ConceptId(1)), None)
            .unwrap(); // 4
        p.add_concept("R2a".into(), Some(ConceptId(2)), None)
            .unwrap(); // 5
        // Pre-order: R1, its subtree (3,4), then R2, its subtree (5).
        assert_eq!(ids(&p), vec![1, 3, 4, 2, 5]);
    }

    #[test]
    fn dfs_preorder_follows_array_order_not_id_order() {
        // Force array order to diverge from ascending ID by constructing the
        // vector directly (the CLI can't yet — allocation is monotonic — but a
        // hand-edited file or positional `mv` can). Siblings sit as 3 then 1.
        let mut p = Project::new();
        let c = |id: u64, parent: Option<u64>| Concept {
            id: ConceptId(id),
            name: format!("c{id}"),
            description: None,
            parent: parent.map(ConceptId),
        };
        p.concepts.push(c(10, None));
        p.concepts.push(c(3, Some(10)));
        p.concepts.push(c(1, Some(10)));
        p.recompute_next_ids();
        // Siblings render in array order: 3 before 1, NOT ascending ID.
        assert_eq!(ids(&p), vec![10, 3, 1]);
        assert_eq!(ids(&p).len(), p.concepts.len());
    }

    // --- Normalization tests ---

    /// A deliberately non-canonical project: gappy IDs, a subtree stored
    /// non-contiguously (concept 9 is a child of 2 but sits after sibling 7),
    /// and a task linked to two concepts.
    ///
    /// Tree:  5(root) → { 2 → {9}, 7 }.  Array order: [5, 2, 7, 9].
    /// DFS pre-order is therefore [5, 2, 9, 7] — different from the array.
    fn scrambled_project() -> Project {
        let mut p = Project::new();
        let c = |id: u64, parent: Option<u64>| Concept {
            id: ConceptId(id),
            name: format!("c{id}"),
            description: None,
            parent: parent.map(ConceptId),
        };
        p.concepts.push(c(5, None));
        p.concepts.push(c(2, Some(5)));
        p.concepts.push(c(7, Some(5)));
        p.concepts.push(c(9, Some(2)));
        p.recompute_next_ids();
        let t = p.add_task("t".into(), None, None, None);
        p.get_task_mut(t).unwrap().concepts = vec![ConceptId(9), ConceptId(5)];
        p
    }

    #[test]
    fn normalize_renumbers_to_dfs_and_remaps_refs() {
        let mut p = scrambled_project();
        let plan = p.plan_normalization().unwrap();
        assert_eq!(
            plan.pairs,
            vec![
                (ConceptId(5), ConceptId(1)),
                (ConceptId(2), ConceptId(2)),
                (ConceptId(9), ConceptId(3)),
                (ConceptId(7), ConceptId(4)),
            ]
        );
        p.apply_normalization(&plan);

        // Array is now DFS pre-order with consecutive IDs 1..4, and the vector
        // was genuinely reordered: old c9 now precedes old c7.
        let array: Vec<(u64, &str)> = p
            .concepts
            .iter()
            .map(|c| (c.id.0, c.name.as_str()))
            .collect();
        assert_eq!(array, vec![(1, "c5"), (2, "c2"), (3, "c9"), (4, "c7")]);
        // Parents remapped: c2's parent 5→1; c9's parent 2→2; c7's parent 5→1.
        assert_eq!(
            p.get_concept(ConceptId(2)).unwrap().parent,
            Some(ConceptId(1))
        );
        assert_eq!(
            p.get_concept(ConceptId(3)).unwrap().parent,
            Some(ConceptId(2))
        );
        assert_eq!(
            p.get_concept(ConceptId(4)).unwrap().parent,
            Some(ConceptId(1))
        );
        // Task links remapped: 9→3, 5→1 (order preserved).
        assert_eq!(p.tasks[0].concepts, vec![ConceptId(3), ConceptId(1)]);
        // Structure stays valid; counter advanced to n+1.
        assert!(crate::graph::tree::validate_tree(&p).is_ok());
        assert_eq!(p.next_concept_id, 5);
    }

    #[test]
    fn normalize_is_idempotent() {
        let mut p = scrambled_project();
        let plan = p.plan_normalization().unwrap();
        p.apply_normalization(&plan);
        let once = p.concepts.clone();
        let links_once: Vec<_> = p.tasks.iter().map(|t| t.concepts.clone()).collect();

        let plan2 = p.plan_normalization().unwrap();
        assert!(plan2.is_identity());
        p.apply_normalization(&plan2);
        assert_eq!(p.concepts, once);
        let links_twice: Vec<_> = p.tasks.iter().map(|t| t.concepts.clone()).collect();
        assert_eq!(links_once, links_twice);
    }

    #[test]
    fn normalize_noop_on_already_canonical() {
        // Built via the normal API => already canonical (monotonic IDs, append).
        let mut p = Project::new();
        p.add_concept("root".into(), None, None).unwrap();
        p.add_concept("child".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("grandchild".into(), Some(ConceptId(2)), None)
            .unwrap();
        let before = p.concepts.clone();
        let plan = p.plan_normalization().unwrap();
        assert!(plan.is_identity());
        p.apply_normalization(&plan);
        assert_eq!(p.concepts, before);
    }

    #[test]
    fn normalize_leaves_task_dependencies_untouched() {
        let mut p = scrambled_project();
        let t2 = p.add_task("t2".into(), None, None, None);
        let t1 = p.tasks[0].id;
        p.get_task_mut(t2).unwrap().depends_on = vec![t1];
        let deps_before: Vec<_> = p.tasks.iter().map(|t| t.depends_on.clone()).collect();

        let plan = p.plan_normalization().unwrap();
        p.apply_normalization(&plan);

        let deps_after: Vec<_> = p.tasks.iter().map(|t| t.depends_on.clone()).collect();
        assert_eq!(deps_before, deps_after);
    }

    // --- Positional move tests ---

    /// root 1 → {a=2, b=3, c=4}; second root root2=5.
    fn forest() -> Project {
        let mut p = Project::new();
        p.add_concept("root".into(), None, None).unwrap();
        p.add_concept("a".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("b".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("c".into(), Some(ConceptId(1)), None).unwrap();
        p.add_concept("root2".into(), None, None).unwrap();
        p
    }

    fn child_order(p: &Project, parent: u64) -> Vec<u64> {
        p.children_of(ConceptId(parent))
            .iter()
            .map(|c| c.id.0)
            .collect()
    }

    #[test]
    fn move_before_reorders_siblings() {
        let mut p = forest();
        // c(4) before a(2): [2,3,4] -> [4,2,3]; 2 and 3 keep relative order.
        p.move_concept_positioned(ConceptId(4), Placement::Before(ConceptId(2)))
            .unwrap();
        assert_eq!(child_order(&p, 1), vec![4, 2, 3]);
    }

    #[test]
    fn move_after_reorders_siblings() {
        let mut p = forest();
        // a(2) after b(3): [2,3,4] -> [3,2,4].
        p.move_concept_positioned(ConceptId(2), Placement::After(ConceptId(3)))
            .unwrap();
        assert_eq!(child_order(&p, 1), vec![3, 2, 4]);
    }

    #[test]
    fn move_positioned_across_parents_reparents() {
        let mut p = forest();
        // a(2) before root2(5): 2 becomes a root, ordered before 5.
        p.move_concept_positioned(ConceptId(2), Placement::Before(ConceptId(5)))
            .unwrap();
        assert_eq!(p.get_concept(ConceptId(2)).unwrap().parent, None);
        let roots: Vec<u64> = p.roots().iter().map(|c| c.id.0).collect();
        assert_eq!(roots, vec![1, 2, 5]);
        assert_eq!(child_order(&p, 1), vec![3, 4]); // 1 lost child 2
    }

    #[test]
    fn move_positioned_rejects_self_anchor() {
        let mut p = forest();
        assert!(matches!(
            p.move_concept_positioned(ConceptId(2), Placement::Before(ConceptId(2))),
            Err(ProjectError::SelfPlacement(_))
        ));
    }

    #[test]
    fn move_positioned_rejects_missing_anchor() {
        let mut p = forest();
        assert!(matches!(
            p.move_concept_positioned(ConceptId(2), Placement::After(ConceptId(99))),
            Err(ProjectError::ConceptNotFound(_))
        ));
    }

    #[test]
    fn move_positioned_rejects_cycle() {
        let mut p = forest();
        p.add_concept("a-child".into(), Some(ConceptId(2)), None)
            .unwrap(); // 6, child of a(2)
        // Moving root(1) beside 6 would make 1 a child of 2 (6's parent) — a cycle.
        assert!(matches!(
            p.move_concept_positioned(ConceptId(1), Placement::Before(ConceptId(6))),
            Err(ProjectError::ConceptCycleDetected { .. })
        ));
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
        p.add_concept("A".into(), None, Some("old desc".into()))
            .unwrap();
        p.edit_concept(ConceptId(1), None, Some(None)).unwrap();
        assert!(p.get_concept(ConceptId(1)).unwrap().description.is_none());
    }

    #[test]
    fn edit_concept_not_found() {
        let mut p = Project::new();
        let err = p
            .edit_concept(ConceptId(99), Some("X".into()), None)
            .unwrap_err();
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
