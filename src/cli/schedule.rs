//! `mindtask schedule` — print the CPM schedule (earliest start/finish, slack,
//! and the critical path) computed by [`mindtask::graph::schedule`]. Read-only.

use anyhow::Result;

use mindtask::graph::schedule::schedule;
use mindtask::model::project::Project;

use super::render::{render_table, resolve_wrap_width};

/// Format a day value, trimming trailing zeros (`2`, `1.5`, `0`).
fn fmt_days(d: f64) -> String {
    let s = format!("{d:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn run(project: &Project, critical_only: bool) -> Result<()> {
    if project.tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let sched = schedule(&project.tasks).map_err(|e| anyhow::anyhow!(e))?;

    // A zero-length project means no task carries a duration; the table would be
    // all zeros, so guide the user instead of printing noise.
    if sched.duration.abs() < 1e-9 {
        println!(
            "No task durations set — nothing to schedule.\n\
             Add estimates with `mindtask task edit <ID> --duration <DAYS>`."
        );
        return Ok(());
    }

    let rows: Vec<Vec<String>> = sched
        .tasks
        .iter()
        .filter(|st| !critical_only || st.critical)
        .map(|st| {
            let name = project
                .get_task(st.id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            vec![
                st.id.to_string(),
                name,
                fmt_days(st.duration),
                fmt_days(st.earliest_start),
                fmt_days(st.earliest_finish),
                fmt_days(st.slack),
                if st.critical {
                    "critical".to_string()
                } else {
                    String::new()
                },
            ]
        })
        .collect();

    println!(
        "{}",
        render_table(
            &["ID", "NAME", "DUR", "START", "FINISH", "SLACK", ""],
            &rows,
            Some(1),
            resolve_wrap_width(project),
        )
    );

    println!("\nProject duration: {} days", fmt_days(sched.duration));
    let critical = sched.critical_path();
    if !critical.is_empty() {
        let names: Vec<String> = critical
            .iter()
            .filter_map(|id| project.get_task(*id).map(|t| t.name.clone()))
            .collect();
        println!("Critical path: {}", names.join(" → "));
    }

    Ok(())
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
