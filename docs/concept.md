# MindTask — Hybrid Mindmap + Task Manager

## Core Idea

A CLI tool that combines two structures:
- A **concept map** (mindmap) for organizing ideas
- A **task graph** for planning and scheduling work

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Concept structure | Strict tree | Each concept has 0 or 1 parent. Classic mindmap. |
| Task structure | DAG | Tasks can depend on other tasks. Acyclic. |
| Data model | Two separate collections | Concepts and tasks are distinct entities linked by ID references |
| Link direction | Tasks → Concepts | Tasks hold concept IDs. Concepts don't know about tasks. |
| Link cardinality | Many-to-many | A task can reference multiple concepts. Multiple tasks can reference the same concept. |
| Storage | JSON file | Local, human-readable, versionable |
| Implementation | Rust CLI | Performance, type safety, single binary |

## Data Model (Draft)

**Concept** (node in a tree):
- `id`: unique identifier
- `name`: display name
- `description`: optional text
- `parent`: optional concept ID (null = root)

**Task** (node in a DAG):
- `id`: unique identifier
- `name`: display name
- `description`: optional text
- `duration`: optional (enables Gantt scheduling)
- `depends_on`: list of task IDs (must be acyclic)
- `concepts`: list of concept IDs (cross-references to concept map)

**Top-level JSON structure**:
```json
{
  "concepts": [ ... ],
  "tasks": [ ... ]
}
```

## Open Questions

- ID format: UUIDs vs integers vs slugs?
- Should we support dependency types beyond simple finish-to-start?
- Should concepts carry metadata (color, tags, priority)?
- Do we need task status (todo/in-progress/done)?
