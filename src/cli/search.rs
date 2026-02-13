use mindtask::model::project::Project;

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
        println!("{:<6} {:<20} PARENT", "ID", "NAME");
        for concept in &matching_concepts {
            let parent = concept
                .parent
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            println!("{:<6} {:<20} {}", concept.id, concept.name, parent);
        }
    }

    if !matching_tasks.is_empty() {
        if !matching_concepts.is_empty() {
            println!();
        }
        println!(
            "{:<6} {:<25} {:<14} {:<15} CONCEPTS",
            "ID", "NAME", "STATUS", "DEPENDS ON"
        );
        for task in &matching_tasks {
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
                task.id, task.name, task.status, deps, concepts
            );
        }
    }
}
