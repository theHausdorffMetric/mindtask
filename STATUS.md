# mindtask — Status

## Current Phase: Visualization via PlantUML export

Phases 1–5 are implemented: data model, concept tree, task DAG, CLI, and PlantUML diagram export. The tool is usable for creating, managing, and visualizing projects from the command line.

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

### Phase 5: Diagram Export
- [x] `mindtask export plantuml tree` — concept tree as PlantUML mindmap
- [x] `mindtask export plantuml dag` — task DAG as PlantUML component diagram (colored by state)
- [x] `mindtask export plantuml gantt` — tasks with due dates as PlantUML Gantt chart
- [x] `mindtask export plantuml wbs` — concept tree with tasks as leaves (work breakdown structure)
- [x] Optional root ID for tree and dag (render subtree/subgraph)
- [x] Mermaid format stubbed for future implementation
- [x] Test fixture generator (`tests/fixtures/generate.sh`) and justfile for rendering

## Architecture

```
src/
  lib.rs              # library root — reusable for future TUI
  main.rs             # thin binary — calls cli::run()
  model/              # Concept, Task, Project, ID newtypes
  store/              # JSON persistence
  graph/              # tree validation, DAG cycle detection (petgraph)
  export/             # diagram generation (PlantUML, Mermaid stub)
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

### Phase 6: Scheduling
- [ ] Forward-pass scheduling (earliest start/finish)
- [ ] Critical path computation

### Phase 7: Polish
- [ ] Mermaid diagram export
- [ ] Colored terminal output
- [ ] Shell completions
- [ ] Edit commands (rename concepts/tasks, update descriptions)

### Phase 8: TUI
- [ ] Interactive terminal interface with `ratatui`
- [ ] Reuses library crate (lib.rs)
