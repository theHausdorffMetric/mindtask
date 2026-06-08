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

[0.3.1]: https://git.sr.ht/~danprobst/mindtask
</content>
</invoke>
