use anyhow::{Context, Result};

use mindtask::graph::dag::validate_project;
use mindtask::model::project::Project;
use mindtask::store::json;

use super::PROJECT_FILE;

pub fn init(timezone: Option<String>) -> Result<()> {
    let path = std::env::current_dir()
        .context("cannot determine current directory")?
        .join(PROJECT_FILE);

    if path.exists() {
        anyhow::bail!("{} already exists in the current directory", PROJECT_FILE);
    }

    if let Some(ref tz) = timezone {
        jiff::tz::TimeZone::get(tz)
            .with_context(|| format!("invalid timezone '{tz}'"))?;
    }

    let mut project = Project::new();
    project.timezone = timezone.clone();
    json::save(&path, &project).context("failed to create project file")?;

    if let Some(tz) = &project.timezone {
        println!("Created {} (timezone: {})", PROJECT_FILE, tz);
    } else {
        println!("Created {}", PROJECT_FILE);
    }
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
