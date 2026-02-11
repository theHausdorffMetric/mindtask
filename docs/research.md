# Research Report: Task Management & Mindmap Data Structures

---

## 1. TaskJuggler

**Overview**: Ruby-based project management tool using a custom DSL (`.tjp` files). One of the most mature open-source scheduling engines.

**Core Entities**:
- `project` — top-level container with timeframe, timezone, currency
- `task` — hierarchical (tasks nest inside tasks), with:
  - `start`, `end`, `duration`, `effort`, `length`
  - `depends` — dependency on other tasks with optional lag
  - `allocate` — resource assignment
  - `complete` — percentage done
  - `priority` — 1-1000
  - `milestone` — zero-duration marker
- `resource` — people/equipment, hierarchical (teams contain members)
- `account` — cost tracking

**Dependency Model**:
```
task "Design" "design" {
  effort 10d
  allocate dev1
}
task "Implement" "impl" {
  depends design  # finish-to-start by default
  # depends design { gapduration 2d }  # with lag
  # depends design { onstart }  # start-to-start
  # depends design { onend }    # finish-to-finish
  effort 20d
  allocate dev1, dev2
}
```

TaskJuggler supports all four dependency types (FS, SS, FF, SF) plus lag/lead times. Tasks are hierarchical — parent tasks automatically span their children.

**Duration vs Effort**:
- `duration` = calendar time (includes weekends)
- `length` = working time (excludes weekends)
- `effort` = person-days (divides across allocated resources)

**Key Insight**: TaskJuggler separates the *declaration* (what tasks exist, their constraints) from the *scheduling* (when they actually occur). The engine solves the schedule. This is powerful but complex — for a CLI tool, we may want manual scheduling or simpler forward-pass only.

---

## 2. GanttProject

**Overview**: Java desktop app, stores data in XML (`.gan` files).

**XML Schema (simplified)**:
```xml
<project name="My Project" version="3.0">
  <tasks>
    <task id="1" name="Design" duration="10" complete="0"
          start="2024-01-01" color="#8cb6ce">
      <task id="2" name="UI Design" duration="5" />
      <task id="3" name="API Design" duration="5" />
    </task>
    <task id="4" name="Implementation" duration="20" start="2024-01-15">
      <depend id="1" type="2" difference="0" hardness="Strong"/>
    </task>
  </tasks>
  <resources>
    <resource id="1" name="Developer" function="Default:1"/>
  </resources>
  <allocations>
    <allocation task-id="4" resource-id="1" responsible="true"/>
  </allocations>
</project>
```

**Dependency Types** (the `type` attribute):
- `1` = Start-to-Start
- `2` = Finish-to-Start (default)
- `3` = Finish-to-Finish
- `4` = Start-to-Finish

**Key Insight**: Tasks are nested (tree), dependencies are separate edges. This is the same two-structure approach we're using (tree for hierarchy, DAG for dependencies). `difference` is the lag time in days. `hardness` can be "Strong" (enforced) or "Rubber" (preferred).

---

## 3. OpenProject

**Overview**: Rails-based web app with REST API. Uses "work packages" as the core unit.

**Work Package Model (JSON API)**:
```json
{
  "id": 42,
  "subject": "Implement OAuth",
  "description": { "format": "markdown", "raw": "..." },
  "type": "Task",
  "status": "In progress",
  "priority": "Normal",
  "startDate": "2024-03-01",
  "dueDate": "2024-03-15",
  "estimatedTime": "PT40H",
  "percentageDone": 25,
  "parent": { "href": "/api/v3/work_packages/10" },
  "relations": [
    {
      "type": "follows",
      "from": { "href": "/api/v3/work_packages/41" },
      "to": { "href": "/api/v3/work_packages/42" },
      "lag": 0
    }
  ]
}
```

**Relation Types**: `follows` (FS), `precedes`, `blocks`, `blocked_by`, `relates`, `duplicates`, `parent`, `children`

**Key Insight**: OpenProject uses ISO 8601 durations (`PT40H`) and treats relations as first-class objects with their own endpoints. Relations go beyond scheduling (e.g., `blocks`, `relates`, `duplicates`).

---

## 4. Mindmap Data Formats

**FreeMind (`.mm` — XML)**:
```xml
<map version="1.0">
  <node TEXT="Project" FOLDED="false">
    <node TEXT="Backend" POSITION="right">
      <node TEXT="API" />
      <node TEXT="Database" />
    </node>
    <node TEXT="Frontend" POSITION="left">
      <node TEXT="Components" />
    </node>
  </node>
</map>
```

Pure tree. Nodes have: TEXT, POSITION (left/right of root), FOLDED, COLOR, LINK, icons, rich text. No concept of dependencies or tasks.

**XMind (`.xmind` — ZIP containing JSON)**:
```json
{
  "id": "root",
  "class": "sheet",
  "title": "Sheet 1",
  "rootTopic": {
    "id": "t1",
    "title": "Central Topic",
    "children": {
      "attached": [
        {
          "id": "t2",
          "title": "Main Topic 1",
          "children": {
            "attached": [
              { "id": "t3", "title": "Subtopic" }
            ]
          }
        }
      ]
    },
    "markers": [{ "markerId": "priority-1" }],
    "labels": ["important"],
    "notes": { "plain": { "content": "Some notes" } }
  }
}
```

Also a tree. XMind supports "relationships" (cross-links between nodes) as separate objects, but they're visual annotations, not structural.

