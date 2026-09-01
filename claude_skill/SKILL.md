---
name: mindtask
description: >
  Use when the user wants to manage project concepts, tasks, or dependencies
  using mindtask. Triggers for requests like "add a task", "create a concept",
  "show the concept tree", "export a diagram", or any project planning activity
  involving mindtask.
allowed-tools: Bash(mindtask *)
metadata:
  # Crate version this reference was last verified against. The
  # `skill_doc_sync` integration test fails on release if this drifts
  # from Cargo.toml — bump it here when cutting a new mindtask version.
  documents-version: "0.10.0"
---

# mindtask — CLI for concept maps + task dependency graphs

mindtask stores everything in a single `.mindtask.json` file in the current directory (the lookup does not walk up into parent directories). Any command accepts a global `-f`/`--file <PATH>` flag to target a specific project file instead.

## Command reference

### Project

```
mindtask init [--timezone <IANA>]     # Create .mindtask.json
mindtask validate                     # Check file integrity
mindtask config timezone [TZ]         # Get/set timezone (--show to display)
mindtask config wrap-width [COLS]     # Get/set description wrap width (--show to display, --clear to auto-detect)
mindtask report [-d[=short|full]] [--state <STATE,...>]  # Whole project: concept tree + task list; -d adds descriptions to both halves
```

### Concepts (tree structure)

```
mindtask concept add <NAME> [--parent <ID>] [--description <DESC>]
mindtask concept rm <ID>
mindtask concept mv <ID> --parent <ID|root>          # Re-parent (append to new parent's children)
mindtask concept mv <ID> --before <SIB> | --after <SIB> # Position among siblings (parent taken from SIB)
mindtask concept edit <ID> [--name <NAME>] [--description <DESC>] [--clear-description]
mindtask concept ls
mindtask concept tree [ID] [-d[=short|full]]  # Full tree (or subtree); -d adds descriptions
mindtask concept show <ID>
mindtask concept report <ID> [-d[=short|full]] [--state <STATE,...>]  # Subtree + linked tasks + upstream deps; -d adds descriptions
mindtask concept normalize [--dry-run] # Renumber IDs to 1..n in tree (DFS pre-order) order
```

Concepts print in file (array) order, not by ID. `normalize` rewrites the file
so IDs run `1..n` in the exact order `tree` prints (each parent immediately
followed by its subtree), remapping `parent` links and task→concept links in
step; task dependencies are untouched. It's an on-demand tidy — run it after
hand-reordering the file or moving concepts around. `--dry-run` shows the
planned old→new mapping without writing.

### Tasks (DAG structure)

```
mindtask task add <NAME> [--description <DESC>] [--duration <DAYS>] [--due <DATETIME>]
                         [--concept <ID> ...]   # --concept is repeatable; links the task at creation
mindtask task rm <ID>
mindtask task edit <ID> [--name <NAME>] [--description <DESC>] [--clear-description]
                        [--duration <DAYS>] [--clear-duration]
mindtask task ls [--state <STATE,...>] [-d[=short|full]]
mindtask task show <ID>
mindtask task state <ID> <todo|in_progress|done>
mindtask task due <ID> [DATE] [--clear]
```

The task-listing commands (`task ls`, `report`, `concept report`) default to
`--state todo,in_progress` — **done tasks are hidden**. `--state` takes any
comma-separated or repeated combination of `todo`, `in_progress`, `done`, or
`all` for everything. A footer accounts for hidden rows, e.g.
`(hidden: 12 done — --state all to show)`; dependency IDs in `DEPENDS ON`
stay verbatim even when the referenced task's row is hidden. In
`concept report`, hidden direct tasks don't pull their upstream chains in.

### Descriptions (`-d`)

`-d`/`--description` adds descriptions to `concept tree`, `concept report`,
`task ls`, and `report`. On `report` and `concept report` it covers **both**
halves — the concept tree *and* the task table.

```
mindtask task ls -d                   # full text, wrapped
mindtask task ls -d=short             # one-line lede per task, elided with …
mindtask report -d=short --state all  # scan a whole project
```

| Form | Shows |
|------|-------|
| `-d`, `--description` | Full text, wrapped over as many lines as needed |
| `-d=short`, `--description=short` | One line per entity, truncated with `…` |
| `-d=full` | Explicit form of bare `-d` |

**The `=` is required.** `-d=short` works; `-d short` fails with
`unexpected argument 'short' found`.

Descriptions render as a bracketed block indented beneath their row or node —
never as a table column — so rows stay one line and columns keep aligning.
Prefer `=short` on a large project: full text can multiply the output length
(on a 133-task project, `report --state all` runs 178 lines plain, 333 with
`-d=short`, 1295 with `-d`).

