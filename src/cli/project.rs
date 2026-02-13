use anyhow::{Context, Result};

use mindtask::graph::dag::validate_project;
use mindtask::model::project::Project;
use mindtask::store::json;

use super::PROJECT_FILE;

pub fn init() -> Result<()> {
    let path = std::env::current_dir()
        .context("cannot determine current directory")?
        .join(PROJECT_FILE);

    if path.exists() {
        anyhow::bail!("{} already exists in the current directory", PROJECT_FILE);
    }

    let project = Project::new();
    json::save(&path, &project).context("failed to create project file")?;
    println!("Created {}", PROJECT_FILE);
    Ok(())
}

pub fn validate(project: &Project) -> Result<()> {
    match validate_project(project) {
        Ok(()) => {
            println!("Project is valid.");
            Ok(())
        }
        Err(msg) => {
            anyhow::bail!("Validation failed: {}", msg);
        }
    }
}