**Mermaid Mindmap Syntax**:
```
mindmap
  root((Project))
    Backend
      API
      Database
    Frontend
      Components
```

Pure tree, text-based. No metadata beyond nesting.

**Key Insight**: All mindmap formats are strict trees. Cross-links (where they exist) are secondary/visual. Our concept tree aligns perfectly with established mindmap data models.

---

## 5. DAG Representation in JSON

**Pattern A: Flat list with parent references (tree)**
```json
[
  { "id": "c1", "name": "Root", "parent": null },
  { "id": "c2", "name": "Child", "parent": "c1" }
]
```
Pros: Easy to move/reparent (change one field). Flat, easy to index.
Cons: Building the tree requires a pass. No guaranteed ordering.

**Pattern B: Nested children (tree)**
```json
{
  "id": "c1", "name": "Root",
  "children": [
    { "id": "c2", "name": "Child", "children": [] }
  ]
}
```
Pros: Tree structure is immediately visible. Natural for rendering.
Cons: Moving a node requires removing from one parent and inserting into another. Deep nesting in JSON.

**Pattern C: Adjacency list (DAG)**
```json
{
  "nodes": [
    { "id": "t1", "title": "Design" },
    { "id": "t2", "title": "Implement" }
  ],
  "edges": [
    { "from": "t1", "to": "t2", "type": "depends_on" }
  ]
}
```
Pros: Clean separation of nodes and edges. Easy to add edge metadata. Standard graph representation.
Cons: Two arrays to maintain. Edge validation needed.

**Pattern D: Inline dependencies (DAG)**
```json
[
  { "id": "t1", "title": "Design", "depends_on": [] },
  { "id": "t2", "title": "Implement", "depends_on": ["t1"] }
]
```
Pros: Self-contained nodes. Simple to read. Easy to serialize/deserialize.
Cons: No edge metadata without changing to objects. Removing a node requires scanning all `depends_on` lists.

**Recommendation for MindTask**:
- **Concepts**: Pattern A (flat with parent refs) — easy reparenting, simple operations
- **Tasks**: Pattern D (inline depends_on) — simpler than separate edge list, sufficient for FS dependencies

---

## 6. Critical Path Method (CPM)

**Algorithm**:
1. **Forward pass**: For each task in topological order, compute earliest start (ES) and earliest finish (EF). `ES = max(EF of all predecessors)`, `EF = ES + duration`.
2. **Backward pass**: From the end, compute latest finish (LF) and latest start (LS). `LF = min(LS of all successors)`, `LS = LF - duration`.
3. **Slack/Float**: `slack = LS - ES`. Tasks with zero slack are on the critical path.

**For MindTask**: Tasks without duration can be treated as milestones (duration=0). The critical path can be computed for the subgraph of tasks that have durations.

---

## 7. Dependency Types in Practice

| Type | Abbreviation | Meaning | Common? |
|---|---|---|---|
| Finish-to-Start | FS | B starts after A finishes | ~90% of dependencies |
| Start-to-Start | SS | B starts when A starts | Occasional |
| Finish-to-Finish | FF | B finishes when A finishes | Occasional |
| Start-to-Finish | SF | B finishes when A starts | Very rare |

**Recommendation**: Start with FS only. It covers the vast majority of real-world dependencies. If needed later, the dependency can become an object: `{ "task": "t1", "type": "FS", "lag": 0 }`.

---

## 8. Rust Ecosystem

**petgraph** (crates.io):
- The standard Rust graph library
- `DiGraph`, `StableDiGraph` — directed graphs
- `toposort()` — topological sorting
- `is_cyclic_directed()` — cycle detection
- `algo::dijkstra`, `algo::bellman_ford` — path algorithms
- Very mature, widely used

**daggy** (crates.io):
- Built on top of petgraph, specifically for DAGs
- Enforces acyclicity at the type level
- `Dag<N, E>` — nodes of type N, edges of type E
- `add_edge()` returns `WouldCycle` error if edge would create cycle
- Good fit for task dependency graphs

**serde + serde_json**:
- Standard for JSON serialization in Rust
- `#[derive(Serialize, Deserialize)]` on structs
- Handles Option<T> as nullable fields

**clap**:
- Standard CLI argument parser
- Derive-based API: `#[derive(Parser)]`
- Subcommands, help generation, completions

**uuid** (crates.io):
- UUID v4 generation
- `Uuid::new_v4().to_string()`

**Notable Rust CLI task tools**:
- `taskwarrior` (C++, but has a well-studied data model)
- `dstask` (Go, but worth studying)
- No dominant Rust-native task manager with Gantt support — this is a gap in the ecosystem

---

## 9. ID Strategy

| Strategy | CLI Ergonomics | Uniqueness | Readability |
|---|---|---|---|
| UUID v4 | Poor (`mindtask show 550e8400-e29b-41d4-a716-446655440000`) | Excellent | Poor |
| Auto-increment int | Excellent (`mindtask show 42`) | Good (within file) | Good |
| Short random | Good (`mindtask show a7x3`) | Good enough | Moderate |
| Human slug | Good (`mindtask show auth-oauth`) | Requires uniqueness check | Excellent |

**Recommendation**: Auto-increment integers for CLI ergonomics. Internally store as strings to allow future migration. Short display IDs like `c1`, `c2`, `t1`, `t2` with type prefix are even better for distinguishing concepts from tasks at a glance.
