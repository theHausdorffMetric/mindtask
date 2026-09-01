# mindtask — Architecture & Code Review (0.9.0)

Reviewed at `f233e5e` on `master` (v0.9.0 + the atomic-save fix).
Supersedes [`CODE_REVIEW.md`](CODE_REVIEW.md), which was written at v0.2.1.

**Grounding.** `cargo build` clean; `cargo fmt --check` clean;
`cargo clippy --all-targets -- -D warnings` clean; 10/10 test binaries,
180 tests passing; `cargo audit` reports no advisories. Source is ~3 700 lines
of code plus ~2 300 lines of unit tests and ~1 200 lines of integration tests.
Every finding below was reproduced against the built binary; the commands are
included so each can be re-checked.

**Scope.** Architecture and layering, correctness/robustness, simplicity, and
idiomatic Rust — the same axes as the previous review, plus an architecture
section it did not have.

---

## Overall assessment

Still a well-built small project, and materially better than at v0.2.1: the
library/binary split holds, the data model is type-safe, invariant enforcement
is centralised in `Project`, load-time validation gates every command, and test
coverage has gone from "unit only" to 180 tests across 10 binaries including
seven integration suites.

The v0.2.1 review's headline risks are gone. What remains is a different and
narrower class of problem, and it clusters in one place: **everything that
crosses the boundary from internal data to generated output**. Names flow
unescaped into PlantUML, unvalidated floats flow into the scheduler and out to
JSON, and `Debug` formatting leaks into user-facing errors. The core is sound;
the edges leak.

**Strengths worth preserving**

- Typed newtype IDs (`ConceptId`/`TaskId`) — two ID spaces that cannot be confused.
- `#[serde(skip)]` ID counters recomputed on load — no persisted-counter drift.
- `add_dependency`'s speculative-insert → cycle-check → rollback.
- Load-time `validate_project` on every command (`cli/mod.rs:load_project`),
  so no command operates on malformed data.
- Deterministic import: `BTreeMap`/`BTreeSet` throughout, so projection output
  is reproducible rather than hash-order dependent.
- Atomic saves (temp + fsync + rename) with permission preservation.
- Genuinely good doc comments — most explain *why*, not *what*.

---

## Status of the previous review (v0.2.1)

Verified against current code, not taken from the old file's own markers.
**The old document is stale: five items it still lists as open are in fact
fixed.**

| Item | Old status | Actual status now | Evidence |
|------|-----------|-------------------|----------|
| C1 `is_ancestor` infinite loop | fixed | **fixed** | `visited` guard, `graph/tree.rs:12` |
| C2 non-atomic saves | open | **fixed** | temp+fsync+rename, `store/json.rs:57` |
| C3 `load` never validates | fixed | **fixed** | `load_project`, `cli/mod.rs` |
| C4 export ignores `root` | fixed | **fixed** | `parse_concept_root`/`parse_task_root` |
| C5 mermaid stubs report success | open | **STILL OPEN** | see C5 below |
| C6 duplicate ID detection | fixed | **fixed** | `validate_unique_ids` |
| C7 timezone validation | fixed | **fixed** | `validate_project` tail |
| S1 `format_due_short` ×3 | **listed open** | **fixed** | one `pub(super)` fn, `cli/task.rs:24` |
| S2 clippy warning | fixed | **fixed** | gate is `-D warnings` |
| S3 stringly-typed errors | open | **STILL OPEN** | 11 sites, see S7 |
| S4 `topological_order` dead | **listed open** | **fixed** | used by `graph/schedule.rs:62` |
| I1 `ValueEnum` migration | open | **STILL OPEN** | and has grown, see I6 |
| I2 `{:<25}` columns | **listed open** | **half fixed** | widths computed; width *unit* still wrong, see I7 |
| I3 `remove_dependency` asymmetric | **listed open** | **fixed** | `DependencyNotFound`, `project.rs:535` |
| T1 no integration tests | **listed open** | **fixed** | 7 suites, 1 189 lines |
| T2 malformed-input tests | addressed | **fixed** | `tests/cli_validation.rs` |

Three genuinely carried forward: **C5**, **S3**, **I1**, plus half of **I2**.

---

## Architecture

