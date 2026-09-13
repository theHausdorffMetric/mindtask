//! Critical Path Method (CPM) scheduling over the task DAG.
//!
//! Given finish-to-start dependencies and per-task durations (in days), a forward
//! pass computes the earliest each task can start/finish, a backward pass the
//! latest it can without delaying the project, and the difference is its
//! **slack**. Tasks with zero slack form the **critical path** — the chain that
//! sets the project duration. All values are derived; nothing is persisted.
//!
//! CPM constrains only on dependencies (unlimited resources). Tasks with no
//! duration are treated as 0-day milestones. See `docs/PHASE-6-PLAN.md`.

use std::collections::HashMap;

use crate::graph::dag::topological_order;
use crate::model::id::TaskId;
use crate::model::task::Task;

/// Slack within this of zero counts as critical (guards f64 rounding).
const EPSILON: f64 = 1e-9;

/// One task's computed schedule. Times are day-offsets from project start.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledTask {
    pub id: TaskId,
    /// Resolved duration in days (0.0 for a no-duration milestone).
    pub duration: f64,
    pub earliest_start: f64,
    pub earliest_finish: f64,
    pub latest_start: f64,
    pub latest_finish: f64,
    /// `latest_start - earliest_start`; zero means critical.
    pub slack: f64,
    pub critical: bool,
}

/// The whole-project schedule, tasks in project (insertion) order.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub tasks: Vec<ScheduledTask>,
    /// Project length in days (the maximum earliest-finish).
    pub duration: f64,
    /// Critical tasks in topological order (dependencies first).
    critical: Vec<TaskId>,
}

impl Schedule {
    pub fn get(&self, id: TaskId) -> Option<&ScheduledTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Critical tasks (zero slack) in topological order. Note that when several
    /// chains tie for longest, this returns every critical task, not one chain.
    pub fn critical_path(&self) -> &[TaskId] {
        &self.critical
    }
}

