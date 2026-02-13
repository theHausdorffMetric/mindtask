mod concept;
mod project;
mod task;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use mindtask::model::id::{ConceptId, TaskId};
use mindtask::model::task::TaskStatus;

const PROJECT_FILE: &str = ".mindtask.json";

#[derive(Parser)]
#[command(name = "mindtask", about = "Combine mindmaps with task dependency graphs")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new project in the current directory
    Init,
    /// Manage concepts in the concept tree
    #[command(subcommand)]
    Concept(ConceptCommand),
    /// Manage tasks
    #[command(subcommand)]
    Task(TaskCommand),
    /// Manage task dependencies
    #[command(subcommand)]
    Depend(DependCommand),
    /// Link a task to a concept
    Link {
        /// Task ID (e.g. t1)
        task_id: TaskId,
        /// Concept ID (e.g. c1)
        concept_id: ConceptId,
    },
    /// Unlink a task from a concept
    Unlink {
        /// Task ID (e.g. t1)
        task_id: TaskId,
        /// Concept ID (e.g. c1)
        concept_id: ConceptId,
    },
    /// Validate the project file
    Validate,
}

#[derive(Subcommand)]
enum ConceptCommand {
    /// Add a new concept
    Add {
        /// Name of the concept
        name: String,
        /// Parent concept ID (e.g. c1)
        #[arg(long)]
        parent: Option<ConceptId>,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Remove a concept
    Rm {
        /// Concept ID to remove (e.g. c1)
        id: ConceptId,
    },
    /// Move a concept to a new parent
    Mv {
        /// Concept ID to move (e.g. c1)
        id: ConceptId,
        /// New parent concept ID (e.g. c2), or "root" to make it a root concept
        #[arg(long)]
        parent: String,
    },
    /// List all concepts
    Ls,
    /// Show details of a concept
    Show {
        /// Concept ID (e.g. c1)
        id: ConceptId,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Add a new task
    Add {
        /// Name of the task
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Duration in days
        #[arg(long)]
        duration: Option<f64>,
    },
    /// Remove a task
    Rm {
        /// Task ID to remove (e.g. t1)
        id: TaskId,
    },
    /// List all tasks
    Ls,
    /// Show details of a task
    Show {
        /// Task ID (e.g. t1)
        id: TaskId,
    },
    /// Set the status of a task
    Status {
        /// Task ID (e.g. t1)
        id: TaskId,
        /// New status: todo, in_progress, or done
        status: TaskStatus,
    },
}

#[derive(Subcommand)]
enum DependCommand {
    /// Add a dependency between tasks
    Add {
        /// Task that depends on another (e.g. t2)
        task_id: TaskId,
        /// Task that must finish first (e.g. t1)
        depends_on: TaskId,
    },
    /// Remove a dependency between tasks
    Rm {
        /// Task that has the dependency (e.g. t2)
        task_id: TaskId,
        /// Dependency to remove (e.g. t1)
        depends_on: TaskId,
    },
}

/// Find the project file by walking up from the current directory.
fn find_project_file() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("cannot determine current directory")?;
    loop {
        let candidate = dir.join(PROJECT_FILE);
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            anyhow::bail!(
                "no {} found in current directory or any parent directory\n\
                 Run 'mindtask init' to create a new project.",
                PROJECT_FILE
            );
        }
    }
}

fn load_project(path: &Path) -> Result<mindtask::model::project::Project> {
    mindtask::store::json::load(path)
        .with_context(|| format!("failed to load project from {}", path.display()))
}

fn save_project(path: &Path, project: &mindtask::model::project::Project) -> Result<()> {
    mindtask::store::json::save(path, project)
        .with_context(|| format!("failed to save project to {}", path.display()))
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => project::init(),
        Command::Validate => {
            let path = find_project_file()?;
            let proj = load_project(&path)?;
            project::validate(&proj)
        }
        Command::Concept(cmd) => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            match cmd {
                ConceptCommand::Add {
                    name,
                    parent,
                    description,
                } => concept::add(&mut proj, name, parent, description)?,
                ConceptCommand::Rm { id } => concept::remove(&mut proj, id)?,
                ConceptCommand::Mv { id, parent } => concept::mv(&mut proj, id, &parent)?,
                ConceptCommand::Ls => {
                    concept::list(&proj);
                    return Ok(());
                }
                ConceptCommand::Show { id } => {
                    concept::show(&proj, id)?;
                    return Ok(());
                }
            }
            save_project(&path, &proj)
        }
        Command::Task(cmd) => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            match cmd {
                TaskCommand::Add {
                    name,
                    description,
                    duration,
                } => task::add(&mut proj, name, description, duration),
                TaskCommand::Rm { id } => task::remove(&mut proj, id)?,
                TaskCommand::Ls => {
                    task::list(&proj);
                    return Ok(());
                }
                TaskCommand::Show { id } => {
                    task::show(&proj, id)?;
                    return Ok(());
                }
                TaskCommand::Status { id, status } => task::set_status(&mut proj, id, status)?,
            }
            save_project(&path, &proj)
        }
        Command::Depend(cmd) => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            match cmd {
                DependCommand::Add {
                    task_id,
                    depends_on,
                } => task::add_dep(&mut proj, task_id, depends_on)?,
                DependCommand::Rm {
                    task_id,
                    depends_on,
                } => task::remove_dep(&mut proj, task_id, depends_on)?,
            }
            save_project(&path, &proj)
        }
        Command::Link {
            task_id,
            concept_id,
        } => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            task::link(&mut proj, task_id, concept_id)?;
            save_project(&path, &proj)
        }
        Command::Unlink {
            task_id,
            concept_id,
        } => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            task::unlink(&mut proj, task_id, concept_id)?;
            save_project(&path, &proj)
        }
    }
}
