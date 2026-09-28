# Refs — citations from tasks and concepts to OKF bundle pages

Status: R1–R3 implemented (2026-09-28, 0.12.0). R4 is open and **low priority** — no consumer of clickable diagrams yet; revisit only when one appears.

Goal: give tasks and concepts a structured list of **URI references** so the
citation bridge between mindtask (the action layer) and OKF bundles (the
knowledge layer) becomes data that `validate` can check, `search` can find,
`show` can print, and a rename helper can rewrite. Today those citations are
prose inside descriptions (`knowledge/llm-wiki.md`, `RND/log.md`, …), so a wiki
rename silently orphans them. The design argument is in the ideas repo,
`knowledge/schema-vs-emergence.md` § "OKF and the action layer": *the bridge is
citations, and it inherits OKF's identity fragility — whatever lints the bundle
must lint the citations too.*

mindtask stays thin: it never parses Markdown or frontmatter, never becomes an
OKF consumer. A ref is a string; the only filesystem question mindtask asks is
"does this file exist?".

## Locked design defaults (v1)

- **Field name `refs`**, on both `Task` and `Concept`, `Vec<String>`, skipped
  when empty. This *is* a file-format change, of the additive kind: a
  version-1 file loads unchanged, and a project without refs serialises
  byte-identically to before. It is also the first format change since the
  `version` field was reserved, so the field starts doing its job now — see
  *Format version and unknown fields*.
- **Values are URI references** (RFC 3986 § 4.1), stored verbatim after
  validation. No custom scheme (`okf://` would click nowhere). Two classes,
  decided by the presence of a scheme:
  - **relative** — `knowledge/llm-wiki.md`, `RND/log.md#2026-08-01`,
    `../other-bundle/index.md`. The OKF case. Resolved against the **directory
    of the project file** (so `-f` and the ideas-repo root both work, and the
    resolution never depends on the cwd). Existence is checked on the
    **path part only** — a `#fragment` is kept but not verified (heading
    identity is even more fragile than path identity); a `?query` is treated
    as part of the path name, which relative filesystem refs simply don't use.
  - **absolute** — anything with a scheme (`https://…`, `mailto:…`).
    Syntax-checked only, never fetched.
- **Rejected at the door** (`validate_ref`, same style as `validate_name`):
  empty after trim; any whitespace or control character (a URI has none, and
  either would break the PlantUML `[[…]]` hyperlink position); a filesystem
  absolute path (`/home/…`) because it is not portable across clones —
  `file://` counts as absolute-with-scheme and is allowed but only
  syntax-checked. A single-letter scheme (`C:\…`) is refused as a Windows
  drive path, not a scheme.
- **Duplicates are rejected** per owner (exact string match); order is
  insertion order and preserved.
- **Broken refs never block a command.** `load_project` keeps rejecting only
  structural corruption; a wiki rename must not brick `task state`. The check
  lives in `mindtask validate` and `mindtask ref ls`.
- **`validate` fails (non-zero) on a broken relative ref**, and says so
  separately from structural errors. This is the lint gate the essay asks
  for; projects without refs are unaffected. No opt-out flag in v1.
- **Reverse direction is out of scope.** Bundles cite mindtask in prose
  (`mindtask task 86`); checking those belongs to the bundle linter
  (ideas task 8, option B). mindtask only has to keep its IDs addressable —
  task IDs already are; concept IDs are renumbered by `concept normalize`,
  so bundles should cite tasks, not concepts.

## Format version and unknown fields

Until now `version` was written as `1` by `Project::new` and never read by any
code path, and `Project` did not `deny_unknown_fields`. The consequence for
this change: a **pre-0.12 binary loads a file that contains `refs`, silently
drops them on its next save, and writes whatever `version` it read straight
back** — so no version number can protect this transition, and the only remedy
is operational (see *Rollout*, step 1: upgrade every writer before the first
`ref add`; the `mindtask` on `$PATH` here is 0.10.0 while the crate is at
0.11.0, so the step is real). Three measures in 0.12 make it the last
transition with that property:

1. **`FORMAT_VERSION = 2`.** `Project::new` writes 2; `store::json::load`
   refuses a file whose `version` is *higher* than it understands, with an
   error that says to upgrade mindtask. A version-1 file still loads (the
   field is additive) and is marked `2` on its next save, so the number
   records "a 0.12+ binary has written this".
