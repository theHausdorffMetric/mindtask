//! Typed identifiers for concepts and tasks.
//!
//! Both [`ConceptId`] and [`TaskId`] are thin wrappers around `u64`, providing
//! type safety so the two ID spaces cannot be accidentally mixed. They implement
//! `Display`, `FromStr`, and serde traits.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Unique identifier for a [`Concept`](super::concept::Concept).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConceptId(pub u64);

/// Unique identifier for a [`Task`](super::task::Task).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

// --- Display ---

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- FromStr ---

/// Errors returned when parsing an ID from a string.
#[derive(Debug, thiserror::Error)]
pub enum IdParseError {
    #[error("invalid concept ID: expected a positive integer, got '{0}'")]
    InvalidConceptId(String),
    #[error("invalid task ID: expected a positive integer, got '{0}'")]
    InvalidTaskId(String),
}

impl FromStr for ConceptId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>()
            .map(ConceptId)
            .map_err(|_| IdParseError::InvalidConceptId(s.to_string()))
    }
}

impl FromStr for TaskId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>()
            .map(TaskId)
            .map_err(|_| IdParseError::InvalidTaskId(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_id_display() {
        assert_eq!(ConceptId(1).to_string(), "1");
        assert_eq!(ConceptId(42).to_string(), "42");
    }

    #[test]
    fn task_id_display() {
        assert_eq!(TaskId(1).to_string(), "1");
        assert_eq!(TaskId(99).to_string(), "99");
    }

    #[test]
    fn concept_id_parse() {
        assert_eq!("1".parse::<ConceptId>().unwrap(), ConceptId(1));
        assert_eq!("42".parse::<ConceptId>().unwrap(), ConceptId(42));
    }

    #[test]
    fn task_id_parse() {
        assert_eq!("1".parse::<TaskId>().unwrap(), TaskId(1));
        assert_eq!("99".parse::<TaskId>().unwrap(), TaskId(99));
    }

    #[test]
    fn parse_errors() {
        assert!("abc".parse::<ConceptId>().is_err());
        assert!("".parse::<ConceptId>().is_err());
        assert!("-1".parse::<ConceptId>().is_err());
        assert!("abc".parse::<TaskId>().is_err());
        assert!("".parse::<TaskId>().is_err());
    }

    #[test]
    fn concept_id_serde_roundtrip() {
        let id = ConceptId(5);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "5");
        let parsed: ConceptId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn task_id_serde_roundtrip() {
        let id = TaskId(10);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "10");
        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
