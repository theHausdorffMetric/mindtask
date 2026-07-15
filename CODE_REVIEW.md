# mindtask — Code Review

Reviewed at commit on `master` (v0.2.1). Grounding: `cargo build` clean,
`cargo test` → 71 passed, `cargo clippy` → 1 warning.

Focus: quality, correctness/robustness, simplicity, idiomatic Rust.

## Overall assessment

A well-built small project. Clean architecture (`model` / `graph` / `store` /
`export` library + thin `cli` binary), type-safe data model, thorough doc
comments, strong unit-test coverage. The findings below are mostly robustness
hardening and DRY cleanup, not fundamental flaws.

**Strengths worth preserving**

- Typed newtype IDs (`ConceptId`/`TaskId`) make the two ID spaces unconfusable.
- ID counters are `#[serde(skip)]` and recomputed on load (`recompute_next_ids`)
  — robust against hand-edited IDs, no persisted-counter drift.
- `add_dependency` uses speculative-insert + cycle-check + rollback — correct and
  well-tested (`model/project.rs:319`).
- Idiomatic error split: `thiserror` enums in the library, `anyhow` + `.context()`
  at the CLI boundary.
- Edition-2024 let-chains used cleanly.

---

## Correctness / robustness

### C1. `is_ancestor` can infinite-loop on malformed data — `src/graph/tree.rs:8` — ✅ FIXED
Added a visited-set guard; the walk now returns `false` on a cyclic chain
instead of hanging. Also defused at the boundary by C3 (load-time validation).

The parent-chain walk has no visited-set guard. `validate_tree` guards against
cycles, but `move_concept` (`src/model/project.rs:233`) calls `is_ancestor`
directly, and `load` (`src/store/json.rs:28`) never validates. A hand-edited
`.mindtask.json` with a cyclic `parent` chain + any `concept mv` → hang.
Fix: add a visited set (mirror `validate_tree`) or bound the walk.

### C2. Non-atomic saves risk data loss — `src/store/json.rs:39`
`save` does `std::fs::write`, which truncates then writes in place. An interrupt
/ crash / full disk mid-write corrupts or empties the project file — the single
source of truth. Fix: write to a sibling temp file, then `fs::rename` onto the
target (atomic on the same filesystem).

### C3. `load` never validates — `src/store/json.rs:28` — ✅ FIXED
The CLI `load_project` helper (`src/cli/mod.rs`) now runs `validate_project`
after load and refuses malformed files with a descriptive error. The `store`
layer stays pure I/O. `validate` loads via the raw `store::json::load` so it
remains the diagnostic command rather than being blocked by the guard.

Dangling deps/concept refs, duplicate IDs, or tree cycles in a hand-edited file
load silently. Only the explicit `validate` command catches them (and C1 can
hang first). Consider validating on load, or document that mutation commands
assume a valid file.

### C4. `export gantt`/`wbs` silently ignore (and never validate) `root` — ✅ FIXED
`wbs` now parses+validates `root` as a concept ID and renders only that subtree
(omitting the "Unlinked" group); `gantt` rejects a supplied root with an error
in `render` (it applies to both formats). Docs aligned (README table/examples,
CLI help, `render` doc comment); tests cover subtree, validation, and rejection.

`render_plantuml`/`render_mermaid` parse+validate `root` for `tree`/`dag` but
pass nothing for `gantt`/`wbs` (`src/export/mod.rs:141-142,160-161`). So
`mindtask export plantuml gantt 999` succeeds, ignoring `999` — yet `render`'s
doc comment claims `root` applies to gantt. Either honor it or reject a supplied
root for those kinds.

### C5. Mermaid stubs report success — `src/export/mermaid.rs`
Every mermaid path prints "… not yet implemented" to stdout with exit code 0;
scripts can't distinguish that from real output. Until implemented, return an
`Err` (nonzero exit) or hide `mermaid` from accepted CLI values.

### C6. `validate` does not detect duplicate concept or task IDs — ✅ FIXED
Added `validate_unique_ids` (`src/graph/dag.rs`), run first inside
`validate_project` so downstream first-match lookups reason about clean data.

### C7. `validate` does not check the project timezone — ✅ FIXED
`validate_project` now verifies `project.timezone` (if set) names a real IANA
zone via `jiff::tz::TimeZone::get`. Previously a hand-edited bad timezone passed
`validate` and the load guard, only failing later when formatting a due date.
`init`/`config timezone` already validated; this closes the hand-edited-file gap.

