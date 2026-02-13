use serde::{Deserialize, Serialize};

use super::id::{ConceptId, TaskId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Todo => write!(f, "todo"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Done => write!(f, "done"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(Self::Todo),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            _ => Err(format!(
                "invalid status '{}': expected 'todo', 'in_progress', or 'done'",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<ConceptId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_serde() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::Todo).unwrap(),
            "\"todo\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Done).unwrap(),
            "\"done\""
        );
    }

    #[test]
    fn task_serde_roundtrip() {
        let task = Task {
            id: TaskId(1),
            title: "Design API".to_string(),
            description: Some("Design the REST API".to_string()),
            duration: Some(2.0),
            status: TaskStatus::InProgress,
            depends_on: vec![],
            concepts: vec![ConceptId(3)],
        };
        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, task.id);
        assert_eq!(parsed.title, task.title);
        assert_eq!(parsed.status, TaskStatus::InProgress);
        assert_eq!(parsed.concepts.len(), 1);
    }

    #[test]
    fn task_skips_empty_vecs() {
        let task = Task {
            id: TaskId(1),
            title: "Simple".to_string(),
            description: None,
            duration: None,
            status: TaskStatus::Todo,
            depends_on: vec![],
            concepts: vec![],
        };
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("depends_on"));
        assert!(!json.contains("concepts"));
        assert!(!json.contains("description"));
        assert!(!json.contains("duration"));
    }

    #[test]
    fn task_status_display_and_parse() {
        for status in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
            let s = status.to_string();
            let parsed: TaskStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }
}