The layering is `model` → `graph` → {`store`, `export`, `import`} → `cli`, with
a thin `main.rs`. Dependencies point one way; there are no cycles. This is the
right shape for the problem and it has held up across five releases.

Four structural observations.

### A1. `Project` enforces invariants by method, but its fields are `pub`

`Project`'s doc comment says it "enforces structural invariants (no cycles, no
dangling references) on every mutation", and its methods genuinely do. But
`concepts`, `tasks`, and both ID counters are `pub`, so
`project.concepts.push(..)` bypasses every guard and every ID allocation.

I checked whether anything actually does this:

```sh
rg -n '\.(concepts|tasks)\.(push|retain|remove|insert)' src/ --glob '!src/model/*'
```

**Production code never bypasses the model.** The only direct pushes are in
`graph/` test modules constructing deliberately malformed data, which is
legitimate. So this is a *latent* risk, not an active defect — but it is the
single thing most likely to decay: the invariant is upheld by discipline, and
nothing tells a future contributor that `push` is off-limits.

The tension is that read-only field access (`project.tasks.iter()`,
`project.concepts.len()`) is used constantly and is entirely fine. Full
encapsulation would mean accessors for all of it, which is a lot of churn for a
risk that has not yet materialised. A cheaper 80% fix is below in the plan.

### A2. "Should this save?" is encoded in control flow, not in data

`cli/mod.rs::run` handles `Concept` and `Task` subcommands in a single block
that ends in `save_project(&path, &proj)`. Read-only subcommands opt out by
`return Ok(())` mid-match:

```rust
ConceptCommand::Ls => {
    concept::list(&proj);
    return Ok(());          // <- the only thing preventing a spurious save
}
```

Two failure modes, both silent: a new read-only subcommand that forgets the
`return` rewrites the file on every invocation; a mutating subcommand that
gains a stray `return` silently discards the user's change.

I verified the current code is correct — eight read-only commands leave the
file's inode and mtime untouched — so this is about fragility, not a live bug.

What makes it worth fixing is that **the codebase already contains the safer
idiom**: `config::timezone`, `config::wrap_width`, and `import::run` all return
`Result<bool>` meaning "modified", and the dispatcher saves iff true. Two
competing conventions for the same decision, and the better one is in the
minority.

### A3. The export API is stringly-typed at a boundary that has already parsed

`export::render(project, format, kind, root: Option<&str>)` takes the root as a
*string* and re-parses it into `ConceptId` or `TaskId` internally, choosing
which based on `kind`. The CLI has typed IDs everywhere else and deliberately
uses newtypes so the two spaces cannot be confused — then hands this one
boundary a raw string and lets the library re-derive the type.

It works, and the `kind`-dependent parse is genuinely awkward to type
(`tree`/`wbs` want a concept, `dag` wants a task, `gantt` wants neither). But an
enum (`Root::Concept(ConceptId) | Root::Task(TaskId) | Root::None`) would move
that decision to the CLI where the user's intent is known, and delete two
parse helpers.

### A4. `anyhow` appears in the model layer

The previous review praised the error split — `thiserror` in the library,
`anyhow` at the CLI boundary — and it is real everywhere except
`model/task.rs`, which imports `anyhow::{Context, Result}` for `parse_due`.
A library function returning `anyhow::Result` denies callers the ability to
match on the failure, and it is the one place the stated boundary is crossed.

### A5. Three functions are long enough to resist review

`clippy::too_many_lines` flags `import::project_tree` (208 lines),
`cli::concept::report` (115), and `cli::mod::run` (101+; ~250 counting the
match arms). `project_tree` is a four-phase pipeline whose phases are already
marked by numbered comments — those comment headers are the natural function
boundaries.

---

## Correctness / robustness

### C5. Mermaid stubs report success — `src/export/mermaid.rs` — *carried forward, still open*

Every mermaid path returns `Ok("Mermaid export not yet implemented\n")`, so:

```sh
$ mindtask export mermaid tree > tree.mmd; echo $?
0
```

A script cannot distinguish this from real output and will happily write a
junk file. Two years of `.mmd` files could accumulate before anyone notices.
Either return `Err` or drop `mermaid` from the accepted CLI values until it
exists. Unchanged since v0.2.1 and still the cheapest correctness win available.

### C8. PlantUML export never escapes names — `src/export/plantuml.rs` — **high**