`validate_project` → `validate_tree` + `validate_dag` + task→concept ref check.
None of these check ID uniqueness (`src/graph/tree.rs:30`, `src/graph/dag.rs:54`).
Two concepts (or tasks) sharing an ID pass validation; lookups silently resolve
to the first match. See the Q&A note at the bottom.

---

## Simplicity / DRY

### S1. `format_due_short` copy-pasted three times
Verbatim in `src/cli/task.rs:19`, `src/cli/concept.rs:201`, `src/cli/search.rs:7`
(plus `format_due` in `task.rs`). The task-table rendering block is also
duplicated across `task::list`, `search::search`, and `concept::report`. Pull
into one shared `cli` helper module (e.g. `cli/fmt.rs`).

### S2. Clippy warning (the only one) — `src/cli/concept.rs:262` — ✅ FIXED
Dropped the empty `""` trailing column from the report header; `cargo clippy
--all-targets` is now clean (0 warnings).

`println!("{:<6} … {}", …, "")` passes an empty literal into `{}`. Drop the
trailing column or label it, so `cargo clippy` is clean.

### S3. Inconsistent error strategy
`model`/`store`/`id` use rich `thiserror` enums; `graph` and `export` fall back
to stringly-typed `Result<_, String>`. Unify on typed errors or document the
boundary.

### S4. `topological_order` is dead in production
`pub`, fully tested, but only called from tests (`src/graph/dag.rs`). Keep as
deliberate library API, otherwise drop it or wire it into a command.

---

## Idiomatic Rust

### I1. Use `#[derive(clap::ValueEnum)]` for `Format`, `DiagramKind`, `TaskState`
They hand-roll `FromStr` and are used as clap args, so valid choices appear only
in error messages — not in `--help` and not in shell completion. `ValueEnum`
gives both for free and deletes the hand-written parsers.

### I2. Table columns use `{:<25}` (a minimum width, not a max)
Long names overflow and break alignment; no Unicode-width handling. A
truncate-to-width helper would keep tables aligned.

### I3. `remove_dependency` is asymmetric — `src/model/project.rs:353`
Errors if the task is missing, but silently succeeds if the edge doesn't exist.
Minor; just worth a deliberate decision.

---

## Testing

### T1. No integration/CLI tests
All 71 tests are unit tests inside `src/`; `tests/` contains only `fixtures/`.
The entire `cli/` layer — arg parsing, the read-vs-write `return Ok(())`
save-skipping pattern in `cli/mod.rs`, `report`/`export` output, file
round-trips through the binary — is untested. A few `assert_cmd`-style tests
would catch C4/C5 and guard against regressions.

### T2. No malformed-input tests — ✅ ADDRESSED
Added `tests/cli_validation.rs`: drives the real binary against malformed
`.mindtask.json` files (duplicate IDs, cyclic tree, dangling deps), asserting
operational commands and `validate` both reject them — and that the cyclic-tree
case terminates rather than hanging. This is also the project's first
integration test (partially addresses T1).

---

## Suggested priority

1. C2 (atomic save) and C1 (cycle guard) — data-loss / hang risks.
2. S2 (clippy) and S1 (de-dup formatting) — quick, high-value cleanups.
3. C4 / C5 — surprising silent behavior.
4. C3 / C6 — validation completeness.
5. T1 / T2 — integration tests to lock the above in.
6. I1 — ValueEnum migration (UX + simplification).

---

## Q&A

**Does `validate` check for duplicate concept IDs?** No. `validate_project`
(`src/graph/dag.rs:77`) runs `validate_tree` + `validate_dag` + a task→concept
reference check. `validate_tree` only verifies that each concept's `parent`
exists and that walking parents reaches a root without a cycle; it never checks
that concept IDs are unique. (Same for task IDs in `validate_dag`.) So a file
with two concepts sharing an ID passes `validate`; all lookups (`get_concept`,
`children_of`, etc.) silently use the first match. Normal CLI usage can't produce
duplicates because `allocate_concept_id`/`recompute_next_ids` guarantee
monotonic IDs — this only bites hand-edited or merged files.
</content>
</invoke>
The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.

