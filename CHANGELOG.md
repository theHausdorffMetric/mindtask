# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-06-08

### Added
- `concept tree` and `concept report` (and the top-level `report`) accept a
  `-d`/`--descriptions` flag that prints each concept's description on the line
  below the node, indented to line up with the tree branches.
- Global `-f`/`--file <PATH>` flag to operate on an explicit project file,
  bypassing the search for `.mindtask.json` in the current directory and its
  parents.

### Changed
- Concept ids now render as `{id}` instead of `[id]` across `concept tree`,
  `concept report`, `report`, `concept ls`, and `concept show`. In `ls` the
  `PARENT` column and in `show` the parent/children/tasks references now read
  as `Name {id}`.
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
- `export` command for PlantUML diagram generation, with podman-based SVG
  conversion and a test fixture generator.

### Changed
- Rewrote the README and fixed clippy warnings.

### Fixed
- Corrected Gantt chart syntax in exported diagrams.

## [0.1.0] - 2026-02-18

### Added
- Initial release. CLI combining a mindmap-style concept tree with a task
  dependency graph: concept and task management, task dependencies, `search`,
  due dates with per-project timezone support, and JSON file storage.

[0.3.1]: https://git.sr.ht/~danprobst/mindtask
</content>
</invoke>
