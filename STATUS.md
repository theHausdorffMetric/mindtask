# mindtask — Status

## Current Phase: Refs (Phase 6.5) on top of scheduling, import, and description output (0.13.0)

**0.13.0 (2026-09-29): `report` grouped by concept** — the top-level
report's task table is grouped under each task's primary (first-listed)
concept, groups in the order the tree prints (DFS pre-order), rows by ID
within a group, breadcrumb titles truncated to the wrap width; concepts with
no visible task are skipped, unlinked tasks close the table under
`(no concept)`, a multi-concept task appears once. `task ls` keeps the flat
ID order as the lookup view. Output-only change (new `cli/report.rs`, a
grouped table renderer in `cli/render.rs`); 11 binary tests in
`tests/report_grouping.rs`. Not yet published to crates.io.

**0.12.0 (2026-09-28): refs + format version 2** — tasks and concepts carry
`refs`, a list of URI references citing the pages they are grounded in
(relative paths resolve against the project file's directory and are checked
for existence by `validate`; URLs are syntax-only). `ref add/rm/ls/mv`,
`--ref` on both `add` commands, `Refs:` in both `show`s, refs in `search`.
The bridge from the action layer to a knowledge bundle, designed in
[docs/REFS-PLAN.md](docs/REFS-PLAN.md). With it the first file-format bump:
`version` is `2`, newer files and unknown fields are refused on load instead
of being read and silently truncated on save. Pre-0.12 binaries *do* drop refs
on save — upgrade every writer of a shared file first. Not yet published to
crates.io; PlantUML hyperlinks (R4 of the plan) deferred, low priority.

**0.10.1 (2026-09-13): metadata-only** — the repository moved from sourcehut to
GitHub (`https://github.com/theHausdorffMetric/mindtask`); `Cargo.toml`
`repository` and the changelog links follow. No code changes.

Phases 1–6 are implemented: data model, concept tree, task DAG, CLI, PlantUML
diagram export, concept subtree reporting, and CPM scheduling (earliest/latest
times, slack, critical path) surfaced by `mindtask schedule` and the
schedule-driven Gantt.

**0.11.0 (2026-09-03): input validation** — closes phase 2 of the 0.9.0
review. `--duration` refuses `nan`/`inf` (which serialised to `null` and
vanished on reload) and negatives (which scheduled a task to finish before it
started); names are trimmed and must be non-empty without control characters.
Both rules live in the model (`model/validate.rs`), so library callers and
`import` get them too, and the CLI reuses the duration rule as a clap
`value_parser` so the error names the flag. `Project::add_task` now returns
`Result`. Not yet published to crates.io.

**0.10.0 (2026-09-01): data safety + output correctness** — closes C2 (atomic saves) and phase 1
of the 5-phase plan in the 0.9.0 architecture review ([CODE_REVIEW-0.9.0.md](CODE_REVIEW-0.9.0.md)).
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

### Phase 6.5: Refs (citations) — 0.12.0
- [x] `refs: Vec<String>` on `Task` and `Concept`; `validate_ref` boundary rule
- [x] `model::reference` (pure classification) + `mindtask::refs` (filesystem check)
- [x] `Project::{add_ref, remove_ref, rename_ref, refs}`
- [x] `mindtask ref add/rm/ls/mv`, `--ref` on `task add` / `concept add`
- [x] `validate` fails on a missing relative target; operational commands never blocked
- [x] `Refs:` in `task show` / `concept show`; refs searched by `search`
- [x] `FORMAT_VERSION = 2`; newer files and unknown fields refused on load
- [x] 12 binary-level tests (`tests/refs.rs`) + unit tests across model/store/refs
- See `docs/REFS-PLAN.md` (R4 — PlantUML hyperlinks — deferred)

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
| File format version | `"version": 2` in JSON root (0.12.0); newer versions and unknown fields refused on load |
| Refs | Plain URI references, relative to the project file; existence is the only check |
| Error handling | `thiserror` in library, `anyhow` in CLI |
| Graph library | petgraph (transient graph for validation) |

## Next Steps

### Phase 6.4: Scheduling extras (deferred)
- [ ] `--start <DATE>` calendar anchoring (jiff + project timezone)
- [ ] `due` overlay / deadline-driven backward pass (negative slack)

### Refs R4 (deferred, low priority)
- [ ] First ref of a node as a PlantUML `[[…]]` hyperlink; settle the base for relative refs first (`docs/REFS-PLAN.md`)

### Phase 7: Polish
- [ ] Mermaid diagram export
- [ ] Colored terminal output
- [ ] Shell completions

### Phase 8: TUI
- [ ] Interactive terminal interface with `ratatui`
- [ ] Reuses library crate (lib.rs)
