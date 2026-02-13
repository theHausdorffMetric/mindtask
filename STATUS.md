# mindtask — Status

## Current Phase: Core complete — ready for visualization

Phases 1–4 are implemented: data model, concept tree, task DAG, and CLI. The tool is usable for creating and managing projects from the command line.

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
- [x] Task status: `todo` / `in_progress` / `done`
- [x] Dependency management with petgraph-based cycle detection
- [x] Task removal cleans up dependency references in other tasks
- [x] Concept linking/unlinking (many-to-many, idempotent)
- [x] Full project validation (tree + DAG + cross-references)
- [x] Topological sort

### Phase 4: CLI Interface
- [x] `mindtask init` — create `.mindtask.json`
- [x] `mindtask concept add/rm/mv/ls/show`
- [x] `mindtask task add/rm/ls/show/status`
- [x] `mindtask depend add/rm`
- [x] `mindtask link/unlink`
- [x] `mindtask validate`
- [x] Project file discovery (walks up parent dirs like `.git`)
- [x] Clear error messages (cycle rejection, missing refs, etc.)

## Architecture

```
src/
  lib.rs              # library root — reusable for future TUI
  main.rs             # thin binary — calls cli::run()
  model/              # Concept, Task, Project, ID newtypes
  store/              # JSON persistence
  graph/              # tree validation, DAG cycle detection (petgraph)
  cli/                # clap definitions and command handlers
```

## Resolved Decisions

| Decision | Choice |
|---|---|
| Project file | `.mindtask.json` (hidden dotfile) |
| ID format | Auto-increment integers (separate sequences per type) |
| Task status | `todo` / `in_progress` / `done` |
| Dependency types | Finish-to-start only |
| File format version | `"version": 1` in JSON root |
| Error handling | `thiserror` in library, `anyhow` in CLI |
| Graph library | petgraph (transient graph for validation) |

## Next Steps

### Phase 5: Visualization
- [ ] Tree view for concepts (box-drawing characters)
- [ ] DAG view for tasks (topological order with dependency indicators)
- [ ] Combined view: concept tree with associated tasks

### Phase 6: Scheduling
- [ ] Forward-pass scheduling (earliest start/finish)
- [ ] Critical path computation
- [ ] Gantt-style text output

### Phase 7: Polish
- [ ] Colored output
- [ ] Shell completions
- [ ] Edit commands (rename concepts/tasks, update descriptions)
- [ ] Error message audit

### Phase 8: TUI
- [ ] Interactive terminal interface with `ratatui`
- [ ] Reuses library crate (lib.rs)
