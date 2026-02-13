use anyhow::{Context, Result};

use mindtask::model::id::{ConceptId, TaskId};
use mindtask::model::project::Project;
use mindtask::model::task::TaskStatus;

pub fn add(
    project: &mut Project,
    title: String,
    description: Option<String>,
    duration: Option<f64>,
) {
    let id = project.add_task(title.clone(), description, duration);
    println!("Added task {} \"{}\"", id, title);
}

pub fn remove(project: &mut Project, id: TaskId) -> Result<()> {
    let title = project
        .get_task(id)
        .map(|t| t.title.clone())
        .unwrap_or_default();
    project.remove_task(id).context("failed to remove task")?;
    println!("Removed task {} \"{}\"", id, title);
    Ok(())
}

pub fn list(project: &Project) {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return;
    }

    println!(
        "{:<6} {:<25} {:<14} {:<15} CONCEPTS",
        "ID", "TITLE", "STATUS", "DEPENDS ON"
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
        println!(
            "{:<6} {:<25} {:<14} {:<15} {}",
            task.id, task.title, task.status, deps, concepts
        );
    }
}

pub fn show(project: &Project, id: TaskId) -> Result<()> {
    let task = project
        .get_task(id)
        .ok_or_else(|| anyhow::anyhow!("task {} not found", id))?;

    println!("ID:          {}", task.id);
    println!("Title:       {}", task.title);
    if let Some(desc) = &task.description {
        println!("Description: {}", desc);
    }
    if let Some(dur) = task.duration {
        println!("Duration:    {} days", dur);
    }
    println!("Status:      {}", task.status);

    if !task.depends_on.is_empty() {
        let dep_strs: Vec<String> = task
            .depends_on
            .iter()
            .map(|d| {
                let title = project
                    .get_task(*d)
                    .map(|t| t.title.as_str())
                    .unwrap_or("???");
                format!("{} ({})", d, title)
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
                format!("{} ({})", c, name)
            })
            .collect();
        println!("Concepts:    {}", concept_strs.join(", "));
    }

    Ok(())
}

pub fn set_status(project: &mut Project, id: TaskId, status: TaskStatus) -> Result<()> {
    project
        .set_task_status(id, status)
        .context("failed to set task status")?;
    println!("Set {} status to {}", id, status);
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
    println!("Removed dependency: {} no longer depends on {}", task_id, depends_on);
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
