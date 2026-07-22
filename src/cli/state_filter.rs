//! The `--state` filter shared by the task-listing commands.
//!
//! `report`, `task ls`, and `concept report` default to showing only open
//! work (`todo,in_progress`); `--state` widens or narrows that, with `all`
//! as shorthand for every state. Parsing lives here, CLI-side, so the
//! model's `TaskState` vocabulary stays free of the `all` pseudo-state.

use std::str::FromStr;

use mindtask::model::task::{Task, TaskState};

/// Every state, in display order (also the footer's count order).
const ALL_STATES: [TaskState; 3] = [TaskState::Todo, TaskState::InProgress, TaskState::Done];

fn index(state: TaskState) -> usize {
    match state {
        TaskState::Todo => 0,
        TaskState::InProgress => 1,
        TaskState::Done => 2,
    }
}

/// One `--state` value: a concrete state, or the `all` shorthand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateArg {
    State(TaskState),
    All,
}

impl FromStr for StateArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all" {
            return Ok(Self::All);
        }
        TaskState::from_str(s).map(Self::State).map_err(|_| {
            format!("invalid state '{s}': expected 'todo', 'in_progress', 'done', or 'all'")
        })
    }
}

/// The resolved set of states a listing shows.
#[derive(Clone, Copy, Debug)]
pub struct StateFilter {
    keep: [bool; 3],
}

impl StateFilter {
    pub fn new(args: &[StateArg]) -> Self {
        let mut keep = [false; 3];
        for arg in args {
            match arg {
                StateArg::All => keep = [true; 3],
                StateArg::State(s) => keep[index(*s)] = true,
            }
        }
        Self { keep }
    }

    pub fn keeps(&self, state: TaskState) -> bool {
        self.keep[index(state)]
    }
}

/// The one-line footer accounting for rows the filter removed, e.g.
/// `(hidden: 2 done — --state all to show)`, or `None` when nothing was.
pub fn hidden_footer<'a, I>(hidden: I) -> Option<String>
where
    I: IntoIterator<Item = &'a Task>,
{
    let mut counts = [0usize; 3];
    for task in hidden {
        counts[index(task.state)] += 1;
    }
    let parts: Vec<String> = ALL_STATES
        .iter()
        .zip(counts)
        .filter(|&(_, c)| c > 0)
        .map(|(s, c)| format!("{c} {s}"))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "(hidden: {} — --state all to show)",
            parts.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mindtask::model::id::TaskId;

    fn task(id: u64, state: TaskState) -> Task {
        Task {
            id: TaskId(id),
            name: format!("t{id}"),
            description: None,
            duration: None,
            state,
            due: None,
            depends_on: vec![],
            concepts: vec![],
        }
    }

    #[test]
    fn state_arg_parses_states_and_all() {
        assert_eq!(
            "todo".parse::<StateArg>().unwrap(),
            StateArg::State(TaskState::Todo)
        );
        assert_eq!(
            "in_progress".parse::<StateArg>().unwrap(),
            StateArg::State(TaskState::InProgress)
        );
        assert_eq!(
            "done".parse::<StateArg>().unwrap(),
            StateArg::State(TaskState::Done)
        );
        assert_eq!("all".parse::<StateArg>().unwrap(), StateArg::All);
    }

    #[test]
    fn state_arg_rejects_unknown_and_names_all() {
        let err = "bogus".parse::<StateArg>().unwrap_err();
        assert!(
            err.contains("'todo', 'in_progress', 'done', or 'all'"),
            "{err}"
        );
    }

    #[test]
    fn filter_keeps_only_selected_states() {
        let f = StateFilter::new(&[
            StateArg::State(TaskState::Todo),
            StateArg::State(TaskState::InProgress),
        ]);
        assert!(f.keeps(TaskState::Todo));
        assert!(f.keeps(TaskState::InProgress));
        assert!(!f.keeps(TaskState::Done));
    }

    #[test]
    fn all_overrides_everything() {
        let f = StateFilter::new(&[StateArg::State(TaskState::Done), StateArg::All]);
        for s in ALL_STATES {
            assert!(f.keeps(s));
        }
    }

    #[test]
    fn footer_lists_nonzero_counts_in_state_order() {
        let tasks = [
            task(1, TaskState::Done),
            task(2, TaskState::Todo),
            task(3, TaskState::Done),
        ];
        assert_eq!(
            hidden_footer(tasks.iter()).unwrap(),
            "(hidden: 1 todo, 2 done — --state all to show)"
        );
    }

    #[test]
    fn footer_absent_when_nothing_hidden() {
        assert_eq!(hidden_footer([].iter()), None);
    }
}
