# mindtask — Status

## Current Phase: Scheduling (Phase 6) + graph import + description output (0.10.0)

**0.10.1 (2026-09-13): metadata-only** — the repository moved from sourcehut to
GitHub (`https://github.com/theHausdorffMetric/mindtask`); `Cargo.toml`
`repository` and the changelog links follow. No code changes.

Phases 1–6 are implemented: data model, concept tree, task DAG, CLI, PlantUML
diagram export, concept subtree reporting, and CPM scheduling (earliest/latest
times, slack, critical path) surfaced by `mindtask schedule` and the
schedule-driven Gantt.

**0.10.0 (2026-09-01): data safety + output correctness** — closes phases 1-2
of the 0.9.0 architecture review ([CODE_REVIEW-0.9.0.md](CODE_REVIEW-0.9.0.md)).
Saves are atomic (temp file + fsync + rename), so an interrupt or full disk can
no longer truncate the project file. The Gantt export identifies tasks by ID
alias rather than by name, which previously made two same-named tasks emit a
self-dependency PlantUML happily drew. Names are sanitised before entering
diagram syntax. `export mermaid` exits non-zero instead of printing a
placeholder. Two small library API changes; the CLI surface is unchanged.
Not yet published to crates.io.

**0.9.0 (2026-09-01): description output** — descriptions were reachable only
through `task show`, one task at a time, and printed unwrapped. `-d` now works
on `task ls` and on the task half of `report`/`concept report` (it previously
reached only the concept tree), with `--description=short` for a one-line lede;
`search -d` prints the excerpt that matched instead of silently widening
recall; `task show` wraps to the resolved width; and wrapping is word-aware
throughout. Not yet published to crates.io.

**0.8.0 (2026-08-02): `mindtask import`** — consumes a typed concept-graph
JSONL stream (the `pdfdex graph --format jsonl` contract) and merges its
`is-a` slice into the concept tree as a strict subtree under a target concept
(multi-parent → highest-weight edge, cycles broken at the weakest edge,
roots grouped under category umbrellas; add-only by default, `--reparent`
opt-in, `--dry-run`). The cross-project seam from the ideas repo (task 6).
Not yet published to crates.io.

## What's Done

### Design & Research
- [x] Core concept defined (hybrid mindmap + task DAG)
- [x] Data model designed (concept tree + task DAG with cross-references)
- [x] Key design decisions made (see [concept.md](docs/concept.md))
- [x] Research on existing tools: TaskJuggler, GanttProject, OpenProject (see [research.md](docs/research.md))
- [x] Research on data formats: mindmap formats, JSON DAG patterns, CPM algorithm
- [x] Rust ecosystem surveyed: petgraph/daggy, serde, clap
- [x] Crate name `mindtask` confirmed available on crates.io

### Phase 1: Core Data Model
- [x] `Concept` and `Task` structs with serde
- [x] JSON file load/save (`.mindtask.json`, pretty-printed)
- [x] Auto-increment integer IDs (separate sequences for concepts and tasks)
- [x] `Project` container with version field for future-proofing
- [x] 43 unit tests covering serialization round-trips, ID parsing, error cases

### Phase 2: Concept Tree Operations
- [x] Add, remove, move (reparent), list, show concepts
- [x] Tree ancestry checks (cycle prevention on move)
- [x] Removal guards: fails if concept has children or linked tasks
- [x] Tree validation (parent refs valid, no orphan chains)

### Phase 3: Task DAG Operations
- [x] Add, remove, list, show tasks
- [x] Task state: `todo` / `in_progress` / `done`
- [x] Dependency management with petgraph-based cycle detection
- [x] Task removal cleans up dependency references in other tasks
- [x] Concept linking/unlinking (many-to-many, idempotent)
- [x] Full project validation (tree + DAG + cross-references)
- [x] Topological sort

