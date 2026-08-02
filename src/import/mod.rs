//! Import a typed concept-graph JSONL stream (the pdfdex contract) into the
//! concept tree: project the `is-a` sub-DAG onto a strict tree and merge it
//! under a target concept.
//!
//! The input is one JSON object per line — nodes first, then edges:
//!
//! ```text
//! {"type":"node","id":"Parametric VaR","docs":3,"category":"finance"}
//! {"type":"edge","kind":"typed","from":"Parametric VaR","to":"Value-at-Risk","rel":"is-a","weight":2}
//! ```
//!
//! Only `is-a` edges carry hierarchy; all other lines are ignored. The `is-a`
//! slice is generally a DAG (multi-parent) and may contain LLM-asserted
//! cycles; the projection makes it a strict tree:
//!
//! 1. **Multi-parent**: keep the highest-weight parent edge (ties: the
//!    alphabetically first parent).
//! 2. **Cycles**: drop the lowest-weight edge in each cycle (ties: the
//!    alphabetically first child); that child becomes a root.
//! 3. **Roots**: concepts with no surviving parent group under an umbrella
//!    concept named after their `category`'s top-level segment; concepts
//!    without a category attach directly to the import target.
//!
//! **Merge semantics: hand curation wins.** Concepts are matched by name
//! within the target subtree. Missing ones are added (parents before
//! children); existing ones are left exactly where they are — a differing
//! projected parent is only *reported* as drift unless `reparent` is set.
//! Nothing is ever deleted, and concepts outside the target subtree are
//! never touched.

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;

use serde::Deserialize;

use crate::model::id::ConceptId;
use crate::model::project::Project;

/// Errors from parsing or applying an import.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// A line was not valid JSON.
    #[error("line {0}: invalid JSON: {1}")]
    BadLine(usize, serde_json::Error),
    /// Reading the input failed.
    #[error("read error: {0}")]
    Io(#[from] std::io::Error),
    /// The target concept does not exist.
    #[error("target concept {0} not found")]
    TargetNotFound(ConceptId),
}

/// One line of the JSONL stream. Unknown `type` values are skipped by the
/// caller (forward compatibility), so this only models what import consumes.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Line {
    Node {
        id: String,
        #[serde(default)]
        docs: usize,
        #[serde(default)]
        category: Option<String>,
    },
    Edge {
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        rel: Option<String>,
        #[serde(default)]
        weight: usize,
    },
}

/// The parsed graph: nodes and the raw `is-a` edges (child → parent).
#[derive(Debug, Default)]
pub struct GraphInput {
    /// node name → (doc count, top-level category segment).
    nodes: BTreeMap<String, (usize, Option<String>)>,
    /// (child, parent, weight) `is-a` assertions.
    is_a: Vec<(String, String, usize)>,
}

/// Parse the JSONL stream. Lines whose `type` is unknown are ignored; edges
/// other than typed `is-a` are ignored. Malformed JSON is an error.
pub fn parse_jsonl(reader: impl BufRead) -> Result<GraphInput, ImportError> {
    let mut input = GraphInput::default();
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        // Tolerate unknown line types (forward compatibility with new
        // emitters); only genuinely malformed JSON is an error.
        let parsed: Result<Line, _> = serde_json::from_str(&line);
        match parsed {
            Ok(Line::Node { id, docs, category }) => {
                let top = category
                    .as_deref()
                    .and_then(|c| c.split('/').next())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                input.nodes.insert(id, (docs, top));
            }
            Ok(Line::Edge {
                from: Some(from),
                to: Some(to),
                rel: Some(rel),
                weight,
            }) if rel == "is-a" => {
                input.is_a.push((from, to, weight));
            }
            Ok(Line::Edge { .. }) => {} // other edge kinds/relations: ignored
            Err(e) => {
                // Distinguish "unknown type tag" (skip) from broken JSON (error).
                if serde_json::from_str::<serde_json::Value>(&line).is_ok() {
                    continue;
                }
                return Err(ImportError::BadLine(i + 1, e));
            }
        }
    }
    Ok(input)
}

