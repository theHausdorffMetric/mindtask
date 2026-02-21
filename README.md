# mindtask

A CLI tool that combines **concept maps** (mindmaps) with **task dependency graphs** for planning and scheduling work.

## The Problem

Mindmaps are great for brainstorming and organizing ideas, but they don't capture *work* — dependencies, scheduling, critical paths. Task managers handle work well but lose the big-picture structure of how ideas relate. You end up maintaining both separately, with no connection between them.

## The Approach

mindtask maintains two linked structures in a single JSON file:

- **Concept tree** — a strict tree (each node has 0 or 1 parent) for organizing ideas, like a classic mindmap
- **Task DAG** — a directed acyclic graph of tasks with dependencies, enabling scheduling and critical path analysis

Tasks reference concepts, creating a bridge between *what you're thinking about* and *what you need to do*. The link is directional: tasks point to concepts, not the other way around. A task can reference multiple concepts, and multiple tasks can reference the same concept.

```
Concept Tree                Task DAG

  Project                   1: Design API ──→ 3: Integrate
  ├── Backend               2: Design DB ───┘
  │   ├── API
  │   └── Database          task 1 concepts = [API]
  └── Frontend              task 2 concepts = [Database]
      └── Components        task 3 concepts = [API, Database]
```

## Data Model

```json
{
  "timezone": "America/New_York",
  "concepts": [
    { "id": 1, "name": "Project" },
    { "id": 2, "name": "Backend", "parent": 1 },
    { "id": 3, "name": "API", "parent": 2 }
  ],
  "tasks": [
    { "id": 1, "name": "Design API", "state": "done", "concepts": [3] },
    { "id": 2, "name": "Implement API", "state": "todo", "due": "2025-03-15T14:00:00-04:00[America/New_York]", "depends_on": [1], "concepts": [3] }
  ]
}
```

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Concept structure | Strict tree | Proven mindmap model. Simple, intuitive. |
| Task structure | DAG | Dependencies must be acyclic for scheduling to work. |
| Linking | Tasks → Concepts | Keeps the concept tree clean and independent. |
| Storage | Single JSON file | Human-readable, versionable with git, no database needed. |
| IDs | Auto-increment integers | Simple to type, context distinguishes concepts from tasks. |

## Features

- Add/remove/move concepts in the tree
- Add/remove tasks with dependency validation (cycle detection)
- Link tasks to concepts (many-to-many)
- Task state tracking (todo / in_progress / done)
- Due dates with timezone support (RFC 9557 / IANA timezones via `jiff`)
- Project-level default timezone
- Project validation (tree integrity + DAG acyclicity + reference checks)
- Search concepts and tasks by name or description
- Export diagrams as PlantUML (tree, DAG, Gantt, WBS)
- JSON persistence (`.mindtask.json`, human-readable)
- Full CLI with project file discovery

### Planned

- Mermaid diagram export
- Critical path analysis for tasks with durations
- Interactive TUI

## Documentation

- [Concept Document](docs/concept.md) — detailed design decisions and data model
- [Research Report](docs/research.md) — analysis of existing tools and data structures

## License

MIT
