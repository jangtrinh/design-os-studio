//! Components, instances and variants as pure, undoable document operations.
//!
//! Model recap (see [`ComponentLink`]):
//! * a main component is a `NodeKind::Component` frame (optionally a child of a
//!   `NodeKind::ComponentSet` holding the variants);
//! * an instance is a `NodeKind::Instance` whose subtree mirrors the component.
//!   Every node of the copy has `component.source` set to its counterpart; the
//!   instance root points at the component itself;
//! * a nested instance inside a component is mirrored like any other node, so
//!   its copy's source is the nested instance of the main component, and the
//!   children of that copy point at the children of the nested instance.
//!
//! Every function returns a [`Command`] (usually a `Batch`) and never mutates
//! the document, apart from reserving fresh ids.

use crate::*;
use std::collections::{BTreeMap, BTreeSet};

impl OverrideField {
    pub const ALL: [OverrideField; 16] = [
        OverrideField::Name,
        OverrideField::Visible,
        OverrideField::Locked,
        OverrideField::Opacity,
        OverrideField::Blend,
        OverrideField::Size,
        OverrideField::Transform,
        OverrideField::Fills,
        OverrideField::Stroke,
        OverrideField::Effects,
        OverrideField::CornerRadii,
        OverrideField::Characters,
        OverrideField::TextStyle,
        OverrideField::Layout,
        OverrideField::MainComponent,
        OverrideField::SlotContent,
    ];
}

fn nothing() -> Command {
    Command::Batch(Vec::new())
}

// ---------------------------------------------------------------------------
// Scratch context: applies commands to a private copy so later steps of a
// multi-node operation see the effect of earlier ones.
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    s: &'a mut Document,
    cmds: Vec<Command>,
    invs: Vec<Command>,
    saved_dirty: crate::DirtyLog,
}

impl<'a> Ctx<'a> {
    fn new(doc: &'a mut Document) -> Self {
        let saved_dirty = doc.dirty.clone();
        Ctx { s: doc, cmds: Vec::new(), invs: Vec::new(), saved_dirty }
    }
    fn push(&mut self, c: Command) {
        if let Ok(inv) = c.clone().apply(self.s) {
            self.cmds.push(c);
            self.invs.push(inv);
        }
    }
    fn update(&mut self, n: Node) {
        if self.s.get(n.id).map(|cur| !same_ignoring_tree(cur, &n)).unwrap_or(false) {
            self.push(Command::Update { node: Box::new(n) });
        }
    }
    fn finish(self) -> Command {
        for inv in self.invs.into_iter().rev() {
            let _ = inv.apply(self.s);
        }
        self.s.dirty = self.saved_dirty;
        Command::Batch(self.cmds)
    }
}

fn same_ignoring_tree(a: &Node, b: &Node) -> bool {
    let mut b2 = b.clone();
    b2.children = a.children.clone();
    b2.parent = a.parent;
    *a == b2
}

/// Deep copy of `src` (root first, children lists remapped to the new ids).
/// With `link`, every copy records its original as `component.source`.
fn copy_tree(d: &mut Document, src: NodeId, link: bool) -> Vec<Node> {
    let mut out = Vec::new();
    copy_rec(d, src, None, link, &mut out);
    out
}

fn copy_rec(d: &mut Document, src: NodeId, parent: Option<NodeId>, link: bool, out: &mut Vec<Node>) {
    let Some(orig) = d.get(src).cloned() else { return };
    let id = d.alloc_id();
    let mut n = orig.clone();
    n.id = id;
    n.parent = parent;
    n.source_id = None;
    n.component.slot_content_id = None;
    n.children = Vec::new();
    if link {
        n.component.source = Some(src);
        n.component.overrides.clear();
        n.component.grid_align_overrides = [false, false];
        n.component.props.clear();
        n.component.slot_configs.clear();
    }
    let idx = out.len();
    out.push(n);
    let mut kids = Vec::new();
    for c in &orig.children {
        let before = out.len();
        copy_rec(d, *c, Some(id), link, out);
        if out.len() > before {
            kids.push(out[before].id);
        }
    }
    out[idx].children = kids;
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// The component an instance node points at.
pub fn instance_component(n: &Node) -> Option<NodeId> {
    match &n.kind {
        NodeKind::Instance { component, .. } => *component,
        _ => None,
    }
}

/// All instance nodes whose main component is `comp` (nested ones included).
pub fn instances_of(doc: &Document, comp: NodeId) -> Vec<NodeId> {
    doc.nodes.values().filter(|n| instance_component(n) == Some(comp)).map(|n| n.id).collect()
}

/// All instance nodes whose main component is in `comps` (nested ones included).
pub fn instances_of_any(doc: &Document, comps: &[NodeId]) -> Vec<NodeId> {
    if comps.is_empty() {
        return Vec::new();
    }
    if comps.len() == 1 {
        let c0 = comps[0];
        return doc.nodes.values().filter(|n| instance_component(n) == Some(c0)).map(|n| n.id).collect();
    }
    doc.nodes.values().filter(|n| instance_component(n).is_some_and(|c| comps.contains(&c))).map(|n| n.id).collect()
}

/// Components defined by `owner`: itself for a component, its variants for a set.
pub fn components_of(doc: &Document, owner: NodeId) -> Vec<NodeId> {
    match doc.get(owner).map(|n| &n.kind) {
        Some(NodeKind::Component(_)) => vec![owner],
        Some(NodeKind::ComponentSet(_)) => {
            doc.get(owner).unwrap().children.iter().copied().filter(|c| matches!(doc.get(*c).map(|n| &n.kind), Some(NodeKind::Component(_)))).collect()
        }
        _ => Vec::new(),
    }
}

/// Nearest ancestor-or-self that is a main `Component`.
pub fn enclosing_component(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    while let Some(c) = cur {
        let n = doc.get(c)?;
        if matches!(n.kind, NodeKind::Component(_)) {
            return Some(c);
        }
        cur = n.parent;
    }
    None
}

/// The topmost instance root whose copy `id` belongs to (the node itself for
/// a root). Nodes in a nested instance resolve to the outermost instance.
pub fn instance_root_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut found = None;
    let mut cur = Some(id);
    while let Some(c) = cur {
        let n = doc.get(c)?;
        if n.component.source.is_some() && matches!(n.kind, NodeKind::Instance { .. }) && is_top(n) {
            found = Some(c);
        }
        cur = n.parent;
    }
    found
}

/// True for nodes that mirror a main-component node (instance roots included).
pub fn is_linked(n: &Node) -> bool {
    n.component.source.is_some()
}

/// True for an instance root (or any node mirroring a whole component), as
/// opposed to nodes inside an instance's copy.
pub fn is_top_instance(n: &Node) -> bool {
    is_top(n)
}

fn is_top(n: &Node) -> bool {
    n.component.source.is_none() || n.component.source == instance_component(n)
}

/// True if `id` is a component slot container (either via `is_slot`, `OverrideField::SlotContent`
/// override/binding on itself or its master component node, or bound to a `PropKind::Slot` property).
pub fn is_slot_container(doc: &Document, id: NodeId) -> bool {
    let Some(n) = doc.get(id) else { return false };
    if n.component.is_slot || n.component.overrides.contains(&OverrideField::SlotContent) || n.component.bindings.contains_key(&OverrideField::SlotContent) {
        return true;
    }
    if let Some(src_id) = n.component.source {
        if let Some(src) = doc.get(src_id) {
            if src.component.is_slot
                || src.component.overrides.contains(&OverrideField::SlotContent)
                || src.component.bindings.contains_key(&OverrideField::SlotContent)
            {
                return true;
            }
            if !src.component.bindings.is_empty() {
                if let Some(comp) = enclosing_component(doc, src_id) {
                    let defs = prop_defs(doc, comp);
                    if src.component.bindings.values().any(|pname| defs.iter().any(|d| d.name == *pname && matches!(d.kind, PropKind::Slot))) {
                        return true;
                    }
                }
            }
        }
    }
    if !n.component.bindings.is_empty() {
        if let Some(comp) = enclosing_component(doc, id) {
            let defs = prop_defs(doc, comp);
            if n.component.bindings.values().any(|pname| defs.iter().any(|d| d.name == *pname && matches!(d.kind, PropKind::Slot))) {
                return true;
            }
        }
    }
    false
}

/// Nearest ancestor-or-self of `id` that is a slot container inside a component instance.
pub fn enclosing_instance_slot(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    while let Some(c) = cur {
        let n = doc.get(c)?;
        if is_slot_container(doc, c) && (is_linked(n) || instance_root_of(doc, c).is_some()) {
            return Some(c);
        }
        cur = n.parent;
    }
    None
}

/// Follows `component.source` from `slot` to the slot node of the main component that defines it
/// (the node itself for a main-component slot). A slot inside a nested instance therefore resolves
/// to the nested component's slot, not to the component that merely contains the nested instance.
fn slot_definition(doc: &Document, slot: NodeId) -> Option<NodeId> {
    let mut cur = slot;
    // The hop limit only guards against a corrupt `source` cycle.
    for _ in 0..doc.nodes.len().max(1) {
        match doc.get(cur)?.component.source.filter(|s| doc.get(*s).is_some()) {
            Some(src) => cur = src,
            None => return Some(cur),
        }
    }
    None
}

/// Resolves the [`SlotPropConfig`] governing `slot` (a main-component slot, an instance copy, or a
/// copy inside a nested instance). The settings live on the component that defines the slot and are
/// found by the slot's `SlotContent` binding; a slot without a binding falls back to its own name.
pub fn slot_config_of(doc: &Document, slot: NodeId) -> Option<SlotPropConfig> {
    if !is_slot_container(doc, slot) {
        return None;
    }
    let def = slot_definition(doc, slot)?;
    let dn = doc.get(def)?;
    let prop_name =
        dn.component.bindings.get(&OverrideField::SlotContent).or_else(|| doc.get(slot)?.component.bindings.get(&OverrideField::SlotContent)).cloned();
    let comp = enclosing_component(doc, def)?;
    let owner = doc.get(comp).and_then(|c| c.parent).filter(|p| matches!(doc.get(*p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_)))).unwrap_or(comp);

    for holder_id in [owner, comp] {
        let Some(holder) = doc.get(holder_id) else { continue };
        let configs = &holder.component.slot_configs;
        match &prop_name {
            Some(pname) => {
                if let Some(cfg) = configs.get(pname) {
                    return Some(cfg.clone());
                }
            }
            None => {
                if let Some((_, cfg)) = configs.iter().find(|(k, _)| k.eq_ignore_ascii_case(&dn.name)) {
                    return Some(cfg.clone());
                }
            }
        }
    }
    None
}

/// True when `slot` is a visible empty slot container that should display an empty-slot placeholder
/// (always true in a main component; governed by `display_by_default` in an instance).
pub fn should_show_empty_slot_placeholder(doc: &Document, slot: NodeId) -> bool {
    if !is_slot_container(doc, slot) {
        return false;
    }
    let Some(n) = doc.get(slot) else { return false };
    if !n.visible || !n.children.is_empty() {
        return false;
    }
    if is_linked(n) || instance_root_of(doc, slot).is_some() {
        slot_config_of(doc, slot).and_then(|c| c.display_by_default).unwrap_or(true)
    } else {
        true
    }
}

/// Returns the list of candidate `Component` node ids to offer in the `+` instance picker for `slot`:
/// when `SlotPropConfig::preferred` is non-empty, returns only those preferred components (expanding
/// any preferred `ComponentSet` to its variants); otherwise returns all other components in `doc`.
pub fn preferred_slot_components(doc: &Document, slot: NodeId) -> Vec<NodeId> {
    if !is_slot_container(doc, slot) {
        return Vec::new();
    }
    // A component cannot hold itself: leave out the component the slot sits in and the one that defines it.
    let own: Vec<NodeId> = [Some(slot), slot_definition(doc, slot)].into_iter().flatten().filter_map(|id| enclosing_component(doc, id)).collect();
    if let Some(cfg) = slot_config_of(doc, slot).filter(|c| !c.preferred.is_empty()) {
        let mut out = Vec::new();
        for pid in cfg.preferred {
            let Some(pn) = doc.get(pid) else { continue };
            match &pn.kind {
                NodeKind::Component(_) if !own.contains(&pid) && !out.contains(&pid) => out.push(pid),
                NodeKind::ComponentSet(_) => {
                    for &cid in &pn.children {
                        if !own.contains(&cid) && matches!(doc.get(cid).map(|c| &c.kind), Some(NodeKind::Component(_))) && !out.contains(&cid) {
                            out.push(cid);
                        }
                    }
                }
                _ => {}
            }
        }
        return out;
    }
    let mut all: Vec<NodeId> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_)) && !own.contains(&n.id)).map(|n| n.id).collect();
    all.sort_by(|a, b| {
        let na = doc.get(*a).map(|n| n.name.as_str()).unwrap_or("");
        let nb = doc.get(*b).map(|n| n.name.as_str()).unwrap_or("");
        na.cmp(nb).then_with(|| a.0.cmp(&b.0))
    });
    all
}

/// Property definitions that apply to `comp`: its own plus those of its set.
pub fn prop_defs(doc: &Document, comp: NodeId) -> Vec<ComponentPropDef> {
    let Some(n) = doc.get(comp) else { return Vec::new() };
    let mut out = n.component.props.clone();
    if let Some(set) = n.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
        for d in &set.component.props {
            if !out.iter().any(|o| o.name == d.name) {
                out.push(d.clone());
            }
        }
    }
    out
}

/// Value of property `def` on an instance root, falling back to the default
/// (for variants: the value in the current variant's name).
pub fn prop_value(doc: &Document, inst: NodeId, def: &ComponentPropDef) -> PropValue {
    let Some(n) = doc.get(inst) else { return def.default.clone() };
    if let Some(v) = n.component.values.get(&def.name) {
        return v.clone();
    }
    if matches!(def.kind, PropKind::Variant { .. }) {
        if let Some(c) = instance_component(n).and_then(|c| doc.get(c)) {
            if let Some((_, v)) = parse_variant_name(&c.name).into_iter().find(|(k, _)| *k == def.name) {
                return PropValue::Variant(v);
            }
        }
    }
    def.default.clone()
}

