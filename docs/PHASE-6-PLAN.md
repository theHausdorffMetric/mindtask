# Phase 6 — Scheduling (Critical Path Method)

Goal: from the task DAG (finish-to-start dependencies + per-task durations in
days), compute each task's earliest/latest start & finish, its **slack**, and the
**critical path**, plus the overall project duration. This is the standard
Critical Path Method (CPM); mindtask already has everything CPM needs (a DAG,
durations, and a topological sort), so the values are *derived* — no data-model
change.

CPM assumes unlimited resources: the only constraints are dependencies. Resource
leveling (capacity limits, "two tasks can't run at once") is explicitly **out of
scope**.

## Locked design defaults (v1)

- **Relative day-offsets** (day 0 = project start), `f64` days. Calendar
  anchoring is a later flag.
- **`due` = overlay**, not a constraint: compute from dependencies, then compare
  against `due` to flag slips. (A deadline-driven backward pass is 6.4.)
- **No-duration tasks = 0-day milestones** (`duration` is `Option`).
- **`done` / `in_progress` tasks still occupy time** (keeps the critical path
  stable). Pinning `done` tasks to actuals is deferred.
- Output in **project order**; the critical path is traced separately.

## File layout

| File | Change |
|---|---|
| `src/graph/schedule.rs` | **new** — CPM algorithm (library core: pure, testable) |
| `src/graph/mod.rs` | `pub mod schedule;` |
| `src/cli/schedule.rs` | **new** — `mindtask schedule` handler (uses `render_table`) |
| `src/cli/mod.rs` | add `Schedule` command variant + dispatch |
| `src/export/plantuml.rs` | rewire the Gantt to the computed schedule (6.3) |
| `STATUS.md` / `README.md` / `CHANGELOG.md` | docs |

The algorithm lives in the **library** (`mindtask::graph::schedule`) so the CLI,
the Gantt export, and a future TUI share it.

## Core API (`graph/schedule.rs`)

```rust
pub struct ScheduledTask {
    pub id: TaskId,
    pub duration: f64,          // resolved; 0.0 for milestones
    pub earliest_start: f64,
    pub earliest_finish: f64,
    pub latest_start: f64,
    pub latest_finish: f64,
    pub slack: f64,
    pub critical: bool,
}

pub struct Schedule {
    pub tasks: Vec<ScheduledTask>, // project order
    pub duration: f64,             // project length = max(EF)
}

impl Schedule {
    pub fn get(&self, id: TaskId) -> Option<&ScheduledTask>;
    pub fn critical_path(&self) -> Vec<TaskId>; // critical tasks, topological order
}

/// CPM forward+backward pass. Err on a cycle (validate_dag prevents this upstream).
pub fn schedule(tasks: &[Task]) -> Result<Schedule, String>;
```

### Algorithm (reuses `dag::topological_order`)

1. Resolve `dur = duration.unwrap_or(0.0)`; build a successors map (invert
   `depends_on`).
2. **Forward** (topo order): `ES = max(EF of depends_on, else 0)`,
   `EF = ES + dur`. `project = max(EF)`.
3. **Backward** (reverse topo): `LF = min(LS of successors, else project)`,
   `LS = LF − dur`.
4. `slack = LS − ES`; `critical = slack.abs() < 1e-9`.

### Worked example (the 6.1 acceptance fixture)

```
A Design API   (2)                          E Frontend (4)   [no deps]
B Design DB    (1)  →  C Implement DB (3)  →  D Implement API (5)
                                  A ─────────↗   (D depends on A and C)
```

| Task | dur | ES | EF | LS | LF | slack | critical |
|---|---|----|----|----|----|------|----------|
| A | 2 | 0 | 2 | 2 | 4 | 2 | |
| B | 1 | 0 | 1 | 0 | 1 | 0 | ✓ |
| C | 3 | 1 | 4 | 1 | 4 | 0 | ✓ |
| D | 5 | 4 | 9 | 4 | 9 | 0 | ✓ |
| E | 4 | 0 | 4 | 5 | 9 | 5 | |

Project duration = 9. Critical path = B → C → D.

## Milestones

### 6.1 — `graph::schedule` (library core)

Implement the two passes + `critical_path()`.
*Tests:* the worked example above; empty project (duration 0); single task; a
milestone (dur 0) anchoring deps; a diamond; `schedule()` on a hand-built cyclic
`&[Task]` returns `Err`.
*Acceptance:* matches the hand-computed table; `cargo test` green.

### 6.2 — `mindtask schedule` command

`cli/schedule.rs` renders via `render_table`:

```
ID  NAME            DUR  START  FINISH  SLACK
2   Design DB       1    0      1       0       critical
3   Implement DB    3    1      4       0       critical
...
Project duration: 9 days   Critical path: Design DB → Implement DB → Implement API
```

- Columns: ID, NAME (flex), DUR, START, FINISH, SLACK, + trailing `critical`
  marker (same pattern as `concept report`'s `(upstream dep)` column).
- f64-day formatter that trims `.0` (`2`, not `2.0`; keeps `1.5`).
- `--critical` flag (only critical tasks).
- Degenerate guard: if project duration is 0 (no durations set), print a hint to
  add `--duration`.
*Tests:* integration in `tests/cli_validation.rs` — build a project, run, assert
the summary + a critical marker.
*Acceptance:* aligned table; the example reproduces.

### 6.3 — Gantt export on the real schedule

Rewire `export/plantuml.rs` Gantt to emit `[Task] starts on day N` / `lasts D
days` from the `Schedule`, and color the critical path. Keep the current
due-date bars only as a fallback when no durations exist.
*Tests:* extend the existing plantuml tests with schedule-driven assertions.
*Acceptance:* Gantt reflects computed start/finish; critical path is distinct.

### 6.4 — Deadline awareness (stretch / defer)

- `--start <DATE>` to anchor relative offsets to calendar dates (jiff + project
  timezone).
- `due` overlay: flag tasks whose finish exceeds `due` (a `LATE` marker);
  optional deadline-driven backward pass (`LF = min(succ LS, due)`) yielding
  **negative slack**.

Defer unless wanted — v1 is dependency-driven CPM.

## Scope line

- **In:** earliest/latest times, slack, critical path, project duration, and
  (6.4) deadline-slip flags — all from the dependency DAG.
- **Out / later:** resource leveling, calendar/working-day arithmetic,
  dependency types beyond finish-to-start, lead/lag.

**v1 = 6.1 + 6.2** (algorithm + command). 6.3 (Gantt) is the high-value
follow-on; 6.4 is optional.
