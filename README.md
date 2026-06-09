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

## Installation

```sh
cargo install mindtask
```

## Quick Start

```sh
# Initialize a new project (optionally with a default timezone)
mindtask init --timezone America/New_York

# Build a concept tree
mindtask concept add "Backend"
mindtask concept add "API" --parent 1
mindtask concept add "Database" --parent 1

# Create tasks and link them to concepts
mindtask task add "Design API" --duration 2 --due 2025-03-15
mindtask link 1 2            # link task 1 → concept 2 (API)
mindtask task add "Implement API" --due 2025-03-20
mindtask depend add 2 1      # task 2 depends on task 1

# Track progress
mindtask task state 1 done
mindtask task ls
```

Data is stored in `.mindtask.json` in the current directory — human-readable, versionable with git. The CLI walks up parent directories to find the project file, so you can run commands from subdirectories. Pass `-f`/`--file <PATH>` (a global flag on any command) to operate on a specific project file instead, bypassing the directory search.

## CLI Reference

### Project

```sh
mindtask init [--timezone <IANA_TZ>]    # Create a new .mindtask.json
mindtask validate                       # Check tree + DAG integrity
mindtask config timezone [<IANA_TZ>]    # Get or set the project timezone
mindtask config timezone --show         # Show the current timezone
mindtask config wrap-width [<COLS>]     # Get or set the description wrap width
mindtask config wrap-width --clear      # Revert to terminal-width auto-detection
mindtask report [-d|--description]     # Whole project: concept tree + task list
```

Any command accepts the global `-f`/`--file <PATH>` flag to target a specific project file instead of searching from the current directory.

### Concepts

Concepts form a tree (each concept has at most one parent).

```sh
mindtask concept add <NAME> [--parent <ID>] [--description <TEXT>]
mindtask concept edit <ID> [--name <TEXT>] [--description <TEXT>] [--clear-description]
mindtask concept rm <ID>
mindtask concept mv <ID> --parent <ID|root>
mindtask concept ls
mindtask concept tree [<ID>] [-d|--description]
mindtask concept show <ID>
mindtask concept report <ID> [-d|--description]
```

Removing a concept fails if it has children or is referenced by tasks — unlink or remove dependents first.

Pass `-d`/`--description` to `concept tree`, `concept report`, or the top-level `report` to print each concept's description on the line below the node, indented to line up with the tree branches. Long descriptions are hard-wrapped to fit the available width, which follows the terminal (falling back to 80 columns when the width is unknown, e.g. piped output). Set a fixed width with `config wrap-width <COLS>` to override detection, or `config wrap-width --clear` to go back to auto-detection.

`concept report` shows the concept subtree, all tasks linked to concepts in that subtree, and all transitive upstream dependencies (tasks required by those tasks, even if linked to concepts outside the subtree). Upstream-only tasks are marked `(upstream dep)`.

### Tasks

Tasks form a dependency DAG. Each task has a workflow state (`todo`, `in_progress`, `done`).

```sh
mindtask task add <NAME> [--description <TEXT>] [--duration <DAYS>] [--due <DATE>] [--concept <ID>]...
mindtask task edit <ID> [--name <TEXT>] [--description <TEXT>] [--clear-description] [--duration <DAYS>] [--clear-duration]
mindtask task rm <ID>
mindtask task ls
mindtask task show <ID>
mindtask task state <ID> <todo|in_progress|done>
mindtask task due <ID> <DATE>
mindtask task due <ID> --clear
```

Due dates accept multiple formats:
- Date only: `2025-03-15` (midnight in project timezone)
- Datetime: `2025-03-15T14:00` (uses project timezone)
- Full RFC 9557: `2025-03-15T14:00:00-04:00[America/New_York]`

### Dependencies

```sh
mindtask depend add <TASK_ID> <DEPENDS_ON_ID>
mindtask depend rm <TASK_ID> <DEPENDS_ON_ID>
```

Adding a dependency that would create a cycle is rejected. Removing a task automatically cleans up references from other tasks' dependency lists.

### Linking Tasks to Concepts

```sh
mindtask link <TASK_ID> <CONCEPT_ID>
mindtask unlink <TASK_ID> <CONCEPT_ID>
```

Links are many-to-many: a task can reference multiple concepts, and a concept can be referenced by multiple tasks. You can also link at creation time with `task add --concept <ID>` (repeatable), avoiding a separate `link` step.

### Search

```sh
mindtask search <QUERY>                  # Search by name (case-insensitive)
mindtask search <QUERY> -d|--description  # Also search description fields
```

### Export

Generate diagrams for external renderers. Supported formats: `plantuml`, `mermaid` (stubbed).

```sh
mindtask export <FORMAT> <DIAGRAM> [ROOT_ID]
```

Diagram types:

| Diagram | Description | Root ID type |
|---------|-------------|--------------|
| `tree`  | Concept tree as a mindmap | Concept ID (renders subtree) |
| `dag`   | Task dependency graph | Task ID (renders downstream) |
| `gantt` | Gantt chart from tasks with due dates | — |
| `wbs`   | Work breakdown structure (concepts + tasks) | — |

Examples:

```sh
mindtask export plantuml tree           # Full concept tree
mindtask export plantuml tree 1         # Subtree rooted at concept 1
mindtask export plantuml dag            # Full task DAG
mindtask export plantuml dag 1          # Task 1 and its downstream dependents
mindtask export plantuml gantt          # Gantt chart (tasks with due dates)
mindtask export plantuml wbs            # Work breakdown structure
```

Output is written to stdout. Pipe to a file and render with PlantUML:

```sh
mindtask export plantuml dag > dag.puml
java -jar plantuml.jar -tsvg dag.puml
```

## Data Model

```json
{
  "version": 1,
  "timezone": "America/New_York",
  "wrap_width": 80,
  "concepts": [
    { "id": 1, "name": "Backend" },
    { "id": 2, "name": "API", "parent": 1 },
    { "id": 3, "name": "Database", "parent": 1 }
  ],
  "tasks": [
    { "id": 1, "name": "Design API", "state": "done", "concepts": [2] },
    {
      "id": 2,
      "name": "Implement API",
      "state": "todo",
      "duration": 5.0,
      "due": "2025-03-15T00:00:00-04:00[America/New_York]",
      "depends_on": [1],
      "concepts": [2]
    }
  ]
}
```

Optional fields (`description`, `duration`, `due`, `depends_on`, `concepts`, `parent`) are omitted from the JSON when empty or unset.

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Concept structure | Strict tree | Proven mindmap model. Simple, intuitive. |
| Task structure | DAG | Dependencies must be acyclic for scheduling to work. |
| Linking | Tasks → Concepts | Keeps the concept tree clean and independent. |
| Storage | Single JSON file | Human-readable, versionable with git, no database needed. |
| IDs | Auto-increment integers | Simple to type, context distinguishes concepts from tasks. |
| Timezones | IANA via `jiff` | RFC 9557 format preserves timezone identity across serialization. |

## Library

mindtask is also usable as a Rust library (`use mindtask::...`):

- `mindtask::model` — `Project`, `Concept`, `Task`, typed IDs (`ConceptId`, `TaskId`)
- `mindtask::graph` — Tree validation, DAG cycle detection, topological sort, project validation
- `mindtask::store` — JSON persistence (load/save)
- `mindtask::export` — Diagram generation (PlantUML, Mermaid stub)

## License

GPL-3.0-only
