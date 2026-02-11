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

  Project                   t1: Design API ──→ t3: Integrate
  ├── Backend               t2: Design DB ───┘
  │   ├── API
  │   └── Database          t1.concepts = [API]
  └── Frontend              t2.concepts = [Database]
      └── Components        t3.concepts = [API, Database]
```

## Data Model

```json
{
  "concepts": [
    { "id": "c1", "name": "Project", "parent": null },
    { "id": "c2", "name": "Backend", "parent": "c1" },
    { "id": "c3", "name": "API", "parent": "c2" }
  ],
  "tasks": [
    { "id": "t1", "title": "Design API", "depends_on": [], "concepts": ["c3"] },
    { "id": "t2", "title": "Implement API", "depends_on": ["t1"], "concepts": ["c3"] }
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
| IDs | Type-prefixed integers (`c1`, `t1`) | Easy to type in a CLI, easy to distinguish at a glance. |

## Planned Features

- Add/remove/move concepts in the tree
- Add/remove tasks with dependency validation (cycle detection)
- Link tasks to concepts
- Tree and DAG visualization in the terminal
- Critical path analysis for tasks with durations
- Gantt-style timeline output

## Documentation

- [Concept Document](docs/concept.md) — detailed design decisions and data model
- [Research Report](docs/research.md) — analysis of existing tools and data structures

## License

MIT