2. **`#[serde(deny_unknown_fields)]`** on `Project`, `Task`, and `Concept`. A
   binary that meets a field it does not know refuses the file instead of
   dropping data. This is the actual fix for the class of hazard; the version
   number gives it a better error message. The repo policy already forbids
   hand-editing the JSON, so the cost is nil. (Aliases such as `status` for
   `state` remain accepted — serde treats them as known.)
3. **CHANGELOG entry** naming the hazard and the upgrade order.

Neither measure rejects anything 0.11 wrote; they only make 0.12 refuse what a
*future* mindtask writes.

## File layout

| File | Change |
|---|---|
| `src/model/task.rs` | `refs: Vec<String>` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`); `#[serde(deny_unknown_fields)]`; update the struct literals in its tests |
| `src/model/concept.rs` | same field, same attribute; same test updates |
| `src/model/validate.rs` | `validate_ref(&str) -> Result<String>` + tests |
| `src/model/reference.rs` | **new** — the pure side: `RefOwner`, `RefKind`, `classify`, `path_part`; no I/O, so the model can use it |
| `src/model/project.rs` | `FORMAT_VERSION = 2`; `deny_unknown_fields`; `ProjectError::{InvalidRef, DuplicateRef, RefNotFound}`; `add_ref` / `remove_ref` / `rename_ref` / `refs()`; `add_task`/`add_concept` literals gain `refs: Vec::new()`; **`apply_normalization` must copy `refs`** (it rebuilds `Concept` field by field — the compiler forces this, but the test in *Tests* pins it) |
| `src/refs.rs` | **new** — `mindtask::refs`: `base_dir(project_file)`, `resolve(base, path)`, `check(project, base) -> Vec<BrokenRef>`; the only module that touches the filesystem for refs |
| `src/lib.rs` | `pub mod refs;` |
| `src/store/json.rs` | `load` refuses `version > FORMAT_VERSION` (`StoreError::UnsupportedVersion`) and marks older files with the current version in memory; `sibling_dir` moves to `refs::base_dir` |
| `src/cli/reference.rs` | **new** — handlers for `ref add/rm/ls/mv` (module named `reference` to avoid the `ref` keyword) |
| `src/cli/mod.rs` | `Command::Ref(RefCommand)`; `--ref` on `task add` and `concept add`; `Validate` runs the ref check after the structural one |
| `src/cli/task.rs`, `src/cli/concept.rs` | `show` prints `Refs:`; `add` accepts `Vec<String>` refs and attaches them after creation (mirrors how `--concept` links today, so `Project::add_task`'s signature does not change again) |
| `src/cli/search.rs` | refs are matched by default (a ref is short and structured, like a name); a ref hit is shown beneath the row like a description excerpt |
| `src/export/plantuml.rs` | *(phase R4, optional)* first ref becomes a `[[…]]` hyperlink |
| `claude_skill/SKILL.md` | `ref` + `ref add/rm/ls/mv` (the `skill_doc_sync` test fails until this is done); bump `documents-version` on release |
| `README.md`, `CHANGELOG.md`, `STATUS.md` | docs |
| `tests/refs.rs` | **new** — binary-level integration tests (see *Tests*) |

## Library API

```rust
// model/reference.rs (pure)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefOwner { Task(TaskId), Concept(ConceptId) }   // Display: "task 8" / "concept 12"
pub enum RefKind<'a> {
    /// Has a scheme; syntax only.
    Absolute { scheme: &'a str },
    /// No scheme; `path` excludes the fragment.
    Relative { path: &'a str, fragment: Option<&'a str> },
}
pub fn classify(uri: &str) -> RefKind<'_>;
pub fn path_part(uri: &str) -> &str;   // everything before the first `#`

// model/project.rs
pub const FORMAT_VERSION: u32 = 2;

impl Project {
    /// Validate, then append. Err: owner missing, InvalidRef, DuplicateRef.
    pub fn add_ref(&mut self, owner: RefOwner, uri: &str) -> Result<()>;
    /// Exact-string removal. Err: owner missing, RefNotFound.
    pub fn remove_ref(&mut self, owner: RefOwner, uri: &str) -> Result<()>;
    /// Rewrite the *path part* `old` → `new` on every ref whose path part is
    /// exactly `old`, preserving each ref's own fragment. Returns the number
    /// of refs rewritten; `new` is validated once up front. Pure; the CLI
    /// decides whether to save (`--dry-run`).
    pub fn rename_ref(&mut self, old: &str, new: &str) -> Result<usize>;
    /// Every (owner, ref) pair in file order — the citation table.
    pub fn refs(&self) -> impl Iterator<Item = (RefOwner, &str)>;
}

// refs.rs (filesystem)
pub fn base_dir(project_file: &Path) -> &Path;         // the file's directory; "." for a bare name
pub fn resolve(base: &Path, path: &str) -> PathBuf;   // base.join(path), no canonicalize
pub struct BrokenRef { pub owner: RefOwner, pub uri: String, pub resolved: PathBuf }
pub fn check(project: &Project, base: &Path) -> Vec<BrokenRef>;
```

`classify` is a hand-rolled scheme test (`[A-Za-z][A-Za-z0-9+.-]*:` with length
≥ 2 before the colon), not the `url` crate — the crate would parse relative
references only against a base URL and would percent-encode on the way out,
which changes the stored string. Nothing here needs more than "is there a
scheme, and where does the fragment start".

`check` uses `Path::exists()` on `resolve(base, path)`; a ref to a directory
(`RND/`) counts as existing — an OKF bundle root is a legitimate citation.
Symlinks are followed by `exists()`; fine.

## CLI

```sh
mindtask task add <NAME> … [--ref <URI>]...       # attach at creation
mindtask concept add <NAME> … [--ref <URI>]...

mindtask ref add (--task <ID> | --concept <ID>) <URI>
mindtask ref rm  (--task <ID> | --concept <ID>) <URI>
mindtask ref ls [--broken]          # KIND ID NAME REF STATUS  (ok | missing | external)
mindtask ref mv <OLD> <NEW> [--dry-run]   # rewrite a renamed page across every ref
```

- `--task`/`--concept` form a required, mutually exclusive clap `ArgGroup`.
- `ref add/rm/mv` return `Result<bool>` ("modified"), the convention `config`
  and `import` already use, so they land on the better side of review finding
  A2 without waiting for phase 3.
- `ref ls` is the citation table for external tooling (the bundle linter of
  ideas task 8 consumes it). Plain table now; a `--json` flag is a follow-up
  the rest of the CLI does not have either.
- `validate` output when refs are broken:

  ```
  Structure is valid.
  2 broken ref(s):
    task 8      knowledge/llm-wiki.md   -> /home/dan/dev/ideas/knowledge/llm-wiki.md
    concept 12  RND/index.md#term       -> /home/dan/dev/ideas/RND/index.md
  Validation failed.
  ```
  exits non-zero. With no broken refs the existing `Project is valid.` line is
  unchanged, so scripts that grep for it keep working.

- `task show` / `concept show` add one `Refs:` block, one ref per line
  (URIs are long; a comma-joined line would wrap badly):

  ```
  Refs:        knowledge/llm-wiki.md
               RND/log.md#2026-08-01
  ```

- `search`: a ref match shows the ref beneath the row (reuse
  `render_table_with_blocks`' block slot, same indent as the description
  excerpt). Refs match in the default (name-only) mode.

## Phases

### R1 — model and persistence

1. `refs` field on `Task` and `Concept`; fix every struct literal (model tests,
   `add_task`, `add_concept`, `apply_normalization`).
2. `FORMAT_VERSION = 2`; `deny_unknown_fields` on the three structs; `load`
   refuses newer files and marks older ones.
3. `validate_ref` in `model/validate.rs`; `model/reference.rs`.
4. `ProjectError` variants; `add_ref`, `remove_ref`, `rename_ref`, `refs()`.
5. Unit tests (below).

Ships nothing user-visible; the file format gains an optional field and a
version guard.

### R2 — resolution and the lint gate

6. `src/refs.rs`: `base_dir`, `resolve`, `check`.
7. `validate` runs `refs::check` against the project file's directory after
   the structural check and reports as above.
8. `ref ls [--broken]`.

### R3 — editing surface and docs

9. `--ref` on `task add` / `concept add`; `ref add`, `ref rm`, `ref mv`.
10. `Refs:` in both `show`s; refs in `search`.
11. `SKILL.md`, `README.md` (new "References" section after "Linking Tasks to
    Concepts"), `CHANGELOG.md` (with the compatibility hazard), `STATUS.md`.
12. Release **0.12.0** — minor: new field, new commands, new library API
    (`add_task`/`add_concept` signatures unchanged; `ProjectError` gains
    variants, which is the same kind of bump 0.11.0 was).

### R4 — export hyperlinks (optional, separate release, **low priority**)

Low priority: nothing consumes clickable diagrams today, rendering needs Java
or a container the gate does not run, and one link per element makes "first
ref" an arbitrary choice. When it comes up, weigh the alternative first: a
PlantUML `note` per node listing *every* ref as text — no base-URL question,
all refs shown, survives PNG — at the cost of not being clickable.

13. Emit the first ref of each node as a PlantUML hyperlink: `* Name [[uri]]`
    in mindmap/WBS, `component "Name" as t1 [[uri]]` in the DAG, and the
    Gantt's `[Name] links to [[uri]]`. Skip a ref containing `]` (only IPv6
    hosts can carry one) rather than mangle it.
14. Open question to settle before doing it: a *relative* ref in an SVG
    resolves against wherever the SVG was written, not the project file.
    Either add `export --link-base <PREFIX>` for relative refs, or emit only
    absolute refs as hyperlinks. Decide when someone actually wants clickable
    diagrams; do not pre-build it.

## Tests

Unit (`cargo test`):

- `validate_ref`: table over accepted (`a.md`, `dir/a.md#frag`, `../x.md`,
  `https://h/p?q#f`, `mailto:x@y`, `file:///etc/hosts`) and rejected
  (`""`, `"  "`, `a b.md`, `a\tb`, `/abs/path.md`, `C:\x.md`).
