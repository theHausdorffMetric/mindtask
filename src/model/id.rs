use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConceptId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

// --- Display ---

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c{}", self.0)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

// --- FromStr ---

#[derive(Debug, thiserror::Error)]
pub enum IdParseError {
    #[error("invalid concept ID format: expected 'c<number>', got '{0}'")]
    InvalidConceptId(String),
    #[error("invalid task ID format: expected 't<number>', got '{0}'")]
    InvalidTaskId(String),
}

impl FromStr for ConceptId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('c')
            .and_then(|n| n.parse::<u64>().ok())
            .map(ConceptId)
            .ok_or_else(|| IdParseError::InvalidConceptId(s.to_string()))
    }
}

impl FromStr for TaskId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('t')
            .and_then(|n| n.parse::<u64>().ok())
            .map(TaskId)
            .ok_or_else(|| IdParseError::InvalidTaskId(s.to_string()))
    }
}

// --- Serde as string ---

impl Serialize for ConceptId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ConceptId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for TaskId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TaskId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_id_display() {
        assert_eq!(ConceptId(1).to_string(), "c1");
        assert_eq!(ConceptId(42).to_string(), "c42");
    }

    #[test]
    fn task_id_display() {
        assert_eq!(TaskId(1).to_string(), "t1");
        assert_eq!(TaskId(99).to_string(), "t99");
    }

    #[test]
    fn concept_id_parse() {
        assert_eq!("c1".parse::<ConceptId>().unwrap(), ConceptId(1));
        assert_eq!("c42".parse::<ConceptId>().unwrap(), ConceptId(42));
    }

    #[test]
    fn task_id_parse() {
        assert_eq!("t1".parse::<TaskId>().unwrap(), TaskId(1));
        assert_eq!("t99".parse::<TaskId>().unwrap(), TaskId(99));
    }

    #[test]
    fn concept_id_parse_errors() {
        assert!("t1".parse::<ConceptId>().is_err());
        assert!("c".parse::<ConceptId>().is_err());
        assert!("c-1".parse::<ConceptId>().is_err());
        assert!("abc".parse::<ConceptId>().is_err());
        assert!("".parse::<ConceptId>().is_err());
    }

    #[test]
    fn task_id_parse_errors() {
        assert!("c1".parse::<TaskId>().is_err());
        assert!("t".parse::<TaskId>().is_err());
        assert!("t-1".parse::<TaskId>().is_err());
        assert!("abc".parse::<TaskId>().is_err());
        assert!("".parse::<TaskId>().is_err());
    }

    #[test]
    fn concept_id_serde_roundtrip() {
        let id = ConceptId(5);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"c5\"");
        let parsed: ConceptId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn task_id_serde_roundtrip() {
        let id = TaskId(10);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"t10\"");
        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