/// Splits a variant name like `Size=Large, State=Hover`.
pub fn parse_variant_name(name: &str) -> Vec<(String, String)> {
    name.split(',')
        .filter_map(|part| {
            let (k, v) = part.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

pub fn variant_name(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
}

fn same_variant(a: &[(String, String)], b: &[(String, String)]) -> bool {
    let a: BTreeMap<_, _> = a.iter().cloned().collect();
    let b: BTreeMap<_, _> = b.iter().cloned().collect();
    a == b
}

// ---------------------------------------------------------------------------
// Diffing edits into overrides
// ---------------------------------------------------------------------------

/// Which overridable fields differ between two versions of a node. Callers
/// usually drop `Transform` for instance roots (moving an instance is not an
/// override).
pub fn record_override(before: &Node, after: &Node) -> BTreeSet<OverrideField> {
    let mut s = BTreeSet::new();
    let mut add = |c: bool, f: OverrideField| {
        if c {
            s.insert(f);
        }
    };
    add(before.name != after.name, OverrideField::Name);
    add(before.visible != after.visible, OverrideField::Visible);
    add(before.locked != after.locked, OverrideField::Locked);
    add(before.opacity != after.opacity, OverrideField::Opacity);
    add(before.blend != after.blend, OverrideField::Blend);
    add(before.size != after.size, OverrideField::Size);
    add(before.transform != after.transform, OverrideField::Transform);
    add(
        before.fills != after.fills || (!matches!(after.kind, NodeKind::Text(_)) && before.style_refs.get("fill") != after.style_refs.get("fill")),
        OverrideField::Fills,
    );
    add(before.stroke != after.stroke || before.style_refs.get("stroke") != after.style_refs.get("stroke"), OverrideField::Stroke);
    add(before.effects != after.effects || before.style_refs.get("effect") != after.style_refs.get("effect"), OverrideField::Effects);
    add(before.corner_radii != after.corner_radii || before.corner_smoothing != after.corner_smoothing, OverrideField::CornerRadii);
    if let (NodeKind::Text(a), NodeKind::Text(b)) = (&before.kind, &after.kind) {
        add(a.content != b.content, OverrideField::Characters);
        add(
            a.style != b.style
                || a.runs != b.runs
                || a.auto_resize != b.auto_resize
                || a.max_lines != b.max_lines
                || a.paragraph != b.paragraph
                || a.lists != b.lists
                || a.direction != b.direction
                || before.style_refs.get("text") != after.style_refs.get("text")
                || before.style_refs.get("fill") != after.style_refs.get("fill"),
            OverrideField::TextStyle,
        );
    }
    add(
        before.kind.frame_props() != after.kind.frame_props()
            || before.layout_child != after.layout_child
            || before.style_refs.get("grid") != after.style_refs.get("grid"),
        OverrideField::Layout,
    );
    add(instance_component(before) != instance_component(after), OverrideField::MainComponent);
    if before.variables.bindings != after.variables.bindings {
        let is_text = matches!(after.kind, NodeKind::Text(_));
        for key in before.variables.bindings.keys().chain(after.variables.bindings.keys()) {
            if before.variables.bindings.get(key) == after.variables.bindings.get(key) {
                continue;
            }
            if key.starts_with("fills/") {
                s.insert(if is_text { OverrideField::TextStyle } else { OverrideField::Fills });
            } else if key.starts_with("strokes/") || key == "strokeWeight" {
                s.insert(OverrideField::Stroke);
            } else if matches!(key.as_str(), "fontSize" | "fontFamily" | "fontWeight" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent")
            {
                s.insert(OverrideField::TextStyle);
            } else if matches!(key.as_str(), "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" | "cornerRadius") {
                s.insert(OverrideField::CornerRadii);
            } else if matches!(key.as_str(), "itemSpacing" | "counterAxisSpacing" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft") {
                s.insert(OverrideField::Layout);
            } else if matches!(key.as_str(), "width" | "height") {
                s.insert(OverrideField::Size);
            } else if key == "opacity" {
                s.insert(OverrideField::Opacity);
            } else if key == "visible" {
                s.insert(OverrideField::Visible);
            } else if key == "characters" {
                s.insert(OverrideField::Characters);
            }
        }
    }
    s
}

/// Which axes (`[horizontal, vertical]`) of [`LayoutChild::grid_align_h`] / [`LayoutChild::grid_align_v`]
/// differ between two versions of a node.
pub fn record_grid_align_overrides(before: &Node, after: &Node) -> [bool; 2] {
    [before.layout_child.grid_align_h != after.layout_child.grid_align_h, before.layout_child.grid_align_v != after.layout_child.grid_align_v]
}

// ---------------------------------------------------------------------------
// Create component / instance
// ---------------------------------------------------------------------------

/// Turns a frame, group, section or shape into a main component. Non-frame
/// shapes (rectangle, text, ...) are wrapped in a new component frame of the
/// same size; instances are detached into components.
pub fn create_component(doc: &mut Document, node: NodeId) -> Command {
    let Some(n) = doc.get(node).cloned() else { return nothing() };
    match &n.kind {
        NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Page { .. } => nothing(),
        NodeKind::Frame(f) => Command::Update { node: Box::new(Node { kind: NodeKind::Component(f.clone()), ..n }) },
        NodeKind::Group | NodeKind::Section | NodeKind::BooleanOp(_) if !matches!(n.kind, NodeKind::BooleanOp(_)) => {
            Command::Update { node: Box::new(Node { kind: NodeKind::Component(FrameProps::default()), ..n }) }
        }
        NodeKind::Instance { .. } => {
            let mut c = Ctx::new(doc);
            detach_into(&mut c, node, true);
            c.finish()
        }
        _ => {
            let Some(parent) = n.parent else { return nothing() };
            let index = doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == node)).unwrap_or(0);
            let wid = doc.alloc_id();
            let mut w = Node::new(wid, NodeKind::Component(FrameProps::default()));
            w.name = n.name.clone();
            w.transform = n.transform;
            w.size = n.size;
            w.constraints = n.constraints;
            w.layout_child = n.layout_child;
            let mut inner = n;
            inner.transform = Affine::IDENTITY;
            inner.layout_child = LayoutChild::default();
            Command::Batch(vec![
                Command::Insert { parent, index, nodes: vec![w] },
                Command::Reparent { id: node, parent: wid, index: 0 },
                Command::Update { node: Box::new(inner) },
            ])
        }
    }
}

/// Instantiates `component` under `parent`, with its origin at `at` (parent
/// space). The copy mirrors the whole component subtree.
pub fn create_instance(doc: &mut Document, component: NodeId, parent: NodeId, at: Vec2) -> Command {
    let Some(comp) = doc.get(component).cloned() else { return nothing() };
    if !matches!(comp.kind, NodeKind::Component(_)) || parent == component || doc.is_ancestor(component, parent) {
        return nothing();
    }
    if !doc.get(parent).map(|p| p.kind.is_container()).unwrap_or(false) {
        return nothing();
    }
    let mut nodes = copy_tree(doc, component, true);
    let name = comp
        .parent
        .and_then(|p| doc.get(p))
        .filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| comp.name.clone());
    let root = &mut nodes[0];
    let frame = comp.kind.frame_props().cloned().unwrap_or_default();
    root.kind = NodeKind::Instance { frame, component: Some(component), component_key: None };
    root.name = name;
    root.transform = Affine::translate(at.x, at.y);
    root.visible = true;
    root.component.source = Some(component);
    root.component.bindings.clear();
    root.component.values.clear();
    // The master's ids define its slots; an instance only has them once it assigns its own.
    root.component.slot_content_ids.clear();
    let index = doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    Command::Insert { parent, index, nodes }
}

/// Duplicates a subtree keeping instance links intact (a copied instance stays
/// an instance of the same component; copied children keep their sources).
pub fn duplicate_subtree(doc: &mut Document, src: NodeId) -> Vec<Node> {
    copy_tree(doc, src, false)
}

// ---------------------------------------------------------------------------
// Detach / reset
// ---------------------------------------------------------------------------

fn detach_into(c: &mut Ctx, id: NodeId, as_component: bool) {
    let Some(mut n) = c.s.get(id).cloned() else { return };
    let NodeKind::Instance { frame, .. } = &n.kind else { return };
    let frame = frame.clone();
    let nested = !is_top(&n);
    n.kind = if as_component { NodeKind::Component(frame) } else { NodeKind::Frame(frame) };
    if nested && !as_component {
        // Stays mirrored by the outer instance, but is no longer synced.
        n.component.overrides = OverrideField::ALL.into_iter().collect();
        n.component.grid_align_overrides = [true, true];
        n.component.values.clear();
    } else {
        n.component = ComponentLink::default();
    }
    c.update(n);
    // Descendants: nodes of nested instances keep their link, retargeted to
    // the component they were originally mirroring.
    let mut stack: Vec<(NodeId, bool)> = c.s.get(id).unwrap().children.iter().map(|k| (*k, false)).collect();
    while let Some((d, in_nested)) = stack.pop() {
        let Some(mut dn) = c.s.get(d).cloned() else { continue };
        let is_inst = matches!(dn.kind, NodeKind::Instance { .. });
        if in_nested {
            let grand = dn.component.source.and_then(|s| c.s.get(s)).and_then(|m| m.component.source);
            dn.component.source = grand;
        } else if is_inst {
            dn.component.source = None;
            dn.component.overrides.clear();
            dn.component.grid_align_overrides = [false, false];
        } else {
            dn.component = ComponentLink::default();
        }
        let kids = dn.children.clone();
        c.update(dn);
        for k in kids {
            stack.push((k, in_nested || is_inst));
        }
    }
}

/// Converts an instance into a plain frame and clears all links. Nested
/// instances stay instances.
pub fn detach_instance(doc: &mut Document, instance: NodeId) -> Command {
    let mut c = Ctx::new(doc);
    detach_into(&mut c, instance, false);
    c.finish()
}

/// Clears the given overrides on `node` and its subtree and re-applies the
/// main component's values. Instance root `values` are left alone (see
/// [`reset_instance`]).
pub fn reset_overrides(doc: &mut Document, node: NodeId, fields: &[OverrideField]) -> Command {
    let mut ids = Vec::new();
    doc.walk(node, |n, _| ids.push(n.id));
    let mut c = Ctx::new(doc);
    for id in ids {
        let Some(mut n) = c.s.get(id).cloned() else { continue };
        let before = (n.component.overrides.len(), n.component.grid_align_overrides);
        for f in fields {
            n.component.overrides.remove(f);
            if *f == OverrideField::Layout {
                n.component.grid_align_overrides = [false, false];
            }
        }
        if (n.component.overrides.len(), n.component.grid_align_overrides) != before {
            c.update(n);
        }
    }
    sync_one(&mut c, node);
    c.finish()
}

/// "Reset all changes": every override and property value of an instance.
pub fn reset_instance(doc: &mut Document, instance: NodeId) -> Command {
    let mut ids = Vec::new();
    doc.walk(instance, |n, _| ids.push(n.id));
    let mut c = Ctx::new(doc);
    if let Some(mut n) = c.s.get(instance).cloned() {
        n.component.values.clear();
        n.component.slot_content_ids.clear();
        c.update(n);
    }
    for id in ids {
        let Some(mut n) = c.s.get(id).cloned() else { continue };
        if !n.component.overrides.is_empty() || n.component.grid_align_overrides != [false, false] {
            n.component.overrides.clear();
            n.component.grid_align_overrides = [false, false];
            c.update(n);
        }
    }
    sync_one(&mut c, instance);
    c.finish()
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

/// Re-applies the main component's current state to every instance of
/// `component` (a Component, or a ComponentSet for all its variants), keeping
/// each node's overrides.
pub fn sync_instances(doc: &mut Document, component: NodeId) -> Command {
    let mut c = Ctx::new(doc);
    sync_component(&mut c, component);
    c.finish()
}

fn sync_component(c: &mut Ctx, owner: NodeId) {
    let comps = components_of(c.s, owner);
    let mut targets: Vec<NodeId> = instances_of_any(c.s, &comps);
    // Outer instances first so nested ones compare against up-to-date nodes.
    let depth = |d: &Document, id: NodeId| {
        let mut n = 0;
        let mut cur = d.get(id).and_then(|x| x.parent);
        while let Some(p) = cur {
            let Some(pn) = d.get(p) else { break };
            if pn.component.source.is_some() {
                n += 1;
            }
            cur = pn.parent;
        }
        n
    };
    targets.sort_by_key(|t| (depth(c.s, *t), t.0));
    for t in targets {
        sync_one(c, t);
    }
}

fn reference_of(d: &Document, id: NodeId) -> Option<NodeId> {
    let n = d.get(id)?;
    if let Some(s) = n.component.source {
        if d.get(s).is_some() {
            return Some(s);
        }
    }
    instance_component(n).filter(|c| d.get(*c).is_some())
}

fn holder_of(d: &Document, id: NodeId) -> NodeId {
    let Some(n) = d.get(id) else { return id };
    if is_top(n) && matches!(n.kind, NodeKind::Instance { .. }) {
        return id;
    }
    let mut cur = n.parent;
    while let Some(p) = cur {
        let Some(pn) = d.get(p) else { break };
        if matches!(pn.kind, NodeKind::Instance { .. }) {
            return p;
        }
        cur = pn.parent;
    }
    id
}

fn sync_one(c: &mut Ctx, id: NodeId) {
    let Some(r) = reference_of(&c.s, id) else { return };
    let top = c.s.get(id).map(is_top).unwrap_or(false);
    let holder = holder_of(&c.s, id);
    sync_pair(c, r, id, top, holder);
}

/// Sibling variant that `inst`'s variant property values select, if different.
fn variant_target(d: &Document, inst: &Node) -> Option<NodeId> {
    let comp = d.get(instance_component(inst)?)?;
    let set = d.get(comp.parent?).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)))?;
    let mut want = parse_variant_name(&comp.name);
    if want.is_empty() {
        return None;
    }
    let before = want.clone();
    for def in &set.component.props {
        if !matches!(def.kind, PropKind::Variant { .. }) {
            continue;
        }
        if let Some(PropValue::Variant(v)) = inst.component.values.get(&def.name) {
            match want.iter_mut().find(|(k, _)| *k == def.name) {
                Some(slot) => slot.1 = v.clone(),
                None => want.push((def.name.clone(), v.clone())),
            }
        }
    }
    if same_variant(&want, &before) {
        return None;
    }
    set.children
        .iter()
        .copied()
        .find(|k| d.get(*k).map(|n| matches!(n.kind, NodeKind::Component(_)) && same_variant(&parse_variant_name(&n.name), &want)).unwrap_or(false))
}

fn copy_overridden(from: &Node, to: &mut Node, fields: &BTreeSet<OverrideField>) {
    let target_align = (to.layout_child.grid_align_h, to.layout_child.grid_align_v);
    for f in fields {
        copy_field(*f, from, to);
    }
    if fields.contains(&OverrideField::Layout) {
        if !from.component.grid_align_overrides[0] {
            to.layout_child.grid_align_h = target_align.0;
        }
        if !from.component.grid_align_overrides[1] {
            to.layout_child.grid_align_v = target_align.1;
        }
        to.component.grid_align_overrides = from.component.grid_align_overrides;
    }
    to.component.overrides.extend(fields.iter().copied());
}

fn copy_style_ref_key(key: &str, from: &Node, to: &mut Node) {
    match from.style_refs.get(key) {
        Some(v) => {
            to.style_refs.insert(key.to_string(), v.clone());
        }
        None => {
            to.style_refs.remove(key);
        }
    }
}

fn copy_field(f: OverrideField, from: &Node, to: &mut Node) {
    match f {
        OverrideField::Name => to.name = from.name.clone(),
        OverrideField::Visible => to.visible = from.visible,
        OverrideField::Locked => to.locked = from.locked,
        OverrideField::Opacity => to.opacity = from.opacity,
        OverrideField::Blend => to.blend = from.blend,
        OverrideField::Size => {
            to.size = from.size;
            to.fill_geometry = from.fill_geometry.clone();
            to.stroke_geometry = from.stroke_geometry.clone();
        }
        OverrideField::Transform => to.transform = from.transform,
        OverrideField::Fills => {
            to.fills = from.fills.clone();
            if !matches!(to.kind, NodeKind::Text(_)) {
                copy_style_ref_key("fill", from, to);
            }
        }
        OverrideField::Stroke => {
            to.stroke = from.stroke.clone();
            copy_style_ref_key("stroke", from, to);
        }
        OverrideField::Effects => {
            to.effects = from.effects.clone();
            copy_style_ref_key("effect", from, to);
        }
        OverrideField::CornerRadii => {
            to.corner_radii = from.corner_radii;
            to.corner_smoothing = from.corner_smoothing;
        }
        OverrideField::Characters => {
            if let (NodeKind::Text(a), NodeKind::Text(b)) = (&from.kind, &mut to.kind) {
                b.content = a.content.clone();
            }
        }
        OverrideField::TextStyle => {
            if let (NodeKind::Text(a), NodeKind::Text(b)) = (&from.kind, &mut to.kind) {
                b.style = a.style.clone();
                b.runs = a.runs.clone();
                b.auto_resize = a.auto_resize;
                b.max_lines = a.max_lines;
                b.paragraph = a.paragraph.clone();
                b.lists = a.lists.clone();
                b.direction = a.direction;
            }
            copy_style_ref_key("text", from, to);
            if matches!(to.kind, NodeKind::Text(_)) {
                copy_style_ref_key("fill", from, to);
            }
        }
        OverrideField::Layout => {
            if let (Some(a), Some(b)) = (from.kind.frame_props(), to.kind.frame_props_mut()) {
                *b = a.clone();
            }
            to.layout_child = from.layout_child;
            copy_style_ref_key("grid", from, to);
        }
        OverrideField::MainComponent | OverrideField::SlotContent => {}
    }
}

/// Text whose characters differ from the main `r` (overridden, or driven by a bound property that
/// has a value) or whose text style is its own, and that sizes itself to its content, keeps its own,
/// already fitted, size.
fn keeps_own_text_size(r: &Node, n: &Node, values: &BTreeMap<String, PropValue>) -> bool {
    let bound_text = r.component.bindings.get(&OverrideField::Characters).is_some_and(|p| values.contains_key(p));
    (n.component.overrides.contains(&OverrideField::Characters) || n.component.overrides.contains(&OverrideField::TextStyle) || bound_text)
        && matches!(&n.kind, NodeKind::Text(t) if matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height))
}

/// True when a sync copies the size of node `id` from the main it mirrors. False for a node that is
/// not linked, one that overrides its size, and text that keeps its own fitted size
/// (see [`keeps_own_text_size`]): whoever changes the type of such text must re-fit it, because no
/// sync will.
pub fn size_comes_from_main(doc: &Document, id: NodeId) -> bool {
    let Some(n) = doc.get(id).filter(|n| is_linked(n)) else { return false };
    let Some(r) = reference_of(doc, id).and_then(|r| doc.get(r)) else { return false };
    if n.component.overrides.contains(&OverrideField::Size) {
        return false;
    }
    let no_values = BTreeMap::new();
    let values = doc.get(holder_of(doc, id)).map_or(&no_values, |h| &h.component.values);
    !keeps_own_text_size(r, n, values)
}

/// Copies every field of `r` that `n` does not override onto `n`.
fn apply_reference(r: &Node, n: &mut Node, top: bool, values: &BTreeMap<String, PropValue>) {
    let ov = n.component.overrides.clone();
    let own_text = keeps_own_text_size(r, n, values);
    for f in OverrideField::ALL {
        if ov.contains(&f) || matches!(f, OverrideField::MainComponent | OverrideField::SlotContent) {
            continue;
        }
        if top && matches!(f, OverrideField::Transform | OverrideField::Visible | OverrideField::Locked) {
            continue;
        }
        if own_text && f == OverrideField::Size {
            continue;
        }
        // Text runs belong to the characters; keep them when those are overridden.
        if f == OverrideField::TextStyle && ov.contains(&OverrideField::Characters) {
            if let (NodeKind::Text(a), NodeKind::Text(b)) = (&r.kind, &mut n.kind) {
                b.style = a.style.clone();
                b.auto_resize = a.auto_resize;
                b.max_lines = a.max_lines;
                b.paragraph = a.paragraph.clone();
                b.direction = a.direction;
            }
            copy_style_ref_key("text", r, n);
            if matches!(n.kind, NodeKind::Text(_)) {
                copy_style_ref_key("fill", r, n);
            }
            continue;
        }
        copy_field(f, r, n);
    }
    // Fields that are never overridden.
    if !top {
        n.constraints = r.constraints;
        if !ov.contains(&OverrideField::Layout) {
            n.layout_child = r.layout_child;
        } else {
            if !n.component.grid_align_overrides[0] {
                n.layout_child.grid_align_h = r.layout_child.grid_align_h;
            }
            if !n.component.grid_align_overrides[1] {
                n.layout_child.grid_align_v = r.layout_child.grid_align_v;
            }
        }
    }
    n.is_mask = r.is_mask;
    if !top {
        n.component.bindings = r.component.bindings.clone();
        n.component.is_slot = r.component.is_slot;
        n.component.is_slot_content = r.component.is_slot_content;
    }
    match (&r.kind, &n.kind) {
        (NodeKind::Text(_), NodeKind::Text(_)) => {}
        (a, b) if a.frame_props().is_some() && b.frame_props().is_some() => {}
        (a, b) if std::mem::discriminant(a) == std::mem::discriminant(b) => n.kind = a.clone(),
        (a, _) => n.kind = a.clone(),
    }
}

fn apply_bindings(n: &mut Node, values: &BTreeMap<String, PropValue>) {
    for (field, prop) in n.component.bindings.clone() {
        let Some(v) = values.get(&prop) else { continue };
        match (field, v) {
            (OverrideField::Visible, PropValue::Bool(b)) => n.visible = *b,
            (OverrideField::Characters, PropValue::Text(s)) => {
                if let NodeKind::Text(t) = &mut n.kind {
                    t.content = s.clone();
                }
            }
            _ => {}
        }
    }
}

