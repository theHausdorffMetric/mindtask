use serde::{Deserialize, Serialize};

use super::id::ConceptId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: ConceptId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ConceptId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_serde_roundtrip() {
        let concept = Concept {
            id: ConceptId(1),
            name: "Backend".to_string(),
            description: Some("Server-side code".to_string()),
            parent: None,
        };
        let json = serde_json::to_string_pretty(&concept).unwrap();
        let parsed: Concept = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, concept.id);
        assert_eq!(parsed.name, concept.name);
        assert_eq!(parsed.description, concept.description);
        assert_eq!(parsed.parent, concept.parent);
    }

    #[test]
    fn concept_skips_none_fields() {
        let concept = Concept {
            id: ConceptId(1),
            name: "Root".to_string(),
            description: None,
            parent: None,
        };
        let json = serde_json::to_string(&concept).unwrap();
        assert!(!json.contains("description"));
        assert!(!json.contains("parent"));
    }

    #[test]
    fn concept_with_parent() {
        let concept = Concept {
            id: ConceptId(2),
            name: "Child".to_string(),
            description: None,
            parent: Some(ConceptId(1)),
        };
        let json = serde_json::to_string(&concept).unwrap();
        assert!(json.contains("\"parent\":1"));
        let parsed: Concept = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.parent, Some(ConceptId(1)));
    }
}
