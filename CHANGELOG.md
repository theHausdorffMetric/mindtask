# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-09-01

Data-safety and output-correctness release, closing the first two phases of the
0.9.0 architecture review. Two small library API changes make this a minor
rather than a patch bump; the CLI surface is unchanged.

### Fixed
- **Saves are now atomic** (code review C2). `save` used `std::fs::write`, which
  truncates the target and then rewrites it in place: an interrupt, crash, or
  full disk mid-write could leave `.mindtask.json` — the single source of truth
  for a whole project — truncated or half-written. The new contents now go to a
  temp file in the same directory, are flushed with `sync_all`, and only then
  replace the target via rename; the parent directory is fsynced so a save that
  reported success survives power loss. A failed save leaves the previous file
  untouched.
- Write failures no longer report themselves as read failures: `StoreError`
  gained a `WriteError` variant, so a full disk during save surfaces as
  "failed to write project file" rather than "failed to read project file".
- **The Gantt export no longer identifies tasks by name** (C9). Task names are
  not unique, so two tasks sharing one collapsed into a single PlantUML
  identifier — and a dependency between them emitted `[X] starts at [X]'s end`,
  a self-dependency the project does not contain. PlantUML *accepts* that, so
  the rendered chart was wrong rather than rejected. Tasks are now declared once
  as `[name] as [t<id>]` and addressed only by that alias, matching what the
  `dag` export already did.
- **Names are sanitised before being written into diagram syntax** (C8). A `"`
  closed a component label early, `[`/`]` unbalanced a Gantt label, and an
  embedded newline split one declaration across two lines. PlantUML has no
  escape mechanism for these, so offending characters are substituted with
  visually equivalent safe ones (`"`→`'`, `[`/`]`→`(`/`)`, whitespace runs
  collapsed); a name that sanitises to nothing renders as `(unnamed)`. Verified
  a no-op on ordinary names: `tree`, `dag`, and `wbs` output is byte-identical
  to 0.9.0 for a real 46-concept/135-task project.
- **`export mermaid` now fails instead of reporting success** (C5, open since
  the v0.2.1 review). Every mermaid path returned `Ok("… not yet implemented")`
  with exit status 0, which a script could not distinguish from a real diagram.
  It now returns an error and exits non-zero, writing nothing to stdout.

### Changed
- **Breaking (library API):** `mindtask::export::mermaid`'s four entry points
  return `Result<String, String>` rather than `String`.
- **Breaking (library API):** `mindtask::store::json::StoreError` gained a
  `WriteError` variant, so exhaustive matches on it need a new arm.
- **Behaviour:** saving into a read-only directory now fails instead of
  succeeding. Temp-file-plus-rename requires write permission on the
  *directory*, which an in-place write did not; this is inherent to atomic
  replacement. The failure is loud rather than falling back to a truncating
  write, which would reintroduce exactly the corruption risk C2 describes.
- Replacing an existing project file keeps its permission bits, so an atomic
  replace cannot silently tighten a 0644 file to the temp file's 0600. A newly
  created file is now 0600 rather than umask-dependent (typically 0644) — a
  safer default for a data file, and a no-op under a 0077 umask.
- `tempfile` moved from a dev-dependency to a runtime dependency (it backs the
  atomic save).

### Documentation
- Full architecture and code review at [`CODE_REVIEW-0.9.0.md`], re-verifying
  every finding of the v0.2.1 review against current code (five it listed as
  open were already fixed) and adding an architecture section the original
  lacked. The v0.2.1 file gains a forward pointer.

[`CODE_REVIEW-0.9.0.md`]: CODE_REVIEW-0.9.0.md

## [0.9.0] - 2026-09-01

### Added
- `-d`/`--description` on `task ls`, and on the *task* half of `report` and
  `concept report` — previously the flag reached only the concept tree, so a
  task's description was visible nowhere but `task show`. Descriptions render
  as a bracketed, indented block beneath their row (rows stay one line, so
  table columns still align), matching how the concept tree already shows them.
- `--description=short` on every command that takes `-d`: a one-line lede per
  entity, elided with `…`. Bare `-d` keeps its existing meaning (full text).
  Full descriptions can multiply the length of a `report` on a large project;
  `=short` keeps it scannable.