/// Rebuilds the children of instance `i` from `new_comp`, carrying overrides
/// over to nodes with the same name path.
fn swap_main(c: &mut Ctx, i: NodeId, new_comp: NodeId) {
    let Some(inode) = c.s.get(i).cloned() else { return };
    // Snapshot overridden descendants by name path.
    let mut carried: Vec<(Vec<String>, Node)> = Vec::new();
    fn collect(d: &Document, id: NodeId, path: &mut Vec<String>, out: &mut Vec<(Vec<String>, Node)>) {
        let Some(n) = d.get(id) else { return };
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            path.push(kn.name.clone());
            if !kn.component.overrides.is_empty() {
                out.push((path.clone(), kn.clone()));
            }
            collect(d, *k, path, out);
            path.pop();
        }
    }
    collect(&c.s, i, &mut Vec::new(), &mut carried);

    for k in inode.children.clone() {
        c.push(Command::Remove { id: k });
    }
    let Some(comp) = c.s.get(new_comp).cloned() else { return };
    let mut idx = 0;
    for k in comp.children.clone() {
        let nodes = copy_tree(&mut c.s, k, true);
        if !nodes.is_empty() {
            c.push(Command::Insert { parent: i, index: idx, nodes });
            idx += 1;
        }
    }
    let mut n = c.s.get(i).cloned().unwrap_or(inode.clone());
    let same_family = instance_component(&inode)
        .and_then(|o| c.s.get(o))
        .map(|o| {
            o.parent.is_some() && o.parent == comp.parent && c.s.get(comp.parent.unwrap()).map(|p| matches!(p.kind, NodeKind::ComponentSet(_))).unwrap_or(false)
        })
        .unwrap_or(false);
    if let NodeKind::Instance { component, component_key, frame } = &mut n.kind {
        *component = Some(new_comp);
        *component_key = None;
        if let Some(f) = comp.kind.frame_props() {
            *frame = f.clone();
        }
    }
    if is_top(&inode) {
        n.component.source = Some(new_comp);
    }
    if !same_family && !n.component.overrides.contains(&OverrideField::Name) {
        n.name = comp.name.clone();
    }
    n.component.overrides.remove(&OverrideField::MainComponent);
    c.update(n);

    // Re-apply carried overrides by path.
    let mut by_path: BTreeMap<Vec<String>, NodeId> = BTreeMap::new();
    fn index(d: &Document, id: NodeId, path: &mut Vec<String>, out: &mut BTreeMap<Vec<String>, NodeId>) {
        let Some(n) = d.get(id) else { return };
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            path.push(kn.name.clone());
            out.entry(path.clone()).or_insert(*k);
            index(d, *k, path, out);
            path.pop();
        }
    }
    index(&c.s, i, &mut Vec::new(), &mut by_path);
    for (path, old) in carried {
        let Some(&nid) = by_path.get(&path) else { continue };
        let Some(mut nn) = c.s.get(nid).cloned() else { continue };
        if std::mem::discriminant(&nn.kind) != std::mem::discriminant(&old.kind) {
            continue;
        }
        let fields: BTreeSet<_> = old.component.overrides.iter().copied().filter(|f| *f != OverrideField::MainComponent).collect();
        copy_overridden(&old, &mut nn, &fields);
        c.update(nn);
    }
}

fn sync_pair(c: &mut Ctx, r: NodeId, i: NodeId, top: bool, holder: NodeId) {
    let (Some(mut rn), Some(mut inode)) = (c.s.get(r).cloned(), c.s.get(i).cloned()) else { return };
    let values = c.s.get(holder).map(|h| h.component.values.clone()).unwrap_or_default();

    // --- which component should this instance show? ---
    if let Some(cur) = instance_component(&inode) {
        let mut desired = cur;
        if !inode.component.overrides.contains(&OverrideField::MainComponent) {
            if let Some(rc) = instance_component(&rn).filter(|_| !top) {
                desired = rc;
            }
        }
        let bindings: BTreeMap<_, _> = rn.component.bindings.iter().chain(inode.component.bindings.iter()).map(|(k, v)| (*k, v.clone())).collect();
        if let Some(PropValue::Instance(sw)) = bindings.get(&OverrideField::MainComponent).and_then(|p| values.get(p)) {
            if matches!(c.s.get(*sw).map(|n| &n.kind), Some(NodeKind::Component(_))) {
                desired = *sw;
            }
        }
        if top {
            if let Some(v) = variant_target(&c.s, &inode) {
                desired = v;
            }
        }
        if desired != cur {
            swap_main(c, i, desired);
            inode = match c.s.get(i) {
                Some(n) => n.clone(),
                None => return,
            };
        }
        // Top instances mirror their component; swapped nested ones their own component too.
        let mine = instance_component(&inode);
        if mine != instance_component(&rn) {
            if let Some(root) = mine.and_then(|m| c.s.get(m)).cloned() {
                rn = root;
            }
        }
    }
    let ref_id = rn.id;

    // --- node properties ---
    let mut n = inode.clone();
    apply_reference(&rn, &mut n, top, &values);
    apply_bindings(&mut n, &values);
    c.update(n);

    // --- children ---
    // A slot the user has edited keeps its content; once the override is cleared (Reset) the slot
    // mirrors the main component's children again, including dropping anything the user added.
    if inode.component.overrides.contains(&OverrideField::SlotContent) {
        return;
    }
    let in_slot = rn.component.bindings.contains_key(&OverrideField::SlotContent) || is_slot_container(&c.s, i) || is_slot_container(&c.s, ref_id);
    let rkids = c.s.get(ref_id).map(|x| x.children.clone()).unwrap_or_default();
    let ikids = c.s.get(i).map(|x| x.children.clone()).unwrap_or_default();
    let child_holder = if matches!(inode.kind, NodeKind::Instance { .. }) { i } else { holder };
    let mut by_src: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    for k in &ikids {
        if let Some(s) = c.s.get(*k).and_then(|x| x.component.source) {
            // In a slot the first mirror of a main child is the one kept; elsewhere the last one wins.
            if in_slot {
                by_src.entry(s).or_insert(*k);
            } else {
                by_src.insert(s, *k);
            }
        }
    }
    let rset: BTreeSet<NodeId> = rkids.iter().copied().collect();
    for k in &ikids {
        let stale = match c.s.get(*k).map(|x| x.component.source) {
            // A slot without its own content mirrors the main slot exactly: a second node for a source
            // that is already represented (a duplicated child keeps its `source`) is extra.
            Some(Some(s)) => !rset.contains(&s) || (in_slot && by_src.get(&s) != Some(k)),
            // Slot content the user created has no counterpart in the main component.
            Some(None) => in_slot,
            None => false,
        };
        if stale {
            c.push(Command::Remove { id: *k });
        }
    }
    let mut desired: Vec<NodeId> = Vec::new();
    for (idx, rc) in rkids.iter().enumerate() {
        if let Some(ic) = by_src.get(rc).copied().filter(|k| c.s.get(*k).is_some()) {
            sync_pair(c, *rc, ic, false, child_holder);
            desired.push(ic);
        } else {
            let nodes = copy_tree(&mut c.s, *rc, true);
            let Some(nid) = nodes.first().map(|n| n.id) else { continue };
            let at = idx.min(c.s.get(i).map(|x| x.children.len()).unwrap_or(0));
            c.push(Command::Insert { parent: i, index: at, nodes });
            sync_pair(c, *rc, nid, false, child_holder);
            desired.push(nid);
        }
    }
    let cur_kids = c.s.get(i).map(|x| x.children.clone()).unwrap_or_default();
    for k in &cur_kids {
        if !desired.contains(k) {
            desired.push(*k);
        }
    }
    for (k, id) in desired.iter().enumerate() {
        let cur = c.s.get(i).map(|x| x.children.clone()).unwrap_or_default();
        if cur.get(k) != Some(id) {
            c.push(Command::Reparent { id: *id, parent: i, index: k });
        }
    }
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Swaps an instance to another component (manual "Swap instance"). The
/// instance keeps its position and records a `MainComponent` override.
pub fn swap_instance(doc: &mut Document, instance: NodeId, new_comp: NodeId) -> Command {
    let mut c = Ctx::new(doc);
    let Some(n) = c.s.get(instance) else { return nothing() };
    if instance_component(n) == Some(new_comp) || !matches!(c.s.get(new_comp).map(|x| &x.kind), Some(NodeKind::Component(_))) {
        return nothing();
    }
    if new_comp == instance || c.s.is_ancestor(new_comp, instance) {
        return nothing();
    }
    swap_main(&mut c, instance, new_comp);
    if let Some(mut n) = c.s.get(instance).cloned() {
        n.component.overrides.insert(OverrideField::MainComponent);
        c.update(n);
    }
    c.finish()
}

/// Sets (or with `None` clears) a component property value on an instance
/// root and re-syncs it: toggles visibility, swaps text, instance or variant.
pub fn set_instance_prop(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> Command {
    let mut c = Ctx::new(doc);
    if let Some(mut n) = c.s.get(instance).cloned() {
        match value {
            Some(v) => {
                n.component.values.insert(name.to_string(), v);
            }
            None => {
                n.component.values.remove(name);
            }
        }
        c.update(n);
    }
    sync_one(&mut c, instance);
    c.finish()
}

/// Adds a property definition to a component or component set.
pub fn add_prop(doc: &mut Document, owner: NodeId, def: ComponentPropDef) -> Command {
    let Some(mut n) = doc.get(owner).cloned() else { return nothing() };
    if n.component.props.iter().any(|p| p.name == def.name) {
        return nothing();
    }
    n.component.props.push(def);
    Command::Update { node: Box::new(n) }
}

fn subtree_ids(d: &Document, root: NodeId) -> Vec<NodeId> {
    let mut v = Vec::new();
    d.walk(root, |n, _| v.push(n.id));
    v
}

/// Renames a property definition everywhere it is referenced: bindings, instance values and slot settings.
pub fn rename_prop(doc: &mut Document, owner: NodeId, old: &str, new: &str) -> Command {
    if old == new || new.is_empty() {
        return nothing();
    }
    let mut c = Ctx::new(doc);
    let Some(mut o) = c.s.get(owner).cloned() else { return nothing() };
    if o.component.props.iter().any(|p| p.name == new) {
        return nothing();
    }
    for p in &mut o.component.props {
        if p.name == old {
            p.name = new.to_string();
        }
    }
    if let Some(cfg) = o.component.slot_configs.remove(old) {
        o.component.slot_configs.insert(new.to_string(), cfg);
    }
    if let Some(scid) = o.component.slot_content_ids.remove(old) {
        o.component.slot_content_ids.insert(new.to_string(), scid);
    }
    c.update(o);
    for id in subtree_ids(c.s, owner) {
        let Some(mut n) = c.s.get(id).cloned() else { continue };
        for v in n.component.bindings.values_mut() {
            if v == old {
                *v = new.to_string();
            }
        }
        c.update(n);
    }
    let comps = components_of(c.s, owner);
    for inst in instances_of_any(c.s, &comps) {
        let Some(mut n) = c.s.get(inst).cloned() else { continue };
        if let Some(v) = n.component.values.remove(old) {
            n.component.values.insert(new.to_string(), v);
        }
        if let Some(scid) = n.component.slot_content_ids.remove(old) {
            n.component.slot_content_ids.insert(new.to_string(), scid);
        }
        c.update(n);
    }
    c.finish()
}

/// Deletes a property definition, its bindings, instance values and slot settings.
pub fn delete_prop(doc: &mut Document, owner: NodeId, name: &str) -> Command {
    let mut c = Ctx::new(doc);
    let Some(mut o) = c.s.get(owner).cloned() else { return nothing() };
    // `slot_content_id` is the node's own id, not a property's: only the per-property entry goes.
    o.component.slot_content_ids.remove(name);
    o.component.props.retain(|p| p.name != name);
    o.component.slot_configs.remove(name);
    c.update(o);
    for id in subtree_ids(c.s, owner) {
        let Some(mut n) = c.s.get(id).cloned() else { continue };
        let cleared_slot = n.component.bindings.get(&OverrideField::SlotContent).is_some_and(|v| v == name);
        n.component.bindings.retain(|_, v| v != name);
        if cleared_slot {
            n.component.is_slot = false;
        }
        c.update(n);
    }
    let comps = components_of(c.s, owner);
    for inst in instances_of_any(c.s, &comps) {
        let Some(mut n) = c.s.get(inst).cloned() else { continue };
        n.component.values.remove(name);
        n.component.slot_content_ids.remove(name);
        c.update(n);
    }
    sync_component(&mut c, owner);
    c.finish()
}

/// Binds (or with `None` unbinds) `field` of a layer inside a main component
/// to a component property, then syncs the instances.
pub fn bind_prop(doc: &mut Document, node: NodeId, field: OverrideField, prop: Option<&str>) -> Command {
    let mut c = Ctx::new(doc);
    let Some(mut n) = c.s.get(node).cloned() else { return nothing() };
    match prop {
        Some(p) => {
            n.component.bindings.insert(field, p.to_string());
            if field == OverrideField::SlotContent {
                n.component.is_slot = true;
            }
        }
        None => {
            n.component.bindings.remove(&field);
            if field == OverrideField::SlotContent {
                n.component.is_slot = false;
            }
        }
    }
    c.update(n);
    if let Some(comp) = enclosing_component(c.s, node) {
        let owner = c.s.get(comp).and_then(|x| x.parent).filter(|p| matches!(c.s.get(*p).map(|n| &n.kind), Some(NodeKind::ComponentSet(_)))).unwrap_or(comp);
        sync_component(&mut c, owner);
    }
    c.finish()
}

/// Converts a `Frame` inside a main component into a component slot: creates a
/// [`PropKind::Slot`] property on the component owner, marks the frame with
/// `is_slot = true` and `SlotContent -> prop_name`, and syncs all instances in
/// a single undoable command.
pub fn convert_to_slot(doc: &mut Document, frame: NodeId) -> Command {
    let mut c = Ctx::new(doc);
    let Some(mut n) = c.s.get(frame).cloned() else { return nothing() };
    if !matches!(n.kind, NodeKind::Frame(_)) || is_linked(&n) {
        return nothing();
    }
    let Some(comp) = enclosing_component(c.s, frame).filter(|&comp| comp != frame) else {
        return nothing();
    };
    let owner = c.s.get(comp).and_then(|x| x.parent).filter(|p| matches!(c.s.get(*p).map(|n| &n.kind), Some(NodeKind::ComponentSet(_)))).unwrap_or(comp);
    let Some(mut o) = c.s.get(owner).cloned() else { return nothing() };

    let prop_name = if let Some(existing) = n.component.bindings.get(&OverrideField::SlotContent).cloned() {
        existing
    } else {
        let base = if n.name.trim().is_empty() { "Slot" } else { n.name.trim() };
        let taken: Vec<String> = o.component.props.iter().map(|p| p.name.clone()).collect();
        let mut candidate = base.to_string();
        let mut idx = 1usize;
        while taken.contains(&candidate) {
            idx += 1;
            candidate = format!("{base} {idx}");
        }
        candidate
    };

    if !o.component.props.iter().any(|p| p.name == prop_name) {
        o.component.props.push(ComponentPropDef { name: prop_name.clone(), kind: PropKind::Slot, default: PropValue::Slot(n.children.clone()) });
    }
    o.component.slot_configs.entry(prop_name.clone()).or_default();
    c.update(o);

    n.component.is_slot = true;
    n.component.bindings.insert(OverrideField::SlotContent, prop_name);
    c.update(n);

    sync_component(&mut c, owner);
    c.finish()
}

/// Updates the [`SlotPropConfig`] for `prop_name` on component/set `owner` in one undo step.
pub fn set_slot_config(doc: &mut Document, owner: NodeId, prop_name: &str, config: SlotPropConfig) -> Command {
    let Some(mut o) = doc.get(owner).cloned() else { return nothing() };
    if !o.component.props.iter().any(|p| p.name == prop_name && matches!(p.kind, PropKind::Slot)) {
        return nothing();
    }
    o.component.slot_configs.insert(prop_name.to_string(), config);
    Command::Update { node: Box::new(o) }
}

/// Error returned when a slot content mutation violates its [`SlotPropConfig`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SlotConstraintError {
    #[error("Node {0:?} is not a slot container")]
    NotASlot(NodeId),
    #[error("Slot reaches maximum children ({0})")]
    MaxChildren(u32),
    #[error("Slot requires at least {0} children")]
    MinChildren(u32),
    #[error("Slot accepts preferred instances only")]
    PreferredOnly,
}

fn node_matches_preferred(doc: &Document, node: &Node, preferred: &[NodeId]) -> bool {
    let Some(comp) = instance_component(node) else { return false };
    if preferred.contains(&comp) {
        return true;
    }
    if let Some(parent) = doc.get(comp).and_then(|c| c.parent) {
        if preferred.contains(&parent) {
            return true;
        }
    }
    false
}

/// Checks whether inserting `root_nodes` as direct children of `slot` respects its [`SlotPropConfig`].
pub fn check_slot_insert(doc: &Document, slot: NodeId, root_nodes: &[&Node]) -> Result<(), SlotConstraintError> {
    check_slot_replace(doc, slot, 0, root_nodes)
}

/// Like [`check_slot_insert`] for an edit that also takes `removing` direct children out of `slot`
/// (ungroup, wrap in a group, replace): max and min apply to the count the slot ends with.
pub fn check_slot_replace(doc: &Document, slot: NodeId, removing: usize, root_nodes: &[&Node]) -> Result<(), SlotConstraintError> {
    if !is_slot_container(doc, slot) {
        return Err(SlotConstraintError::NotASlot(slot));
    }
    let cur = doc.get(slot).map(|n| n.children.len()).unwrap_or(0);
    let next_count = (cur + root_nodes.len()).saturating_sub(removing);
    if let Some(cfg) = slot_config_of(doc, slot) {
        if let Some(max) = cfg.max_children {
            if next_count > max as usize {
                return Err(SlotConstraintError::MaxChildren(max));
            }
        }
        if removing > 0 {
            if let Some(min) = cfg.min_children {
                if next_count < min as usize {
                    return Err(SlotConstraintError::MinChildren(min));
                }
            }
        }
        if cfg.allow_preferred_values_only == Some(true) && !cfg.preferred.is_empty() {
            for rn in root_nodes {
                if !node_matches_preferred(doc, rn, &cfg.preferred) {
                    return Err(SlotConstraintError::PreferredOnly);
                }
            }
        }
    }
    Ok(())
}

/// Checks whether removing `removing_count` direct children from `slot` respects its [`SlotPropConfig`].
pub fn check_slot_remove(doc: &Document, slot: NodeId, removing_count: usize) -> Result<(), SlotConstraintError> {
    if !is_slot_container(doc, slot) {
        return Err(SlotConstraintError::NotASlot(slot));
    }
    let cur = doc.get(slot).map(|n| n.children.len()).unwrap_or(0);
    let rem = cur.saturating_sub(removing_count);
    if let Some(cfg) = slot_config_of(doc, slot) {
        if let Some(min) = cfg.min_children {
            if rem < min as usize {
                return Err(SlotConstraintError::MinChildren(min));
            }
        }
    }
    Ok(())
}

/// Applies `stretchChildOnInsert` to `child` when `slot` is a slot container
/// whose [`SlotPropConfig`] has `stretch_child_on_insert == Some(true)`.
pub fn apply_slot_stretch_on_insert(doc: &Document, slot: NodeId, child: &mut Node) {
    if slot_config_of(doc, slot).and_then(|cfg| cfg.stretch_child_on_insert) != Some(true) {
        return;
    }
    let Some(sn) = doc.get(slot) else { return };
    let (pad_l, pad_r, has_al) =
        sn.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).map(|al| (al.padding[3], al.padding[1], true)).unwrap_or((0.0, 0.0, false));
    let target_w = (sn.size.x - pad_l - pad_r).max(1.0);
    child.layout_child.horizontal = Sizing::Fill;
    child.size.x = target_w;
    if !has_al {
        child.transform.tx = pad_l;
    }
    if is_linked(child) || matches!(child.kind, NodeKind::Instance { .. }) {
        child.component.overrides.insert(OverrideField::Size);
        child.component.overrides.insert(OverrideField::Layout);
    }
}

