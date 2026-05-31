use anyhow::{Context, Result};

use mindtask::graph::dag::validate_project;
use mindtask::model::project::Project;
use mindtask::store::json;

use super::PROJECT_FILE;

pub fn init(timezone: String) -> Result<()> {
    let path = std::env::current_dir()
        .context("cannot determine current directory")?
        .join(PROJECT_FILE);

    if path.exists() {
        anyhow::bail!("{} already exists in the current directory", PROJECT_FILE);
    }

    jiff::tz::TimeZone::get(&timezone)
        .with_context(|| format!("invalid timezone '{timezone}'"))?;

    let mut project = Project::new();
    project.timezone = Some(timezone);
    json::save(&path, &project).context("failed to create project file")?;

    println!(
        "Created {} (timezone: {})",
        PROJECT_FILE,
        project.timezone.as_deref().unwrap_or_default()
    );
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
