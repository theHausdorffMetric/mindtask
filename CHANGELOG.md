# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