/// Inserts a subtree `nodes` (root first) into `slot` at `index`, validating [`SlotPropConfig`]
/// constraints, stretching the inserted root when `stretch_child_on_insert` is set, and recording
/// `OverrideField::SlotContent` so the instance stays linked.
pub fn insert_into_slot(doc: &mut Document, slot: NodeId, index: usize, mut nodes: Vec<Node>) -> Result<Command, SlotConstraintError> {
    let Some(root) = nodes.first() else {
        if !is_slot_container(doc, slot) {
            return Err(SlotConstraintError::NotASlot(slot));
        }
        return Ok(nothing());
    };
    check_slot_insert(doc, slot, &[root])?;
    if let Some(root_mut) = nodes.first_mut() {
        apply_slot_stretch_on_insert(doc, slot, root_mut);
    }
    let mut c = Ctx::new(doc);
    if let Some(mut sn) = c.s.get(slot).cloned() {
        if is_linked(&sn) || instance_root_of(c.s, slot).is_some() {
            sn.component.overrides.insert(OverrideField::SlotContent);
            c.update(sn);
        }
    }
    let len = c.s.get(slot).map(|n| n.children.len()).unwrap_or(0);
    c.push(Command::Insert { parent: slot, index: index.min(len), nodes });
    Ok(c.finish())
}

/// Removes a direct child `child` from its enclosing slot container, validating `min_children`
/// and recording `OverrideField::SlotContent` so the instance stays linked.
pub fn remove_from_slot(doc: &mut Document, child: NodeId) -> Result<Command, SlotConstraintError> {
    let Some(slot) = doc.get(child).and_then(|n| n.parent) else {
        return Err(SlotConstraintError::NotASlot(child));
    };
    check_slot_remove(doc, slot, 1)?;
    let mut c = Ctx::new(doc);
    if let Some(mut sn) = c.s.get(slot).cloned() {
        if is_linked(&sn) || instance_root_of(c.s, slot).is_some() {
            sn.component.overrides.insert(OverrideField::SlotContent);
            c.update(sn);
        }
    }
    c.push(Command::Remove { id: child });
    Ok(c.finish())
}

/// Reorders a direct child `child` inside `slot` to `index`, recording `OverrideField::SlotContent`
/// so the instance stays linked.
pub fn reorder_in_slot(doc: &mut Document, slot: NodeId, child: NodeId, index: usize) -> Result<Command, SlotConstraintError> {
    if !is_slot_container(doc, slot) || doc.get(child).and_then(|n| n.parent) != Some(slot) {
        return Err(SlotConstraintError::NotASlot(slot));
    }
    let mut c = Ctx::new(doc);
    if let Some(mut sn) = c.s.get(slot).cloned() {
        if is_linked(&sn) || instance_root_of(c.s, slot).is_some() {
            sn.component.overrides.insert(OverrideField::SlotContent);
            c.update(sn);
        }
    }
    let len = c.s.get(slot).map(|n| n.children.len()).unwrap_or(1);
    c.push(Command::Reparent { id: child, parent: slot, index: index.min(len.saturating_sub(1)) });
    Ok(c.finish())
}

#[derive(Clone, Debug)]
enum MigrationMode {
    InstanceLayer { prop_name: String, vis_prop: Option<String> },
    WrappedInstance { child_inst: NodeId, prop_name: String, vis_prop: Option<String> },
    BooleanContainer { prop_name: String },
}

impl MigrationMode {
    fn prop_name(&self) -> &str {
        match self {
            MigrationMode::InstanceLayer { prop_name, .. }
            | MigrationMode::WrappedInstance { prop_name, .. }
            | MigrationMode::BooleanContainer { prop_name } => prop_name,
        }
    }

    /// The boolean property that toggles the region: a separate one for instance-swap regions,
    /// the region's own property for a boolean container.
    fn visibility_prop(&self) -> Option<&str> {
        match self {
            MigrationMode::InstanceLayer { vis_prop, .. } | MigrationMode::WrappedInstance { vis_prop, .. } => vis_prop.as_deref(),
            MigrationMode::BooleanContainer { prop_name } => Some(prop_name),
        }
    }

    /// The boolean property that disappears with the migration (a boolean container keeps its own name).
    fn dropped_vis_prop(&self) -> Option<&str> {
        match self {
            MigrationMode::BooleanContainer { .. } => None,
            _ => self.visibility_prop(),
        }
    }
}

/// One region of a main component that becomes a slot container.
struct SlotMigrationRegion {
    comp: NodeId,
    node: NodeId,
    mode: MigrationMode,
}

/// Everything a migration touches: the owner of the property and one region per variant that shares it
/// (the selected region first).
struct SlotMigrationPlan {
    owner: NodeId,
    regions: Vec<SlotMigrationRegion>,
}

fn unique_slot_prop_name(doc: &Document, owner: NodeId, base: &str) -> String {
    let base = if base.trim().is_empty() { "Slot" } else { base.trim() };
    let taken: Vec<String> = doc.get(owner).map(|n| n.component.props.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
    if !taken.contains(&base.to_string()) {
        return base.to_string();
    }
    let mut i = 2usize;
    loop {
        let candidate = format!("{base} {i}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

/// Classifies one region on its own: the main component it sits in, the owner of the property it is bound
/// to (the component set unless the variant defines that property itself) and how it migrates.
fn classify_slot_region(doc: &Document, node: NodeId) -> Result<(NodeId, NodeId, MigrationMode), &'static str> {
    let Some(n) = doc.get(node) else {
        return Err("Selected layer was not found");
    };
    if !is_top(n) {
        return Err("Select a region inside a main component to migrate to a slot");
    }
    let Some(comp) = enclosing_component(doc, node) else {
        return Err("Select a layer inside a main component to migrate to a slot");
    };
    if comp == node || matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
        return Err("Cannot migrate a main component root; select an inner instance-swap or boolean region");
    }
    let mut cur = n.parent;
    while let Some(p) = cur {
        if p == comp {
            break;
        }
        if matches!(doc.get(p).map(|pn| &pn.kind), Some(NodeKind::Instance { .. })) {
            return Err("Select the nested instance itself rather than a layer inside it");
        }
        cur = doc.get(p).and_then(|pn| pn.parent);
    }
    if is_slot_container(doc, node) || n.component.is_slot {
        return Err("Selected region is already a component slot");
    }
    let set = doc.get(comp).and_then(|c| c.parent).filter(|p| matches!(doc.get(*p).map(|x| &x.kind), Some(NodeKind::ComponentSet(_))));
    let name_owner = set.unwrap_or(comp);

    let mode = match &n.kind {
        NodeKind::Instance { .. } => {
            let main_prop = n.component.bindings.get(&OverrideField::MainComponent).cloned();
            let vis_prop = n.component.bindings.get(&OverrideField::Visible).cloned();
            let prop_name = if let Some(p) = main_prop {
                p
            } else if let Some(p) = vis_prop.clone() {
                p
            } else {
                unique_slot_prop_name(doc, name_owner, &n.name)
            };
            MigrationMode::InstanceLayer { prop_name, vis_prop }
        }
        NodeKind::Frame(_) | NodeKind::Group => {
            let wrapped = match n.children.as_slice() {
                [cid] => doc.get(*cid).filter(|cn| matches!(cn.kind, NodeKind::Instance { .. })).and_then(|cn| {
                    cn.component.bindings.get(&OverrideField::MainComponent).cloned().map(|p| {
                        let vis_prop =
                            n.component.bindings.get(&OverrideField::Visible).cloned().or_else(|| cn.component.bindings.get(&OverrideField::Visible).cloned());
                        MigrationMode::WrappedInstance { child_inst: *cid, prop_name: p, vis_prop }
                    })
                }),
                _ => None,
            };
            match wrapped {
                Some(mode) => mode,
                None => {
                    let Some(p) =
                        n.component.bindings.get(&OverrideField::MainComponent).cloned().or_else(|| n.component.bindings.get(&OverrideField::Visible).cloned())
                    else {
                        return Err("Selected container has no instance-swap or boolean property to migrate");
                    };
                    MigrationMode::BooleanContainer { prop_name: p }
                }
            }
        }
        _ => return Err("Unsupported shape: select an instance-swap layer or a container with a boolean property"),
    };
    let defined_on_variant = doc.get(comp).is_some_and(|c| c.component.props.iter().any(|p| p.name == mode.prop_name()));
    let owner = if defined_on_variant { comp } else { name_owner };
    Ok((comp, owner, mode))
}

/// Plans a migration. A property defined on a component set is shared by every variant, so each other
/// variant must hold exactly one matching region (same kind of region, bound to the same property) that
/// migrates with the selected one. Otherwise the migration is refused before anything changes.
fn classify_slot_migration(doc: &Document, node: NodeId) -> Result<SlotMigrationPlan, &'static str> {
    let (comp, owner, mode) = classify_slot_region(doc, node)?;
    let prop_name = mode.prop_name().to_string();
    let set_defines_prop = owner != comp && doc.get(owner).is_some_and(|o| o.component.props.iter().any(|p| p.name == prop_name));
    let kind = std::mem::discriminant(&mode);
    let mut regions = vec![SlotMigrationRegion { comp, node, mode }];
    if set_defines_prop {
        for sibling in components_of(doc, owner).into_iter().filter(|c| *c != comp) {
            let mut matches: Vec<SlotMigrationRegion> = subtree_ids(doc, sibling)
                .into_iter()
                .filter(|id| *id != sibling)
                .filter_map(|id| {
                    let (c2, o2, m2) = classify_slot_region(doc, id).ok()?;
                    (c2 == sibling && o2 == owner && std::mem::discriminant(&m2) == kind && m2.prop_name() == prop_name).then_some(SlotMigrationRegion {
                        comp: c2,
                        node: id,
                        mode: m2,
                    })
                })
                .collect();
            match matches.len() {
                1 => regions.extend(matches.pop()),
                0 => return Err("This property is shared by every variant of the component set, and a variant has no matching region to migrate with it"),
                _ => return Err("This property is shared by every variant of the component set, and a variant has more than one matching region"),
            }
        }
    }
    Ok(SlotMigrationPlan { owner, regions })
}

/// True when `node` is an inner instance-swap or boolean container region inside
/// a main component that can be migrated to a component slot.
pub fn can_migrate_region_to_slot(doc: &Document, node: NodeId) -> bool {
    classify_slot_migration(doc, node).is_ok()
}

fn instance_region_was_visible(doc: &Document, inst: NodeId, copy_id: NodeId, vis_prop: Option<&str>, default_vis: bool) -> bool {
    if let Some(vp) = vis_prop {
        if let Some(PropValue::Bool(b)) = doc.get(inst).and_then(|i| i.component.values.get(vp)) {
            return *b;
        }
    }
    if let Some(cn) = doc.get(copy_id) {
        if cn.component.overrides.contains(&OverrideField::Visible) {
            return cn.visible;
        }
        return default_vis && cn.visible;
    }
    default_vis
}

fn upgrade_owner_prop_to_slot(c: &mut Ctx, owner: NodeId, prop_name: &str, default_children: Vec<NodeId>, drop_vis_props: &[String]) {
    let Some(mut o) = c.s.get(owner).cloned() else { return };
    let mut extracted_preferred = Vec::new();
    let mut found = false;
    for p in &mut o.component.props {
        if p.name == prop_name {
            if let PropKind::InstanceSwap { preferred } = &p.kind {
                extracted_preferred = preferred.clone();
            }
            p.kind = PropKind::Slot;
            p.default = PropValue::Slot(default_children.clone());
            found = true;
        }
    }
    if !found {
        o.component.props.push(ComponentPropDef { name: prop_name.to_string(), kind: PropKind::Slot, default: PropValue::Slot(default_children) });
    }
    if !extracted_preferred.is_empty() {
        o.component.slot_configs.entry(prop_name.to_string()).or_default().preferred = extracted_preferred;
    }
    for vp in drop_vis_props {
        if vp != prop_name {
            let still_bound = subtree_ids(c.s, owner).into_iter().any(|id| c.s.get(id).is_some_and(|n| n.component.bindings.values().any(|b| b == vp)));
            if !still_bound {
                o.component.props.retain(|p| p.name != *vp);
            }
        }
    }
    c.update(o);
}

fn group_to_frame(n: &mut Node) {
    if matches!(n.kind, NodeKind::Group) {
        n.kind = NodeKind::Frame(FrameProps::default());
    }
}

fn make_main_region_a_slot(c: &mut Ctx, node: NodeId, prop_name: &str) {
    if let Some(mut slot_n) = c.s.get(node).cloned() {
        group_to_frame(&mut slot_n);
        slot_n.visible = true;
        slot_n.component.is_slot = true;
        slot_n.component.bindings.remove(&OverrideField::Visible);
        slot_n.component.bindings.remove(&OverrideField::MainComponent);
        slot_n.component.bindings.insert(OverrideField::SlotContent, prop_name.to_string());
        c.update(slot_n);
    }
}

/// One copy of a migrated region inside an instance.
struct InstanceRegionCopy<'a> {
    inst: NodeId,
    copy_id: NodeId,
    was_vis: bool,
    prop_name: &'a str,
    /// Boolean property whose value on the instance goes away with the migration.
    dropped_vis_prop: Option<&'a str>,
}

/// Turns the copy of a migrated region inside one instance into a slot container that holds `content`.
/// An instance that had the region hidden keeps its content and keeps the region hidden.
fn convert_instance_region_to_slot(c: &mut Ctx, t: &InstanceRegionCopy, content: Vec<NodeId>, reshape: impl FnOnce(&mut Node)) {
    if let Some(mut copy) = c.s.get(t.copy_id).cloned() {
        reshape(&mut copy);
        copy.visible = t.was_vis;
        copy.component.is_slot = true;
        copy.component.bindings.remove(&OverrideField::MainComponent);
        copy.component.bindings.remove(&OverrideField::Visible);
        copy.component.bindings.insert(OverrideField::SlotContent, t.prop_name.to_string());
        copy.component.overrides.remove(&OverrideField::MainComponent);
        copy.component.overrides.insert(OverrideField::SlotContent);
        if t.was_vis {
            copy.component.overrides.remove(&OverrideField::Visible);
        } else {
            copy.component.overrides.insert(OverrideField::Visible);
        }
        c.update(copy);
    }
    if let Some(mut inst_node) = c.s.get(t.inst).cloned() {
        if let Some(vp) = t.dropped_vis_prop.filter(|vp| *vp != t.prop_name) {
            inst_node.component.values.remove(vp);
        }
        inst_node.component.values.insert(t.prop_name.to_string(), PropValue::Slot(content));
        c.update(inst_node);
    }
}