Names are interpolated raw into generated diagram syntax at every site:
`component "{name}"`, `[{name}] lasts`, `* {name}`. Nothing escapes `"`, `[`,
`]`, or newlines.

```sh
$ mindtask task add 'He said "hello"' --duration 2
$ mindtask export plantuml dag
component "He said "hello"" as t1        # unbalanced quotes — PlantUML error
$ mindtask export plantuml gantt
[Fix [urgent] bug] lasts 1 days          # nested brackets — PlantUML error
```

A name containing a newline splits one declaration across two lines, which can
produce output that is *valid but wrong* rather than a clean parse error.

### C9. The Gantt export identifies tasks by name, so duplicate names collide — **high**

This is the most serious finding, because it corrupts output silently rather
than producing a syntax error. `gantt_scheduled` and `binding_predecessor` both
use `task.name` as the PlantUML identifier. Task names are not unique and
nothing suggests they should be.

```sh
$ mindtask task add 'Duplicate name' --duration 1   # id 3
$ mindtask task add 'Duplicate name' --duration 1   # id 4
$ mindtask depend add 4 3
$ mindtask export plantuml gantt
[Duplicate name] lasts 1 days
[Duplicate name] lasts 1 days
[Duplicate name] starts at [Duplicate name]'s end   # <- self-dependency
```

The generated chart asserts a task depends on itself — a constraint that exists
nowhere in the project. The `dag` export gets this right (`as t{id}`, name only
as a label); the Gantt should use the same discipline. PlantUML Gantt supports
aliases, so the fix is mechanical.

### C10. Non-finite durations are accepted and silently lost — **medium**

`--duration` is parsed as `f64`, which accepts `nan` and `inf`. `serde_json`
serialises non-finite floats as `null`, and `Option<f64>` reads `null` back as
`None`:

```sh
$ mindtask task add "nan task" --duration nan
Added task 1 "nan task"
$ grep duration .mindtask.json
"duration": null
$ mindtask schedule
No task durations set — nothing to schedule.
```

The value is accepted without complaint, written as JSON `null`, and gone after
one round-trip. Silent data loss with a success message.

### C11. Negative durations are accepted — **medium**

```sh
$ mindtask task add "neg" --duration=-5
$ mindtask schedule
ID  NAME  DUR  START  FINISH  SLACK
1   neg   -5   0      -5      5
```

A task that finishes five days before the project begins. CPM has no meaning
for negative durations; they should be rejected at parse time alongside C10.

### C14. Empty names and names containing newlines are accepted — **low**

`mindtask task add ""` succeeds and produces a blank row; an embedded newline
breaks table alignment and diagram output. A minimum validation (non-empty
after trimming, no control characters) at the model boundary would close C8's
newline vector and this together.

### C13. Unguarded recursion in the PlantUML tree/WBS writers — **low**

`write_mindmap_node` and `write_wbs_concept` recurse through `children_of` with
no depth or visited guard. A cyclic concept tree would overflow the stack.
**Not reachable through the CLI** — `load_project` validates before any command
runs, and `tests/cli_validation.rs` covers it — so this is a library-API-only
concern, listed for completeness. It is the same class as the v0.2.1 C1 finding
in `is_ancestor`, which was fixed with exactly such a guard.

---

## Simplicity / DRY

### S3 → S7. `graph` and `export` still use `Result<_, String>` — *carried forward*

11 sites. `model`, `store`, `id`, and `import` all use rich `thiserror` enums;
`graph` and `export` stringify. Callers cannot match on failure kind, and the
CLI re-wraps with `anyhow!("{e}")`, losing structure that was never there.
Unchanged since v0.2.1.

### S5. The cycle guard is duplicated between the two move paths

`move_concept` and `move_concept_positioned` carry the same
"parent is not self and not a descendant" check; the second even comments
"Same cycle guard as move_concept". One private `fn check_move_legal(&self, id,
new_parent) -> Result<()>` removes the duplication and the risk of the two
drifting.

### S6. `config wrap-width --clear` always reports "modified"

It returns `Ok(true)` unconditionally, so clearing an already-unset width
rewrites the file:

```sh
$ mindtask config wrap-width --clear   # inode 40447 -> 40438
```