- `classify`: scheme detection incl. the single-letter refusal; fragment
  split (`a.md#x#y` → path `a.md`, fragment `x#y`); `#only` → empty path
  (rejected upstream by `validate_ref`).
- serde: empty `refs` is absent from JSON (extend `task_skips_empty_vecs` and
  `concept_skips_none_fields`); non-empty round-trips in order.
- `Project`: `add_ref` on missing owner / duplicate / invalid; `remove_ref`
  unknown; `rename_ref` rewrites path part, keeps fragments, counts
  correctly, is a no-op on `old` absent; **`apply_normalization` preserves
  concept refs** (regression pin for the field-by-field rebuild).
- `refs::check`: tempdir with a fake bundle; ok / missing / external /
  directory-as-target / fragment ignored.

Integration (`tests/refs.rs`, same harness as `tests/cli_validation.rs`):

- `task add --ref` then `task show` prints the `Refs:` block; the JSON on disk
  contains it; a second `ref add` of the same URI exits non-zero and leaves
  the file untouched (mtime).
- `validate` passes with the target present; delete the file; `validate` exits
  non-zero and names the owner and the ref; `ref ls --broken` lists exactly
  it; `ref mv old new` fixes it; `validate` passes again.
- **Resolution base is the project file's directory, not the cwd:** run with
  `-f <dir>/.mindtask.json` from a different cwd and confirm the ref resolves.