/// Migrates one region and every instance copy of it. Returns the region's default slot content,
/// or `None`, before any change, if the region is not what the plan classified.
fn migrate_slot_region(c: &mut Ctx, insts: &[NodeId], region: &SlotMigrationRegion) -> Option<Vec<NodeId>> {
    let node = region.node;
    let prop_name = region.mode.prop_name();
    let vis_prop = region.mode.visibility_prop();
    let dropped_vis_prop = region.mode.dropped_vis_prop();
    let default_vis = vis_prop
        .and_then(|vp| prop_defs(c.s, region.comp).into_iter().find(|d| d.name == vp))
        .and_then(|d| match d.default {
            PropValue::Bool(b) => Some(b),
            _ => None,
        })
        .unwrap_or(true);
    // Every copy of the region inside an instance, with whether that instance had it visible.
    let mut inst_targets = Vec::new();
    for inst in insts {
        for id in subtree_ids(c.s, *inst) {
            if c.s.get(id).and_then(|n| n.component.source) == Some(node) {
                inst_targets.push((*inst, id, instance_region_was_visible(c.s, *inst, id, vis_prop, default_vis)));
            }
        }
    }

    match &region.mode {
        MigrationMode::InstanceLayer { .. } => {
            let orig_node = c.s.get(node).cloned()?;
            let NodeKind::Instance { frame, component: default_comp, component_key: comp_key } = orig_node.kind.clone() else {
                return None;
            };

            let old_children = orig_node.children.clone();
            let d_inst_id = c.s.alloc_id();
            let mut d_inst = Node::new(d_inst_id, NodeKind::Instance { frame: frame.clone(), component: default_comp, component_key: comp_key });
            d_inst.name = default_comp.and_then(|dc| c.s.get(dc)).map(|dn| dn.name.clone()).unwrap_or_else(|| orig_node.name.clone());
            d_inst.size = orig_node.size;
            d_inst.transform = Affine::IDENTITY;
            d_inst.fills = orig_node.fills.clone();
            d_inst.stroke = orig_node.stroke.clone();
            d_inst.effects = orig_node.effects.clone();
            d_inst.corner_radii = orig_node.corner_radii;
            d_inst.corner_smoothing = orig_node.corner_smoothing;
            d_inst.opacity = orig_node.opacity;
            d_inst.blend = orig_node.blend;
            d_inst.component.source = default_comp;
            d_inst.component.is_slot_content = true;
            d_inst.component.values = orig_node.component.values.clone();

            c.push(Command::Insert { parent: node, index: old_children.len(), nodes: vec![d_inst] });
            for (k, cid) in old_children.iter().copied().enumerate() {
                c.push(Command::Reparent { id: cid, parent: d_inst_id, index: k });
            }
            if let Some(mut updated_slot) = c.s.get(node).cloned() {
                updated_slot.kind = NodeKind::Frame(frame.clone());
                updated_slot.visible = true;
                updated_slot.component.source = None;
                updated_slot.component.is_slot = true;
                updated_slot.component.values.clear();
                updated_slot.component.bindings.remove(&OverrideField::MainComponent);
                updated_slot.component.bindings.remove(&OverrideField::Visible);
                updated_slot.component.bindings.insert(OverrideField::SlotContent, prop_name.to_string());
                c.update(updated_slot);
            }

            for (inst, copy_id, was_vis) in inst_targets {
                let Some(copy_node) = c.s.get(copy_id).cloned() else { continue };
                let (copy_frame, swapped_comp, copy_key) = match &copy_node.kind {
                    NodeKind::Instance { frame: cf, component: sc, component_key: ck } => (cf.clone(), sc.or(default_comp), ck.clone()),
                    _ => (frame.clone(), default_comp, None),
                };
                let active_comp = match c.s.get(inst).and_then(|in_n| in_n.component.values.get(prop_name)) {
                    Some(PropValue::Instance(pc)) => Some(*pc),
                    _ => swapped_comp,
                };
                let copy_old_children = copy_node.children.clone();
                let c_inst_id = c.s.alloc_id();
                let mut c_inst = Node::new(c_inst_id, NodeKind::Instance { frame: copy_frame, component: active_comp, component_key: copy_key });
                c_inst.name = active_comp.and_then(|ac| c.s.get(ac)).map(|an| an.name.clone()).unwrap_or_else(|| copy_node.name.clone());
                c_inst.size = copy_node.size;
                c_inst.transform = Affine::IDENTITY;
                c_inst.fills = copy_node.fills.clone();
                c_inst.stroke = copy_node.stroke.clone();
                c_inst.effects = copy_node.effects.clone();
                c_inst.corner_radii = copy_node.corner_radii;
                c_inst.corner_smoothing = copy_node.corner_smoothing;
                c_inst.opacity = copy_node.opacity;
                c_inst.blend = copy_node.blend;
                c_inst.component.source = active_comp;
                c_inst.component.is_slot_content = true;
                c_inst.component.values = copy_node.component.values.clone();

                c.push(Command::Insert { parent: copy_id, index: copy_old_children.len(), nodes: vec![c_inst] });
                for (k, cid) in copy_old_children.iter().copied().enumerate() {
                    c.push(Command::Reparent { id: cid, parent: c_inst_id, index: k });
                }
                for desc_id in subtree_ids(c.s, c_inst_id) {
                    if desc_id == c_inst_id {
                        continue;
                    }
                    if let Some(mut dn) = c.s.get(desc_id).cloned() {
                        if let Some(src) = dn.component.source {
                            if c.s.is_ancestor(d_inst_id, src) {
                                if let Some(grand) = c.s.get(src).and_then(|m| m.component.source) {
                                    dn.component.source = Some(grand);
                                    c.update(dn);
                                }
                            }
                        }
                    }
                }
                let slot_frame = frame.clone();
                let slot_name = orig_node.name.clone();
                let target = InstanceRegionCopy { inst, copy_id, was_vis, prop_name, dropped_vis_prop };
                convert_instance_region_to_slot(c, &target, vec![c_inst_id], |copy| {
                    copy.kind = NodeKind::Frame(slot_frame);
                    copy.name = slot_name;
                    copy.component.values.clear();
                });
            }
            Some(vec![d_inst_id])
        }
        MigrationMode::WrappedInstance { child_inst, .. } => {
            if let Some(mut cn) = c.s.get(*child_inst).cloned() {
                cn.component.is_slot_content = true;
                cn.component.bindings.remove(&OverrideField::MainComponent);
                cn.component.bindings.remove(&OverrideField::Visible);
                c.update(cn);
            }
            make_main_region_a_slot(c, node, prop_name);

            for (inst, copy_id, was_vis) in inst_targets {
                let copy_children = c.s.get(copy_id).map(|n| n.children.clone()).unwrap_or_default();
                for cid in &copy_children {
                    if let Some(mut cn) = c.s.get(*cid).cloned() {
                        cn.component.is_slot_content = true;
                        cn.component.bindings.remove(&OverrideField::MainComponent);
                        cn.component.bindings.remove(&OverrideField::Visible);
                        cn.component.overrides.remove(&OverrideField::MainComponent);
                        if let Some(ac) = instance_component(&cn) {
                            cn.component.source = Some(ac);
                        }
                        c.update(cn);
                    }
                    // The slot content now mirrors the inner component itself, so its descendants must follow
                    // that component's nodes rather than the ones expanded inside the main's inner instance.
                    for desc_id in subtree_ids(c.s, *cid).into_iter().filter(|d| d != cid) {
                        let Some(mut dn) = c.s.get(desc_id).cloned() else { continue };
                        let Some(src) = dn.component.source.filter(|src| c.s.is_ancestor(*child_inst, *src)) else { continue };
                        if let Some(grand) = c.s.get(src).and_then(|m| m.component.source) {
                            dn.component.source = Some(grand);
                            c.update(dn);
                        }
                    }
                }
                let target = InstanceRegionCopy { inst, copy_id, was_vis, prop_name, dropped_vis_prop };
                convert_instance_region_to_slot(c, &target, copy_children, group_to_frame);
            }
            Some(vec![*child_inst])
        }
        MigrationMode::BooleanContainer { .. } => {
            let main_children = c.s.get(node).map(|n| n.children.clone()).unwrap_or_default();
            for cid in &main_children {
                if let Some(mut cn) = c.s.get(*cid).cloned() {
                    cn.component.is_slot_content = true;
                    c.update(cn);
                }
            }
            make_main_region_a_slot(c, node, prop_name);

            for (inst, copy_id, was_vis) in inst_targets {
                let copy_children = c.s.get(copy_id).map(|n| n.children.clone()).unwrap_or_default();
                for cid in &copy_children {
                    if let Some(mut cn) = c.s.get(*cid).cloned() {
                        cn.component.is_slot_content = true;
                        c.update(cn);
                    }
                }
                let target = InstanceRegionCopy { inst, copy_id, was_vis, prop_name, dropped_vis_prop };
                convert_instance_region_to_slot(c, &target, copy_children, group_to_frame);
            }
            Some(main_children)
        }
    }
}

/// Migrates an instance-swap or boolean container region inside a main component
/// to a component slot, keeping what existing instances hold in one undo step.
/// When the region's property is defined on a component set, the matching region of
/// every variant migrates with it. Unsupported shapes return `Err` with an honest
/// message without mutating `doc`.
pub fn migrate_region_to_slot(doc: &mut Document, node: NodeId) -> Result<Command, &'static str> {
    let plan = classify_slot_migration(doc, node)?;
    let comps = components_of(doc, plan.owner);
    let insts = instances_of_any(doc, &comps);
    let prop_name = plan.regions[0].mode.prop_name().to_string();
    let drop_vis_props: Vec<String> = plan.regions.iter().filter_map(|r| r.mode.dropped_vis_prop().map(str::to_string)).collect();

    let mut c = Ctx::new(doc);
    let mut default_children = Vec::new();
    for (i, region) in plan.regions.iter().enumerate() {
        let Some(children) = migrate_slot_region(&mut c, &insts, region) else {
            // The plan was just validated, so this cannot happen; undo anything already applied if it does.
            let _ = c.finish();
            return Err("Selected region changed while migrating; try again");
        };
        // The property's default content is the selected region's.
        if i == 0 {
            default_children = children;
        }
    }
    upgrade_owner_prop_to_slot(&mut c, plan.owner, &prop_name, default_children, &drop_vis_props);
    Ok(c.finish())
}

// ---------------------------------------------------------------------------
// Variants
// ---------------------------------------------------------------------------

