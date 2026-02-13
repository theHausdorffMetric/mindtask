//! Concept tree operations: ancestry checks and structural validation.

use crate::model::id::ConceptId;
use crate::model::project::Project;

/// Check if `ancestor_id` is an ancestor of `descendant_id` in the concept tree.
/// Walks up the parent chain from `descendant_id`.
pub fn is_ancestor(project: &Project, ancestor_id: ConceptId, descendant_id: ConceptId) -> bool {
    let mut current = descendant_id;
    loop {
        let concept = match project.get_concept(current) {
            Some(c) => c,
            None => return false,
        };
        match concept.parent {
            Some(parent_id) => {
                if parent_id == ancestor_id {
                    return true;
                }
                current = parent_id;
            }
            None => return false,
        }
    }
}

/// Validate that the concept tree is well-formed:
/// - All parent references point to existing concepts
/// - No orphan chains (all paths reach a root)
pub fn validate_tree(project: &Project) -> std::result::Result<(), String> {
    for concept in &project.concepts {
        if let Some(parent_id) = concept.parent
            && project.get_concept(parent_id).is_none()
        {
            return Err(format!(
                "concept {} references non-existent parent {}",
                concept.id, parent_id
            ));
        }

        // Walk up to root to detect cycles / orphan chains
        let mut visited = std::collections::HashSet::new();
        let mut current = concept.id;
        loop {
            if !visited.insert(current) {
                return Err(format!(
                    "cycle detected in concept tree involving {}",
                    current
                ));
            }
            match project.get_concept(current).and_then(|c| c.parent) {
                Some(parent_id) => current = parent_id,
                None => break,
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_ancestor_basic() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), Some(ConceptId(1)), None)
            .unwrap();
        p.add_concept("C".into(), Some(ConceptId(2)), None)
            .unwrap();

        assert!(is_ancestor(&p, ConceptId(1), ConceptId(2)));
        assert!(is_ancestor(&p, ConceptId(1), ConceptId(3)));
        assert!(is_ancestor(&p, ConceptId(2), ConceptId(3)));
        assert!(!is_ancestor(&p, ConceptId(3), ConceptId(1)));
        assert!(!is_ancestor(&p, ConceptId(2), ConceptId(1)));
    }

    #[test]
    fn validate_tree_valid() {
        let mut p = Project::new();
        p.add_concept("A".into(), None, None).unwrap();
        p.add_concept("B".into(), Some(ConceptId(1)), None)
            .unwrap();
        assert!(validate_tree(&p).is_ok());
    }

    #[test]
    fn validate_tree_detects_invalid_parent() {
        let mut p = Project::new();
        // Manually create a concept with a bad parent
        p.concepts.push(crate::model::concept::Concept {
            id: ConceptId(1),
            name: "Bad".into(),
            description: None,
            parent: Some(ConceptId(99)),
        });
        assert!(validate_tree(&p).is_err());
    }
}
