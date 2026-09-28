use std::path::Path;

use anyhow::{Context, Result};

use mindtask::graph::dag::validate_project;
use mindtask::model::project::Project;
use mindtask::refs;
use mindtask::store::json;

use super::PROJECT_FILE;

pub fn init(timezone: String) -> Result<()> {
    let path = std::env::current_dir()
        .context("cannot determine current directory")?
        .join(PROJECT_FILE);

    if path.exists() {
        anyhow::bail!("{} already exists in the current directory", PROJECT_FILE);
    }

    jiff::tz::TimeZone::get(&timezone).with_context(|| format!("invalid timezone '{timezone}'"))?;

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

/// Structural validation, then the ref check against `base` (the project
/// file's directory). A broken ref fails validation — this is the lint gate
/// for citations — but is reported apart from structural errors, and never
/// stops an operational command (only `validate` runs this check).
pub fn validate(project: &Project, base: &Path) -> Result<()> {
    if let Err(msg) = validate_project(project) {
        anyhow::bail!("Validation failed: {}", msg);
    }

    let broken = refs::check(project, base);
    if broken.is_empty() {
        println!("Project is valid.");
        return Ok(());
    }

    println!("Structure is valid.");
    println!("{} broken ref(s):", broken.len());
    let owner_width = broken
        .iter()
        .map(|b| b.owner.to_string().len())
        .max()
        .unwrap_or(0);
    let uri_width = broken.iter().map(|b| b.uri.len()).max().unwrap_or(0);
    for b in &broken {
        println!(
            "  {:<owner_width$}  {:<uri_width$}  -> {}",
            b.owner.to_string(),
            b.uri,
            b.resolved.display()
        );
    }
    anyhow::bail!("Validation failed: {} broken ref(s)", broken.len());
}