Wrapping is word-aware and follows `config wrap-width`, else the terminal, else
80 columns. To read **one** task's description in full, use `task show <ID>`.

### Dependencies (between tasks)

```
mindtask depend add <TASK_ID> <DEPENDS_ON>   # TASK_ID depends on DEPENDS_ON
mindtask depend rm <TASK_ID> <DEPENDS_ON>
```

The first argument is the task that **has** the dependency; the second is the task that must finish first.

### Schedule (critical path)

```
mindtask schedule [--critical]        # Earliest start/finish, slack, critical path, project duration
```

Computes a Critical Path Method (CPM) schedule from task durations and
dependencies: each task's earliest start/finish and slack, the critical path,
and the overall project duration. `--critical` restricts output to the tasks on
the critical path. The PlantUML Gantt export (`export plantuml gantt`) is
schedule-driven off the same computation when tasks have dependencies.

### Linking (task <-> concept)

```
mindtask link <TASK_ID> <CONCEPT_ID>
mindtask unlink <TASK_ID> <CONCEPT_ID>
```

### Search

```
mindtask search <QUERY> [-d]          # -d also searches descriptions, and shows the matching excerpt
```

Case-insensitive substring match across concepts and tasks. Plain `search`
matches names only. With `-d` it also matches description text and prints an
excerpt of the surrounding text beneath the row, `…` marking each trimmed end —
so a hit always explains itself (including an unexpected substring match, e.g.
`search ARP -d` matching "K**arp**athy"). Here `-d` is a plain boolean; it takes
no `=short`/`=full` value.

### Import (concept-graph JSONL → concept subtree)

```
mindtask import <FILE|-> --under <ID> [--min-docs <N>] [--reparent] [--dry-run]
```

Reads a typed concept-graph JSONL stream (one JSON object per line — the
contract `pdfdex graph --format jsonl` emits), takes its `is-a` edges, and
projects that sub-DAG onto a strict tree merged under concept `<ID>`:
multi-parent nodes keep their highest-weight parent, cycles are broken at
their weakest edge, and parentless concepts group under a category umbrella.
Merging is by name within the target subtree and **add-only by default**:
existing concepts are never moved or deleted — a differing projected parent
is reported as drift unless `--reparent` is passed. `--dry-run` prints the
full projection and merge report without saving.

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

### PDF report (Typst)

To pretty-print a whole project as one PDF, combine the text report with the
four diagrams via Typst (SVG embeds natively — no LaTeX, no rsvg toolchain):

```sh
mindtask report > report.txt
for d in tree wbs dag gantt; do mindtask export plantuml $d > $d.puml; done
# render each .puml to SVG with any PlantUML renderer (plantuml -tsvg …)
typst compile main.typ project-report.pdf
```

`main.typ` — portrait text page, landscape diagram pages, fit-to-page:

```typst
#set page(paper: "a4", margin: 15mm)
#set text(font: "DejaVu Sans", size: 9pt)
= Project report
#text(size: 7.5pt)[#raw(read("report.txt"), block: true)]

#let diagram(title, path) = {
  set page(paper: "a4", flipped: true, margin: 10mm)
  pagebreak(weak: true)
  heading(level: 2, title)
  align(center + horizon, image(path, fit: "contain", width: 100%, height: 88%))
}
#diagram("Concept tree", "tree.svg")
#diagram("Work breakdown structure", "wbs.svg")
#diagram("Task dependency DAG", "dag.svg")
#diagram("Gantt", "gantt.svg")
```

Requires mindtask ≥ 0.8.0 for the gantt export when tasks have dependencies
(0.7.0 emitted forward references PlantUML rejects).

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
- Build the concept tree first, then add tasks, linking them at creation with `task add --concept <ID>` (repeatable) instead of a separate `link` step.
- `add` commands echo the new ID (`Added task 1 "..."`); capture it from that output instead of re-running `search`/`ls`.
- Use `mindtask concept tree` to review structure before exporting; add `-d` to see each concept's description inline.
- To survey what a project is actually about, `mindtask report -d=short --state all` — one lede per concept and task. See [Descriptions](#descriptions--d); note the `=` is required.
- To read one task's description in full, `mindtask task show <ID>`; to find which tasks mention something, `mindtask search <TERM> -d`, which prints the matching excerpt.
- `mindtask export plantuml wbs` gives the richest view: concepts + tasks together.
- Chain commands: add a task (with `--concept` to link it), set its dependency, then export.
- Use `mindtask search` to find IDs of existing items before editing or linking.
- Use `mindtask concept report <ID>` to see all work needed for a concept area, including transitive dependencies from outside the subtree.
- Listings hide done tasks by default; add `--state all` when you need the full history (a footer line tells you when rows were hidden).
