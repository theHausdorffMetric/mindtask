mod concept;
mod config;
mod project;
mod search;
mod task;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use mindtask::model::id::{ConceptId, TaskId};
use mindtask::model::task::TaskState;

const PROJECT_FILE: &str = ".mindtask.json";

/// Timezone assigned to a new project when `init` is run without `--timezone`.
const DEFAULT_TIMEZONE: &str = "Europe/Zurich";

#[derive(Parser)]
#[command(name = "mindtask", about = "Combine mindmaps with task dependency graphs", version)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new project in the current directory
    Init {
        /// Default timezone (IANA name, e.g. "America/New_York")
        #[arg(long, default_value = DEFAULT_TIMEZONE)]
        timezone: String,
    },
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
        /// Task ID (e.g. 1)
        task_id: TaskId,
        /// Concept ID (e.g. 1)
        concept_id: ConceptId,
    },
    /// Unlink a task from a concept
    Unlink {
        /// Task ID (e.g. 1)
        task_id: TaskId,
        /// Concept ID (e.g. 1)
        concept_id: ConceptId,
    },
    /// Search concepts and tasks by name
    Search {
        /// Search query (case-insensitive substring match)
        query: String,
        /// Also search description fields
        #[arg(long, short)]
        description: bool,
    },
    /// Export project data as a diagram
    Export {
        /// Output format (plantuml, mermaid)
        format: mindtask::export::Format,
        /// Diagram type (tree, dag, gantt, wbs)
        diagram: mindtask::export::DiagramKind,
        /// Optional root ID (concept ID for tree/wbs, task ID for dag)
        root: Option<String>,
    },
    /// Report the whole project: concept tree followed by the task list
    Report,
    /// Validate the project file
    Validate,
    /// Manage project configuration
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand)]
enum ConceptCommand {
    /// Add a new concept
    Add {
        /// Name of the concept
        name: String,
        /// Parent concept ID (e.g. 1)
        #[arg(long)]
        parent: Option<ConceptId>,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Remove a concept
    Rm {
        /// Concept ID to remove (e.g. 1)
        id: ConceptId,
    },
    /// Edit a concept's name or description
    Edit {
        /// Concept ID (e.g. 1)
        id: ConceptId,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New description (use --clear-description to remove)
        #[arg(long)]
        description: Option<String>,
        /// Clear the description
        #[arg(long)]
        clear_description: bool,
    },
    /// Move a concept to a new parent
    Mv {
        /// Concept ID to move (e.g. 1)
        id: ConceptId,
        /// New parent concept ID (e.g. 2), or "root" to make it a root concept
        #[arg(long)]
        parent: String,
    },
    /// List all concepts
    Ls,
    /// Display the concept tree
    Tree {
        /// Root concept ID to display a subtree (e.g. 1)
        id: Option<ConceptId>,
    },
    /// Show details of a concept
    Show {
        /// Concept ID (e.g. 1)
        id: ConceptId,
    },
    /// Report on a concept subtree and all related tasks
    Report {
        /// Root concept ID (e.g. 1)
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
        /// Due date (e.g. 2025-03-15T14:00, 2025-03-15T14:00[America/New_York])
        #[arg(long)]
        due: Option<String>,
        /// Concept ID to link the task to (repeatable, e.g. --concept 1 --concept 2)
        #[arg(long = "concept")]
        concepts: Vec<ConceptId>,
    },
    /// Edit a task's name, description, or duration
    Edit {
        /// Task ID (e.g. 1)
        id: TaskId,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New description (use --clear-description to remove)
        #[arg(long)]
        description: Option<String>,
        /// Clear the description
        #[arg(long)]
        clear_description: bool,
        /// New duration in days
        #[arg(long)]
        duration: Option<f64>,
        /// Clear the duration
        #[arg(long)]
        clear_duration: bool,
    },
    /// Remove a task
    Rm {
        /// Task ID to remove (e.g. 1)
        id: TaskId,
    },
    /// List all tasks
    Ls,
    /// Show details of a task
    Show {
        /// Task ID (e.g. 1)
        id: TaskId,
    },
    /// Set the state of a task
    State {
        /// Task ID (e.g. 1)
        id: TaskId,
        /// New state: todo, in_progress, or done
        state: TaskState,
    },
    /// Set or clear the due date of a task
    Due {
        /// Task ID (e.g. 1)
        id: TaskId,
        /// Due date (e.g. 2025-03-15T14:00, 2025-03-15T14:00[America/New_York])
        date: Option<String>,
        /// Clear the due date
        #[arg(long)]
        clear: bool,
    },
}

#[derive(Subcommand)]
enum DependCommand {
    /// Add a dependency between tasks
    Add {
        /// Task that depends on another (e.g. 2)
        task_id: TaskId,
        /// Task that must finish first (e.g. 1)
        depends_on: TaskId,
    },
    /// Remove a dependency between tasks
    Rm {
        /// Task that has the dependency (e.g. 2)
        task_id: TaskId,
        /// Dependency to remove (e.g. 1)
        depends_on: TaskId,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Get or set the project timezone
    Timezone {
        /// Timezone to set (IANA name, e.g. "America/New_York")
        timezone: Option<String>,
        /// Show the current timezone
        #[arg(long)]
        show: bool,
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
    let proj = mindtask::store::json::load(path)
        .with_context(|| format!("failed to load project from {}", path.display()))?;
    // Reject malformed on-disk files (duplicate IDs, dangling refs, cyclic tree)
    // before any command operates on them. Some of these would otherwise cause
    // incorrect results or hangs. `mindtask validate` reports the same errors.
    mindtask::graph::dag::validate_project(&proj).map_err(|e| {
        anyhow::anyhow!(
            "{} is invalid: {e}\n\
             Fix the file or run 'mindtask validate' for details.",
            path.display()
        )
    })?;
    Ok(proj)
}

fn save_project(path: &Path, project: &mindtask::model::project::Project) -> Result<()> {
    mindtask::store::json::save(path, project)
        .with_context(|| format!("failed to save project to {}", path.display()))
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { timezone } => project::init(timezone),
        Command::Report => {
            let path = find_project_file()?;
            let proj = load_project(&path)?;
            concept::tree(&proj, None)?;
            println!();
            task::list(&proj);
            Ok(())
        }
        Command::Validate => {
            let path = find_project_file()?;
            // Load without the validating wrapper so this command can produce its
            // own diagnostic instead of being blocked by load_project's check.
            let proj = mindtask::store::json::load(&path)
                .with_context(|| format!("failed to load project from {}", path.display()))?;
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
                ConceptCommand::Edit {
                    id,
                    name,
                    description,
                    clear_description,
                } => concept::edit(&mut proj, id, name, description, clear_description)?,
                ConceptCommand::Mv { id, parent } => concept::mv(&mut proj, id, &parent)?,
                ConceptCommand::Ls => {
                    concept::list(&proj);
                    return Ok(());
                }
                ConceptCommand::Tree { id } => {
                    concept::tree(&proj, id)?;
                    return Ok(());
                }
                ConceptCommand::Show { id } => {
                    concept::show(&proj, id)?;
                    return Ok(());
                }
                ConceptCommand::Report { id } => {
                    concept::report(&proj, id)?;
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
                    due,
                    concepts,
                } => task::add(&mut proj, name, description, duration, due, concepts)?,
                TaskCommand::Edit {
                    id,
                    name,
                    description,
                    clear_description,
                    duration,
                    clear_duration,
                } => task::edit(
                    &mut proj,
                    id,
                    name,
                    description,
                    clear_description,
                    duration,
                    clear_duration,
                )?,
                TaskCommand::Rm { id } => task::remove(&mut proj, id)?,
                TaskCommand::Ls => {
                    task::list(&proj);
                    return Ok(());
                }
                TaskCommand::Show { id } => {
                    task::show(&proj, id)?;
                    return Ok(());
                }
                TaskCommand::State { id, state } => task::set_state(&mut proj, id, state)?,
                TaskCommand::Due { id, date, clear } => {
                    task::set_due(&mut proj, id, date, clear)?
                }
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
        Command::Search { query, description } => {
            let path = find_project_file()?;
            let proj = load_project(&path)?;
            search::search(&proj, &query, description);
            Ok(())
        }
        Command::Export {
            format,
            diagram,
            root,
        } => {
            let path = find_project_file()?;
            let proj = load_project(&path)?;
            let output = mindtask::export::render(&proj, format, diagram, root.as_deref())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            print!("{output}");
            Ok(())
        }
        Command::Config(cmd) => {
            let path = find_project_file()?;
            let mut proj = load_project(&path)?;
            match cmd {
                ConfigCommand::Timezone { timezone, show } => {
                    if config::timezone(&mut proj, timezone, show)? {
                        save_project(&path, &proj)?;
                    }
                }
            }
            Ok(())
        }
    }
}
