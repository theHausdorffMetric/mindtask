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

Data is stored in `.mindtask.json` in the current directory — human-readable, versionable with git. The CLI looks for the project file in the current directory only (it does not walk up into parent directories). Pass `-f`/`--file <PATH>` (a global flag on any command) to operate on a specific project file instead.

## CLI Reference

### Project

```sh
mindtask init [--timezone <IANA_TZ>]    # Create a new .mindtask.json
mindtask validate                       # Check tree + DAG integrity
mindtask config timezone [<IANA_TZ>]    # Get or set the project timezone
mindtask config timezone --show         # Show the current timezone
mindtask config wrap-width [<COLS>]     # Get or set the description wrap width
mindtask config wrap-width --clear      # Revert to terminal-width auto-detection
mindtask report [-d|--description[=short|full]] [--state <STATE,...>]  # Whole project: concept tree + task list
```

Any command accepts the global `-f`/`--file <PATH>` flag to target a specific project file instead of searching from the current directory.

### Concepts

Concepts form a tree (each concept has at most one parent).

```sh
mindtask concept add <NAME> [--parent <ID>] [--description <TEXT>]
mindtask concept edit <ID> [--name <TEXT>] [--description <TEXT>] [--clear-description]
mindtask concept rm <ID>
mindtask concept mv <ID> --parent <ID|root>          # Re-parent (append to new parent's children)
mindtask concept mv <ID> --before <SIB>|--after <SIB> # Position among siblings (parent taken from SIB)
mindtask concept ls
mindtask concept tree [<ID>] [-d|--description[=short|full]]
mindtask concept show <ID>
mindtask concept report <ID> [-d|--description[=short|full]] [--state <STATE,...>]
mindtask concept normalize [--dry-run]               # Renumber IDs to 1..n in tree (DFS pre-order) order
```

Removing a concept fails if it has children or is referenced by tasks — unlink or remove dependents first.

Concepts print in file (array) order, not by ID. Sibling order therefore follows the file: `concept mv --before/--after` repositions a concept among siblings (taking the new parent from the anchor), and `concept normalize` rewrites IDs to `1..n` in the exact order `tree` prints — remapping `parent` and task→concept links in step — as an on-demand tidy after reordering. `--dry-run` previews the old→new mapping without writing.

