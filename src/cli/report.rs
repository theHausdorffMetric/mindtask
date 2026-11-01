//! The top-level `report`: the concept tree, then every task grouped under
//! the concept it belongs to, in the order the tree was just printed.
//!
//! `task ls` is the flat, ID-ordered list for looking a task up; `report` is
//! the orientation view, so its table follows the shape of the project
//! instead of the order tasks happened to be created in. The grouping itself
//! lives in [`mindtask::report`]; this module renders it.

use anyhow::Result;

use mindtask::model::project::Project;
use mindtask::report::Report;

use super::concept;
use super::format::{OutputFormat, print_json};
use super::render::{DescMode, TableGroup, render_table_grouped, resolve_wrap_width};
use super::state_filter::{StateFilter, hidden_footer};
use super::task::{TASK_HEADERS, desc_block_for, join_ids, row_cells};

pub fn run(
    project: &Project,
    filter: &StateFilter,
    desc: DescMode,
    format: OutputFormat,
) -> Result<()> {
    let report = Report::build(project, &filter.states());
    match format {
        OutputFormat::Json => print_json(&report),
        OutputFormat::Text => {
            concept::tree_text(project, None, desc)?;
            println!();
            tasks_by_concept(project, &report, desc);
            Ok(())
        }
    }
}

/// The task half of the text `report`: one table whose rows are grouped by
/// each task's *primary* concept, in the order the tree printed above (see
/// [`mindtask::report::group_by_primary_concept`] for the rules).
fn tasks_by_concept(project: &Project, report: &Report, desc: DescMode) {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return;
    }

    if report.groups.is_empty() {
        println!("No tasks in the selected states.");
    } else {
        let width = resolve_wrap_width(project);
        let groups: Vec<TableGroup> = report
            .groups
            .iter()
            .map(|g| TableGroup {
                title: g.title.clone(),
                rows: g
                    .tasks
                    .iter()
                    .map(|row| {
                        let mut cells = row_cells(row);
                        cells.push(join_ids(&row.concepts));
                        cells
                    })
                    .collect(),
                blocks: g
                    .tasks
                    .iter()
                    .map(|row| desc_block_for(row.description.as_deref(), desc, width))
                    .collect(),
            })
            .collect();
        println!(
            "{}",
            render_table_grouped(&TASK_HEADERS, &groups, Some(1), width)
        );
    }
    if let Some(footer) = hidden_footer(&report.hidden) {
        println!("{footer}");
    }
}
