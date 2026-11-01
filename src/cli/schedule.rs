//! `mindtask schedule` — print the CPM schedule (earliest start/finish, slack,
//! and the critical path) computed by [`mindtask::graph::schedule`]. Read-only.

use anyhow::Result;

use mindtask::model::project::Project;
use mindtask::report::ScheduleView;

use super::format::{OutputFormat, print_json};
use super::markdown;
use super::render::{render_table, resolve_wrap_width};

/// Format a day value, trimming trailing zeros (`2`, `1.5`, `0`).
pub(super) fn fmt_days(d: f64) -> String {
    let s = format!("{d:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Column headers of the schedule table; the unnamed last column carries the
/// `critical` marker.
pub(super) const SCHEDULE_HEADERS: [&str; 7] =
    ["ID", "NAME", "DUR", "START", "FINISH", "SLACK", ""];

/// The cells of one schedule row, in [`SCHEDULE_HEADERS`] order.
pub(super) fn schedule_cells(row: &mindtask::report::ScheduleRow) -> Vec<String> {
    vec![
        row.id.to_string(),
        row.name.clone(),
        fmt_days(row.duration),
        fmt_days(row.earliest_start),
        fmt_days(row.earliest_finish),
        fmt_days(row.slack),
        if row.critical {
            "critical".to_string()
        } else {
            String::new()
        },
    ]
}

pub fn run(project: &Project, critical_only: bool, format: OutputFormat) -> Result<()> {
    let view = ScheduleView::build(project, critical_only).map_err(|e| anyhow::anyhow!(e))?;
    match format {
        OutputFormat::Json => print_json(&view),
        OutputFormat::Md => {
            print!("{}", markdown::schedule(project, &view));
            Ok(())
        }
        OutputFormat::Text => {
            run_text(project, &view);
            Ok(())
        }
    }
}

fn run_text(project: &Project, view: &ScheduleView) {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return;
    }

    // A zero-length project means no task carries a duration; the table would be
    // all zeros, so guide the user instead of printing noise.
    if !view.has_durations() {
        println!(
            "No task durations set — nothing to schedule.\n\
             Add estimates with `mindtask task edit <ID> --duration <DAYS>`."
        );
        return;
    }

    let rows: Vec<Vec<String>> = view.tasks.iter().map(schedule_cells).collect();
    println!(
        "{}",
        render_table(
            &SCHEDULE_HEADERS,
            &rows,
            Some(1),
            resolve_wrap_width(project)
        )
    );

    println!("\nProject duration: {} days", fmt_days(view.duration));
    if !view.critical_path.is_empty() {
        let names: Vec<String> = view
            .critical_path
            .iter()
            .filter_map(|id| project.get_task(*id).map(|t| t.name.clone()))
            .collect();
        println!("Critical path: {}", names.join(" → "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_days_trims_trailing_zeros() {
        assert_eq!(fmt_days(2.0), "2");
        assert_eq!(fmt_days(1.5), "1.5");
        assert_eq!(fmt_days(0.0), "0");
        assert_eq!(fmt_days(-0.0), "0");
        assert_eq!(fmt_days(3.25), "3.25");
    }
}