`concept report` shows the concept subtree, all tasks linked to concepts in that subtree, and all transitive upstream dependencies (tasks required by those tasks, even if linked to concepts outside the subtree). Upstream-only tasks are marked `(upstream dep)`. The task table honors the `--state` filter (see [Tasks](#tasks)): hidden direct tasks don't pull their dependency chains into the report.

### Tasks

Tasks form a dependency DAG. Each task has a workflow state (`todo`, `in_progress`, `done`).

```sh
mindtask task add <NAME> [--description <TEXT>] [--duration <DAYS>] [--due <DATE>] [--concept <ID>]...
mindtask task edit <ID> [--name <TEXT>] [--description <TEXT>] [--clear-description] [--duration <DAYS>] [--clear-duration]
mindtask task rm <ID>
mindtask task ls [--state <STATE,...>] [-d|--description[=short|full]]
mindtask task show <ID>
mindtask task state <ID> <todo|in_progress|done>
mindtask task due <ID> <DATE>
mindtask task due <ID> --clear
```

The task-listing commands — `task ls`, the top-level `report`, and `concept report` — default to showing open work only (`--state todo,in_progress`). Pass `--state` with any comma-separated or repeated combination of `todo`, `in_progress`, and `done`, or `--state all` for everything. Whenever the filter hides rows, a footer accounts for them, e.g. `(hidden: 12 done — --state all to show)`. `DEPENDS ON` cells always list dependency IDs verbatim, even when the referenced task's own row is hidden.

Due dates accept multiple formats:
- Date only: `2025-03-15` (midnight in project timezone)
- Datetime: `2025-03-15T14:00` (uses project timezone)
- Full RFC 9557: `2025-03-15T14:00:00-04:00[America/New_York]`

### Descriptions

Pass `-d`/`--description` to `concept tree`, `concept report`, `task ls`, or the top-level `report` to print each description on the line(s) below its node or row, bracketed and indented. On `report` and `concept report` the flag covers both halves of the output — the concept tree *and* the task table.

Descriptions are prose, so they render as a block beneath a row rather than as a table column, and every row stays one line so the columns still align. Two densities are available:

| Form | Shows |
| --- | --- |
| `-d`, `--description` | The full text, wrapped over as many lines as it needs |
| `--description=short` | A one-line lede per entity, elided with `…` |

Reach for `=short` on large projects: full descriptions can easily multiply the length of a `report`.

Wrapping is word-aware — a token too long to fit (a URL, a long path) is split rather than allowed to overflow — and fits the available width, which follows the terminal (falling back to 80 columns when the width is unknown, e.g. piped output). Set a fixed width with `config wrap-width <COLS>` to override detection, or `config wrap-width --clear` to go back to auto-detection.

**The `=` is required** when selecting a mode: `-d=short` and
`--description=short` work, but `-d short` fails with
`unexpected argument 'short' found`.

To read a single task's description in full, use `task show <ID>` — it wraps to
the same resolved width. To find which items mention a term, see
[Search](#search).

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
mindtask search <QUERY> -d|--description  # Also search descriptions, and show the matching excerpt
```

With `-d`, a row whose *description* matched is followed by an excerpt of the
surrounding text, with `…` marking each trimmed end — so a hit is never a black
box, and an unexpected substring match explains itself:

```
ID   NAME                                       STATE  DUE  DEPENDS ON  CONCEPTS
111  Fix Container Station lxdbr0 / Virtual S…  done   -    -           40
     …4 deletion), 10.0.7.1 no longer answers ARP from the LAN — verified from …
```

### Scheduling

`mindtask schedule` computes a Critical Path Method (CPM) schedule from task
durations (in days) and finish-to-start dependencies: each task's earliest
start/finish, its slack, the critical path, and the overall project duration.

```sh
mindtask schedule              # full schedule table + critical path
mindtask schedule --critical   # only the critical-path tasks
```

Times are day-offsets from the project start. Tasks without a `--duration` are
treated as zero-day milestones; if no task has a duration there is nothing to
schedule and the command says so.

### Import

Merge a typed concept-graph JSONL stream (the contract `pdfdex graph --format
jsonl` emits — one JSON object per line) into the concept tree as a strict
subtree.

```sh
mindtask import <FILE|-> --under <ID> [--min-docs <N>] [--reparent] [--dry-run]
```

The importer takes the graph's `is-a` edges and projects that sub-DAG onto a
tree merged under concept `<ID>`: multi-parent nodes keep their highest-weight
parent, cycles are broken at their weakest edge, and parentless concepts are
grouped under a category umbrella. `--min-docs <N>` drops nodes seen in fewer
than `N` documents (default 1).

Merging is by name within the target subtree and **add-only by default** —
existing concepts are never moved or deleted. When a projected parent differs
from the current one, the drift is reported but not applied unless you pass
`--reparent`. `--dry-run` prints the full projection and merge report without
writing anything.

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
| `gantt` | Gantt chart — CPM schedule when tasks have dependencies, else due dates | — (rejected if given) |
| `wbs`   | Work breakdown structure (concepts + tasks) | Concept ID (renders subtree) |

Examples:

```sh
mindtask export plantuml tree           # Full concept tree
mindtask export plantuml tree 1         # Subtree rooted at concept 1
mindtask export plantuml dag            # Full task DAG
mindtask export plantuml dag 1          # Task 1 and its downstream dependents
mindtask export plantuml gantt          # Gantt chart (computed schedule, or due dates)
mindtask export plantuml wbs            # Work breakdown structure
mindtask export plantuml wbs 1          # WBS subtree rooted at concept 1
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
