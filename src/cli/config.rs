use anyhow::{Context, Result};

use mindtask::model::project::Project;

/// Handle the `config timezone` subcommand.
/// Returns `true` if the project was modified and needs saving.
pub fn timezone(
    project: &mut Project,
    timezone: Option<String>,
    show: bool,
) -> Result<bool> {
    if show || timezone.is_none() {
        match &project.timezone {
            Some(tz) => println!("Timezone: {}", tz),
            None => println!("Timezone: not set (defaults to UTC)"),
        }
        return Ok(false);
    }

    let tz = timezone.unwrap();
    jiff::tz::TimeZone::get(&tz)
        .with_context(|| format!("invalid timezone '{tz}'"))?;

    project.timezone = Some(tz.clone());
    println!("Set timezone to {}", tz);
    Ok(true)
}