Harmless before; now it means a needless fsync and a changed mtime. One-line
fix: return whether the value actually changed.

---

## Idiomatic Rust

### I1 → I6. `ValueEnum` migration — *carried forward, and grown*

`Format` and `DiagramKind` hand-roll `FromStr`, so valid values appear only in
error messages — not in `--help`, not in completions. Since v0.2.1 the pattern
has spread: `TaskState`, `StateArg`, and (as of 0.9.0) `DescMode` all hand-roll
it too. That is now five types. Migrating them would delete five parsers, make
`--help` self-documenting, and is a prerequisite for the shell completions on
the Phase 7 roadmap.

### I2 → I7. Table columns still measure the wrong unit — *half carried forward*

The truncation half of I2 is fixed — `render_table` computes widths and
truncates with `…`. But `col_width` counts **Unicode scalar values, not display
columns**, and `render.rs` says so in a comment. East Asian and emoji names
misalign every column to their right:

```
ID  NAME             STATE  DUE
1   ascii name here  todo   -
2   日本語のタスク名です       todo   -     # 10 chars, 20 columns
```

This matters more now than at v0.2.1, because 0.9.0 put descriptions into every
listing. `unicode-width` is the standard fix and the wrapping helpers are
already centralised in `render.rs`, so it is a contained change.

### I4. `Debug` formatting leaks into user-facing errors

`ProjectError::ConceptReferencedByTasks` formats its `Vec<TaskId>` with `{1:?}`:

```sh
$ mindtask concept rm 1
Caused by:
    cannot remove concept 1: tasks reference it: [TaskId(1), TaskId(2)]
```

`TaskId` has a `Display` impl written specifically for user output. The message
should read `tasks reference it: 1, 2`.

### I5. Pedantic lints: mostly cosmetic, one cluster worth doing

`clippy::pedantic` (minus the noisy-by-design lints) reports 63 warnings in the
binary and 24 in the library. The distribution matters more than the count:
35 are `uninlined_format_args` (`{}` + arg where `{name}` would do, concentrated
in `cli/task.rs`), 21 are missing doc backticks, 8 are `map().unwrap_or()`.
None are correctness issues. The format-args cluster is `clippy --fix`-able in
one pass and would make the codebase consistent with its own newer files, which
already use inline args.

---

## Testing

Coverage is now genuinely good — 180 tests, and the integration suites drive the
real binary rather than library internals. Two gaps.

### T3. The `cli` layer is unit-tested only where it was recently touched

`cli/render.rs` (214 test lines) and `cli/state_filter.rs` (80) are well
covered. `cli/concept.rs` (456 lines), `cli/task.rs` (328), `cli/mod.rs` (644),
and `cli/search.rs` (158) have **zero** in-file tests. Integration suites cover
some of their behaviour end-to-end, but `concept::report`'s upstream-dependency
collection — the most intricate logic in the CLI layer — is only exercised
through `tests/state_filter.rs`.

### T4. No test asserts generated diagrams are well-formed

`export/plantuml.rs` has 297 lines of tests, all asserting on *expected happy
output*. None feeds a hostile name through, which is why C8 and C9 survived to
0.9.0. A single property-style test — every name in a fixture round-trips
through export without producing unbalanced delimiters — would have caught both.

---

## Implementation plan

Ordered by (severity × confidence) ÷ effort. Phases 1 and 2 are the ones that
fix live defects; 3–5 are hardening and consistency.

### Phase 1 — Output correctness (fixes silent wrongness)

1. **C9 — Gantt identity.** Emit `[Task name] as [t<id>]` aliases and reference
   tasks by alias in `starts at` constraints and colour directives. Removes the
   duplicate-name collision. *Do this first: it is the only finding that
   fabricates a relationship that does not exist.*
2. **C8 — escaping.** Add one private `escape_label(&str) -> String` in
   `plantuml.rs`; apply at every interpolation site. Strip/replace newlines,
   escape `"` for `component`, and bracket-safe the Gantt labels (aliasing from
   step 1 already removes brackets from the identifier position).
3. **T4 — regression test.** A table-driven test over hostile names
   (`"`, `[`, `]`, newline, empty, duplicate) asserting balanced delimiters and
   one declaration per line, across all four diagram kinds.
