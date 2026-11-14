//! The sensing half of the design-system scan: which variables each component binds, which components each
//! component and each page frame uses, and the page frames themselves.
//!
//! Rules (the Figma plugin applies the same ones):
//! - **Instance boundary.** A walk goes through frames, groups and sections and stops at an instance: the instance
//!   node itself is read and counted, its descendants are not visited. What is inside a nested instance belongs to
//!   that component's own entry, and instance content is most of the nodes of a real file. Known loss: a variable
//!   bound as an override on a node inside a nested instance is not sensed.
//! - **Bindings.** Every value of `Node::variables.bindings` of a visited node: the importers flatten each place
//!   Figma keeps a binding (node fields, fills and strokes item by item including gradient stops, effects, layout
//!   grids, text, `componentProperties/<name>`) into that one map, and variable-bound defaults of component property
//!   definitions live in `ComponentLink::prop_default_variables`. A binding to a variable of a remote collection
//!   is listed by id like any other. Not sensed: variables reached only through a style applied to a node.
//! - **Frames.** Every frame that is a direct child of a page, or of a section at any depth under a page. The walk of
//!   a frame reads no binding, and a component or set met inside it is a boundary: neither counted nor entered.
//! - **No self-reference.** A component or set never lists itself in its own `instances` (a variant using a sibling
//!   variant of its set resolves to the set): the count carries nothing for the closure and would only make a cycle.
//! - **Resolving an instance.** A local main component gives the id of its set when it is a variant, else its own
//!   id; a library main component gives the key of its set when known, else its own key, or its Figma id when it is an
//!   unpublished symbol on the file's internal canvas. An instance whose main
//!   component cannot be resolved has no `component` and is counted in `unresolvedInstances`.
//!
//! The functions return `Err(reason)` instead of a partial answer: a half-walked tree would assert a list the scan
//! never fully saw. The caller then leaves every sensed field out.

use crate::design_system_scan::{component_key, figma_id, is_missing_symbol_stand_in, page_ref, PageRef};
use designos_doc::{is_library_node, Document, Node, NodeId, NodeKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstanceCount {
    pub component: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanFrame {
    pub id: String,
    pub name: String,
    pub page: PageRef,
    /// Innermost enclosing section; absent for a direct child of the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub w: serde_json::Value,
    pub h: serde_json::Value,
    pub instances: Vec<InstanceCount>,
    /// Written only when above zero.
    #[serde(rename = "unresolvedInstances", skip_serializing_if = "Option::is_none")]
    pub unresolved_instances: Option<usize>,
}

/// Bound variable ids, used components and the number of instances that could not be resolved, for one component.
pub(crate) struct ComponentSensed {
    pub bound: Vec<String>,
    pub instances: Vec<InstanceCount>,
    pub unresolved: usize,
}

pub(crate) struct Sensed {
    /// One entry per component root, in the order the roots were given: sorted bound ids and instance counts.
    pub components: Vec<ComponentSensed>,
    pub frames: Vec<ScanFrame>,
}

/// What one walk under the instance boundary found.
struct Walked {
    bound: BTreeSet<String>,
    instances: Vec<NodeId>,
}

/// Which kind of root a walk starts from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// A component or set: its bindings are read.
    Component,
    /// A page frame: no binding is read, and a component or set met on the way is a boundary (it is neither counted
    /// nor entered: what is inside belongs to that component's own entry).
    Frame,
}

/// Visits `root` and everything below it, but not below an instance other than `root` itself.
fn walk_to_instance_boundary(doc: &Document, root: NodeId, walk: Walk) -> Result<Walked, String> {
    let mut walked = Walked { bound: BTreeSet::new(), instances: Vec::new() };
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let n = doc.get(id).ok_or_else(|| format!("node {} is listed as a child but is missing from the document", id.0))?;
        if walk == Walk::Frame && id != root && matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
            continue;
        }
        if walk == Walk::Component {
            walked.bound.extend(n.variables.bindings.values().cloned());
            for p in &n.component.props {
                if let Some(var) = n.component.prop_default_variables.get(&p.name) {
                    walked.bound.insert(var.clone());
                }
            }
        }
        if matches!(n.kind, NodeKind::Instance { .. }) && id != root {
            walked.instances.push(id);
            continue;
        }
        stack.extend(n.children.iter().rev().copied());
    }
    Ok(walked)
}

