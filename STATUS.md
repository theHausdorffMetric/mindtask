# mindtask — Status

## Current Phase: Pre-implementation

The concept and research phases are complete. The tool is ready for implementation.

## What's Done

- [x] Core concept defined (hybrid mindmap + task DAG)
- [x] Data model designed (concept tree + task DAG with cross-references)
- [x] Key design decisions made (see [concept.md](docs/concept.md))
- [x] Research on existing tools: TaskJuggler, GanttProject, OpenProject (see [research.md](docs/research.md))
- [x] Research on data formats: mindmap formats, JSON DAG patterns, CPM algorithm
- [x] Rust ecosystem surveyed: petgraph/daggy, serde, clap
- [x] Crate name `mindtask` confirmed available on crates.io
- [x] Rust crate initialized

## Next Steps

### Milestone 1: Core Data Model
- [ ] Define `Concept` and `Task` structs with serde
- [ ] Implement JSON file load/save
- [ ] ID generation (type-prefixed auto-increment: `c1`, `c2`, `t1`, `t2`)
- [ ] Concept tree operations: add, remove, move (reparent), list
- [ ] Task operations: add, remove, update
- [ ] Dependency validation (cycle detection using petgraph/daggy)
- [ ] Concept linking: attach/detach tasks from concepts

### Milestone 2: CLI Interface
- [ ] Set up clap with subcommands
- [ ] `mindtask concept add/rm/mv/ls/show`
- [ ] `mindtask task add/rm/show/ls`
- [ ] `mindtask task depend add/rm` (manage dependencies)
- [ ] `mindtask task link/unlink` (manage concept references)
- [ ] `mindtask init` (create empty project file)

### Milestone 3: Visualization
- [ ] Tree view for concepts (indented text or box-drawing chars)
- [ ] DAG view for tasks (topological order with dependency indicators)
- [ ] Combined view: concept tree with associated tasks

### Milestone 4: Scheduling
- [ ] Duration field on tasks
- [ ] Forward-pass scheduling (earliest start/finish)
- [ ] Critical path computation
- [ ] Gantt-style text output

## Open Decisions

- [ ] Resolve ID format — leaning toward `c1`/`t1` prefixed integers
- [ ] Dependency types — start with finish-to-start only, expand later?
- [ ] Task status field — add `todo`/`in_progress`/`done`?
- [ ] Concept metadata — tags, color, priority?
