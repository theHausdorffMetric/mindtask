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

/// Handle the `config wrap-width` subcommand.
/// Returns `true` if the project was modified and needs saving.
pub fn wrap_width(
    project: &mut Project,
    width: Option<u32>,
    clear: bool,
    show: bool,
) -> Result<bool> {
    if clear {
        project.wrap_width = None;
        println!("Cleared wrap width (descriptions follow the terminal width, falling back to 80)");
        return Ok(true);
    }

    if show || width.is_none() {
        match project.wrap_width {
            Some(w) => println!("Wrap width: {}", w),
            None => println!("Wrap width: not set (follows terminal width, falls back to 80)"),
        }
        return Ok(false);
    }

    let w = width.unwrap();
    if w == 0 {
        anyhow::bail!("wrap width must be greater than 0");
    }

    project.wrap_width = Some(w);
    println!("Set wrap width to {}", w);
    Ok(true)
}