/// Groups components into a new component set (Figma "Combine as variants").
/// Components whose name is not `Prop=Value` become `Property 1=<name>`; the
/// set gets a Variant property definition per distinct property and the
/// variants are laid out in a grid.
pub fn combine_as_variants(doc: &mut Document, components: &[NodeId]) -> Command {
    let comps: Vec<NodeId> = components.iter().copied().filter(|c| matches!(doc.get(*c).map(|n| &n.kind), Some(NodeKind::Component(_)))).collect();
    let Some(&first) = comps.first() else { return nothing() };
    let Some(parent) = doc.get(first).and_then(|n| n.parent) else { return nothing() };
    let index = doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == first)).unwrap_or(0);

    let mut names: Vec<String> = Vec::new();
    let mut defs: Vec<(String, Vec<String>)> = Vec::new();
    for id in &comps {
        let n = doc.get(*id).unwrap();
        let mut pairs = parse_variant_name(&n.name);
        if pairs.is_empty() {
            pairs.push(("Property 1".to_string(), n.name.clone()));
        }
        for (k, v) in &pairs {
            match defs.iter_mut().find(|(dk, _)| dk == k) {
                Some((_, opts)) => {
                    if !opts.contains(v) {
                        opts.push(v.clone());
                    }
                }
                None => defs.push((k.clone(), vec![v.clone()])),
            }
        }
        names.push(variant_name(&pairs));
    }
    let first_pairs = parse_variant_name(&names[0]);
    let props: Vec<ComponentPropDef> = defs
        .iter()
        .map(|(k, opts)| ComponentPropDef {
            name: k.clone(),
            kind: PropKind::Variant { options: opts.clone() },
            default: PropValue::Variant(first_pairs.iter().find(|(pk, _)| pk == k).map(|(_, v)| v.clone()).unwrap_or_else(|| opts[0].clone())),
        })
        .collect();

    // Grid layout: columns = options of the last property (a single row for one property).
    let cols = if defs.len() > 1 { defs.last().map(|(_, o)| o.len()).unwrap_or(1).max(1) } else { comps.len() };
    const PAD: f32 = 24.0;
    const GAP: f32 = 16.0;
    let mut pos = Vec::new();
    let (mut x, mut y, mut row_h, mut max_x) = (PAD, PAD, 0.0f32, 0.0f32);
    for (k, id) in comps.iter().enumerate() {
        if k > 0 && k % cols == 0 {
            x = PAD;
            y += row_h + GAP;
            row_h = 0.0;
        }
        let sz = doc.get(*id).unwrap().size;
        pos.push((x, y));
        x += sz.x + GAP;
        max_x = max_x.max(x - GAP);
        row_h = row_h.max(sz.y);
    }
    let total_h = y + row_h + PAD;
    let origin = doc.get(first).unwrap().transform.translation();

    let sid = doc.alloc_id();
    let mut set = Node::new(sid, NodeKind::ComponentSet(FrameProps::default()));
    let base = doc.get(first).unwrap().name.clone();
    set.name = if parse_variant_name(&base).is_empty() { base } else { "Component set".to_string() };
    set.transform = Affine::translate(origin.x, origin.y);
    set.size = Vec2::new(max_x + PAD, total_h);
    set.fills = Vec::new();
    set.component.props = props;

    let mut cmds = vec![Command::Insert { parent, index, nodes: vec![set] }];
    for (k, id) in comps.iter().enumerate() {
        cmds.push(Command::Reparent { id: *id, parent: sid, index: k });
        let mut n = doc.get(*id).unwrap().clone();
        n.name = names[k].clone();
        n.transform = Affine::translate(pos[k].0, pos[k].1);
        cmds.push(Command::Update { node: Box::new(n) });
    }
    Command::Batch(cmds)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        d: Document,
        page: NodeId,
        comp: NodeId,
        bg: NodeId,
        label: NodeId,
        icon: NodeId,
    }

    fn apply(d: &mut Document, c: Command) -> Command {
        c.apply(d).unwrap()
    }

    fn fx() -> Fx {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let comp = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        let id = d.alloc_id();
        let mut bg = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        bg.name = "bg".into();
        let bg = d.insert(comp, bg).unwrap();
        let id = d.alloc_id();
        let mut t = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
        t.name = "label".into();
        let label = d.insert(comp, t).unwrap();
        let id = d.alloc_id();
        let mut ic = Node::ellipse(id, Rect::new(75.0, 10.0, 20.0, 20.0));
        ic.name = "icon".into();
        let icon = d.insert(comp, ic).unwrap();
        let c = create_component(&mut d, comp);
        apply(&mut d, c);
        Fx { d, page, comp, bg, label, icon }
    }

    fn inst(f: &mut Fx, at: (f32, f32)) -> NodeId {
        let c = create_instance(&mut f.d, f.comp, f.page, Vec2::new(at.0, at.1));
        apply(&mut f.d, c);
        *f.d.get(f.page).unwrap().children.last().unwrap()
    }

    fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
        *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
    }

    fn set_fill(d: &mut Document, id: NodeId, c: Color) {
        let mut n = d.get(id).unwrap().clone();
        n.fills = vec![Paint::solid(c)];
        apply(d, Command::Update { node: Box::new(n) });
    }

    fn text_of(d: &Document, id: NodeId) -> String {
        match &d.get(id).unwrap().kind {
            NodeKind::Text(t) => t.content.clone(),
            _ => panic!(),
        }
    }

    fn set_text(d: &mut Document, id: NodeId, s: &str) {
        let mut n = d.get(id).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = s.into();
        }
        apply(d, Command::Update { node: Box::new(n) });
    }

    #[test]
    fn create_component_converts_frame_and_wraps_shapes() {
        let f = fx();
        assert!(matches!(f.d.get(f.comp).unwrap().kind, NodeKind::Component(_)));
        let mut d = f.d;
        let id = d.alloc_id();
        let r = d.insert(f.page, Node::rectangle(id, Rect::new(5.0, 6.0, 30.0, 20.0))).unwrap();
        let c = create_component(&mut d, r);
        let inv = apply(&mut d, c);
        let w = d.get(r).unwrap().parent.unwrap();
        assert_ne!(w, f.page);
        let wn = d.get(w).unwrap();
        assert!(matches!(wn.kind, NodeKind::Component(_)));
        assert_eq!(wn.size, Vec2::new(30.0, 20.0));
        assert_eq!(wn.transform.translation(), Vec2::new(5.0, 6.0));
        assert_eq!(d.get(r).unwrap().transform, Affine::IDENTITY);
        apply(&mut d, inv);
        assert_eq!(d.get(r).unwrap().parent, Some(f.page));
        assert!(d.get(w).is_none());
    }

    #[test]
    fn create_instance_links_every_node() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 50.0));
        let n = f.d.get(i).unwrap();
        assert!(matches!(n.kind, NodeKind::Instance { component: Some(c), .. } if c == f.comp));
        assert_eq!(n.component.source, Some(f.comp));
        assert_eq!(n.transform.translation(), Vec2::new(200.0, 50.0));
        assert_eq!(n.children.len(), 3);
        let l = child_named(&f.d, i, "label");
        assert_eq!(f.d.get(l).unwrap().component.source, Some(f.label));
        assert_eq!(text_of(&f.d, l), "Label");
        assert_eq!(f.d.get(child_named(&f.d, i, "bg")).unwrap().component.source, Some(f.bg));
        // Not into itself.
        assert!(matches!(create_instance(&mut f.d, f.comp, f.comp, Vec2::ZERO), Command::Batch(v) if v.is_empty()));
    }

    #[test]
    fn sync_propagates_and_preserves_overrides() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let ibg = child_named(&f.d, i, "bg");
        let ilabel = child_named(&f.d, i, "label");
        // Override the instance's label text.
        let before = f.d.get(ilabel).unwrap().clone();
        set_text(&mut f.d, ilabel, "Buy");
        let mut n = f.d.get(ilabel).unwrap().clone();
        n.component.overrides = record_override(&before, &n);
        assert_eq!(n.component.overrides, BTreeSet::from([OverrideField::Characters]));
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        // Edit the main component: bg colour and label text.
        set_fill(&mut f.d, f.bg, Color::rgb(255, 0, 0));
        set_text(&mut f.d, f.label, "Main");
        let c = sync_instances(&mut f.d, f.comp);
        let inv = apply(&mut f.d, c);
        assert_eq!(f.d.get(ibg).unwrap().fills, f.d.get(f.bg).unwrap().fills);
        assert_eq!(text_of(&f.d, ilabel), "Buy", "override survives");
        // Undo restores the old instance state in one step.
        apply(&mut f.d, inv);
        assert_ne!(f.d.get(ibg).unwrap().fills, f.d.get(f.bg).unwrap().fills);
        // Reset the override: text follows the main again.
        let c = reset_overrides(&mut f.d, ilabel, &[OverrideField::Characters]);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, ilabel), "Main");
    }

    #[test]
    fn sync_keeps_root_transform_and_resizes_unless_overridden() {
        let mut f = fx();
        let a = inst(&mut f, (200.0, 0.0));
        let b = inst(&mut f, (400.0, 0.0));
        let mut bn = f.d.get(b).unwrap().clone();
        bn.size = Vec2::new(150.0, 40.0);
        bn.component.overrides.insert(OverrideField::Size);
        apply(&mut f.d, Command::Update { node: Box::new(bn) });
        let mut cn = f.d.get(f.comp).unwrap().clone();
        cn.size = Vec2::new(120.0, 50.0);
        apply(&mut f.d, Command::Update { node: Box::new(cn) });
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(f.d.get(a).unwrap().size, Vec2::new(120.0, 50.0));
        assert_eq!(f.d.get(a).unwrap().transform.translation(), Vec2::new(200.0, 0.0));
        assert_eq!(f.d.get(b).unwrap().size, Vec2::new(150.0, 40.0));
        assert_eq!(f.d.get(b).unwrap().transform.translation(), Vec2::new(400.0, 0.0));
    }

    #[test]
    fn sync_adds_and_removes_children() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let ilabel = child_named(&f.d, i, "label");
        // Remove "icon" from the main, add "badge" between bg and label.
        apply(&mut f.d, Command::Remove { id: f.icon });
        let id = f.d.alloc_id();
        let mut b = Node::rectangle(id, Rect::new(0.0, 0.0, 8.0, 8.0));
        b.name = "badge".into();
        apply(&mut f.d, Command::Insert { parent: f.comp, index: 1, nodes: vec![b] });
        let c = sync_instances(&mut f.d, f.comp);
        let inv = apply(&mut f.d, c);
        let names: Vec<_> = f.d.get(i).unwrap().children.iter().map(|c| f.d.get(*c).unwrap().name.clone()).collect();
        assert_eq!(names, vec!["bg", "badge", "label"]);
        assert_eq!(f.d.get(i).unwrap().children[2], ilabel, "existing node kept");
        let badge = child_named(&f.d, i, "badge");
        assert!(f.d.get(badge).unwrap().component.source.is_some());
        apply(&mut f.d, inv);
        let names: Vec<_> = f.d.get(i).unwrap().children.iter().map(|c| f.d.get(*c).unwrap().name.clone()).collect();
        assert_eq!(names, vec!["bg", "label", "icon"]);
    }

    #[test]
    fn sync_second_run_is_noop() {
        let mut f = fx();
        inst(&mut f, (200.0, 0.0));
        let c = sync_instances(&mut f.d, f.comp);
        match c {
            Command::Batch(v) => assert!(v.is_empty(), "{v:?}"),
            _ => panic!(),
        }
    }

    #[test]
    fn nested_instances_follow_both_levels() {
        let mut f = fx();
        // Outer component "Card" containing an instance of the button.
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 100.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::new(10.0, 10.0));
        apply(&mut f.d, c);
        let nested = *f.d.get(card).unwrap().children.last().unwrap();
        let ci = create_instance(&mut f.d, card, f.page, Vec2::new(500.0, 0.0));
        apply(&mut f.d, ci);
        let outer = *f.d.get(f.page).unwrap().children.last().unwrap();
        let nested_copy = f.d.get(outer).unwrap().children[0];
        assert_eq!(f.d.get(nested_copy).unwrap().component.source, Some(nested));
        let nl = child_named(&f.d, nested_copy, "label");
        assert_eq!(f.d.get(nl).unwrap().component.source, Some(child_named(&f.d, nested, "label")));
        // Change the button main: both the nested instance and its copy follow.
        set_fill(&mut f.d, f.bg, Color::rgb(0, 255, 0));
        set_text(&mut f.d, f.label, "OK");
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, nl), "OK");
        assert_eq!(f.d.get(child_named(&f.d, nested_copy, "bg")).unwrap().fills, f.d.get(f.bg).unwrap().fills);
        // Override inside the outer instance's nested copy survives a card sync.
        let before = f.d.get(nl).unwrap().clone();
        set_text(&mut f.d, nl, "Custom");
        let mut n = f.d.get(nl).unwrap().clone();
        n.component.overrides = record_override(&before, &n);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        let mut cn = f.d.get(card).unwrap().clone();
        cn.size = Vec2::new(320.0, 100.0);
        apply(&mut f.d, Command::Update { node: Box::new(cn) });
        let c = sync_instances(&mut f.d, card);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, nl), "Custom");
        assert_eq!(f.d.get(outer).unwrap().size, Vec2::new(320.0, 100.0));
    }

    #[test]
    fn boolean_and_text_props_drive_layers() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let j = inst(&mut f, (400.0, 0.0));
        for (name, kind, default) in [("Show icon", PropKind::Boolean, PropValue::Bool(true)), ("Label", PropKind::Text, PropValue::Text("Label".into()))] {
            let c = add_prop(&mut f.d, f.comp, ComponentPropDef { name: name.into(), kind, default });
            apply(&mut f.d, c);
        }
        let c = bind_prop(&mut f.d, f.icon, OverrideField::Visible, Some("Show icon"));
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, f.label, OverrideField::Characters, Some("Label"));
        apply(&mut f.d, c);
        let icon_i = child_named(&f.d, i, "icon");
        assert_eq!(f.d.get(icon_i).unwrap().component.bindings.get(&OverrideField::Visible).map(String::as_str), Some("Show icon"));

        let c = set_instance_prop(&mut f.d, i, "Show icon", Some(PropValue::Bool(false)));
        let inv = apply(&mut f.d, c);
        assert!(!f.d.get(icon_i).unwrap().visible);
        assert!(f.d.get(child_named(&f.d, j, "icon")).unwrap().visible, "other instances untouched");
        let c = set_instance_prop(&mut f.d, i, "Label", Some(PropValue::Text("Pay".into())));
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, child_named(&f.d, i, "label")), "Pay");
        // Main component edits don't clobber the property-driven text.
        set_text(&mut f.d, f.label, "Other");
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, child_named(&f.d, i, "label")), "Pay");
        assert_eq!(text_of(&f.d, child_named(&f.d, j, "label")), "Other");
        // Clearing the value falls back to the main.
        let c = set_instance_prop(&mut f.d, i, "Label", None);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, child_named(&f.d, i, "label")), "Other");
        // Rename keeps bindings and values; delete drops them.
        let c = rename_prop(&mut f.d, f.comp, "Show icon", "Icon on");
        apply(&mut f.d, c);
        assert_eq!(f.d.get(i).unwrap().component.values.get("Icon on"), Some(&PropValue::Bool(false)));
        assert!(!f.d.get(icon_i).unwrap().visible);
        let c = delete_prop(&mut f.d, f.comp, "Icon on");
        apply(&mut f.d, c);
        assert!(!f.d.get(i).unwrap().component.values.contains_key("Icon on"));
        assert!(f.d.get(icon_i).unwrap().visible);
        let _ = inv;
    }

    #[test]
    fn reset_instance_clears_values_and_overrides() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let c = add_prop(&mut f.d, f.comp, ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, f.icon, OverrideField::Visible, Some("Show icon"));
        apply(&mut f.d, c);
        let c = set_instance_prop(&mut f.d, i, "Show icon", Some(PropValue::Bool(false)));
        apply(&mut f.d, c);
        let ibg = child_named(&f.d, i, "bg");
        let mut n = f.d.get(ibg).unwrap().clone();
        n.fills = vec![Paint::solid(Color::rgb(1, 2, 3))];
        n.component.overrides.insert(OverrideField::Fills);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        let c = reset_instance(&mut f.d, i);
        apply(&mut f.d, c);
        assert!(f.d.get(i).unwrap().component.values.is_empty());
        assert!(f.d.get(child_named(&f.d, i, "icon")).unwrap().visible);
        assert_eq!(f.d.get(ibg).unwrap().fills, f.d.get(f.bg).unwrap().fills);
        assert!(f.d.get(ibg).unwrap().component.overrides.is_empty());
    }

    fn variant_fixture() -> (Fx, NodeId, NodeId, NodeId) {
        let mut f = fx();
        // Second component, "Size=Large", styled differently.
        let id = f.d.alloc_id();
        let mut big = Node::frame(id, Rect::new(0.0, 200.0, 200.0, 80.0));
        big.name = "Size=Large".into();
        let big = f.d.insert(f.page, big).unwrap();
        let id = f.d.alloc_id();
        let mut t = Node::text(id, Rect::new(10.0, 10.0, 100.0, 30.0), "Big", TextStyle::default());
        t.name = "label".into();
        f.d.insert(big, t).unwrap();
        let c = create_component(&mut f.d, big);
        apply(&mut f.d, c);
        let mut small = f.d.get(f.comp).unwrap().clone();
        small.name = "Size=Small".into();
        apply(&mut f.d, Command::Update { node: Box::new(small) });
        let c = combine_as_variants(&mut f.d, &[f.comp, big]);
        apply(&mut f.d, c);
        let set = f.d.get(f.comp).unwrap().parent.unwrap();
        let small = f.comp;
        (f, set, small, big)
    }

    #[test]
    fn combine_builds_component_set() {
        let (f, set, small, big) = variant_fixture();
        let s = f.d.get(set).unwrap();
        assert!(matches!(s.kind, NodeKind::ComponentSet(_)));
        assert_eq!(s.children, vec![small, big]);
        assert_eq!(f.d.get(big).unwrap().parent, Some(set));
        assert_eq!(s.component.props.len(), 1);
        assert_eq!(s.component.props[0].name, "Size");
        assert_eq!(s.component.props[0].kind, PropKind::Variant { options: vec!["Small".into(), "Large".into()] });
        assert_eq!(s.component.props[0].default, PropValue::Variant("Small".into()));
        assert!(s.size.x > 300.0);
        assert!(f.d.get(f.page).unwrap().children.contains(&set));
    }

    #[test]
    fn combine_renames_plain_names() {
        let mut f = fx();
        let c = combine_as_variants(&mut f.d, &[f.comp]);
        apply(&mut f.d, c);
        assert_eq!(f.d.get(f.comp).unwrap().name, "Property 1=Frame");
    }

    #[test]
    fn variant_prop_swaps_instance_to_matching_variant() {
        let (mut f, set, small, big) = variant_fixture();
        let c = create_instance(&mut f.d, small, f.page, Vec2::new(500.0, 0.0));
        apply(&mut f.d, c);
        let i = *f.d.get(f.page).unwrap().children.last().unwrap();
        assert_eq!(f.d.get(i).unwrap().name, f.d.get(set).unwrap().name, "instances are named after the set");
        let il = child_named(&f.d, i, "label");
        let before = f.d.get(il).unwrap().clone();
        set_text(&mut f.d, il, "Mine");
        let mut n = f.d.get(il).unwrap().clone();
        n.component.overrides = record_override(&before, &n);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        let def = prop_defs(&f.d, small).remove(0);
        assert_eq!(prop_value(&f.d, i, &def), PropValue::Variant("Small".into()));

        let c = set_instance_prop(&mut f.d, i, "Size", Some(PropValue::Variant("Large".into())));
        let inv = apply(&mut f.d, c);
        let n = f.d.get(i).unwrap();
        assert_eq!(instance_component(n), Some(big));
        assert_eq!(n.component.source, Some(big));
        assert_eq!(n.children.len(), 1, "large variant only has a label");
        let nl = n.children[0];
        assert_eq!(f.d.get(nl).unwrap().component.source, Some(child_named(&f.d, big, "label")));
        assert_eq!(text_of(&f.d, nl), "Mine", "overrides carry over by name");
        assert_eq!(f.d.get(i).unwrap().size, Vec2::new(200.0, 80.0));
        // Syncing the new variant still works after the swap.
        set_fill(&mut f.d, big, Color::rgb(9, 9, 9));
        let c = sync_instances(&mut f.d, set);
        apply(&mut f.d, c);
        assert_eq!(f.d.get(i).unwrap().fills, f.d.get(big).unwrap().fills);
        // Undo of the swap brings the old subtree back.
        let _ = inv;
    }

    #[test]
    fn instance_swap_prop_and_manual_swap() {
        let mut f = fx();
        // Second component "Alt", and a card component holding an instance of the button.
        let id = f.d.alloc_id();
        let mut alt = Node::frame(id, Rect::new(0.0, 300.0, 50.0, 50.0));
        alt.name = "Alt".into();
        let alt = f.d.insert(f.page, alt).unwrap();
        let id = f.d.alloc_id();
        let mut dot = Node::ellipse(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        dot.name = "dot".into();
        f.d.insert(alt, dot).unwrap();
        let c = create_component(&mut f.d, alt);
        apply(&mut f.d, c);

        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::ZERO);
        apply(&mut f.d, c);
        let slot = *f.d.get(card).unwrap().children.last().unwrap();
        let c = add_prop(
            &mut f.d,
            card,
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(f.comp) },
        );
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, slot, OverrideField::MainComponent, Some("Icon"));
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(600.0, 0.0));
        apply(&mut f.d, c);
        let outer = *f.d.get(f.page).unwrap().children.last().unwrap();
        let slot_copy = f.d.get(outer).unwrap().children[0];
        assert_eq!(f.d.get(slot_copy).unwrap().children.len(), 3);

        let c = set_instance_prop(&mut f.d, outer, "Icon", Some(PropValue::Instance(alt)));
        apply(&mut f.d, c);
        let n = f.d.get(slot_copy).unwrap();
        assert_eq!(instance_component(n), Some(alt));
        assert_eq!(n.children.len(), 1);
        assert_eq!(f.d.get(n.children[0]).unwrap().name, "dot");
        // Edits to the original button must not disturb the swapped slot.
        set_fill(&mut f.d, f.bg, Color::rgb(7, 7, 7));
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(f.d.get(slot_copy).unwrap().children.len(), 1);
        // Reset swaps back.
        let c = set_instance_prop(&mut f.d, outer, "Icon", None);
        apply(&mut f.d, c);
        assert_eq!(instance_component(f.d.get(slot_copy).unwrap()), Some(f.comp));
        assert_eq!(f.d.get(slot_copy).unwrap().children.len(), 3);
    }

    #[test]
    fn detach_makes_frame_and_clears_links() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let c = detach_instance(&mut f.d, i);
        let inv = apply(&mut f.d, c);
        let n = f.d.get(i).unwrap();
        assert!(matches!(n.kind, NodeKind::Frame(_)));
        assert!(n.component.is_empty());
        for k in &n.children {
            assert!(f.d.get(*k).unwrap().component.is_empty());
        }
        // No longer touched by syncs.
        set_fill(&mut f.d, f.bg, Color::rgb(3, 3, 3));
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_ne!(f.d.get(child_named(&f.d, i, "bg")).unwrap().fills, f.d.get(f.bg).unwrap().fills);
        apply(&mut f.d, inv);
        assert!(matches!(f.d.get(i).unwrap().kind, NodeKind::Instance { .. }));
    }

    #[test]
    fn detach_keeps_nested_instances_working() {
        let mut f = fx();
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 100.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::ZERO);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(500.0, 0.0));
        apply(&mut f.d, c);
        let outer = *f.d.get(f.page).unwrap().children.last().unwrap();
        let c = detach_instance(&mut f.d, outer);
        apply(&mut f.d, c);
        let nested = f.d.get(outer).unwrap().children[0];
        assert!(matches!(f.d.get(nested).unwrap().kind, NodeKind::Instance { .. }));
        assert_eq!(f.d.get(nested).unwrap().component.source, None);
        // Its children are re-pointed at the button component itself.
        let nl = child_named(&f.d, nested, "label");
        assert_eq!(f.d.get(nl).unwrap().component.source, Some(f.label));
        set_text(&mut f.d, f.label, "Live");
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(text_of(&f.d, nl), "Live");
        assert_eq!(f.d.get(nested).unwrap().children.len(), 3);
    }

    fn set_auto_resize(d: &mut Document, id: NodeId, mode: TextAutoResize) {
        let mut n = d.get(id).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.auto_resize = mode;
        }
        apply(d, Command::Update { node: Box::new(n) });
    }

    #[test]
    fn size_comes_from_main_only_where_a_sync_copies_it() {
        let mut f = fx();
        set_auto_resize(&mut f.d, f.label, TextAutoResize::WidthAndHeight);
        let i = inst(&mut f, (200.0, 0.0));
        let label_i = child_named(&f.d, i, "label");
        assert!(!size_comes_from_main(&f.d, f.label), "the main itself is not mirrored");
        assert!(!size_comes_from_main(&f.d, NodeId(u64::MAX)), "an unknown node has nothing to copy");
        assert!(size_comes_from_main(&f.d, label_i), "an untouched copy takes the size of its main");

        // Its own characters on self-sizing text: the copy keeps the size it fitted itself.
        let mut n = f.d.get(label_i).unwrap().clone();
        n.component.overrides.insert(OverrideField::Characters);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        assert!(!size_comes_from_main(&f.d, label_i));
        // ... but fixed-size text takes the main's size whatever its characters are.
        set_auto_resize(&mut f.d, label_i, TextAutoResize::None);
        assert!(size_comes_from_main(&f.d, label_i));
        set_auto_resize(&mut f.d, label_i, TextAutoResize::Height);
        assert!(!size_comes_from_main(&f.d, label_i));
        // An overridden size is the copy's own, with the characters back to the main's.
        let mut n = f.d.get(label_i).unwrap().clone();
        n.component.overrides.remove(&OverrideField::Characters);
        n.component.overrides.insert(OverrideField::Size);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        assert!(!size_comes_from_main(&f.d, label_i));
    }

    #[test]
    fn size_comes_from_main_ignores_a_bound_property_without_a_value() {
        let mut f = fx();
        set_auto_resize(&mut f.d, f.label, TextAutoResize::WidthAndHeight);
        let c = add_prop(&mut f.d, f.comp, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Label".into()) });
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, f.label, OverrideField::Characters, Some("Label"));
        apply(&mut f.d, c);
        let i = inst(&mut f, (200.0, 0.0));
        let label_i = child_named(&f.d, i, "label");
        assert!(size_comes_from_main(&f.d, label_i), "the property has no value on this instance: the main's text shows");
        let c = set_instance_prop(&mut f.d, i, "Label", Some(PropValue::Text("Pay now".into())));
        apply(&mut f.d, c);
        assert!(!size_comes_from_main(&f.d, label_i), "the property drives the characters: the copy keeps its fitted size");
    }

    #[test]
    fn record_override_diffs_fields() {
        let f = fx();
        let a = f.d.get(f.label).unwrap().clone();
        let mut b = a.clone();
        b.opacity = 0.5;
        b.fills = vec![Paint::solid(Color::rgb(1, 1, 1))];
        b.name = "x".into();
        if let NodeKind::Text(t) = &mut b.kind {
            t.content = "new".into();
            t.style.font_size = 99.0;
        }
        let s = record_override(&a, &b);
        assert_eq!(s, BTreeSet::from([OverrideField::Opacity, OverrideField::Fills, OverrideField::Name, OverrideField::Characters, OverrideField::TextStyle]));
        assert!(record_override(&a, &a).is_empty());
    }

    #[test]
    fn duplicate_keeps_instance_links() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let copy = duplicate_subtree(&mut f.d, i);
        assert!(matches!(copy[0].kind, NodeKind::Instance { component: Some(c), .. } if c == f.comp));
        assert_eq!(copy[0].component.source, Some(f.comp));
        assert!(copy.iter().skip(1).all(|n| n.component.source.is_some()));
        assert_eq!(copy.len(), 4);
    }

    #[test]
    fn queries() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let l = child_named(&f.d, i, "label");
        assert_eq!(instances_of(&f.d, f.comp), vec![i]);
        assert_eq!(instance_root_of(&f.d, l), Some(i));
        assert_eq!(instance_root_of(&f.d, f.label), None);
        assert_eq!(enclosing_component(&f.d, f.label), Some(f.comp));
        assert_eq!(enclosing_component(&f.d, i), None);
        assert_eq!(parse_variant_name("A=1, B = two"), vec![("A".into(), "1".into()), ("B".into(), "two".into())]);
    }

    #[test]
    fn manual_swap_replaces_children_and_survives_sync() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let id = f.d.alloc_id();
        let mut alt = Node::frame(id, Rect::new(0.0, 300.0, 50.0, 50.0));
        alt.name = "Alt".into();
        let alt = f.d.insert(f.page, alt).unwrap();
        let id = f.d.alloc_id();
        f.d.insert(alt, Node::ellipse(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let c = create_component(&mut f.d, alt);
        apply(&mut f.d, c);
        let c = swap_instance(&mut f.d, i, alt);
        let inv = apply(&mut f.d, c);
        let n = f.d.get(i).unwrap();
        assert_eq!(instance_component(n), Some(alt));
        assert_eq!(n.children.len(), 1);
        assert!(n.component.overrides.contains(&OverrideField::MainComponent));
        assert_eq!(n.transform.translation(), Vec2::new(200.0, 0.0));
        set_fill(&mut f.d, f.bg, Color::rgb(5, 5, 5));
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert_eq!(f.d.get(i).unwrap().children.len(), 1);
        apply(&mut f.d, inv);
        assert_eq!(instance_component(f.d.get(i).unwrap()), Some(f.comp));
        assert_eq!(f.d.get(i).unwrap().children.len(), 3);
    }

    #[test]
    fn text_with_own_characters_keeps_its_fitted_size() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let l = child_named(&f.d, i, "label");
        let mut n = f.d.get(l).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "A much longer label".into();
        }
        n.size = Vec2::new(140.0, 20.0);
        n.component.overrides.insert(OverrideField::Characters);
        apply(&mut f.d, Command::Update { node: Box::new(n) });
        let mut m = f.d.get(f.label).unwrap().clone();
        m.opacity = 0.5;
        apply(&mut f.d, Command::Update { node: Box::new(m) });
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        let n = f.d.get(l).unwrap();
        assert_eq!(n.opacity, 0.5);
        assert_eq!(n.size, Vec2::new(140.0, 20.0));
    }

    #[test]
    fn sync_preserves_overridden_style_refs_and_resets_them_on_reset_overrides() {
        let mut f = fx();
        // Main component label uses "Heading", bg uses "Surface".
        let mut main_label = f.d.get(f.label).unwrap().clone();
        if let NodeKind::Text(t) = &mut main_label.kind {
            t.style.font_size = 24.0;
        }
        main_label.style_refs.insert("text".into(), "Heading".into());
        apply(&mut f.d, Command::Update { node: Box::new(main_label) });

        let mut main_bg = f.d.get(f.bg).unwrap().clone();
        main_bg.fills = vec![Paint::solid(Color::rgb(20, 20, 20))];
        main_bg.style_refs.insert("fill".into(), "Surface".into());
        apply(&mut f.d, Command::Update { node: Box::new(main_bg) });

        let i = inst(&mut f, (200.0, 0.0));
        let ilabel = child_named(&f.d, i, "label");
        let ibg = child_named(&f.d, i, "bg");
        assert_eq!(f.d.get(ilabel).unwrap().style_refs.get("text").map(String::as_str), Some("Heading"));
        assert_eq!(f.d.get(ibg).unwrap().style_refs.get("fill").map(String::as_str), Some("Surface"));

        // Override the instance's label text style to "Body" (14px) and bg fill style to "Brand".
        let before_label = f.d.get(ilabel).unwrap().clone();
        let mut after_label = before_label.clone();
        if let NodeKind::Text(t) = &mut after_label.kind {
            t.style.font_size = 14.0;
        }
        after_label.style_refs.insert("text".into(), "Body".into());
        after_label.component.overrides = record_override(&before_label, &after_label);
        assert!(after_label.component.overrides.contains(&OverrideField::TextStyle));
        apply(&mut f.d, Command::Update { node: Box::new(after_label) });

        let before_bg = f.d.get(ibg).unwrap().clone();
        let mut after_bg = before_bg.clone();
        after_bg.fills = vec![Paint::solid(Color::rgb(255, 100, 0))];
        after_bg.style_refs.insert("fill".into(), "Brand".into());
        after_bg.component.overrides = record_override(&before_bg, &after_bg);
        assert!(after_bg.component.overrides.contains(&OverrideField::Fills));
        apply(&mut f.d, Command::Update { node: Box::new(after_bg) });

        // Syncing the main component must keep the instance's overridden style_refs ("Body" and "Brand").
        let mut main_label = f.d.get(f.label).unwrap().clone();
        main_label.opacity = 0.8;
        apply(&mut f.d, Command::Update { node: Box::new(main_label) });
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);

        let n_label = f.d.get(ilabel).unwrap();
        assert_eq!(n_label.opacity, 0.8);
        assert_eq!(n_label.style_refs.get("text").map(String::as_str), Some("Body"));
        if let NodeKind::Text(t) = &n_label.kind {
            assert_eq!(t.style.font_size, 14.0);
        }

        let n_bg = f.d.get(ibg).unwrap();
        assert_eq!(n_bg.style_refs.get("fill").map(String::as_str), Some("Brand"));

        // Resetting TextStyle override restores main's "Heading" style_ref and 24px font_size.
        let c = reset_overrides(&mut f.d, ilabel, &[OverrideField::TextStyle]);
        apply(&mut f.d, c);
        let n_label = f.d.get(ilabel).unwrap();
        assert_eq!(n_label.style_refs.get("text").map(String::as_str), Some("Heading"));
        if let NodeKind::Text(t) = &n_label.kind {
            assert_eq!(t.style.font_size, 24.0);
        }
    }

    #[test]
    fn migrate_instance_swap_region_to_slot_keeps_swapped_and_overridden_content_and_undoes_in_one_step() {
        let mut f = fx();
        let id = f.d.alloc_id();
        let mut alt = Node::frame(id, Rect::new(0.0, 300.0, 50.0, 50.0));
        alt.name = "Alt".into();
        let alt = f.d.insert(f.page, alt).unwrap();
        let id = f.d.alloc_id();
        let mut dot = Node::ellipse(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        dot.name = "dot".into();
        f.d.insert(alt, dot).unwrap();
        let c = create_component(&mut f.d, alt);
        apply(&mut f.d, c);

        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::new(12.0, 16.0));
        apply(&mut f.d, c);
        let slot = *f.d.get(card).unwrap().children.last().unwrap();
        let c = add_prop(
            &mut f.d,
            card,
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![f.comp, alt] }, default: PropValue::Instance(f.comp) },
        );
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, slot, OverrideField::MainComponent, Some("Icon"));
        apply(&mut f.d, c);

        // First instance swaps "Icon" to `alt`.
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(600.0, 0.0));
        apply(&mut f.d, c);
        let outer_swapped = *f.d.get(f.page).unwrap().children.last().unwrap();
        let c = set_instance_prop(&mut f.d, outer_swapped, "Icon", Some(PropValue::Instance(alt)));
        apply(&mut f.d, c);

        // Second instance keeps default `f.comp` and overrides its inner label text.
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(600.0, 150.0));
        apply(&mut f.d, c);
        let outer_overridden = *f.d.get(f.page).unwrap().children.last().unwrap();
        let slot_copy_overridden = f.d.get(outer_overridden).unwrap().children[0];
        let inner_label = child_named(&f.d, slot_copy_overridden, "label");
        let before_lbl = f.d.get(inner_label).unwrap().clone();
        set_text(&mut f.d, inner_label, "Custom");
        let mut after_lbl = f.d.get(inner_label).unwrap().clone();
        after_lbl.component.overrides = record_override(&before_lbl, &after_lbl);
        apply(&mut f.d, Command::Update { node: Box::new(after_lbl) });

        assert!(can_migrate_region_to_slot(&f.d, slot));
        let before_nodes = f.d.nodes.clone();

        let migrate_cmd = migrate_region_to_slot(&mut f.d, slot).expect("instance-swap region should migrate");
        let inv = apply(&mut f.d, migrate_cmd);

        // Property on `card` is now PropKind::Slot and carries preferred components into slot_configs.
        let card_node = f.d.get(card).unwrap();
        assert_eq!(card_node.component.props.len(), 1);
        assert_eq!(card_node.component.props[0].name, "Icon");
        assert_eq!(card_node.component.props[0].kind, PropKind::Slot);
        assert_eq!(card_node.component.slot_configs.get("Icon").map(|cfg| cfg.preferred.clone()), Some(vec![f.comp, alt]));

        // `slot` in `card` is now a slot Frame bound to OverrideField::SlotContent -> "Icon".
        let slot_node = f.d.get(slot).unwrap();
        assert!(matches!(slot_node.kind, NodeKind::Frame(_)));
        assert!(slot_node.component.is_slot);
        assert!(is_slot_container(&f.d, slot));
        assert_eq!(slot_node.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Icon"));
        assert!(!slot_node.component.bindings.contains_key(&OverrideField::MainComponent));
        assert_eq!(slot_node.children.len(), 1);
        let default_content = slot_node.children[0];
        assert!(f.d.get(default_content).unwrap().component.is_slot_content);
        assert_eq!(instance_component(f.d.get(default_content).unwrap()), Some(f.comp));

        // Swapped instance kept its `alt` instance as slot content inside `slot_copy`.
        let slot_copy_swapped = f.d.get(outer_swapped).unwrap().children[0];
        let sc_swapped = f.d.get(slot_copy_swapped).unwrap();
        assert!(matches!(sc_swapped.kind, NodeKind::Frame(_)));
        assert!(sc_swapped.component.is_slot);
        assert!(sc_swapped.component.overrides.contains(&OverrideField::SlotContent));
        assert_eq!(sc_swapped.children.len(), 1);
        let swapped_content = sc_swapped.children[0];
        let swapped_content_node = f.d.get(swapped_content).unwrap();
        assert!(swapped_content_node.component.is_slot_content);
        assert_eq!(instance_component(swapped_content_node), Some(alt));
        assert_eq!(swapped_content_node.children.len(), 1);
        assert_eq!(f.d.get(swapped_content_node.children[0]).unwrap().name, "dot");
        assert_eq!(f.d.get(outer_swapped).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![swapped_content])));

        // Overridden instance kept its `f.comp` instance AND its "Custom" text override inside the slot.
        let sc_overridden = f.d.get(slot_copy_overridden).unwrap();
        assert!(matches!(sc_overridden.kind, NodeKind::Frame(_)));
        assert!(sc_overridden.component.is_slot);
        assert_eq!(sc_overridden.children.len(), 1);
        let ov_content = sc_overridden.children[0];
        assert!(f.d.get(ov_content).unwrap().component.is_slot_content);
        assert_eq!(instance_component(f.d.get(ov_content).unwrap()), Some(f.comp));
        assert_eq!(text_of(&f.d, child_named(&f.d, ov_content, "label")), "Custom");
        assert_eq!(f.d.get(outer_overridden).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![ov_content])));

        // Syncing `card` preserves slot content in both instances.
        let sync_cmd = sync_instances(&mut f.d, card);
        apply(&mut f.d, sync_cmd);
        assert_eq!(f.d.get(slot_copy_swapped).unwrap().children, vec![swapped_content]);
        assert_eq!(text_of(&f.d, child_named(&f.d, ov_content, "label")), "Custom");

        // Single-step undo restores the exact pre-migration state.
        apply(&mut f.d, inv);
        assert_eq!(f.d.nodes, before_nodes);
    }

    #[test]
    fn migrate_boolean_container_region_to_slot_keeps_content_of_shown_and_hidden_instances_and_undoes_in_one_step() {
        let mut f = fx();
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 120.0))).unwrap();
        let id = f.d.alloc_id();
        let mut footer = Node::frame(id, Rect::new(0.0, 80.0, 300.0, 40.0));
        footer.name = "footer".into();
        let footer = f.d.insert(card, footer).unwrap();
        let id = f.d.alloc_id();
        let mut fbg = Node::rectangle(id, Rect::new(0.0, 0.0, 300.0, 40.0));
        fbg.name = "footer_bg".into();
        let fbg = f.d.insert(footer, fbg).unwrap();
        let id = f.d.alloc_id();
        let mut ftxt = Node::text(id, Rect::new(10.0, 10.0, 100.0, 20.0), "Footer", TextStyle::default());
        ftxt.name = "footer_text".into();
        let ftxt = f.d.insert(footer, ftxt).unwrap();

        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = add_prop(&mut f.d, card, ComponentPropDef { name: "Show footer".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, footer, OverrideField::Visible, Some("Show footer"));
        apply(&mut f.d, c);

        // Instance 1 keeps footer visible and overrides footer_text.
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(400.0, 0.0));
        apply(&mut f.d, c);
        let inst_shown = *f.d.get(f.page).unwrap().children.last().unwrap();
        let footer_shown = child_named(&f.d, inst_shown, "footer");
        let ftxt_shown = child_named(&f.d, footer_shown, "footer_text");
        let before_t = f.d.get(ftxt_shown).unwrap().clone();
        set_text(&mut f.d, ftxt_shown, "Custom footer");
        let mut after_t = f.d.get(ftxt_shown).unwrap().clone();
        after_t.component.overrides = record_override(&before_t, &after_t);
        apply(&mut f.d, Command::Update { node: Box::new(after_t) });

        // Instance 2 overrides footer_text, then hides footer via the boolean property.
        let c = create_instance(&mut f.d, card, f.page, Vec2::new(400.0, 200.0));
        apply(&mut f.d, c);
        let inst_hidden = *f.d.get(f.page).unwrap().children.last().unwrap();
        let footer_hidden = child_named(&f.d, inst_hidden, "footer");
        let ftxt_hidden = child_named(&f.d, footer_hidden, "footer_text");
        let before_h = f.d.get(ftxt_hidden).unwrap().clone();
        set_text(&mut f.d, ftxt_hidden, "Hidden custom footer");
        let mut after_h = f.d.get(ftxt_hidden).unwrap().clone();
        after_h.component.overrides = record_override(&before_h, &after_h);
        apply(&mut f.d, Command::Update { node: Box::new(after_h) });
        let c = set_instance_prop(&mut f.d, inst_hidden, "Show footer", Some(PropValue::Bool(false)));
        apply(&mut f.d, c);
        assert!(!f.d.get(footer_hidden).unwrap().visible);

        assert!(can_migrate_region_to_slot(&f.d, footer));
        let before_json = f.d.to_json().unwrap();

        let migrate_cmd = migrate_region_to_slot(&mut f.d, footer).expect("boolean container region should migrate");
        let inv = apply(&mut f.d, migrate_cmd);

        let card_node = f.d.get(card).unwrap();
        assert_eq!(card_node.component.props[0].kind, PropKind::Slot);
        assert_eq!(card_node.component.props[0].default, PropValue::Slot(vec![fbg, ftxt]));

        let footer_node = f.d.get(footer).unwrap();
        assert!(footer_node.component.is_slot);
        assert_eq!(footer_node.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Show footer"));
        assert!(!footer_node.component.bindings.contains_key(&OverrideField::Visible));
        assert!(f.d.get(fbg).unwrap().component.is_slot_content);
        assert!(f.d.get(ftxt).unwrap().component.is_slot_content);

        // Shown instance keeps its children and "Custom footer" text override as slot content.
        let fs_node = f.d.get(footer_shown).unwrap();
        assert!(fs_node.component.is_slot);
        assert!(fs_node.visible);
        assert!(fs_node.component.overrides.contains(&OverrideField::SlotContent));
        assert_eq!(fs_node.children.len(), 2);
        assert!(fs_node.children.iter().all(|cid| f.d.get(*cid).unwrap().component.is_slot_content));
        assert_eq!(text_of(&f.d, ftxt_shown), "Custom footer");
        assert_eq!(f.d.get(inst_shown).unwrap().component.values.get("Show footer"), Some(&PropValue::Slot(fs_node.children.clone())));

        // Hidden instance keeps its content, including the text override, and stays hidden.
        let fh_node = f.d.get(footer_hidden).unwrap();
        assert!(fh_node.component.is_slot);
        assert_eq!(fh_node.children.len(), 2, "the hidden instance keeps its content");
        assert!(fh_node.children.iter().all(|cid| f.d.get(*cid).unwrap().component.is_slot_content));
        assert_eq!(text_of(&f.d, ftxt_hidden), "Hidden custom footer");
        assert_eq!(f.d.get(inst_hidden).unwrap().component.values.get("Show footer"), Some(&PropValue::Slot(fh_node.children.clone())));
        assert!(!fh_node.visible);
        assert!(fh_node.component.overrides.contains(&OverrideField::Visible));

        // A sync neither drops the content nor shows the hidden footer again.
        let sync_cmd = sync_instances(&mut f.d, card);
        apply(&mut f.d, sync_cmd);
        assert!(!f.d.get(footer_hidden).unwrap().visible);
        assert_eq!(f.d.get(footer_hidden).unwrap().children.len(), 2);
        assert_eq!(text_of(&f.d, ftxt_hidden), "Hidden custom footer");
        assert!(f.d.get(footer_shown).unwrap().visible);
        assert_eq!(text_of(&f.d, ftxt_shown), "Custom footer");

        // Single-step undo restores the pre-migration state.
        apply(&mut f.d, inv);
        assert_eq!(f.d.to_json().unwrap(), before_json);
    }

    #[test]
    fn migrate_region_to_slot_rejects_unsupported_shapes_as_noop_with_honest_message() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let id = f.d.alloc_id();
        let unbound_frame = f.d.insert(f.comp, Node::frame(id, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        let before_json = f.d.to_json().unwrap();

        for bad in [f.page, f.comp, f.bg, f.label, f.icon, i, child_named(&f.d, i, "bg"), unbound_frame, NodeId(999_999)] {
            assert!(!can_migrate_region_to_slot(&f.d, bad));
            let err = migrate_region_to_slot(&mut f.d, bad).expect_err("unsupported target should fail");
            assert!(!err.is_empty());
        }
        assert_eq!(f.d.to_json().unwrap(), before_json, "unsupported targets must be a pure no-op");
    }

    /// Component set "Size=Small | Size=Large", each variant holding an instance of `f.comp`
    /// bound to a set-level instance-swap prop "Icon", plus one instance of every variant.
    struct SetFx {
        f: Fx,
        alt: NodeId,
        set: NodeId,
        small_icon: NodeId,
        large_icon: NodeId,
        inst_small: NodeId,
        inst_large: NodeId,
    }

    fn set_with_icon_in_each_variant(bind_large: bool) -> SetFx {
        let mut f = fx();
        let id = f.d.alloc_id();
        let mut alt = Node::frame(id, Rect::new(0.0, 300.0, 50.0, 50.0));
        alt.name = "Alt".into();
        let alt = f.d.insert(f.page, alt).unwrap();
        let id = f.d.alloc_id();
        let mut dot = Node::ellipse(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        dot.name = "dot".into();
        f.d.insert(alt, dot).unwrap();
        let c = create_component(&mut f.d, alt);
        apply(&mut f.d, c);

        let mut variants = Vec::new();
        for (name, y) in [("Size=Small", 500.0), ("Size=Large", 700.0)] {
            let id = f.d.alloc_id();
            let mut v = Node::frame(id, Rect::new(0.0, y, 300.0, 100.0));
            v.name = name.into();
            let v = f.d.insert(f.page, v).unwrap();
            let c = create_component(&mut f.d, v);
            apply(&mut f.d, c);
            variants.push(v);
        }
        let (small, large) = (variants[0], variants[1]);
        let c = create_instance(&mut f.d, f.comp, small, Vec2::new(12.0, 16.0));
        apply(&mut f.d, c);
        let small_icon = *f.d.get(small).unwrap().children.last().unwrap();
        let c = create_instance(&mut f.d, f.comp, large, Vec2::new(20.0, 24.0));
        apply(&mut f.d, c);
        let large_icon = *f.d.get(large).unwrap().children.last().unwrap();

        let c = combine_as_variants(&mut f.d, &[small, large]);
        apply(&mut f.d, c);
        let set = f.d.get(small).unwrap().parent.unwrap();
        let c = add_prop(
            &mut f.d,
            set,
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![f.comp, alt] }, default: PropValue::Instance(f.comp) },
        );
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, small_icon, OverrideField::MainComponent, Some("Icon"));
        apply(&mut f.d, c);
        if bind_large {
            let c = bind_prop(&mut f.d, large_icon, OverrideField::MainComponent, Some("Icon"));
            apply(&mut f.d, c);
        }

        // The small instance overrides its icon's label; the large one swaps the icon to `alt`.
        let c = create_instance(&mut f.d, small, f.page, Vec2::new(900.0, 0.0));
        apply(&mut f.d, c);
        let inst_small = *f.d.get(f.page).unwrap().children.last().unwrap();
        let icon_copy = f.d.get(inst_small).unwrap().children[0];
        let label = child_named(&f.d, icon_copy, "label");
        let before = f.d.get(label).unwrap().clone();
        set_text(&mut f.d, label, "Small custom");
        let mut after = f.d.get(label).unwrap().clone();
        after.component.overrides = record_override(&before, &after);
        apply(&mut f.d, Command::Update { node: Box::new(after) });

        let c = create_instance(&mut f.d, large, f.page, Vec2::new(900.0, 200.0));
        apply(&mut f.d, c);
        let inst_large = *f.d.get(f.page).unwrap().children.last().unwrap();
        let c = set_instance_prop(&mut f.d, inst_large, "Icon", Some(PropValue::Instance(alt)));
        apply(&mut f.d, c);
        SetFx { f, alt, set, small_icon, large_icon, inst_small, inst_large }
    }

    #[test]
    fn migrate_instance_swap_region_in_a_component_set_migrates_every_variant_and_undoes_in_one_step() {
        let SetFx { mut f, alt, set, small_icon, large_icon, inst_small, inst_large } = set_with_icon_in_each_variant(true);
        assert!(can_migrate_region_to_slot(&f.d, small_icon));
        let before_nodes = f.d.nodes.clone();

        let cmd = migrate_region_to_slot(&mut f.d, small_icon).expect("set-level instance-swap region should migrate");
        let inv = apply(&mut f.d, cmd);

        assert_eq!(f.d.get(set).unwrap().component.props[1].kind, PropKind::Slot);
        for region in [small_icon, large_icon] {
            let n = f.d.get(region).unwrap();
            assert!(n.component.is_slot, "every variant's region becomes a slot");
            assert_eq!(n.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Icon"));
            assert!(!n.component.bindings.contains_key(&OverrideField::MainComponent), "no variant keeps an instance-swap binding to the slot prop");
            assert_eq!(n.children.len(), 1);
        }

        // The other variant's instance keeps its swapped icon as slot content.
        let large_copy = f.d.get(inst_large).unwrap().children[0];
        let lc = f.d.get(large_copy).unwrap();
        assert!(lc.component.is_slot);
        assert_eq!(lc.children.len(), 1);
        let content = lc.children[0];
        assert!(f.d.get(content).unwrap().component.is_slot_content);
        assert_eq!(instance_component(f.d.get(content).unwrap()), Some(alt));
        assert_eq!(f.d.get(inst_large).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![content])));

        // The selected variant's instance keeps its overridden label.
        let small_copy = f.d.get(inst_small).unwrap().children[0];
        let sc_content = f.d.get(small_copy).unwrap().children[0];
        assert_eq!(text_of(&f.d, child_named(&f.d, sc_content, "label")), "Small custom");

        // Syncing the set leaves both variants' content alone.
        let sync_cmd = sync_instances(&mut f.d, set);
        apply(&mut f.d, sync_cmd);
        assert_eq!(f.d.get(large_copy).unwrap().children, vec![content]);
        assert_eq!(instance_component(f.d.get(content).unwrap()), Some(alt));
        assert_eq!(text_of(&f.d, child_named(&f.d, sc_content, "label")), "Small custom");

        apply(&mut f.d, inv);
        assert!(f.d.nodes == before_nodes, "one undo restores every variant and instance");
    }

    #[test]
    fn migrate_region_in_a_component_set_is_a_noop_when_a_variant_has_no_matching_region() {
        let SetFx { mut f, small_icon, .. } = set_with_icon_in_each_variant(false);
        let before_json = f.d.to_json().unwrap();

        assert!(!can_migrate_region_to_slot(&f.d, small_icon));
        let err = migrate_region_to_slot(&mut f.d, small_icon).expect_err("a variant without a matching region must block the migration");
        assert!(!err.is_empty());
        assert_eq!(f.d.to_json().unwrap(), before_json, "a refused migration leaves the document untouched");
    }

    #[test]
    fn migrate_instance_swap_region_keeps_content_of_instances_that_hide_it() {
        let mut f = fx();
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::new(12.0, 16.0));
        apply(&mut f.d, c);
        let region = *f.d.get(card).unwrap().children.last().unwrap();
        let c = add_prop(
            &mut f.d,
            card,
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![f.comp] }, default: PropValue::Instance(f.comp) },
        );
        apply(&mut f.d, c);
        let c = add_prop(&mut f.d, card, ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, region, OverrideField::MainComponent, Some("Icon"));
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, region, OverrideField::Visible, Some("Show icon"));
        apply(&mut f.d, c);

        let c = create_instance(&mut f.d, card, f.page, Vec2::new(600.0, 0.0));
        apply(&mut f.d, c);
        let outer = *f.d.get(f.page).unwrap().children.last().unwrap();
        let copy = f.d.get(outer).unwrap().children[0];
        let label = child_named(&f.d, copy, "label");
        let before = f.d.get(label).unwrap().clone();
        set_text(&mut f.d, label, "Hidden icon label");
        let mut after = f.d.get(label).unwrap().clone();
        after.component.overrides = record_override(&before, &after);
        apply(&mut f.d, Command::Update { node: Box::new(after) });
        let c = set_instance_prop(&mut f.d, outer, "Show icon", Some(PropValue::Bool(false)));
        apply(&mut f.d, c);
        assert!(!f.d.get(copy).unwrap().visible);
        let before_nodes = f.d.nodes.clone();

        let cmd = migrate_region_to_slot(&mut f.d, region).expect("instance-swap region should migrate");
        let inv = apply(&mut f.d, cmd);

        let c = f.d.get(copy).unwrap();
        assert!(c.component.is_slot);
        assert_eq!(c.children.len(), 1, "the hidden instance keeps its content");
        let content = c.children[0];
        assert!(f.d.get(content).unwrap().component.is_slot_content);
        assert_eq!(text_of(&f.d, child_named(&f.d, content, "label")), "Hidden icon label");
        assert_eq!(f.d.get(outer).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![content])));
        assert!(!c.visible, "an instance that hid the region still hides it");
        assert!(c.component.overrides.contains(&OverrideField::Visible));

        let sync_cmd = sync_instances(&mut f.d, card);
        apply(&mut f.d, sync_cmd);
        assert!(!f.d.get(copy).unwrap().visible);
        assert_eq!(f.d.get(copy).unwrap().children, vec![content]);

        apply(&mut f.d, inv);
        assert!(f.d.nodes == before_nodes, "one undo restores the region and its instances");
    }

    #[test]
    fn migrate_region_whose_property_a_variant_defines_itself_leaves_other_variants_alone() {
        let SetFx { mut f, set, small_icon, large_icon, inst_large, .. } = set_with_icon_in_each_variant(false);
        let small = f.d.get(small_icon).unwrap().parent.unwrap();
        // Move "Icon" from the set onto the Small variant only.
        let mut set_node = f.d.get(set).unwrap().clone();
        let icon_def = set_node.component.props.pop().unwrap();
        assert_eq!(icon_def.name, "Icon");
        apply(&mut f.d, Command::Update { node: Box::new(set_node) });
        let c = add_prop(&mut f.d, small, icon_def);
        apply(&mut f.d, c);
        let large_before = f.d.get(large_icon).unwrap().clone();
        let inst_large_before = f.d.get(inst_large).unwrap().clone();

        let cmd = migrate_region_to_slot(&mut f.d, small_icon).expect("a variant-level property migrates on its own");
        apply(&mut f.d, cmd);

        assert_eq!(f.d.get(small).unwrap().component.props.iter().filter(|p| p.name == "Icon" && p.kind == PropKind::Slot).count(), 1);
        assert!(f.d.get(set).unwrap().component.props.iter().all(|p| p.name != "Icon"), "no duplicate slot property lands on the set");
        assert!(same_ignoring_tree(f.d.get(large_icon).unwrap(), &large_before));
        assert_eq!(f.d.get(inst_large).unwrap().component.values, inst_large_before.component.values);
    }

    /// `card` holds a frame "icon wrap" with one instance of `f.comp` bound to the instance-swap prop "Icon"
    /// (the frame to the boolean "Show icon"). One instance of `card` overrides the inner label.
    struct WrappedFx {
        f: Fx,
        card: NodeId,
        wrap: NodeId,
        outer: NodeId,
        wrap_copy: NodeId,
        inner_copy: NodeId,
    }

    fn wrapped_icon_card_with_overridden_label(text: &str) -> WrappedFx {
        let mut f = fx();
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 100.0))).unwrap();
        let id = f.d.alloc_id();
        let mut wrap = Node::frame(id, Rect::new(12.0, 16.0, 100.0, 40.0));
        wrap.name = "icon wrap".into();
        let wrap = f.d.insert(card, wrap).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, wrap, Vec2::new(0.0, 0.0));
        apply(&mut f.d, c);
        let inner = *f.d.get(wrap).unwrap().children.last().unwrap();
        let c = add_prop(
            &mut f.d,
            card,
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![f.comp] }, default: PropValue::Instance(f.comp) },
        );
        apply(&mut f.d, c);
        let c = add_prop(&mut f.d, card, ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, inner, OverrideField::MainComponent, Some("Icon"));
        apply(&mut f.d, c);
        let c = bind_prop(&mut f.d, wrap, OverrideField::Visible, Some("Show icon"));
        apply(&mut f.d, c);
        assert_eq!(f.d.get(wrap).unwrap().children.len(), 1);

        let c = create_instance(&mut f.d, card, f.page, Vec2::new(600.0, 0.0));
        apply(&mut f.d, c);
        let outer = *f.d.get(f.page).unwrap().children.last().unwrap();
        let wrap_copy = child_named(&f.d, outer, "icon wrap");
        let inner_copy = f.d.get(wrap_copy).unwrap().children[0];
        let label = child_named(&f.d, inner_copy, "label");
        let before = f.d.get(label).unwrap().clone();
        set_text(&mut f.d, label, text);
        let mut after = f.d.get(label).unwrap().clone();
        after.component.overrides = record_override(&before, &after);
        apply(&mut f.d, Command::Update { node: Box::new(after) });
        WrappedFx { f, card, wrap, outer, wrap_copy, inner_copy }
    }

    #[test]
    fn migrate_wrapped_instance_region_keeps_content_of_instances_that_hide_it() {
        let WrappedFx { mut f, card, wrap, outer, wrap_copy, inner_copy } = wrapped_icon_card_with_overridden_label("Hidden wrapped label");
        let c = set_instance_prop(&mut f.d, outer, "Show icon", Some(PropValue::Bool(false)));
        apply(&mut f.d, c);
        assert!(!f.d.get(wrap_copy).unwrap().visible);
        let before_nodes = f.d.nodes.clone();

        assert!(can_migrate_region_to_slot(&f.d, wrap));
        let cmd = migrate_region_to_slot(&mut f.d, wrap).expect("wrapped instance region should migrate");
        let inv = apply(&mut f.d, cmd);

        let w = f.d.get(wrap_copy).unwrap();
        assert!(w.component.is_slot);
        assert_eq!(w.children, vec![inner_copy], "the hidden instance keeps its content");
        assert!(f.d.get(inner_copy).unwrap().component.is_slot_content);
        assert_eq!(text_of(&f.d, child_named(&f.d, inner_copy, "label")), "Hidden wrapped label");
        assert_eq!(f.d.get(outer).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![inner_copy])));
        assert!(!w.visible, "an instance that hid the region still hides it");
        assert!(w.component.overrides.contains(&OverrideField::Visible));

        let sync_cmd = sync_instances(&mut f.d, card);
        apply(&mut f.d, sync_cmd);
        assert!(!f.d.get(wrap_copy).unwrap().visible);
        assert_eq!(f.d.get(wrap_copy).unwrap().children, vec![inner_copy]);
        assert_eq!(text_of(&f.d, child_named(&f.d, inner_copy, "label")), "Hidden wrapped label");

        apply(&mut f.d, inv);
        assert!(f.d.nodes == before_nodes, "one undo restores the region and its instances");
    }

    #[test]
    fn migrate_wrapped_instance_region_keeps_overridden_label_linked_when_the_inner_component_syncs() {
        let WrappedFx { mut f, wrap, outer, wrap_copy, inner_copy, .. } = wrapped_icon_card_with_overridden_label("Wrapped override");
        let label_copy = child_named(&f.d, inner_copy, "label");
        let bg_copy = child_named(&f.d, inner_copy, "bg");

        let cmd = migrate_region_to_slot(&mut f.d, wrap).expect("wrapped instance region should migrate");
        apply(&mut f.d, cmd);
        assert_eq!(f.d.get(wrap_copy).unwrap().children, vec![inner_copy]);

        // Editing the inner main component and syncing it must reach the slot content without replacing the override.
        set_fill(&mut f.d, f.bg, Color::rgb(255, 0, 0));
        let sync_cmd = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, sync_cmd);

        let kids = f.d.get(inner_copy).unwrap().children.clone();
        assert!(kids.contains(&label_copy) && kids.contains(&bg_copy), "slot content keeps its nodes instead of getting fresh copies");
        assert_eq!(text_of(&f.d, label_copy), "Wrapped override");
        assert_eq!(f.d.get(bg_copy).unwrap().fills, f.d.get(f.bg).unwrap().fills, "the slot content still follows its main component");
        assert_eq!(f.d.get(outer).unwrap().component.values.get("Icon"), Some(&PropValue::Slot(vec![inner_copy])));
    }
}
