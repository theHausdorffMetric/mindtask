use std::path::Path;

use crate::model::project::Project;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("project file not found: {0}")]
    NotFound(String),
    #[error("failed to read project file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("failed to parse project file: {0}")]
    ParseError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub fn load(path: &Path) -> Result<Project> {
    if !path.exists() {
        return Err(StoreError::NotFound(path.display().to_string()));
    }
    let contents = std::fs::read_to_string(path)?;
    let project: Project = serde_json::from_str(&contents)?;
    Ok(project)
}

pub fn save(path: &Path, project: &Project) -> Result<()> {
    let json = serde_json::to_string_pretty(project)?;
    std::fs::write(path, json + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::id::ConceptId;
    use tempfile::NamedTempFile;

    #[test]
    fn save_and_load_roundtrip() {
        let mut project = Project::new();
        project.add_concept("Root".into(), None, None).unwrap();
        project
            .add_concept("Child".into(), Some(ConceptId(1)), Some("desc".into()))
            .unwrap();
        project.add_task("Do stuff".into(), Some("details".into()), Some(2.5));

        let file = NamedTempFile::new().unwrap();
        save(file.path(), &project).unwrap();
        let loaded = load(file.path()).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.concepts.len(), 2);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.next_concept_id, 3);
        assert_eq!(loaded.next_task_id, 2);
    }

    #[test]
    fn load_nonexistent_file() {
        let err = load(Path::new("/tmp/nonexistent_mindtask_test.json")).unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }
}
