use anyhow::{Context, Result};

use mindtask::model::id::{ConceptId, TaskId};
use mindtask::model::project::Project;
use mindtask::model::task::{TaskState, parse_due};

/// Format a Zoned datetime for display, converting to the project timezone.
fn format_due(due: &jiff::Zoned, project: &Project) -> String {
    let tz_name = project.timezone_or_utc();
    if let Ok(tz) = jiff::tz::TimeZone::get(tz_name) {
        let converted = due.with_time_zone(tz);
        format!("{}", converted)
    } else {
        format!("{}", due)
    }
}

/// Format a short due date for list columns (date + time, no seconds).
fn format_due_short(due: &jiff::Zoned, project: &Project) -> String {
    let tz_name = project.timezone_or_utc();
    let z = if let Ok(tz) = jiff::tz::TimeZone::get(tz_name) {
        due.with_time_zone(tz)
    } else {
        due.clone()
    };
    format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        z.year(),
        z.month(),
        z.day(),
        z.hour(),
        z.minute()
    )
}

pub fn add(
    project: &mut Project,
    name: String,
    description: Option<String>,
    duration: Option<f64>,
    due: Option<String>,
    concepts: Vec<ConceptId>,
) -> Result<()> {
    let due = due
        .map(|d| parse_due(&d, project.timezone_or_utc()))
        .transpose()
        .context("invalid due date")?;

    // Validate all concepts exist before creating the task, so a bad ID doesn't
    // leave behind a half-linked task.
    for &cid in &concepts {
        if project.get_concept(cid).is_none() {
            anyhow::bail!("concept {cid} not found");
        }
    }

    let id = project.add_task(name.clone(), description, duration, due);
    for cid in &concepts {
        // Concepts validated above; link_concept is idempotent and re-checks.
        project
            .link_concept(id, *cid)
            .context("failed to link concept")?;
    }

    if concepts.is_empty() {
        println!("Added task {} \"{}\"", id, name);
    } else {
        let linked = concepts
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "Added task {} \"{}\" (linked to concept(s) {})",
            id, name, linked
        );
    }
    Ok(())
}

pub fn edit(
    project: &mut Project,
    id: TaskId,
    name: Option<String>,
    description: Option<String>,
    clear_description: bool,
    duration: Option<f64>,
    clear_duration: bool,
) -> Result<()> {
    let desc = if clear_description {
        Some(None)
    } else {
        description.map(Some)
    };

    let dur = if clear_duration {
        Some(None)
    } else {
        duration.map(Some)
    };

    if name.is_none() && desc.is_none() && dur.is_none() {
        anyhow::bail!(
            "nothing to edit: provide --name, --description, --duration, \
             --clear-description, and/or --clear-duration"
        );
    }

    project
        .edit_task(id, name, desc, dur)
        .context("failed to edit task")?;
    println!("Updated task {}", id);
    Ok(())
}

pub fn remove(project: &mut Project, id: TaskId) -> Result<()> {
    let name = project
        .get_task(id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    project.remove_task(id).context("failed to remove task")?;
    println!("Removed task {} \"{}\"", id, name);
    Ok(())
}

pub fn list(project: &Project) {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return;
    }

    println!(
        "{:<6} {:<25} {:<14} {:<18} {:<15} CONCEPTS",
        "ID", "NAME", "STATE", "DUE", "DEPENDS ON"
    );
    for task in &project.tasks {
        let deps = if task.depends_on.is_empty() {
            "-".to_string()
        } else {
            task.depends_on
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let concepts = if task.concepts.is_empty() {
            "-".to_string()
        } else {
            task.concepts
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let due = task
            .due
            .as_ref()
            .map(|d| format_due_short(d, project))
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<6} {:<25} {:<14} {:<18} {:<15} {}",
            task.id, task.name, task.state, due, deps, concepts
        );
    }
}

pub fn show(project: &Project, id: TaskId) -> Result<()> {
    let task = project
        .get_task(id)
        .ok_or_else(|| anyhow::anyhow!("task {} not found", id))?;

    println!("ID:          {}", task.id);
    println!("Name:        {}", task.name);
    if let Some(desc) = &task.description {
        println!("Description: {}", desc);
    }
    if let Some(dur) = task.duration {
        println!("Duration:    {} days", dur);
    }
    println!("State:       {}", task.state);
    if let Some(due) = &task.due {
        println!("Due:         {}", format_due(due, project));
    }

    if !task.depends_on.is_empty() {
        let dep_strs: Vec<String> = task
            .depends_on
            .iter()
            .map(|d| {
                let name = project
                    .get_task(*d)
                    .map(|t| t.name.as_str())
                    .unwrap_or("???");
                format!("{} {{{}}}", name, d)
            })
            .collect();
        println!("Depends on:  {}", dep_strs.join(", "));
    }

    if !task.concepts.is_empty() {
        let concept_strs: Vec<String> = task
            .concepts
            .iter()
            .map(|c| {
                let name = project
                    .get_concept(*c)
                    .map(|co| co.name.as_str())
                    .unwrap_or("???");
                format!("{} {{{}}}", name, c)
            })
            .collect();
        println!("Concepts:    {}", concept_strs.join(", "));
    }

    Ok(())
}

pub fn set_state(project: &mut Project, id: TaskId, state: TaskState) -> Result<()> {
    project
        .set_task_state(id, state)
        .context("failed to set task state")?;
    println!("Set {} state to {}", id, state);
    Ok(())
}

pub fn set_due(project: &mut Project, id: TaskId, date: Option<String>, clear: bool) -> Result<()> {
    if clear {
        project
            .set_task_due(id, None)
            .context("failed to clear due date")?;
        println!("Cleared due date for {}", id);
    } else if let Some(date_str) = date {
        let due = parse_due(&date_str, project.timezone_or_utc()).context("invalid due date")?;
        project
            .set_task_due(id, Some(due.clone()))
            .context("failed to set due date")?;
        println!("Set {} due to {}", id, format_due(&due, project));
    } else {
        anyhow::bail!("provide a date or use --clear");
    }
    Ok(())
}

pub fn add_dep(project: &mut Project, task_id: TaskId, depends_on: TaskId) -> Result<()> {
    project
        .add_dependency(task_id, depends_on)
        .context("failed to add dependency")?;
    println!("Added dependency: {} depends on {}", task_id, depends_on);
    Ok(())
}

pub fn remove_dep(project: &mut Project, task_id: TaskId, depends_on: TaskId) -> Result<()> {
    project
        .remove_dependency(task_id, depends_on)
        .context("failed to remove dependency")?;
    println!(
        "Removed dependency: {} no longer depends on {}",
        task_id, depends_on
    );
    Ok(())
}

pub fn link(project: &mut Project, task_id: TaskId, concept_id: ConceptId) -> Result<()> {
    project
        .link_concept(task_id, concept_id)
        .context("failed to link concept")?;
    println!("Linked {} to {}", task_id, concept_id);
    Ok(())
}

pub fn unlink(project: &mut Project, task_id: TaskId, concept_id: ConceptId) -> Result<()> {
    project
        .unlink_concept(task_id, concept_id)
        .context("failed to unlink concept")?;
    println!("Unlinked {} from {}", task_id, concept_id);
    Ok(())
}
