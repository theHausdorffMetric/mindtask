use anyhow::{Context, Result};
use termtree::Tree;

use mindtask::model::concept::Concept;
use mindtask::model::id::ConceptId;
use mindtask::model::project::Project;

pub fn add(
    project: &mut Project,
    name: String,
    parent: Option<ConceptId>,
    description: Option<String>,
) -> Result<()> {
    let id = project
        .add_concept(name.clone(), parent, description)
        .context("failed to add concept")?;
    println!("Added concept {} \"{}\"", id, name);
    Ok(())
}

pub fn remove(project: &mut Project, id: ConceptId) -> Result<()> {
    let name = project
        .get_concept(id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    project
        .remove_concept(id)
        .context("failed to remove concept")?;
    println!("Removed concept {} \"{}\"", id, name);
    Ok(())
}

pub fn mv(project: &mut Project, id: ConceptId, parent_str: &str) -> Result<()> {
    let new_parent = if parent_str == "root" {
        None
    } else {
        Some(
            parent_str
                .parse::<ConceptId>()
                .context("invalid parent ID (use a concept ID like '1' or 'root')")?,
        )
    };

    project
        .move_concept(id, new_parent)
        .context("failed to move concept")?;

    match new_parent {
        Some(pid) => println!("Moved concept {} under {}", id, pid),
        None => println!("Moved concept {} to root", id),
    }
    Ok(())
}

pub fn list(project: &Project) {
    if project.concepts.is_empty() {
        println!("No concepts.");
        return;
    }

    println!("{:<6} {:<20} PARENT", "ID", "NAME");
    for concept in &project.concepts {
        let parent = concept
            .parent
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!("{:<6} {:<20} {}", concept.id, concept.name, parent);
    }
}

pub fn show(project: &Project, id: ConceptId) -> Result<()> {
    let concept = project
        .get_concept(id)
        .ok_or_else(|| anyhow::anyhow!("concept {} not found", id))?;

    println!("ID:          {}", concept.id);
    println!("Name:        {}", concept.name);
    if let Some(desc) = &concept.description {
        println!("Description: {}", desc);
    }
    match concept.parent {
        Some(pid) => {
            let parent_name = project
                .get_concept(pid)
                .map(|c| c.name.as_str())
                .unwrap_or("???");
            println!("Parent:      {} ({})", pid, parent_name);
        }
        None => println!("Parent:      (root)"),
    }

    let children = project.children_of(id);
    if !children.is_empty() {
        let child_strs: Vec<String> = children.iter().map(|c| format!("{} ({})", c.id, c.name)).collect();
        println!("Children:    {}", child_strs.join(", "));
    }

    let linked_tasks: Vec<_> = project
        .tasks
        .iter()
        .filter(|t| t.concepts.contains(&id))
        .collect();
    if !linked_tasks.is_empty() {
        let task_strs: Vec<String> = linked_tasks
            .iter()
            .map(|t| format!("{} ({})", t.id, t.name))
            .collect();
        println!("Tasks:       {}", task_strs.join(", "));
    }

    Ok(())
}

pub fn tree(project: &Project, root_id: Option<ConceptId>) -> Result<()> {
    if project.concepts.is_empty() {
        println!("No concepts.");
        return Ok(());
    }

    let trees: Vec<Tree<String>> = match root_id {
        Some(id) => {
            let concept = project
                .get_concept(id)
                .ok_or_else(|| anyhow::anyhow!("concept {} not found", id))?;
            vec![build_tree(project, concept)]
        }
        None => project.roots().into_iter().map(|c| build_tree(project, c)).collect(),
    };

    for t in &trees {
        print!("{t}");
    }
    Ok(())
}

fn build_tree(project: &Project, concept: &Concept) -> Tree<String> {
    let mut node = Tree::new(format!("{} [{}]", concept.name, concept.id));
    for child in project.children_of(concept.id) {
        node.push(build_tree(project, child));
    }
    node
}
