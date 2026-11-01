# Output formats — Markdown, JSON, Graphviz; PDF and HTML outside the binary

Status: planned 2026-11-01 for 0.14.0; steps land in order, each one green
(fmt, clippy `-D warnings`, tests) and committed on its own. Tracked in the
ideas repo as tasks 234–238 under review phase 5 (task 141).

Goal: let a project be read somewhere other than a terminal — a Markdown
page beside the project file, a PDF, a web page — without turning mindtask
into a document tool. The rule that decides what goes where:

- **The binary owns the data and one text-like format per medium.** It
  already renders terminal tables; it gains Markdown (the format the
  surrounding knowledge bundles are written in) and JSON (the data feed
  for everything else). Nothing else: no PDF engine, no HTML templating,
  no new dependencies.
- **PDF and HTML are produced by tools that already exist on the machine**
  from those two outputs: Typst reads the JSON directly (`json()`), pandoc
  turns the Markdown into a standalone page. Both are `just` recipes, the
  same pattern as the ruux / aranet-oxide / rndoxide reports in the ideas
  repo.
- **Diagrams need a renderer that is actually installed.** PlantUML needs
  Java, which the workstation does not have; Graphviz `dot` is there. So
  `export` gains a `dot` format for the two diagrams that are graphs (tree,
  dag) and the recipes embed the resulting SVGs.

What exists before this plan: every listing (`report`, `task ls`,
`concept tree`, `concept report`, `schedule`, `search`) prints terminal
text; `export plantuml {tree,dag,gantt,wbs}` prints PlantUML source;
`export mermaid` is a stub that errors. No `--format`, no JSON, no Markdown.

## Step 1 — view models and `--format text|json` (task 234)

A new library module `mindtask::report` holds the **view models**: the data
each listing shows, independent of rendering.

- `ConceptNode { id, name, description?, refs, children }` — the tree.
- `TaskRow { id, name, state, description?, duration?, due?, depends_on,
  concepts, refs }` — one task as every listing shows it; `due` converted
  to the project timezone.
- `TaskGroup { concept?, title, tasks }` — one group of the grouped table
  (`title` is the breadcrumb; `concept` is `null` for the unlinked group).
- `Hidden { todo, in_progress, done }` — rows the `--state` filter removed.
- `Report { timezone?, states, concepts, groups, hidden }`,
  `TaskListing { states, tasks, hidden }`, `ScheduleView { duration,
  critical_path, tasks }` — the three documents.

The grouping logic (`group_by_primary_concept`, `breadcrumb`) moves from
`cli/report.rs` into this module; the text renderers consume the models.
Text output stays byte-identical — the existing binary tests guard it.

`--format <text|md|json>` (long flag only; `-f` is the project file) on
`report`, `task ls`, `concept tree`, `schedule`. `json` is pretty-printed
serde of the model plus a trailing newline. JSON always carries the full
description regardless of `-d`; `-d` is a presentation choice, JSON is data.
JSON output is structured even when the text form prints a sentence (`No
tasks.`): an empty `tasks` array, not prose.

## Step 2 — Markdown renderer (task 235)

`cli/markdown.rs`: GitHub-flavoured Markdown over the same models.

- Each command's output is a standalone document with an H1 (`# Project
  report`, `# Tasks`, `# Concepts`, `# Schedule`).
- Concept tree → nested bullet list, `Name {id}`; with `-d` the description
  follows after ` — ` (short: the lede truncated to the wrap width; full:
  the whole text, newlines as `<br>`); refs follow as links.
- Task groups → one `###` heading per group (the breadcrumb), one GFM table
  each: `ID | Name | State | Due | Depends on | Concepts`, plus a
  `Description` column when `-d` is given and a `Refs` column when any
  shown task has refs (relative refs stay relative to the project file —
  write the page beside it).
- Cells escape `|`, collapse newlines to `<br>`; nothing else is escaped
  (names are validated to carry no control characters).
- The `--state` footer and the "no tasks" sentences appear in italics.

## Step 3 — Graphviz export (task 236)

`export dot tree|dag [ROOT]` beside `plantuml`/`mermaid`:

- `tree`: `digraph`, `rankdir=LR`, rounded boxes, `c<id>` nodes, parent →
  child edges; same root semantics as the PlantUML mindmap (a single root
  stands alone, several roots hang under a synthetic *Project* node).
- `dag`: `t<id>` nodes labelled `name\n{id}`; done = grey filled, in
  progress = gold, todo = plain; edges dependency → dependent; when the
  project has durations the critical path (tasks and the edges between
  them) is drawn bold.
- `gantt`/`wbs` return the same kind of error the Mermaid stub does.
- Labels: whitespace collapsed, `"` and `\` escaped, `(unnamed)` for an
  empty result — the sanitising helpers move to `export/mod.rs` and both
  exporters share them.
- `just render-dot` renders the fixture's two diagrams to SVG with `dot`.

## Step 4 — PDF and HTML recipes (task 237)

`templates/report.typ` reads `report.json` (and `schedule.json`) from its
build directory and lays out: title, the concept tree, the grouped task
tables, the schedule when the project has durations, then the SVG
diagrams it is told about via `--input diagrams=tree,dag`. Recipes in the
crate `justfile`, all writing into `build/`:

- `report-json FILE` / `report-md FILE` — the two binary outputs.
- `report-svg FILE` — `export dot` → `dot -Tsvg` for tree and dag.
- `report-pdf FILE` — json + svg + `typst compile`.
- `report-html FILE` — md → `pandoc --standalone --embed-resources` with a
  small stylesheet.

The ideas repo's top-level `justfile` gets `tracker-pdf` / `tracker-html`
that call these on the live `.mindtask.json`.

## Step 5 — docs and 0.14.0 (task 238)

README (an *Output formats* section, `dot` under Export), `claude_skill/
SKILL.md` (the `skill_doc_sync` test must keep passing; its Typst section
is replaced by the recipe), CHANGELOG, STATUS. Version bump to 0.14.0,
commit and push; the tag and `cargo publish` are Dan's step.

## Deferred

- `concept report --format` — the subtree report; same models apply, add
  when wanted.
- Mermaid stays a stub: it only renders by itself on GitHub, which is not
  where these projects live.
- A native HTML renderer (`pulldown-cmark` over the Markdown output) if a
  pandoc-free page is ever needed.