/// One planned concept in parents-before-children order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedConcept {
    /// Concept name (node id from the stream, or an umbrella category name).
    pub name: String,
    /// Projected parent name; `None` = directly under the import target.
    pub parent: Option<String>,
    /// Doc count from the stream (0 for umbrella concepts).
    pub docs: usize,
    /// True for synthetic category umbrella concepts.
    pub umbrella: bool,
}

/// The strict-tree projection of the `is-a` slice, plus what it cost.
#[derive(Debug, Default)]
pub struct Projection {
    /// Concepts in insertion order (every parent precedes its children).
    pub concepts: Vec<PlannedConcept>,
    /// (child, dropped parent) edges removed by multi-parent resolution.
    pub multi_parent_dropped: Vec<(String, String)>,
    /// (child, parent) edges removed to break cycles.
    pub cycle_dropped: Vec<(String, String)>,
}

/// Project the `is-a` sub-DAG onto a strict forest. Only concepts touched by
/// at least one surviving `is-a` edge are included (plus their umbrellas);
/// isolated nodes stay out of the tree. `min_docs` drops weak nodes first.
pub fn project_tree(input: &GraphInput, min_docs: usize) -> Projection {
    let mut proj = Projection::default();

    // Nodes eligible by doc count. Edge endpoints missing from the node list
    // (defensive) count as docs=0, category unknown.
    let node_info = |name: &str| -> (usize, Option<String>) {
        input.nodes.get(name).cloned().unwrap_or((0, None))
    };
    let eligible = |name: &str| -> bool { min_docs <= 1 || node_info(name).0 >= min_docs };

    // 1. Multi-parent resolution: best edge per child (weight desc, parent asc).
    let mut best: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for (child, parent, weight) in &input.is_a {
        if child == parent || !eligible(child) || !eligible(parent) {
            continue;
        }
        match best.get(child.as_str()) {
            Some(&(cur_parent, cur_w))
                if cur_w > *weight || (cur_w == *weight && cur_parent <= parent.as_str()) =>
            {
                proj.multi_parent_dropped
                    .push((child.clone(), parent.clone()));
            }
            Some(&(cur_parent, _)) => {
                proj.multi_parent_dropped
                    .push((child.clone(), cur_parent.to_string()));
                best.insert(child, (parent, *weight));
            }
            None => {
                best.insert(child, (parent, *weight));
            }
        }
    }

    // 2. Cycle breaking on the child→parent functional graph: walk each chain;
    // when a walk returns to an in-progress node, drop the weakest edge in the
    // cycle (ties: alphabetically first child).
    let mut parent_of: BTreeMap<String, (String, usize)> = best
        .into_iter()
        .map(|(c, (p, w))| (c.to_string(), (p.to_string(), w)))
        .collect();
    let children: Vec<String> = parent_of.keys().cloned().collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    for start in &children {
        if done.contains(start) {
            continue;
        }
        let mut path: Vec<String> = Vec::new();
        let mut cur = start.clone();
        loop {
            if done.contains(&cur) {
                break;
            }
            if let Some(pos) = path.iter().position(|n| n == &cur) {
                // Cycle: path[pos..] loops back to `cur`. Drop its weakest edge.
                let cycle = &path[pos..];
                let victim = cycle
                    .iter()
                    .min_by_key(|n| (parent_of[n.as_str()].1, n.as_str()))
                    .expect("cycle is non-empty")
                    .clone();
                let (vp, _) = parent_of.remove(&victim).expect("victim has a parent");
                proj.cycle_dropped.push((victim, vp));
                break;
            }
            path.push(cur.clone());
            match parent_of.get(&cur) {
                Some((p, _)) => cur = p.clone(),
                None => break,
            }
        }
        done.extend(path);
    }

    // 3. Assemble the forest: every node touched by a surviving edge.
    let mut in_tree: BTreeSet<&str> = BTreeSet::new();
    for (c, (p, _)) in &parent_of {
        in_tree.insert(c);
        in_tree.insert(p);
    }
    // Roots (no parent) group under a category umbrella when they have one.
    let mut umbrellas: BTreeSet<String> = BTreeSet::new();
    let mut planned: BTreeMap<&str, PlannedConcept> = BTreeMap::new();
    for &name in &in_tree {
        let (docs, category) = node_info(name);
        let parent = match parent_of.get(name) {
            Some((p, _)) => Some(p.clone()),
            None => category.inspect(|c| {
                umbrellas.insert(c.clone());
            }),
        };
        planned.insert(
            name,
            PlannedConcept {
                name: name.to_string(),
                parent,
                docs,
                umbrella: false,
            },
        );
    }

    // 4. Emit umbrellas first, then the forest parents-before-children (DFS
    // from roots, children in name order).
    for u in &umbrellas {
        proj.concepts.push(PlannedConcept {
            name: u.clone(),
            parent: None,
            docs: 0,
            umbrella: true,
        });
    }
    let mut children_of: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut roots: Vec<&str> = Vec::new();
    for (name, pc) in &planned {
        match pc.parent.as_deref() {
            Some(p) if planned.contains_key(p) => {
                children_of.entry(p).or_default().push(name);
            }
            _ => roots.push(name),
        }
    }
    let mut stack: Vec<&str> = roots.into_iter().rev().collect();
    while let Some(name) = stack.pop() {
        proj.concepts.push(planned[name].clone());
        if let Some(kids) = children_of.get(name) {
            for k in kids.iter().rev() {
                stack.push(k);
            }
        }
    }
    proj
}