### Phase 4: CLI Interface
- [x] `mindtask init` — create `.mindtask.json`
- [x] `mindtask concept add/rm/mv/ls/show`
- [x] `mindtask task add/rm/ls/show/state`
- [x] `mindtask task due` — set/clear due dates
- [x] `mindtask depend add/rm`
- [x] `mindtask link/unlink`
- [x] `mindtask validate`
- [x] `mindtask config timezone` — get/set project timezone
- [x] Project file discovery (walks up parent dirs like `.git`)
- [x] Clear error messages (cycle rejection, missing refs, etc.)

### Phase 4.5: Due Dates & Timezones
- [x] `due: Option<Zoned>` on tasks (RFC 9557 serialization via `jiff`)
- [x] `--due` flag on `task add`
- [x] `task due <id> <date>` / `task due <id> --clear` subcommand
- [x] Project-level default timezone (`timezone` field in project JSON)
- [x] `mindtask init --timezone` to set timezone at creation
- [x] `mindtask config timezone` to get/set timezone after creation
- [x] Due dates displayed in project timezone in `task show`, `task ls`, `search`
- [x] Flexible input parsing: full RFC 9557, datetime, or date-only (falls back to project tz)

### Phase 4.7: Search
- [x] `mindtask search <query>` — case-insensitive substring match across concepts and tasks
- [x] `--description` flag to also search description fields

### Phase 4.8: Concept Report
- [x] `mindtask concept report <ID>` — show concept subtree + all related tasks
- [x] Collects tasks linked to any concept in the subtree
- [x] Walks transitive upstream dependencies (tasks required by linked tasks)
- [x] Marks upstream-only tasks with `(upstream dep)` in output

### Phase 5: Diagram Export
- [x] `mindtask export plantuml tree` — concept tree as PlantUML mindmap
- [x] `mindtask export plantuml dag` — task DAG as PlantUML component diagram (colored by state)
- [x] `mindtask export plantuml gantt` — tasks with due dates as PlantUML Gantt chart
- [x] `mindtask export plantuml wbs` — concept tree with tasks as leaves (work breakdown structure)
- [x] Optional root ID for tree and dag (render subtree/subgraph)
- [x] Mermaid format accepted as a value but unimplemented — every call errors (never emits a placeholder)
- [x] Test fixture generator (`tests/fixtures/generate.sh`) and justfile for rendering

### Phase 6: Scheduling (CPM)
- [x] `graph::schedule` library module: forward/backward pass, slack, critical path
- [x] `mindtask schedule` command (ES/EF/slack table, critical path, `--critical`)
- [x] Schedule-driven PlantUML Gantt (relative days + critical-path highlight)
- [x] Relative day-offsets; no-duration tasks are zero-day milestones
- See `docs/PHASE-6-PLAN.md` (6.4 — calendar anchoring / deadline slack — deferred)

## Architecture

```
src/
  lib.rs              # library root — reusable for future TUI
  main.rs             # thin binary — calls cli::run()
  model/              # Concept, Task, Project, ID newtypes
  store/              # JSON persistence
  graph/              # tree validation, DAG cycle detection (petgraph)
  export/             # diagram generation (PlantUML; Mermaid errors)
  cli/                # clap definitions and command handlers
```

## Resolved Decisions

| Decision | Choice |
|---|---|
| Project file | `.mindtask.json` (hidden dotfile) |
| ID format | Auto-increment integers (separate sequences per type) |
| Task state | `todo` / `in_progress` / `done` |
| Dependency types | Finish-to-start only |
| File format version | `"version": 1` in JSON root |
| Error handling | `thiserror` in library, `anyhow` in CLI |
| Graph library | petgraph (transient graph for validation) |

## Next Steps

### Phase 6.4: Scheduling extras (deferred)
- [ ] `--start <DATE>` calendar anchoring (jiff + project timezone)
- [ ] `due` overlay / deadline-driven backward pass (negative slack)

### Phase 7: Polish
- [ ] Mermaid diagram export
- [ ] Colored terminal output
- [ ] Shell completions

### Phase 8: TUI
- [ ] Interactive terminal interface with `ratatui`
- [ ] Reuses library crate (lib.rs)