4. **C5 — mermaid.** Return `Err("mermaid export not yet implemented")` so the
   exit code is non-zero. Two lines plus a CLI test.

*Contained to `export/`. No API changes. Ships as 0.9.1.*

### Phase 2 — Input validation (fixes silent data loss)

5. **C10/C11 — duration.** Reject non-finite and negative values at the CLI
   parse boundary with a clap `value_parser`, so the error names the flag.
   Add a model-level guard too, since `edit_task` is a public library entry.
6. **C14 — names.** Reject empty/whitespace-only names and strip control
   characters, in `add_concept`/`add_task`/both `edit_*`.
7. **I4 — error formatting.** Replace `{1:?}` with a `Display` join in
   `ConceptReferencedByTasks`. Audit the other `ProjectError` variants for the
   same.

*Small, mechanical, each independently testable. Ships as 0.9.2.*

### Phase 3 — Structural hardening

8. **A2 — save-or-not as data.** Change `cli::concept::*` and `cli::task::*`
   handlers to return `Result<Outcome>` where
   `enum Outcome { Unchanged, Modified }`, matching what `config`/`import`
   already do; the dispatcher saves iff any arm returned `Modified`. Deletes
   every mid-match `return Ok(())`. Mechanical, ~9 call sites, and it makes a
   whole class of future bug unrepresentable.
9. **A1 — encapsulation, cheap version.** Keep the fields readable but make the
   mutation path explicit: leave `concepts`/`tasks` public for iteration, make
   `next_concept_id`/`next_task_id` private with accessors (nothing outside
   `model` touches them), and add a `// invariant: mutate only through the
   methods below` note plus a `#[doc(hidden)]`-style warning on the two Vecs.
   Full encapsulation is not worth the churn for a risk that has not fired.
10. **S5 — shared move guard.** Extract `check_move_legal`.
11. **S6 — wrap-width clear.** Return the real "changed" answer.

### Phase 4 — Type-level consistency

12. **I6 — `ValueEnum`.** Migrate `Format`, `DiagramKind`, `TaskState`,
    `StateArg`, `DescMode`. Note `StateArg`/`DescMode` have custom semantics
    (`all`, optional value) — check each still parses identically, and keep the
    existing tests as the contract. Unlocks shell completions (Phase 7 roadmap).
13. **S7 — typed errors.** `GraphError` and `ExportError` enums replacing the
    11 `Result<_, String>` sites.
14. **A4 — `anyhow` out of the model.** Give `parse_due` a `DueParseError`;
    the CLI keeps its `.context()` wrapper.
15. **A3 — typed export root.** `Root` enum; delete the two parse helpers.

### Phase 5 — Presentation and polish

16. **I7 — display width.** Add `unicode-width`; route `col_width` and the
    wrap helpers through it. Contained to `render.rs`; extend its existing
    test suite with a CJK/emoji case.
17. **I5 — format args.** `cargo clippy --fix` for `uninlined_format_args`,
    then the doc-backtick pass. One commit, no behaviour change.
18. **A5 — split long functions.** `project_tree` along its four existing
    numbered phase comments; `concept::report` along its five.
19. **T3 — CLI unit tests.** Start with `concept::report`'s upstream collection.

### Housekeeping

20. Mark the five stale items in `CODE_REVIEW.md` as fixed, or replace that
    file with a pointer here. Five of its sixteen findings currently misreport
    the codebase, which makes it actively misleading to a newcomer.

---

## Q&A / judgement calls

**Why is C9 ranked above C8?** C8 produces output PlantUML rejects — loud, and
you find out immediately. C9 produces output PlantUML *accepts* that asserts a
dependency the project does not contain. Silent wrongness outranks loud
breakage.

**Why not full encapsulation for A1?** Because nothing bypasses the model
today, read access is used pervasively and legitimately, and the accessor churn
would touch most of the codebase to close a hole that has not been walked
through in five releases. The Phase 3 middle path costs almost nothing.

**Is the O(n²) worth fixing?** No. `get_concept` and `children_of` are linear
scans, so `dfs_preorder_ids`, `validate_tree`, and `apply_normalization` are
quadratic. At this project's scale (46 concepts, 135 tasks) the whole validate
pass is imperceptible, and an index would add an invalidation burden to
`Project`. Revisit only if a project reaches thousands of nodes.
