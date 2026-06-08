---
name: mindtask
description: >
  Use when the user wants to manage project concepts, tasks, or dependencies
  using mindtask. Triggers for requests like "add a task", "create a concept",
  "show the concept tree", "export a diagram", or any project planning activity
  involving mindtask.
allowed-tools: Bash(mindtask *)
---

# mindtask — CLI for concept maps + task dependency graphs

mindtask stores everything in a single `.mindtask.json` file, searched for in the current directory and its parents. Any command accepts a global `-f`/`--file <PATH>` flag to target a specific project file instead.

## Command reference

### Project

```
mindtask init [--timezone <IANA>]     # Create .mindtask.json
mindtask validate                     # Check file integrity
mindtask config timezone [TZ]         # Get/set timezone (--show to display)
mindtask report [-d|--descriptions]   # Whole project: concept tree + task list
```

### Concepts (tree structure)

```
mindtask concept add <NAME> [--parent <ID>] [--description <DESC>]
mindtask concept rm <ID>
mindtask concept mv <ID> --parent <ID|root>
mindtask concept edit <ID> [--name <NAME>] [--description <DESC>] [--clear-description]
mindtask concept ls
mindtask concept tree [ID] [-d]       # Full tree (or subtree); -d adds descriptions
mindtask concept show <ID>
mindtask concept report <ID> [-d]     # Subtree + linked tasks + upstream deps; -d adds descriptions
```

### Tasks (DAG structure)

```
mindtask task add <NAME> [--description <DESC>] [--duration <DAYS>] [--due <DATETIME>]
mindtask task rm <ID>
mindtask task edit <ID> [--name <NAME>] [--description <DESC>] [--clear-description]
                        [--duration <DAYS>] [--clear-duration]
mindtask task ls
mindtask task show <ID>
mindtask task state <ID> <todo|in_progress|done>
mindtask task due <ID> [DATE] [--clear]
```

### Dependencies (between tasks)

```
mindtask depend add <TASK_ID> <DEPENDS_ON>   # TASK_ID depends on DEPENDS_ON
mindtask depend rm <TASK_ID> <DEPENDS_ON>
```

The first argument is the task that **has** the dependency; the second is the task that must finish first.

### Linking (task <-> concept)

```
mindtask link <TASK_ID> <CONCEPT_ID>
mindtask unlink <TASK_ID> <CONCEPT_ID>
```

### Search

```
mindtask search <QUERY> [-d]          # -d also searches descriptions
```

Case-insensitive substring match across concepts and tasks.

### Export

```
mindtask export <FORMAT> <DIAGRAM> [ROOT]
```

Formats: `plantuml`, `mermaid` (mermaid not yet implemented)

Diagram types and what they show:

| Diagram | Content               | ROOT argument          |
|---------|-----------------------|------------------------|
| `tree`  | Concept hierarchy     | Concept ID (subtree)   |
| `wbs`   | Concepts + linked tasks as WBS | Concept ID (subtree) |
| `dag`   | Task dependency graph | Task ID (subgraph)     |
| `gantt` | Task schedule         | —                      |

Currently supported: **PlantUML** for all four diagram types.

## Date format

Due dates accept these formats:

- `2026-03-15T14:00` — naive datetime (uses project timezone)
- `2026-03-15T14:00[America/New_York]` — datetime with IANA timezone

## Data model

- **Concepts** form a tree (forest). Each concept has an ID, name, optional description, and optional parent.
- **Tasks** form a DAG via dependencies. Each task has an ID, name, optional description, state (`todo`/`in_progress`/`done`), optional duration (days), and optional due date.
- **Links** bridge concepts and tasks — a task can be linked to one or more concepts.

## Workflow tips

- Always `mindtask init` before other commands — it creates `.mindtask.json`.
- Build the concept tree first, then add tasks and link them.
- Use `mindtask concept tree` to review structure before exporting; add `-d` to see each concept's description inline.
- `mindtask export plantuml wbs` gives the richest view: concepts + tasks together.
- Chain commands: add a task, set its dependency, link it, then export.
- Use `mindtask search` to find IDs before editing or linking.
- Use `mindtask concept report <ID>` to see all work needed for a concept area, including transitive dependencies from outside the subtree.
