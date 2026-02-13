use anyhow::{Context, Result};
use jiff::Zoned;
use serde::{Deserialize, Serialize};

use super::id::{ConceptId, TaskId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Todo,
    InProgress,
    Done,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Todo => write!(f, "todo"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Done => write!(f, "done"),
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(Self::Todo),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            _ => Err(format!(
                "invalid state '{}': expected 'todo', 'in_progress', or 'done'",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, alias = "status")]
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Zoned>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<ConceptId>,
}

/// Parse a due date string, falling back to the given default timezone
/// if the input doesn't include one.
///
/// Accepted formats:
/// - Full RFC 9557: `2025-03-15T14:00:00-04:00[America/New_York]`
/// - Datetime without timezone: `2025-03-15T14:00` (uses `default_tz`)
/// - Date only: `2025-03-15` (midnight in `default_tz`)
pub fn parse_due(input: &str, default_tz: &str) -> Result<Zoned> {
    // Try full Zoned parse first (has timezone annotation)
    if let Ok(zoned) = input.parse::<Zoned>() {
        return Ok(zoned);
    }

    let tz = jiff::tz::TimeZone::get(default_tz)
        .with_context(|| format!("invalid timezone '{default_tz}'"))?;

    // Try as civil datetime
    if let Ok(dt) = input.parse::<jiff::civil::DateTime>() {
        return dt
            .to_zoned(tz)
            .context("failed to convert datetime to zoned");
    }

    // Try as civil date (midnight)
    if let Ok(date) = input.parse::<jiff::civil::Date>() {
        return date
            .to_zoned(tz)
            .context("failed to convert date to zoned");
    }

    anyhow::bail!(
        "invalid due date '{input}': expected format like \
         2025-03-15, 2025-03-15T14:00, or 2025-03-15T14:00[America/New_York]"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_serde() {
        assert_eq!(
            serde_json::to_string(&TaskState::Todo).unwrap(),
            "\"todo\""
        );
        assert_eq!(
            serde_json::to_string(&TaskState::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&TaskState::Done).unwrap(),
            "\"done\""
        );
    }

    #[test]
    fn task_serde_roundtrip() {
        let task = Task {
            id: TaskId(1),
            name: "Design API".to_string(),
            description: Some("Design the REST API".to_string()),
            duration: Some(2.0),
            state: TaskState::InProgress,
            due: None,
            depends_on: vec![],
            concepts: vec![ConceptId(3)],
        };
        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, task.id);
        assert_eq!(parsed.name, task.name);
        assert_eq!(parsed.state, TaskState::InProgress);
        assert_eq!(parsed.concepts.len(), 1);
    }

    #[test]
    fn task_serde_roundtrip_with_due() {
        let due = parse_due("2025-03-15T14:00", "America/New_York").unwrap();
        let task = Task {
            id: TaskId(1),
            name: "Deploy".to_string(),
            description: None,
            duration: None,
            state: TaskState::Todo,
            due: Some(due),
            depends_on: vec![],
            concepts: vec![],
        };
        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert!(parsed.due.is_some());
        assert_eq!(parsed.due.unwrap().time_zone().iana_name(), Some("America/New_York"));
    }

    #[test]
    fn task_skips_empty_vecs() {
        let task = Task {
            id: TaskId(1),
            name: "Simple".to_string(),
            description: None,
            duration: None,
            state: TaskState::Todo,
            due: None,
            depends_on: vec![],
            concepts: vec![],
        };
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("depends_on"));
        assert!(!json.contains("concepts"));
        assert!(!json.contains("description"));
        assert!(!json.contains("duration"));
        assert!(!json.contains("due"));
    }

    #[test]
    fn parse_due_with_timezone() {
        let zoned = parse_due("2025-03-15T14:00:00-04:00[America/New_York]", "UTC").unwrap();
        assert_eq!(zoned.time_zone().iana_name(), Some("America/New_York"));
    }

    #[test]
    fn parse_due_datetime_uses_default_tz() {
        let zoned = parse_due("2025-03-15T14:00", "America/New_York").unwrap();
        assert_eq!(zoned.time_zone().iana_name(), Some("America/New_York"));
    }

    #[test]
    fn parse_due_date_only() {
        let zoned = parse_due("2025-03-15", "UTC").unwrap();
        assert_eq!(zoned.time_zone().iana_name(), Some("UTC"));
        assert_eq!(zoned.hour(), 0);
        assert_eq!(zoned.minute(), 0);
    }

    #[test]
    fn parse_due_invalid() {
        assert!(parse_due("not-a-date", "UTC").is_err());
    }

    #[test]
    fn task_state_display_and_parse() {
        for status in [TaskState::Todo, TaskState::InProgress, TaskState::Done] {
            let s = status.to_string();
            let parsed: TaskState = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }
}