/// The `component` value of the main component of `inst`, `Ok(None)` when it cannot be named.
fn resolve_instance(doc: &Document, inst: &Node) -> Result<Option<String>, String> {
    let NodeKind::Instance { component, .. } = &inst.kind else { return Ok(None) };
    let Some(main) = component.and_then(|c| doc.get(c)) else { return Ok(None) };
    if !matches!(main.kind, NodeKind::Component(_)) {
        return Ok(None);
    }
    let set = main.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
    if is_library_node(doc, main.id) {
        let inst_key = || match &inst.kind {
            NodeKind::Instance { component_key: Some(k), .. } if !k.is_empty() && !is_missing_symbol_stand_in(main, k) => Some(k.clone()),
            _ => None,
        };
        let own = || component_key(doc, main).or_else(inst_key);
        if let Some(key) = set.and_then(|s| component_key(doc, s)).or_else(own) {
            return Ok(Some(key));
        }
        return Ok(set.unwrap_or(main).source_id.clone().filter(|s| !s.is_empty()));
    }
    // A local component with no parent was removed from the document: it names nothing.
    if main.parent.is_none() {
        return Ok(None);
    }
    figma_id(set.unwrap_or(main), "component").map(Some).map_err(|e| e.0)
}

/// Counts per component, sorted by `component`, and how many instances had none.
fn count_instances(doc: &Document, instances: &[NodeId]) -> Result<(Vec<InstanceCount>, usize), String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unresolved = 0;
    for &id in instances {
        let inst = doc.get(id).ok_or_else(|| format!("instance {} is missing from the document", id.0))?;
        match resolve_instance(doc, inst)? {
            Some(c) => *counts.entry(c).or_default() += 1,
            None => unresolved += 1,
        }
    }
    Ok((counts.into_iter().map(|(component, count)| InstanceCount { component, count }).collect(), unresolved))
}

/// Frames that are direct children of a page or of a section at any depth under it, in document order, each with
/// its page and the name of its innermost enclosing section.
fn frame_roots(doc: &Document) -> Vec<(NodeId, NodeId, Option<String>)> {
    let mut out = Vec::new();
    for &page in &doc.pages {
        let Some(p) = doc.get(page) else { continue };
        // Pre-order: a section's frames come where the section stands among its siblings.
        let mut stack: Vec<(NodeId, Option<String>)> = p.children.iter().rev().map(|&c| (c, None)).collect();
        while let Some((id, section)) = stack.pop() {
            let Some(n) = doc.get(id) else { continue };
            match n.kind {
                NodeKind::Frame(_) => out.push((page, id, section)),
                NodeKind::Section => stack.extend(n.children.iter().rev().map(|&c| (c, Some(n.name.clone())))),
                _ => {}
            }
        }
    }
    out
}

fn round2(x: f32) -> serde_json::Value {
    let r = (f64::from(x) * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        serde_json::Value::from(r as i64)
    } else {
        serde_json::Number::from_f64(r).map_or(serde_json::Value::Null, serde_json::Value::Number)
    }
}

/// Senses the component roots and the page frames of `doc`.
pub(crate) fn sense(doc: &Document, component_roots: &[NodeId]) -> Result<Sensed, String> {
    let mut components = Vec::with_capacity(component_roots.len());
    for &root in component_roots {
        let walked = walk_to_instance_boundary(doc, root, Walk::Component)?;
        let (mut instances, unresolved) = count_instances(doc, &walked.instances)?;
        let own = doc.get(root).and_then(|n| n.source_id.as_deref());
        instances.retain(|i| Some(i.component.as_str()) != own);
        components.push(ComponentSensed { bound: walked.bound.into_iter().collect(), instances, unresolved });
    }
    let mut frames = Vec::new();
    for (page, frame, section) in frame_roots(doc) {
        let n = doc.get(frame).ok_or_else(|| format!("frame {} is missing from the document", frame.0))?;
        let walked = walk_to_instance_boundary(doc, frame, Walk::Frame)?;
        let (instances, unresolved) = count_instances(doc, &walked.instances)?;
        frames.push(ScanFrame {
            id: figma_id(n, "frame").map_err(|e| e.0)?,
            name: n.name.clone(),
            page: page_ref(doc, page).map_err(|e| e.0)?,
            section,
            w: round2(n.size.x),
            h: round2(n.size.y),
            instances,
            unresolved_instances: (unresolved > 0).then_some(unresolved),
        });
    }
    Ok(Sensed { components, frames })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_walk_reads_no_binding_but_the_component_walk_does() {
        let mut doc = Document::new("walk");
        let page = doc.add_page("P");
        let mut ids = Vec::new();
        for (kind, name) in [(NodeKind::Frame(Default::default()), "Frame"), (NodeKind::Component(Default::default()), "Comp")] {
            let id = doc.alloc_id();
            let mut n = Node::new(id, kind);
            n.name = name.into();
            n.variables.bindings.insert("fills/0".into(), format!("v-{name}"));
            ids.push(doc.insert(page, n).unwrap());
        }
        assert!(walk_to_instance_boundary(&doc, ids[0], Walk::Frame).unwrap().bound.is_empty(), "a frame's bindings are never read");
        assert_eq!(walk_to_instance_boundary(&doc, ids[1], Walk::Component).unwrap().bound.len(), 1);
    }
}