- `search -d` now prints an excerpt of the text around the match, with `…`
  marking each trimmed end. The flag previously widened the search to
  descriptions but showed nothing of what matched, so a hit — including an
  unexpected substring match — was a black box.

### Fixed
- `task show` printed the description as a single unwrapped line, ignoring both
  the detected terminal width and `config wrap-width`. It is now wrapped to the
  resolved width, with continuation lines aligned under the value column.

### Changed
- Description wrapping is word-aware: lines break at whitespace instead of
  mid-word. A token too long to fit (a URL, a long path) is still hard-split so
  no line exceeds the width. This also affects `concept tree -d`.

### Documentation
- README and the bundled Claude skill both gained a dedicated **Descriptions**
  section: `-d` is now a cross-cutting output flag rather than a concept-tree
  footnote, and the `=` in `-d=short` is required (`-d short` is a parse error).
- README documents `mindtask import`, which shipped in 0.8.0 undocumented there.
- Skill `documents-version` pinned to 0.9.0 (enforced by `skill_doc_sync`).

## [0.8.0] - 2026-08-02

### Added
- `mindtask import <FILE|-> --under <ID>`: consume a typed concept-graph
  JSONL stream (the `pdfdex graph --format jsonl` contract) and merge its
  `is-a` slice into the concept tree as a strict subtree. Multi-parent
  nodes keep their highest-weight parent, cycles are broken at their
  weakest edge, and parentless concepts group under a category umbrella.
  Merging is by name within the target subtree and add-only by default:
  existing concepts are never moved or deleted; parent drift is reported
  and only applied with `--reparent`. `--min-docs` prunes weak nodes,
  `--dry-run` rehearses without saving. New `mindtask::import` lib module.

### Fixed
- `export plantuml gantt` (schedule-driven variant) emitted `starts at`
  constraints interleaved with task declarations, producing PlantUML forward
  references — a dependency on a task declared later in schedule order made
  PlantUML fail with `Error line N`. All `lasts`/colour declarations are now
  emitted first, followed by all `starts at` constraints.

## [0.7.0] - 2026-07-22

### Added
- `--state <STATE,...>` filter on `report`, `task ls`, and `concept report`:
  any comma-separated or repeated combination of `todo`, `in_progress`, and
  `done`, or `all` for every state. When the filter hides rows, a one-line
  footer accounts for them (`(hidden: 12 done — --state all to show)`).

### Changed
- **Breaking (output):** `report`, `task ls`, and `concept report` now default
  to `--state todo,in_progress` — done tasks are hidden unless requested; pass
  `--state all` for the previous behavior. In `concept report`, a hidden
  direct task no longer pulls its upstream dependency chain into the report,
  and the `N direct + M upstream` summary counts visible tasks only.

## [0.6.0] - 2026-07-15

### Added
- `export plantuml wbs <CONCEPT_ID>` renders only the WBS subtree rooted at
  that concept (the "Unlinked" group is omitted). Previously the root argument
  was silently ignored for `wbs`.

### Fixed
- `export … gantt <ROOT>` and `export … wbs <ROOT>` no longer silently ignore
  the root argument (CODE_REVIEW C4): `wbs` validates it as a concept ID and
  honors it; `gantt` rejects a supplied root with an error.

## [0.5.1] - 2026-07-01

### Changed
- The default `.mindtask.json` lookup no longer walks up into parent
  directories — it resolves the file in the current directory only. Use
  `--file <PATH>` to operate on a project elsewhere. This avoids accidentally
  operating on an ancestor project from a subdirectory.

## [0.5.0] - 2026-07-01

### Added
- Critical Path Method (CPM) scheduling. `mindtask schedule` prints each task's
  earliest start/finish and slack, flags the critical path, and reports the
  overall project duration; `--critical` shows only the path. Built on a new
  pure library module, `mindtask::graph::schedule`.
- The PlantUML Gantt export (`export plantuml gantt`) is now schedule-driven:
  when tasks have dependencies it positions them by the computed schedule
  (relative days) and highlights the critical path, falling back to the
  due-date chart when there are no dependencies.