- `task state` on a project with a broken ref still succeeds (broken refs do
  not block operational commands).
- `skill_doc_sync` passes once `SKILL.md` lists `ref` and its subcommands.

Gate as usual: `cargo fmt --check`, `cargo clippy -D warnings`, build, test.

## Rollout in the ideas repo

1. Upgrade every writer: `cargo install --path crates.io/mindtask` on the
   workstation; rebuild on the tardibot VM (no GitHub token there, so build
   from the monorepo checkout). Do this **before** step 2.
2. Migrate the existing prose citations — six on the mindtask side today
   (tasks citing `knowledge/llm-wiki.md` ×2, `knowledge/schema-vs-emergence.md`,
   `RND/log.md`, `RND/methods/arbitrage-free-smoothing.md`, `claw/index.md`) —
   with `ref add`; leave the prose sentences in place, they carry the *why*.
3. `mindtask validate` becomes the citation lint. Add one line to the ideas
   `CLAUDE.md` mindtask section: *cite bundle pages with `mindtask ref add`,
   never only in prose; `validate` checks them.*
4. Update ideas task 8 (llm-wiki implementation path): option B's
   "mindtask→wiki citation integrity" half is then done by mindtask itself;
   what remains for B is the bundle-side lint (bundle links, and the reverse
   `mindtask task N` mentions against `ref ls`).

## Out of scope (deliberately)

- Typed edges or relation kinds on refs. The essay reserves typed relations for
  the bundle layer's `relations:` frontmatter; a ref is a citation, one kind.
- Parsing bundle pages, checking that a fragment matches a heading, or
  following `resource:` frontmatter.
- Refs on dependencies or on the project itself.
- Fetching absolute URLs.
- Renumbering-safe concept citations from bundles (`normalize` could emit its
  old→new map for a bundle linter; not until a bundle actually cites a concept).