/// What `apply` did (or would do, on a dry run).
#[derive(Debug, Default)]
pub struct ImportReport {
    /// Names added, in insertion order.
    pub added: Vec<String>,
    /// Names that already existed in the subtree and were left untouched.
    pub kept: Vec<String>,
    /// Existing concepts whose projected parent differs from their actual one:
    /// `(name, actual parent, projected parent)`.
    pub drift: Vec<(String, String, String)>,
    /// Names re-parented (only with `reparent`).
    pub reparented: Vec<String>,
}

/// Merge a projection into the tree under `target`. Matching is by name
/// within the target's subtree; see the module docs for the semantics.
pub fn apply(
    project: &mut Project,
    target: ConceptId,
    projection: &Projection,
    reparent: bool,
) -> Result<ImportReport, ImportError> {
    if project.get_concept(target).is_none() {
        return Err(ImportError::TargetNotFound(target));
    }

    // Name → id for every concept currently in the target's subtree.
    let mut by_name: BTreeMap<String, ConceptId> = BTreeMap::new();
    let mut stack = vec![target];
    while let Some(id) = stack.pop() {
        for child in project.children_of(id) {
            by_name.insert(child.name.clone(), child.id);
            stack.push(child.id);
        }
    }

    let mut report = ImportReport::default();
    for pc in &projection.concepts {
        // Projected parent id: the named parent if planned/known, else target.
        let parent_id = pc
            .parent
            .as_ref()
            .and_then(|p| by_name.get(p))
            .copied()
            .unwrap_or(target);
        match by_name.get(&pc.name) {
            Some(&existing) => {
                let actual = project
                    .get_concept(existing)
                    .and_then(|c| c.parent)
                    .unwrap_or(target);
                if actual != parent_id {
                    let actual_name = project
                        .get_concept(actual)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    let projected_name = project
                        .get_concept(parent_id)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    if reparent && project.move_concept(existing, Some(parent_id)).is_ok() {
                        report.reparented.push(pc.name.clone());
                    } else {
                        report
                            .drift
                            .push((pc.name.clone(), actual_name, projected_name));
                    }
                } else {
                    report.kept.push(pc.name.clone());
                }
            }
            None => {
                let description = if pc.umbrella {
                    Some("pdfdex import: category umbrella".to_string())
                } else {
                    Some(format!("pdfdex import: {} docs", pc.docs))
                };
                let id = project
                    .add_concept(pc.name.clone(), Some(parent_id), description)
                    .expect("parent id verified above");
                by_name.insert(pc.name.clone(), id);
                report.added.push(pc.name.clone());
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(s: &str) -> GraphInput {
        parse_jsonl(Cursor::new(s)).unwrap()
    }

    const SIMPLE: &str = r#"
{"type":"node","id":"Parametric VaR","docs":3,"category":"finance/risk"}
{"type":"node","id":"Value-at-Risk","docs":9,"category":"finance/risk"}
{"type":"node","id":"Isolated","docs":5,"category":"statistics"}
{"type":"edge","kind":"cooccur","a":"x","b":"y","weight":4}
{"type":"edge","kind":"typed","from":"Parametric VaR","to":"Value-at-Risk","rel":"is-a","weight":2}
{"type":"edge","kind":"typed","from":"Parametric VaR","to":"Value-at-Risk","rel":"uses","weight":9}
"#;

    #[test]
    fn parse_keeps_only_is_a_edges_and_all_nodes() {
        let g = parse(SIMPLE);
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.is_a.len(), 1);
        assert_eq!(g.nodes["Parametric VaR"], (3, Some("finance".into())));
    }

    #[test]
    fn parse_rejects_malformed_json_but_skips_unknown_types() {
        assert!(parse_jsonl(Cursor::new("{not json}")).is_err());
        let g = parse(r#"{"type":"meta","hello":1}"#);
        assert!(g.nodes.is_empty());
    }

    #[test]
    fn projection_groups_roots_under_category_umbrella() {
        let g = parse(SIMPLE);
        let p = project_tree(&g, 1);
        // Umbrella "finance" first, then Value-at-Risk under it, then the child.
        let names: Vec<(&str, Option<&str>)> = p
            .concepts
            .iter()
            .map(|c| (c.name.as_str(), c.parent.as_deref()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("finance", None),
                ("Value-at-Risk", Some("finance")),
                ("Parametric VaR", Some("Value-at-Risk")),
            ]
        );
        // The isolated node stays out.
        assert!(!p.concepts.iter().any(|c| c.name == "Isolated"));
    }

    #[test]
    fn multi_parent_keeps_highest_weight() {
        let g = parse(
            r#"
{"type":"node","id":"VPIN","docs":2,"category":"finance"}
{"type":"node","id":"Volume Measure","docs":3}
{"type":"node","id":"Toxicity Metric","docs":3}
{"type":"edge","kind":"typed","from":"VPIN","to":"Volume Measure","rel":"is-a","weight":1}
{"type":"edge","kind":"typed","from":"VPIN","to":"Toxicity Metric","rel":"is-a","weight":3}
"#,
        );
        let p = project_tree(&g, 1);
        let vpin = p.concepts.iter().find(|c| c.name == "VPIN").unwrap();
        assert_eq!(vpin.parent.as_deref(), Some("Toxicity Metric"));
        assert_eq!(
            p.multi_parent_dropped,
            vec![("VPIN".into(), "Volume Measure".into())]
        );
    }

    #[test]
    fn cycle_is_broken_at_weakest_edge() {
        let g = parse(
            r#"
{"type":"node","id":"A","docs":1}
{"type":"node","id":"B","docs":1}
{"type":"node","id":"C","docs":1}
{"type":"edge","kind":"typed","from":"A","to":"B","rel":"is-a","weight":3}
{"type":"edge","kind":"typed","from":"B","to":"C","rel":"is-a","weight":2}
{"type":"edge","kind":"typed","from":"C","to":"A","rel":"is-a","weight":1}
"#,
        );
        let p = project_tree(&g, 1);
        // Weakest edge C→A dropped: C becomes root, chain A→B→C survives.
        assert_eq!(p.cycle_dropped, vec![("C".into(), "A".into())]);
        let c = p.concepts.iter().find(|c| c.name == "C").unwrap();
        assert_eq!(c.parent, None);
        let a = p.concepts.iter().find(|c| c.name == "A").unwrap();
        assert_eq!(a.parent.as_deref(), Some("B"));
    }

    #[test]
    fn min_docs_prunes_weak_nodes() {
        let g = parse(SIMPLE);
        let p = project_tree(&g, 5);
        // Parametric VaR (3 docs) is pruned; nothing is left connected.
        assert!(p.concepts.is_empty());
    }

    fn project_with_target() -> (Project, ConceptId) {
        let mut proj = Project::new();
        let root = proj.add_concept("pdf-library".into(), None, None).unwrap();
        (proj, root)
    }

    #[test]
    fn apply_adds_parents_before_children_and_is_idempotent() {
        let (mut proj, root) = project_with_target();
        let g = parse(SIMPLE);
        let plan = project_tree(&g, 1);

        let r1 = apply(&mut proj, root, &plan, false).unwrap();
        assert_eq!(r1.added, vec!["finance", "Value-at-Risk", "Parametric VaR"]);
        assert!(r1.kept.is_empty());
        assert!(crate::graph::tree::validate_tree(&proj).is_ok());

        // Second run: everything matches by name, nothing changes.
        let r2 = apply(&mut proj, root, &plan, false).unwrap();
        assert!(r2.added.is_empty());
        assert_eq!(r2.kept.len(), 3);
        assert_eq!(proj.concepts.len(), 4); // root + 3 imported
    }

    #[test]
    fn apply_reports_drift_but_keeps_hand_curation() {
        let (mut proj, root) = project_with_target();
        let g = parse(SIMPLE);
        let plan = project_tree(&g, 1);
        apply(&mut proj, root, &plan, false).unwrap();

        // Dan moves "Parametric VaR" directly under the root by hand.
        let var = proj
            .concepts
            .iter()
            .find(|c| c.name == "Parametric VaR")
            .unwrap()
            .id;
        proj.move_concept(var, Some(root)).unwrap();

        let r = apply(&mut proj, root, &plan, false).unwrap();
        assert_eq!(r.drift.len(), 1);
        assert_eq!(r.drift[0].0, "Parametric VaR");
        // Still where Dan put it.
        assert_eq!(proj.get_concept(var).unwrap().parent, Some(root));

        // With reparent, the projection wins.
        let r = apply(&mut proj, root, &plan, true).unwrap();
        assert_eq!(r.reparented, vec!["Parametric VaR"]);
        assert_ne!(proj.get_concept(var).unwrap().parent, Some(root));
    }

    #[test]
    fn apply_never_touches_concepts_outside_the_target() {
        let (mut proj, root) = project_with_target();
        // A hand-curated concept with a colliding name OUTSIDE the target.
        proj.add_concept("Value-at-Risk".into(), None, Some("mine".into()))
            .unwrap();
        let g = parse(SIMPLE);
        let plan = project_tree(&g, 1);
        let r = apply(&mut proj, root, &plan, false).unwrap();
        // A fresh one is added inside the subtree; the outside one is untouched.
        assert!(r.added.contains(&"Value-at-Risk".to_string()));
        let outside = proj
            .concepts
            .iter()
            .find(|c| c.description.as_deref() == Some("mine"))
            .unwrap();
        assert_eq!(outside.parent, None);
    }

    #[test]
    fn apply_missing_target_errors() {
        let mut proj = Project::new();
        let plan = Projection::default();
        assert!(matches!(
            apply(&mut proj, ConceptId(9), &plan, false),
            Err(ImportError::TargetNotFound(_))
        ));
    }
}
