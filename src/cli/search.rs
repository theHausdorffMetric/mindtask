use mindtask::model::project::Project;

use super::render::{render_table, resolve_wrap_width};
use super::task::{join_ids, task_cells};

fn matches(haystack: &str, query: &str) -> bool {
    haystack.to_lowercase().contains(query)
}

pub fn search(project: &Project, query: &str, search_description: bool) {
    let query = query.to_lowercase();

    let matching_concepts: Vec<_> = project
        .concepts
        .iter()
        .filter(|c| {
            matches(&c.name, &query)
                || (search_description
                    && c.description.as_deref().is_some_and(|d| matches(d, &query)))
        })
        .collect();

    let matching_tasks: Vec<_> = project
        .tasks
        .iter()
        .filter(|t| {
            matches(&t.name, &query)
                || (search_description
                    && t.description.as_deref().is_some_and(|d| matches(d, &query)))
        })
        .collect();

    if matching_concepts.is_empty() && matching_tasks.is_empty() {
        println!("No matches found.");
        return;
    }

    if !matching_concepts.is_empty() {
        let rows: Vec<Vec<String>> = matching_concepts
            .iter()
            .map(|concept| {
                let parent = concept
                    .parent
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());
                vec![concept.id.to_string(), concept.name.clone(), parent]
            })
            .collect();
        println!(
            "{}",
            render_table(
                &["ID", "NAME", "PARENT"],
                &rows,
                Some(1),
                resolve_wrap_width(project)
            )
        );
    }

    if !matching_tasks.is_empty() {
        if !matching_concepts.is_empty() {
            println!();
        }
        let rows: Vec<Vec<String>> = matching_tasks
            .iter()
            .map(|&task| {
                let mut cells = task_cells(task, project);
                cells.push(join_ids(&task.concepts));
                cells
            })
            .collect();
        println!(
            "{}",
            render_table(
                &["ID", "NAME", "STATE", "DUE", "DEPENDS ON", "CONCEPTS"],
                &rows,
                Some(1),
                resolve_wrap_width(project),
            )
        );
    }
}