- `mindtask concept normalize` renumbers concept IDs to `1..n` in tree
  (DFS pre-order) order — the order `concept tree` prints — reordering the file
  to match and remapping `parent` and task→concept links in step (task
  dependencies are untouched). It's an on-demand tidy after reordering the tree;
  `--dry-run` shows the planned old→new mapping without writing.
- `mindtask concept mv` gained `--before <SIB>` / `--after <SIB>` to position a
  concept among its siblings (taking the new parent from the anchor), since
  sibling order follows file order. The existing `--parent` form is unchanged.

## [0.4.1] - 2026-06-21

### Fixed
- Table column alignment in the `task` and `search` listings. The `Display`
  impls for task/concept ids and task state ignored the formatter's
  width/alignment flags, so the ID and STATE columns were never padded and
  every row shifted out of line with the header. They now honor the flags.

### Changed
- Task and concept tables are now rendered through a shared `render_table`
  helper that sizes columns adaptively to their contents and shrinks them to
  fit the available width, replacing the previous fixed-width column formats
  and removing the duplicated per-command formatting code.

## [0.4.0] - 2026-06-09

### Added
- Concept descriptions printed with `-d`/`--description` in `concept tree`,
  `concept report`, and the top-level `report` are now hard-wrapped to the
  available width, keeping continuation lines aligned with the tree branches
  instead of overflowing and wrapping back to column 0. The wrap width follows
  the terminal (falling back to 80 when the width is unknown, e.g. piped output).
- `config wrap-width [<COLS>]` to get or set a fixed wrap width that overrides
  terminal detection; `--clear` reverts to auto-detection and `--show` prints
  the current value. The setting persists as `wrap_width` in the project file.

## [0.3.1] - 2026-06-08

### Added
- `concept tree` and `concept report` (and the top-level `report`) accept a
  `-d`/`--description` flag that prints each concept's description on the line
  below the node, indented to line up with the tree branches.
- Global `-f`/`--file <PATH>` flag to operate on an explicit project file,
  bypassing the search for `.mindtask.json` in the current directory and its
  parents.

### Changed
- Concept ids now render as `{id}` instead of `[id]` across `concept tree`,
  `concept report`, and `report`. Inline references in `concept ls` (the
  `PARENT` column), `concept show`, and `task show` (parent, children,
  dependencies, and linked concepts/tasks) now read as `Name {id}`.
- Standardized the description toggle on `-d`/`--description` (singular) across
  `search`, `concept tree`, `concept report`, and `report`. The display flag,
  briefly named `--descriptions` during development, is `--description` to match
  the existing `search` flag.
- Refreshed dependency lockfile (`cargo update`).

## [0.3.0] - 2026-05-31

### Added
- `concept report` subcommand: report on a concept subtree together with all
  related tasks (including transitive upstream dependencies).
- Top-level `report` command: the whole-project concept tree followed by the
  task list.
- Task–concept linking (`link`/`unlink`) and link-aware task creation.

### Changed
- Hardened load-time validation (duplicate ids, dangling dependencies, cyclic
  concept trees) and made dependency-edge removal strict.
- `init` now defaults the project timezone to `Europe/Zurich`, and timezones
  are validated.
- Excluded `CODE_REVIEW.md` from the published package.

## [0.2.0] - 2026-03-11

### Added
- Claude Code skill for the mindtask CLI.

### Changed
- README updates; excluded `claude_skill/` from the published package.

## [0.1.3] - 2026-03-09

### Added
- `concept edit` and `task edit` subcommands.

## [0.1.2] - 2026-02-24

### Added
- `concept tree` subcommand for hierarchical display of the concept tree.

### Changed
- Upgraded `petgraph` from 0.7 to 0.8.

## [0.1.1] - 2026-02-21

### Added
- `export` command for PlantUML diagram generation (including Gantt charts).

### Changed
- Rewrote the README and fixed clippy warnings.

## [0.1.0] - 2026-02-18

### Added
- Initial release. CLI combining a mindmap-style concept tree with a task
  dependency graph: concept and task management, task dependencies, `search`,
  due dates with per-project timezone support, and JSON file storage.

[0.4.0]: https://git.sr.ht/~danprobst/mindtask
[0.3.1]: https://git.sr.ht/~danprobst/mindtask