/// Compute the CPM schedule. Returns `Err` if the dependencies contain a cycle
/// (a valid project never does — `validate_dag` rejects cycles on load).
pub fn schedule(tasks: &[Task]) -> Result<Schedule, String> {
    // Dependencies-first order; also our cycle guard.
    let order = topological_order(tasks)?;

    let by_id: HashMap<TaskId, &Task> = tasks.iter().map(|t| (t.id, t)).collect();
    let dur = |id: TaskId| by_id.get(&id).and_then(|t| t.duration).unwrap_or(0.0);

    // Invert depends_on into a successors map.
    let mut successors: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
    for t in tasks {
        for &dep in &t.depends_on {
            successors.entry(dep).or_default().push(t.id);
        }
    }

    // Forward pass: earliest start/finish.
    let mut es: HashMap<TaskId, f64> = HashMap::new();
    let mut ef: HashMap<TaskId, f64> = HashMap::new();
    for &id in &order {
        let t = by_id[&id];
        // Dangling deps (shouldn't occur in a valid project) are skipped.
        let start = t
            .depends_on
            .iter()
            .filter_map(|d| ef.get(d).copied())
            .fold(0.0_f64, f64::max);
        es.insert(id, start);
        ef.insert(id, start + dur(id));
    }
    let project = ef.values().copied().fold(0.0_f64, f64::max);

    // Backward pass: latest start/finish (reverse topological order).
    let mut ls: HashMap<TaskId, f64> = HashMap::new();
    let mut lf: HashMap<TaskId, f64> = HashMap::new();
    for &id in order.iter().rev() {
        let finish = match successors.get(&id) {
            Some(s) if !s.is_empty() => s
                .iter()
                .filter_map(|x| ls.get(x).copied())
                .fold(f64::INFINITY, f64::min),
            _ => project,
        };
        lf.insert(id, finish);
        ls.insert(id, finish - dur(id));
    }

    // Assemble per-task results in project order.
    let scheduled: Vec<ScheduledTask> = tasks
        .iter()
        .map(|t| {
            let id = t.id;
            let slack = ls[&id] - es[&id];
            ScheduledTask {
                id,
                duration: dur(id),
                earliest_start: es[&id],
                earliest_finish: ef[&id],
                latest_start: ls[&id],
                latest_finish: lf[&id],
                slack,
                critical: slack.abs() < EPSILON,
            }
        })
        .collect();

    // Critical tasks in topological order.
    let critical: Vec<TaskId> = order
        .iter()
        .copied()
        .filter(|id| (ls[id] - es[id]).abs() < EPSILON)
        .collect();

    Ok(Schedule {
        tasks: scheduled,
        duration: project,
        critical,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::id::TaskId;
    use crate::model::project::Project;
    use crate::model::task::{Task, TaskState};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Assert a task's full schedule row.
    #[allow(clippy::too_many_arguments)] // test helper: one arg per CPM field
    fn assert_row(
        s: &Schedule,
        id: u64,
        es: f64,
        ef: f64,
        ls: f64,
        lf: f64,
        slack: f64,
        crit: bool,
    ) {
        let t = s
            .get(TaskId(id))
            .unwrap_or_else(|| panic!("task {id} missing"));
        assert!(
            approx(t.earliest_start, es),
            "task {id} ES: {} != {es}",
            t.earliest_start
        );
        assert!(
            approx(t.earliest_finish, ef),
            "task {id} EF: {} != {ef}",
            t.earliest_finish
        );
        assert!(
            approx(t.latest_start, ls),
            "task {id} LS: {} != {ls}",
            t.latest_start
        );
        assert!(
            approx(t.latest_finish, lf),
            "task {id} LF: {} != {lf}",
            t.latest_finish
        );
        assert!(
            approx(t.slack, slack),
            "task {id} slack: {} != {slack}",
            t.slack
        );
        assert_eq!(t.critical, crit, "task {id} critical");
    }

    #[test]
    fn worked_example_matches_hand_computation() {
        // A(2) E(4) independent; B(1)->C(3)->D(5); D also depends on A.
        let mut p = Project::new();
        p.add_task("A Design API".into(), None, Some(2.0), None)
            .unwrap(); // 1
        p.add_task("B Design DB".into(), None, Some(1.0), None)
            .unwrap(); // 2
        p.add_task("C Implement DB".into(), None, Some(3.0), None)
            .unwrap(); // 3
        p.add_task("D Implement API".into(), None, Some(5.0), None)
            .unwrap(); // 4
        p.add_task("E Frontend".into(), None, Some(4.0), None)
            .unwrap(); // 5
        p.add_dependency(TaskId(3), TaskId(2)).unwrap(); // C depends on B
        p.add_dependency(TaskId(4), TaskId(1)).unwrap(); // D depends on A
        p.add_dependency(TaskId(4), TaskId(3)).unwrap(); // D depends on C

        let s = schedule(&p.tasks).unwrap();
        assert!(approx(s.duration, 9.0), "project: {}", s.duration);
        assert_row(&s, 1, 0.0, 2.0, 2.0, 4.0, 2.0, false); // A
        assert_row(&s, 2, 0.0, 1.0, 0.0, 1.0, 0.0, true); // B
        assert_row(&s, 3, 1.0, 4.0, 1.0, 4.0, 0.0, true); // C
        assert_row(&s, 4, 4.0, 9.0, 4.0, 9.0, 0.0, true); // D
        assert_row(&s, 5, 0.0, 4.0, 5.0, 9.0, 5.0, false); // E
        assert_eq!(s.critical_path(), &[TaskId(2), TaskId(3), TaskId(4)]);
    }

    #[test]
    fn empty_project_is_zero_length() {
        let s = schedule(&[]).unwrap();
        assert!(s.tasks.is_empty());
        assert!(approx(s.duration, 0.0));
        assert!(s.critical_path().is_empty());
    }

    #[test]
    fn single_task_is_critical() {
        let mut p = Project::new();
        p.add_task("only".into(), None, Some(3.0), None).unwrap();
        let s = schedule(&p.tasks).unwrap();
        assert!(approx(s.duration, 3.0));
        assert_row(&s, 1, 0.0, 3.0, 0.0, 3.0, 0.0, true);
    }

    #[test]
    fn no_duration_tasks_are_zero_day_milestones() {
        let mut p = Project::new();
        p.add_task("milestone".into(), None, None, None).unwrap(); // 1, dur 0
        p.add_task("work".into(), None, Some(2.0), None).unwrap(); // 2
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        let s = schedule(&p.tasks).unwrap();
        assert!(approx(s.duration, 2.0));
        assert!(approx(s.get(TaskId(1)).unwrap().duration, 0.0));
        assert_row(&s, 1, 0.0, 0.0, 0.0, 0.0, 0.0, true);
        assert_row(&s, 2, 0.0, 2.0, 0.0, 2.0, 0.0, true);
    }

    #[test]
    fn diamond_dependencies() {
        // A(1) -> B(2), A -> C(4), B -> D(1), C -> D. Longest path A,C,D = 6.
        let mut p = Project::new();
        p.add_task("A".into(), None, Some(1.0), None).unwrap(); // 1
        p.add_task("B".into(), None, Some(2.0), None).unwrap(); // 2
        p.add_task("C".into(), None, Some(4.0), None).unwrap(); // 3
        p.add_task("D".into(), None, Some(1.0), None).unwrap(); // 4
        p.add_dependency(TaskId(2), TaskId(1)).unwrap();
        p.add_dependency(TaskId(3), TaskId(1)).unwrap();
        p.add_dependency(TaskId(4), TaskId(2)).unwrap();
        p.add_dependency(TaskId(4), TaskId(3)).unwrap();
        let s = schedule(&p.tasks).unwrap();
        assert!(approx(s.duration, 6.0));
        // B is off the critical path (slack 2: C takes 4 vs B's 2).
        assert_row(&s, 2, 1.0, 3.0, 3.0, 5.0, 2.0, false);
        assert_eq!(s.critical_path(), &[TaskId(1), TaskId(3), TaskId(4)]);
    }

    #[test]
    fn cycle_is_an_error() {
        // Hand-built cyclic tasks (the CLI/load path would reject these).
        let mk = |id: u64, dep: u64| Task {
            id: TaskId(id),
            name: format!("t{id}"),
            description: None,
            duration: Some(1.0),
            state: TaskState::Todo,
            due: None,
            depends_on: vec![TaskId(dep)],
            concepts: vec![],
        };
        let tasks = vec![mk(1, 2), mk(2, 1)];
        assert!(schedule(&tasks).is_err());
    }
}
