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
    pub const ALL: [OverrideField; 18] = [
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
        OverrideField::LockAspect,
        OverrideField::Arc,
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
    /// The swaps a sync left alone because they would make a main hold itself, each as (the main the instance stays on,
    /// the main it asked for).
    refused_swap: Vec<(NodeId, NodeId)>,
    /// Whether a main holds an instance of itself, asked the first time a sync meets it inside its own mirror. The
    /// answer holds for the whole sync: a mirror only repeats what the main already reaches.
    holds_itself: BTreeMap<NodeId, bool>,
    /// Number of nested swaps and override fields dropped during this context's swap operations.
    dropped_changes: usize,
    /// Number of parked overrides kept on instances for a later return during this context's swap operations.
    kept_for_return: usize,
    /// Instances that entered or received a non-empty parked list during this context's swap operations.
    had_parked: BTreeSet<NodeId>,
}

impl<'a> Ctx<'a> {
    fn new(doc: &'a mut Document) -> Self {
        let saved_dirty = doc.dirty.clone();
        Ctx {
            s: doc,
            cmds: Vec::new(),
            invs: Vec::new(),
            saved_dirty,
            refused_swap: Vec::new(),
            holds_itself: BTreeMap::new(),
            dropped_changes: 0,
            kept_for_return: 0,
            had_parked: BTreeSet::new(),
        }
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
    copy_tree_reserving(d, src, link, &mut std::collections::BTreeSet::new())
}

/// [`copy_tree`] whose Motion ids also avoid `reserved`, the ids of copies made earlier in the same batch.
fn copy_tree_reserving(d: &mut Document, src: NodeId, link: bool, reserved: &mut std::collections::BTreeSet<crate::MotionId>) -> Vec<Node> {
    let mut out = Vec::new();
    copy_rec(d, src, None, link, &mut out);
    let mut copied: Vec<&mut Node> = out.iter_mut().collect();
    crate::reid_copied_motion_with(d, &mut copied, true, reserved);
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
        n.component.parked_overrides.clear();
        release_own_variant_bindings(&mut n);
        // A linked copy inherits property values from its main-side source.
        // Property values belong only to explicit instance overrides; copying
        // the source map would freeze nested instances at creation time.
        n.component.values.clear();
        n.component.grid_align_overrides = [false, false];
        n.component.props.clear();
        n.component.slot_configs.clear();
        n.component.prop_default_variables.clear();
        n.component.expose_nested_instances = false;
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

/// True when `id` is a slot container inside a component instance (either linked to a main
/// component slot or nested inside an instance root).
pub fn is_instance_slot(doc: &Document, id: NodeId) -> bool {
    is_slot_container(doc, id) && (doc.get(id).is_some_and(is_linked) || instance_root_of(doc, id).is_some())
}

/// Nearest ancestor-or-self of `id` that is a slot container inside a component instance.
pub fn enclosing_instance_slot(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    while let Some(c) = cur {
        let n = doc.get(c)?;
        if is_instance_slot(doc, c) {
            return Some(c);
        }
        cur = n.parent;
    }
    None
}

/// Marks `root` and every node in its subtree with `is_slot_content = true`, and marks any
/// enclosing instance slot container with [`OverrideField::SlotContent`].
pub fn mark_slot_content_subtree(doc: &mut Document, root: NodeId) {
    if let Some(slot_id) = enclosing_instance_slot(doc, root) {
        if let Some(slot) = doc.get_mut(slot_id) {
            slot.component.overrides.insert(OverrideField::SlotContent);
        }
    }
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(n) = doc.get_mut(id) {
            n.component.is_slot_content = true;
            stack.extend(n.children.iter().copied());
        }
    }
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
    prop_value_of(doc, n, def)
}

/// [`prop_value`] for the instance node `n`, which need not be the one `doc` holds under its id.
fn prop_value_of(doc: &Document, n: &Node, def: &ComponentPropDef) -> PropValue {
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
    let mut cur_src = n.component.source;
    let mut hops = 0usize;
    while let Some(sid) = cur_src {
        hops += 1;
        if hops > 64 {
            break;
        }
        let Some(sn) = doc.get(sid) else { break };
        if !matches!(sn.kind, NodeKind::Instance { .. }) {
            break;
        }
        if let Some(v) = sn.component.values.get(&def.name) {
            return v.clone();
        }
        cur_src = sn.component.source;
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
    add(before.lock_aspect != after.lock_aspect, OverrideField::LockAspect);
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
    if let (NodeKind::Ellipse(a), NodeKind::Ellipse(b)) = (&before.kind, &after.kind) {
        add(a != b, OverrideField::Arc);
    }
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
                || before.text_style_overrides != after.text_style_overrides
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
            } else if key.starts_with("effects/") {
                s.insert(OverrideField::Effects);
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

/// Clears partial `.pen` override markers (`pen:explicit_stroke_fields`,
/// `pen:explicit_width`, `pen:explicit_height`) when a user edit records `Stroke`
/// or `Size` overrides on `node`, so all user-edited stroke/size subfields survive
/// `sync_instances` and `.pen` export.
pub fn clear_stale_pen_override_markers(node: &mut Node, fields: &BTreeSet<OverrideField>) {
    if fields.contains(&OverrideField::Stroke) {
        node.plugin_data.remove("pen:explicit_stroke_fields");
    }
    if fields.contains(&OverrideField::Size) {
        node.plugin_data.remove("pen:explicit_width");
        node.plugin_data.remove("pen:explicit_height");
    }
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

/// The main components that `ids` (with their layers) hold an instance of, directly or through the content of the
/// mains those instances link to.
pub fn mains_held_by(doc: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    let mut direct = Vec::new();
    for id in ids {
        doc.walk(*id, |n, _| direct.extend(instance_component(n)));
    }
    mains_held_through(doc, direct)
}

/// The main component that moving the layers `ids` under `new_parent` would make hold an instance of itself, if any.
pub fn reparent_held_main(doc: &Document, ids: &[NodeId], new_parent: NodeId) -> Option<NodeId> {
    mains_held_by(doc, ids).into_iter().find(|m| new_parent == *m || doc.is_ancestor(*m, new_parent))
}

/// Whether moving the layers `ids` under `new_parent` would make a main hold itself: a layer that holds an instance
/// of a main (directly or through other mains) does not go into that main, nor into a layer inside it. A move into the
/// layer's own subtree is a different refusal, which the callers keep.
pub fn reparent_would_hold_itself(doc: &Document, ids: &[NodeId], new_parent: NodeId) -> bool {
    reparent_held_main(doc, ids, new_parent).is_some()
}

/// `mains` and every main that the content of those holds an instance of, through any number of mains.
pub fn mains_held_through(doc: &Document, mains: impl IntoIterator<Item = NodeId>) -> Vec<NodeId> {
    let mut held: Vec<NodeId> = Vec::new();
    let mut to_walk: Vec<NodeId> = Vec::new();
    for main in mains {
        if !held.contains(&main) {
            held.push(main);
            to_walk.push(main);
        }
    }
    while let Some(root) = to_walk.pop() {
        doc.walk(root, |n, _| {
            if let Some(main) = instance_component(n).filter(|m| !held.contains(m)) {
                held.push(main);
                to_walk.push(main);
            }
        });
    }
    held
}

/// The main component that an instance of `component` at `at` (the parent it goes under, or the instance it replaces)
/// would make hold an instance of itself, if any.
pub fn held_main_at(doc: &Document, component: NodeId, at: NodeId) -> Option<NodeId> {
    let mut enclosing = Vec::new();
    let mut cur = Some(at);
    while let Some(n) = cur.and_then(|id| doc.get(id)) {
        if matches!(n.kind, NodeKind::Component(_)) {
            enclosing.push(n.id);
        }
        cur = n.parent;
    }
    if enclosing.is_empty() {
        return None;
    }
    mains_held_through(doc, [component]).into_iter().find(|m| enclosing.contains(m))
}

/// User-facing label for a component or layer involved in a cycle refusal.
pub fn cycle_node_label(doc: &Document, id: NodeId) -> String {
    doc.get(id).map(|n| if n.name.trim().is_empty() { n.kind.default_name().to_string() } else { n.name.clone() }).unwrap_or_else(|| "Layer".to_string())
}

pub(crate) fn container_labels_between(doc: &Document, ancestor: NodeId, leaf_parent: Option<NodeId>) -> Vec<String> {
    let mut chain = Vec::new();
    let mut cur = leaf_parent;
    let mut guard = 0usize;
    while let Some(pid) = cur {
        if pid == ancestor || guard > doc.nodes.len() {
            break;
        }
        chain.push(cycle_node_label(doc, pid));
        cur = doc.get(pid).and_then(|p| p.parent);
        guard += 1;
    }
    chain.reverse();
    chain
}

pub(crate) fn component_chain_path(doc: &Document, start_comp: NodeId, goal_comp: NodeId) -> Option<Vec<String>> {
    let edges_from = |m: NodeId| -> Vec<(NodeId, NodeId)> {
        let mut out = Vec::new();
        doc.walk(m, |n, _| {
            if let Some(target) = instance_component(n) {
                out.push((target, n.id));
            }
        });
        out
    };

    let mut queue = std::collections::VecDeque::new();
    let mut visited = BTreeSet::new();
    let mut parent_edge: std::collections::BTreeMap<NodeId, (NodeId, NodeId)> = std::collections::BTreeMap::new();
    queue.push_back(start_comp);
    while let Some(cur) = queue.pop_front() {
        for (target, inst_id) in edges_from(cur) {
            if target == goal_comp {
                let mut hops = vec![(cur, inst_id, target)];
                let mut back = cur;
                while back != start_comp {
                    let &(prev_comp, prev_inst) = parent_edge.get(&back)?;
                    hops.push((prev_comp, prev_inst, back));
                    back = prev_comp;
                }
                hops.reverse();
                let mut path = vec![cycle_node_label(doc, start_comp)];
                for (from_comp, via_inst, to_comp) in hops {
                    let inst_parent = doc.get(via_inst).and_then(|n| n.parent);
                    path.extend(container_labels_between(doc, from_comp, inst_parent));
                    path.push(cycle_node_label(doc, to_comp));
                }
                return Some(path);
            }
            if visited.insert(target) {
                parent_edge.insert(target, (cur, inst_id));
                queue.push_back(target);
            }
        }
    }
    None
}

/// If `comp` is a main component that holds an instance of itself (directly or through other
/// main components), returns the human-readable cycle path starting and ending with `comp`'s label
/// (for example `["B", "X", "B"]`).
pub fn self_holding_cycle_path(doc: &Document, comp: NodeId) -> Option<Vec<String>> {
    if !matches!(doc.get(comp).map(|n| &n.kind), Some(NodeKind::Component(_))) {
        return None;
    }
    component_chain_path(doc, comp, comp)
}

/// Finds the first main component in `doc` (ordered by [`NodeId`]) that holds an instance of itself,
/// returning `(component_id, cycle_path_labels)`.
pub fn first_self_holding_component_cycle(doc: &Document) -> Option<(NodeId, Vec<String>)> {
    let mut comps: Vec<NodeId> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();
    comps.sort();
    comps.into_iter().find_map(|id| self_holding_cycle_path(doc, id).map(|path| (id, path)))
}

fn collect_cycle_candidate_mains_from_command(main: &Document, working: &Document, cmd: &crate::Command, out: &mut BTreeSet<NodeId>) {
    let add_enclosing_components = |doc: &Document, start: NodeId, set: &mut BTreeSet<NodeId>| {
        let mut cur = Some(start);
        let mut hops = 0usize;
        let cap = doc.nodes.len() + 1;
        while let Some(id) = cur {
            if hops > cap {
                break;
            }
            hops += 1;
            let Some(n) = doc.get(id) else { break };
            if matches!(n.kind, NodeKind::Component(_)) {
                set.insert(id);
            }
            cur = n.parent;
        }
    };
    let add_subtree_components_and_targets = |doc: &Document, root: NodeId, set: &mut BTreeSet<NodeId>| {
        doc.walk(root, |n, _| {
            if matches!(n.kind, NodeKind::Component(_)) {
                set.insert(n.id);
            }
            if let Some(target) = instance_component(n) {
                set.insert(target);
            }
        });
    };

    match cmd {
        crate::Command::Insert { parent, nodes, .. } => {
            add_enclosing_components(working, *parent, out);
            for node in nodes {
                let has_comp_or_inst = matches!(node.kind, NodeKind::Component(_) | NodeKind::Instance { .. })
                    || working.get(node.id).is_some_and(|wn| matches!(wn.kind, NodeKind::Component(_) | NodeKind::Instance { .. }) || !wn.children.is_empty());
                if has_comp_or_inst {
                    add_enclosing_components(working, node.id, out);
                    add_subtree_components_and_targets(working, node.id, out);
                }
            }
        }
        crate::Command::Reparent { id, parent, .. } => {
            add_enclosing_components(working, *parent, out);
            add_enclosing_components(working, *id, out);
            add_subtree_components_and_targets(working, *id, out);
        }
        crate::Command::Update { node } => {
            let prev = main.get(node.id);
            let became_comp = matches!(node.kind, NodeKind::Component(_)) && !prev.is_some_and(|p| matches!(p.kind, NodeKind::Component(_)));
            let new_target = instance_component(node);
            let old_target = prev.and_then(instance_component);
            let changed_inst_target = new_target.is_some() && new_target != old_target;
            if became_comp || changed_inst_target {
                add_enclosing_components(working, node.id, out);
                if let Some(target) = new_target {
                    out.insert(target);
                }
            }
        }
        crate::Command::Batch(cmds) => {
            for sub in cmds {
                collect_cycle_candidate_mains_from_command(main, working, sub, out);
            }
        }
        _ => {}
    }
}

/// Finds the first main component in `working` whose self-holding instance cycle was newly
/// introduced by applying `commands` (that is, the component was not already self-holding in
/// `main` or `branch`).
pub fn first_new_self_holding_component_cycle(
    main: &Document,
    branch: &Document,
    working: &Document,
    commands: &[crate::Command],
) -> Option<(NodeId, Vec<String>)> {
    let mut candidates = BTreeSet::new();
    for cmd in commands {
        collect_cycle_candidate_mains_from_command(main, working, cmd, &mut candidates);
    }
    candidates.into_iter().find_map(|comp| {
        let path = self_holding_cycle_path(working, comp)?;
        if self_holding_cycle_path(main, comp).is_some() || self_holding_cycle_path(branch, comp).is_some() {
            return None;
        }
        Some((comp, path))
    })
}

/// If placing an instance of `component` under `at` (optionally through `extra_containers`) would
/// make an enclosing main component hold an instance of itself, returns `(held_main, cycle_path_labels)`.
pub fn held_main_cycle_path_at(doc: &Document, component: NodeId, at: NodeId, extra_containers: &[String]) -> Option<(NodeId, Vec<String>)> {
    let held_main = held_main_at(doc, component, at)?;
    let mut path = vec![cycle_node_label(doc, held_main)];
    path.extend(container_labels_between(doc, held_main, Some(at)));
    path.extend(extra_containers.iter().cloned());
    if component == held_main {
        path.push(cycle_node_label(doc, held_main));
    } else if let Some(tail) = component_chain_path(doc, component, held_main) {
        path.extend(tail);
    } else {
        path.push(cycle_node_label(doc, component));
        path.push(cycle_node_label(doc, held_main));
    }
    Some((held_main, path))
}

/// If reparenting `ids` under `new_parent` would make an enclosing main component hold an instance
/// of itself, returns `(held_main, cycle_path_labels)`.
pub fn reparent_held_main_cycle_path(doc: &Document, ids: &[NodeId], new_parent: NodeId) -> Option<(NodeId, Vec<String>)> {
    let held_main = reparent_held_main(doc, ids, new_parent)?;
    for &root in ids {
        let mut found = None;
        doc.walk(root, |n, _| {
            if found.is_some() {
                return;
            }
            let Some(target) = instance_component(n) else { return };
            if target == held_main || mains_held_through(doc, [target]).contains(&held_main) {
                let mut extra = Vec::new();
                if n.id != root {
                    extra.push(cycle_node_label(doc, root));
                    extra.extend(container_labels_between(doc, root, n.parent));
                }
                found = held_main_cycle_path_at(doc, target, new_parent, &extra);
            }
        });
        if let Some(res) = found {
            return Some(res);
        }
    }
    Some((held_main, vec![cycle_node_label(doc, held_main), cycle_node_label(doc, new_parent), cycle_node_label(doc, held_main)]))
}

/// Whether an instance of `component` at `at` (the parent it goes under, or the instance it replaces) would make a
/// main hold itself: `at` is, or sits in, `component` or a main that the content of `component` holds an instance of.
/// Such a main mirrors itself without end.
pub fn would_hold_itself(doc: &Document, component: NodeId, at: NodeId) -> bool {
    held_main_at(doc, component, at).is_some()
}

/// Instantiates `component` under `parent`, with its origin at `at` (parent
/// space). The copy mirrors the whole component subtree. Nothing is created
/// where the instance would make a main hold itself: in the component, or in a
/// main that the component holds an instance of, however deep.
pub fn create_instance(doc: &mut Document, component: NodeId, parent: NodeId, at: Vec2) -> Command {
    create_instance_reserving(doc, component, parent, at, &mut std::collections::BTreeSet::new())
}

/// [`create_instance`] for one of several instances made before any is inserted: the Motion ids it hands out avoid
/// `reserved` (what the earlier ones of the batch use) and are added to it.
pub fn create_instance_reserving(
    doc: &mut Document,
    component: NodeId,
    parent: NodeId,
    at: Vec2,
    reserved: &mut std::collections::BTreeSet<crate::MotionId>,
) -> Command {
    let Some(comp) = doc.get(component).cloned() else { return nothing() };
    if !matches!(comp.kind, NodeKind::Component(_)) || would_hold_itself(doc, component, parent) {
        return nothing();
    }
    if !doc.get(parent).map(|p| p.kind.is_container()).unwrap_or(false) {
        return nothing();
    }
    let mut nodes = copy_tree_reserving(doc, component, true, reserved);
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
    root.component.prop_default_variables.clear();
    root.component.parked_overrides.clear();
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

/// Points the instance `n` at `source`. A boolean, text or instance-swap value that `n` holds none of itself, and
/// reads through the source it has now but would not read through `source`, is written onto `n` first. The instance
/// keeps showing it, and every property `source` still gives a value keeps following that source.
fn relink_keeping_values(doc: &Document, n: &mut Node, source: Option<NodeId>) {
    let defs = instance_component(n).map(|comp| prop_defs(doc, comp)).unwrap_or_default();
    let read_before: Vec<PropValue> = defs.iter().map(|def| prop_value_of(doc, n, def)).collect();
    n.component.source = source;
    for (def, before) in defs.into_iter().zip(read_before) {
        let carried = matches!(def.kind, PropKind::Boolean | PropKind::Text | PropKind::InstanceSwap { .. });
        if carried && !n.component.values.contains_key(&def.name) && prop_value_of(doc, n, &def) != before {
            n.component.values.insert(def.name, before);
        }
    }
}

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
        n.component.prop_default_variables.clear();
        n.component.parked_overrides.clear();
        release_own_variant_bindings(&mut n);
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
            relink_keeping_values(c.s, &mut dn, grand);
        } else if is_inst {
            relink_keeping_values(c.s, &mut dn, None);
            dn.component.overrides.clear();
            dn.component.parked_overrides.clear();
            release_own_variant_bindings(&mut dn);
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
        let before = (
            n.component.overrides.len(),
            n.component.grid_align_overrides,
            n.plugin_data.len(),
            n.component.own_variant_bindings.len(),
            n.component.variant_picks.len(),
            n.component.parked_overrides.len(),
        );
        for f in fields {
            n.component.overrides.remove(f);
            if *f == OverrideField::MainComponent {
                n.component.parked_overrides.clear();
            } else {
                n.component.parked_overrides.retain(|p| p.field != *f);
            }
            match f {
                // The variant a nested copy shows is the pick or its own binding: resetting it gives the main's back.
                OverrideField::MainComponent => {
                    release_own_variant_bindings(&mut n);
                }
                OverrideField::Layout => {
                    n.component.grid_align_overrides = [false, false];
                }
                OverrideField::Size => {
                    n.plugin_data.remove("pen:explicit_width");
                    n.plugin_data.remove("pen:explicit_height");
                }
                OverrideField::Stroke => {
                    n.plugin_data.remove("pen:explicit_stroke_fields");
                }
                _ => {}
            }
        }
        if (
            n.component.overrides.len(),
            n.component.grid_align_overrides,
            n.plugin_data.len(),
            n.component.own_variant_bindings.len(),
            n.component.variant_picks.len(),
            n.component.parked_overrides.len(),
        ) != before
        {
            c.update(n);
        }
    }
    sync_one(&mut c, node);
    c.finish()
}

/// The property values of the main-side instance that `n` mirrors, if it mirrors one.
fn mirrored_values<'a>(doc: &'a Document, n: &Node) -> Option<&'a BTreeMap<String, PropValue>> {
    n.component.source.and_then(|s| doc.get(s)).map(|sn| &sn.component.values)
}

/// The properties of nested instance `n` whose value of its own differs from the main-side instance it mirrors. A
/// property `n` holds no value for is inherited: it follows the main-side instance, so it is not an override, even when
/// the main-side instance has a value for it.
fn overriding_value_keys(doc: &Document, n: &Node) -> Vec<String> {
    let source = mirrored_values(doc, n);
    n.component.values.iter().filter(|(name, value)| source.is_none_or(|v| v.get(*name) != Some(*value))).map(|(name, _)| name.clone()).collect()
}

/// The exposed nested instances of `outer` with a property value that overrides the main-side instance they mirror.
/// These values are changes of `outer`: "Reset all changes" takes them back. The values of a nested instance that is
/// not exposed are not, and stay.
pub fn exposed_nested_value_overrides(doc: &Document, outer: NodeId) -> Vec<NodeId> {
    let mut found = exposed_nested_instances(doc, outer);
    found.retain(|&id| doc.get(id).is_some_and(|n| !overriding_value_keys(doc, n).is_empty()));
    found
}

/// "Reset all changes": every override and property value of an instance.
pub fn reset_instance(doc: &mut Document, instance: NodeId) -> Command {
    let mut ids = Vec::new();
    doc.walk(instance, |n, _| ids.push(n.id));
    let mut c = Ctx::new(doc);
    if let Some(mut n) = c.s.get(instance).cloned() {
        n.component.values.clear();
        n.component.slot_content_ids.clear();
        n.component.prop_default_variables.clear();
        n.component.parked_overrides.clear();
        c.update(n);
    }
    let exposed_values = exposed_nested_value_overrides(c.s, instance);
    for id in ids {
        let Some(mut n) = c.s.get(id).cloned() else { continue };
        let own_values = if exposed_values.contains(&id) { overriding_value_keys(c.s, &n) } else { Vec::new() };
        let released_binding = release_own_variant_bindings(&mut n);
        if released_binding
            || !n.component.overrides.is_empty()
            || !n.component.parked_overrides.is_empty()
            || n.component.grid_align_overrides != [false, false]
            || !own_values.is_empty()
        {
            n.component.overrides.clear();
            n.component.parked_overrides.clear();
            n.component.grid_align_overrides = [false, false];
            for name in &own_values {
                n.component.values.remove(name);
            }
            n.plugin_data.remove("pen:explicit_width");
            n.plugin_data.remove("pen:explicit_height");
            n.plugin_data.remove("pen:explicit_stroke_fields");
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

/// The main nested copy `id` shows when no pick is recorded on it and its bound variant properties `choices` drive it:
/// its reference's main, switched to the sibling variant the choices select. `None` when the reference cannot be found.
pub(crate) fn bound_target_of_copy(d: &Document, id: NodeId, choices: &BTreeMap<String, PropValue>) -> Option<NodeId> {
    let r = reference_of(d, id)?;
    let rc = main_of_reference(d.get(r)?)?;
    Some(variant_target_from(d, rc, choices).unwrap_or(rc))
}

/// Whether the reference of nested copy `id` (the main's instance it mirrors) binds property `name` to a variable.
pub(crate) fn reference_binds_prop(d: &Document, id: NodeId, name: &str) -> bool {
    reference_of(d, id).and_then(|r| d.get(r)).is_some_and(|r| r.variables.bindings.contains_key(&prop_binding_key(name)))
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
    let mut seen = mains_around(c.s, r);
    sync_pair(c, r, id, top, true, holder, &mut seen);
}

/// The mains whose content `id` sits in: the main it is inside, and the main of every instance it is inside.
/// An instance of one of these mains, inside what mirrors `id`, is a repeat.
fn mains_around(d: &Document, id: NodeId) -> Vec<NodeId> {
    let mut mains = Vec::new();
    let mut cur = d.get(id).and_then(|n| n.parent);
    while let Some(n) = cur.and_then(|p| d.get(p)) {
        let main = if matches!(n.kind, NodeKind::Component(_)) { Some(n.id) } else { instance_component(n) };
        mains.extend(main.filter(|m| !mains.contains(m)));
        cur = n.parent;
    }
    mains
}

/// The main a reference node stands for: a main itself, or the main an instance links to.
fn main_of_reference(n: &Node) -> Option<NodeId> {
    if matches!(n.kind, NodeKind::Component(_)) {
        Some(n.id)
    } else {
        instance_component(n)
    }
}

/// Sibling variant that `inst`'s variant property values select, if different.
fn variant_target(d: &Document, inst: &Node) -> Option<NodeId> {
    variant_target_from(d, instance_component(inst)?, &inst.component.values)
}

/// Sibling variant that `values`'s variant property assignments select starting from `base_comp`, if different.
fn variant_target_from(d: &Document, base_comp: NodeId, values: &BTreeMap<String, PropValue>) -> Option<NodeId> {
    let comp = d.get(base_comp)?;
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
        if let Some(PropValue::Variant(v)) = values.get(&def.name) {
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

fn same_text_sizing_props(a: &TextProps, b: &TextProps) -> bool {
    a.style.font_family == b.style.font_family
        && a.style.font_weight == b.style.font_weight
        && a.style.italic == b.style.italic
        && a.style.font_size == b.style.font_size
        && a.style.line_height == b.style.line_height
        && a.style.letter_spacing == b.style.letter_spacing
        && a.style.paragraph_spacing == b.style.paragraph_spacing
        && a.style.paragraph_indent == b.style.paragraph_indent
        && a.style.list_spacing == b.style.list_spacing
        && a.style.leading_trim == b.style.leading_trim
        && a.style.case == b.style.case
        && a.style.features == b.style.features
        && a.style.variation_axes == b.style.variation_axes
        && a.runs == b.runs
        && a.auto_resize == b.auto_resize
        && a.max_lines == b.max_lines
        && a.paragraph == b.paragraph
        && a.lists == b.lists
        && a.direction == b.direction
        && a.text_wrap == b.text_wrap
}

fn same_text_sizing(from: &Node, to: &Node) -> bool {
    let (NodeKind::Text(a), NodeKind::Text(b)) = (&from.kind, &to.kind) else { return false };
    same_text_sizing_props(a, b)
        && same_binding_keys(from, to, &["fontSize", "fontFamily", "fontWeight", "lineHeight", "letterSpacing", "paragraphSpacing", "paragraphIndent"])
}

fn copy_overridden(from: &Node, to: &mut Node, fields: &BTreeSet<OverrideField>) {
    let target_align = (to.layout_child.grid_align_h, to.layout_child.grid_align_v);
    for f in fields {
        if *f == OverrideField::Size && !matches!(to.kind, NodeKind::Text(_)) && to.kind.frame_props().is_none() {
            crate::layout_constraints::apply_constrained_geometry(to, to.transform, from.size);
            copy_variable_bindings_for_field(*f, from, to);
        } else {
            copy_field(*f, from, to);
        }
    }
    if !fields.contains(&OverrideField::Size) && fields.contains(&OverrideField::Characters) && same_text_sizing(from, to) {
        if let NodeKind::Text(b) = &to.kind {
            match b.auto_resize {
                TextAutoResize::WidthAndHeight => to.size = from.size,
                TextAutoResize::Height if (to.size.x - from.size.x).abs() <= 1e-4 => to.size.y = from.size.y,
                _ => {}
            }
        }
    }
    if fields.contains(&OverrideField::Size) {
        for key in ["pen:explicit_width", "pen:explicit_height"] {
            if let Some(v) = from.plugin_data.get(key) {
                to.plugin_data.insert(key.into(), v.clone());
            }
        }
    }
    if fields.contains(&OverrideField::Stroke) {
        if let Some(v) = from.plugin_data.get("pen:explicit_stroke_fields") {
            to.plugin_data.insert("pen:explicit_stroke_fields".into(), v.clone());
        }
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
    // The overrides of a text style go with the link: an instance taking its main's text style takes
    // what the main overrides of it, and one resetting to the main has none left over.
    if key == "text" {
        to.text_style_overrides = from.text_style_overrides;
    }
    match from.style_refs.get(key) {
        Some(v) => {
            to.style_refs.insert(key.to_string(), v.clone());
        }
        None => {
            to.style_refs.remove(key);
        }
    }
}

fn binding_override_field(key: &str, is_text: bool) -> Option<OverrideField> {
    if key.starts_with("fills/") {
        Some(if is_text { OverrideField::TextStyle } else { OverrideField::Fills })
    } else if key.starts_with("strokes/") || matches!(key, "strokeWeight" | "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight")
    {
        Some(OverrideField::Stroke)
    } else if matches!(key, "fontSize" | "fontFamily" | "fontWeight" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent") {
        Some(OverrideField::TextStyle)
    } else if matches!(key, "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" | "cornerRadius") {
        Some(OverrideField::CornerRadii)
    } else if matches!(key, "itemSpacing" | "counterAxisSpacing" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft") {
        Some(OverrideField::Layout)
    } else if matches!(key, "width" | "height") {
        Some(OverrideField::Size)
    } else if key == "opacity" {
        Some(OverrideField::Opacity)
    } else if key == "visible" {
        Some(OverrideField::Visible)
    } else if key == "characters" {
        Some(OverrideField::Characters)
    } else {
        None
    }
}

fn copy_variable_bindings_for_field(f: OverrideField, from: &Node, to: &mut Node) {
    let is_text = matches!(to.kind, NodeKind::Text(_));
    to.variables.bindings.retain(|k, _| binding_override_field(k, is_text) != Some(f));
    for (k, v) in &from.variables.bindings {
        if binding_override_field(k, is_text) == Some(f) {
            to.variables.bindings.insert(k.clone(), v.clone());
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
            if let (NodeKind::Text(a), NodeKind::Text(b)) = (&from.kind, &mut to.kind) {
                b.style.fills = a.style.fills.clone();
            }
            copy_style_ref_key("fill", from, to);
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
        OverrideField::LockAspect => to.lock_aspect = from.lock_aspect,
        OverrideField::Arc => {
            if let (NodeKind::Ellipse(a), NodeKind::Ellipse(b)) = (&from.kind, &mut to.kind) {
                *b = *a;
            }
        }
        OverrideField::MainComponent | OverrideField::SlotContent => {}
    }
    copy_variable_bindings_for_field(f, from, to);
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

/// Drops a baked vector outline on a parametric shape so the renderer rebuilds it from its parameters.
pub fn drop_baked_outline_of_parametric_shape(n: &mut Node) {
    if matches!(
        n.kind,
        NodeKind::Ellipse(_)
            | NodeKind::Polygon { .. }
            | NodeKind::Star { .. }
            | NodeKind::Rectangle
            | NodeKind::Frame(_)
            | NodeKind::Component(_)
            | NodeKind::ComponentSet(_)
            | NodeKind::Instance { .. }
            | NodeKind::Section
    ) {
        n.fill_geometry = None;
        n.stroke_geometry = None;
    }
}

/// Copies every field of `r` that `n` does not override onto `n`.
fn apply_reference(r: &Node, n: &mut Node, top: bool, values: &BTreeMap<String, PropValue>) {
    let ov = n.component.overrides.clone();
    let own_arc_before = match &n.kind {
        NodeKind::Ellipse(a) => Some(*a),
        _ => None,
    };
    let own_radii_before = (n.corner_radii, n.corner_smoothing);
    let own_text = keeps_own_text_size(r, n, values);
    let keep_text_fill = matches!(n.kind, NodeKind::Text(_)) && (ov.contains(&OverrideField::Fills) || ov.contains(&OverrideField::TextStyle));
    let saved_text_fill = keep_text_fill.then(|| {
        let fills = match &n.kind {
            NodeKind::Text(tp) => tp.style.fills.clone(),
            _ => Vec::new(),
        };
        (fills, n.style_refs.get("fill").cloned())
    });
    for f in OverrideField::ALL {
        if f == OverrideField::Size
            && ov.contains(&OverrideField::Size)
            && !own_text
            && (n.plugin_data.contains_key("pen:explicit_width") || n.plugin_data.contains_key("pen:explicit_height"))
        {
            if n.plugin_data.get("pen:explicit_width").map(String::as_str) != Some("true") {
                n.size.x = r.size.x;
                n.layout_child.horizontal = r.layout_child.horizontal;
                n.layout_child.min_w = r.layout_child.min_w;
                n.layout_child.max_w = r.layout_child.max_w;
            }
            if n.plugin_data.get("pen:explicit_height").map(String::as_str) != Some("true") {
                n.size.y = r.size.y;
                n.layout_child.vertical = r.layout_child.vertical;
                n.layout_child.min_h = r.layout_child.min_h;
                n.layout_child.max_h = r.layout_child.max_h;
            }
            continue;
        }
        if f == OverrideField::Stroke && ov.contains(&OverrideField::Stroke) {
            if let Some(raw_fields) = n.plugin_data.get("pen:explicit_stroke_fields").cloned() {
                let has_field = |name: &str| raw_fields.split(',').map(str::trim).any(|p| p == name);
                if !has_field("paints") {
                    n.stroke.paints = r.stroke.paints.clone();
                    copy_style_ref_key("stroke", r, n);
                }
                if !has_field("width") {
                    n.stroke.weight = r.stroke.weight;
                    n.stroke.side_weights = r.stroke.side_weights;
                }
                if !has_field("align") {
                    n.stroke.align = r.stroke.align;
                }
                if !has_field("cap") {
                    n.stroke.cap = r.stroke.cap;
                }
                if !has_field("join") {
                    n.stroke.join = r.stroke.join;
                }
                if !has_field("dashes") {
                    n.stroke.dashes = r.stroke.dashes.clone();
                }
            }
            continue;
        }
        if ov.contains(&f) || matches!(f, OverrideField::MainComponent | OverrideField::SlotContent) {
            continue;
        }
        if top && matches!(f, OverrideField::Name | OverrideField::Transform | OverrideField::Visible | OverrideField::Locked) {
            continue;
        }
        if f == OverrideField::Name && matches!(n.kind, NodeKind::Instance { .. }) {
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
    if let Some((fills, fill_ref)) = saved_text_fill {
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.style.fills = fills;
        }
        match fill_ref {
            Some(v) => {
                n.style_refs.insert("fill".into(), v);
            }
            None => {
                n.style_refs.remove("fill");
            }
        }
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
    n.mask_type = r.mask_type;
    if !top {
        n.component.bindings = r.component.bindings.clone();
        n.component.is_slot = r.component.is_slot;
        n.component.is_slot_content = r.component.is_slot_content;
    }
    // Size sync copies the main's baked paths, but a local corner-radius override (or a main corner-radius
    // change on a size-overridden shape) makes those paths stale. Let the renderer rebuild parametric geometry.
    let radii_is_own = ov.contains(&OverrideField::CornerRadii);
    let radii_outline_stale = if radii_is_own {
        !ov.contains(&OverrideField::Size)
    } else {
        ov.contains(&OverrideField::Size) && (r.corner_radii, r.corner_smoothing) != own_radii_before
    };
    if radii_outline_stale {
        drop_baked_outline_of_parametric_shape(n);
    }
    // A baked outline draws the arc it was made for and outranks the arc: one that no longer matches is dropped, so
    // the ring is drawn from the arc and the size again.
    if let (NodeKind::Ellipse(ra), Some(own)) = (&r.kind, own_arc_before) {
        let arc_is_own = ov.contains(&OverrideField::Arc);
        let outline_is_stale = if arc_is_own { !ov.contains(&OverrideField::Size) } else { ov.contains(&OverrideField::Size) && *ra != own };
        if outline_is_stale {
            n.fill_geometry = None;
            n.stroke_geometry = None;
        }
    }
    match (&r.kind, &n.kind) {
        (NodeKind::Text(_), NodeKind::Text(_)) => {}
        (a, b) if a.frame_props().is_some() && b.frame_props().is_some() => {}
        (NodeKind::Ellipse(_), NodeKind::Ellipse(_)) if ov.contains(&OverrideField::Arc) => {}
        (a, b) if std::mem::discriminant(a) == std::mem::discriminant(b) => n.kind = a.clone(),
        (a, _) => n.kind = a.clone(),
    }
    if ov.contains(&OverrideField::Size) && n.size != r.size && !matches!(n.kind, NodeKind::Text(_)) && n.kind.frame_props().is_none() {
        let target_size = n.size;
        let (own_fill, own_stroke) = (n.fill_geometry.take(), n.stroke_geometry.take());
        // A vector without a bake of its own is resized as the main's vector is: the kernel sees the
        // main's bake, because that is what the paths just copied from the main were drawn with.
        if own_stroke.is_none() && matches!(n.kind, NodeKind::Vector { .. }) {
            n.stroke_geometry = r.stroke_geometry.clone();
        }
        n.size = r.size;
        crate::layout_constraints::apply_constrained_geometry(n, n.transform, target_size);
        n.fill_geometry = own_fill;
        if own_stroke.is_some() {
            n.stroke_geometry = own_stroke;
        }
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

fn orig_in_main<'a>(d: &'a Document, comp: Option<NodeId>, kn: &Node) -> Option<&'a Node> {
    let oc = comp?;
    let mut cur_src = kn.component.source;
    let mut hops = 0;
    while let Some(sid) = cur_src {
        hops += 1;
        if hops > 64 {
            break;
        }
        if sid == oc || d.is_ancestor(oc, sid) {
            return d.get(sid);
        }
        cur_src = d.get(sid)?.component.source;
    }
    None
}

fn effective_child_overrides(d: &Document, old_comp: Option<NodeId>, kn: &Node) -> BTreeSet<OverrideField> {
    let mut fields = kn.component.overrides.clone();
    let mut chain_fields = BTreeSet::new();
    let mut cur_src = kn.component.source;
    let mut orig_in_old_comp: Option<&Node> = None;
    let mut hops = 0;
    while let Some(sid) = cur_src {
        hops += 1;
        if hops > 64 {
            break;
        }
        if old_comp.is_some_and(|oc| sid == oc || d.is_ancestor(oc, sid)) {
            orig_in_old_comp = d.get(sid);
            break;
        }
        let Some(sn) = d.get(sid) else { break };
        chain_fields.extend(sn.component.overrides.iter().copied());
        cur_src = sn.component.source;
    }
    if let Some(orig) = orig_in_old_comp {
        fields.extend(chain_fields);
        for f in record_override(orig, kn) {
            if orig.component.bindings.contains_key(&f) || kn.component.bindings.contains_key(&f) {
                continue;
            }
            if matches!(
                f,
                OverrideField::Fills
                    | OverrideField::Stroke
                    | OverrideField::Opacity
                    | OverrideField::Blend
                    | OverrideField::Visible
                    | OverrideField::Effects
                    | OverrideField::CornerRadii
                    | OverrideField::Arc
                    | OverrideField::Characters
                    | OverrideField::TextStyle
            ) {
                fields.insert(f);
            }
        }
        if orig.variables.bindings != kn.variables.bindings {
            if orig.variables.bindings.keys().chain(kn.variables.bindings.keys()).any(|k| matches!(k.as_str(), "width" | "height")) {
                fields.insert(OverrideField::Size);
            }
            if orig.variables.bindings.keys().chain(kn.variables.bindings.keys()).any(|k| k.starts_with("fills/")) {
                fields.insert(OverrideField::Fills);
            }
            if orig.variables.bindings.keys().chain(kn.variables.bindings.keys()).any(|k| k.starts_with("strokes/")) {
                fields.insert(OverrideField::Stroke);
            }
            if orig.variables.bindings.get("opacity") != kn.variables.bindings.get("opacity") {
                fields.insert(OverrideField::Opacity);
            }
            if orig.variables.bindings.get("visible") != kn.variables.bindings.get("visible") {
                fields.insert(OverrideField::Visible);
            }
        }
    }
    fields.remove(&OverrideField::MainComponent);
    fields
}

fn set_or_remove_style_ref(to: &mut Node, key: &str, style_ref: Option<&str>) {
    match style_ref {
        Some(v) => {
            to.style_refs.insert(key.to_string(), v.to_string());
        }
        None => {
            to.style_refs.remove(key);
        }
    }
}

fn field_applies_to_kind(field: OverrideField, kind: &NodeKind) -> bool {
    match field {
        OverrideField::Characters | OverrideField::TextStyle => matches!(kind, NodeKind::Text(_)),
        OverrideField::Layout => kind.frame_props().is_some(),
        OverrideField::Arc => matches!(kind, NodeKind::Ellipse(_)),
        OverrideField::MainComponent | OverrideField::SlotContent => false,
        OverrideField::Name
        | OverrideField::Visible
        | OverrideField::Locked
        | OverrideField::Opacity
        | OverrideField::Blend
        | OverrideField::Size
        | OverrideField::Transform
        | OverrideField::Fills
        | OverrideField::Stroke
        | OverrideField::Effects
        | OverrideField::CornerRadii
        | OverrideField::LockAspect => true,
    }
}

fn extract_parked_override(path: &[NestedStep], from: &Node, orig_id: Option<NodeId>, field: OverrideField) -> Option<ParkedOverride> {
    let value = match field {
        OverrideField::Name => ParkedOverrideValue::Name(from.name.clone()),
        OverrideField::Visible => ParkedOverrideValue::Visible(from.visible),
        OverrideField::Locked => ParkedOverrideValue::Locked(from.locked),
        OverrideField::Opacity => ParkedOverrideValue::Opacity(from.opacity),
        OverrideField::Blend => ParkedOverrideValue::Blend(from.blend),
        OverrideField::Size => ParkedOverrideValue::Size {
            size: from.size,
            fill_geometry: from.fill_geometry.clone(),
            stroke_geometry: from.stroke_geometry.clone(),
            explicit_width: from.plugin_data.get("pen:explicit_width").cloned(),
            explicit_height: from.plugin_data.get("pen:explicit_height").cloned(),
        },
        OverrideField::Transform => ParkedOverrideValue::Transform(from.transform),
        OverrideField::Fills => ParkedOverrideValue::Fills {
            fills: from.fills.clone(),
            text_fills: match &from.kind {
                NodeKind::Text(tp) => Some(tp.style.fills.clone()),
                _ => None,
            },
            style_ref: from.style_refs.get("fill").cloned(),
        },
        OverrideField::Stroke => ParkedOverrideValue::Stroke {
            stroke: Box::new(from.stroke.clone()),
            style_ref: from.style_refs.get("stroke").cloned(),
            explicit_stroke_fields: from.plugin_data.get("pen:explicit_stroke_fields").cloned(),
        },
        OverrideField::Effects => ParkedOverrideValue::Effects { effects: from.effects.clone(), style_ref: from.style_refs.get("effect").cloned() },
        OverrideField::CornerRadii => ParkedOverrideValue::CornerRadii { radii: from.corner_radii, smoothing: from.corner_smoothing },
        OverrideField::Characters => {
            let NodeKind::Text(tp) = &from.kind else { return None };
            let fitted_size = matches!(tp.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height).then_some(from.size);
            ParkedOverrideValue::Characters { content: tp.content.clone(), fitted_size, sizing_text: fitted_size.map(|_| Box::new(tp.clone())) }
        }
        OverrideField::TextStyle => {
            let NodeKind::Text(tp) = &from.kind else { return None };
            ParkedOverrideValue::TextStyle {
                text: Box::new(tp.clone()),
                text_style_ref: from.style_refs.get("text").cloned(),
                text_style_overrides: from.text_style_overrides,
                fill_style_ref: from.style_refs.get("fill").cloned(),
            }
        }
        OverrideField::Layout => {
            let fp = from.kind.frame_props()?;
            ParkedOverrideValue::Layout {
                frame: Box::new(fp.clone()),
                layout_child: from.layout_child,
                grid_align_overrides: from.component.grid_align_overrides,
                grid_style_ref: from.style_refs.get("grid").cloned(),
            }
        }
        OverrideField::LockAspect => ParkedOverrideValue::LockAspect(from.lock_aspect),
        OverrideField::Arc => {
            let NodeKind::Ellipse(a) = &from.kind else { return None };
            ParkedOverrideValue::Arc(*a)
        }
        OverrideField::MainComponent | OverrideField::SlotContent => return None,
    };
    let is_text = matches!(from.kind, NodeKind::Text(_));
    let bindings = from
        .variables
        .bindings
        .iter()
        .filter(|(k, _)| {
            binding_override_field(k, is_text) == Some(field)
                || (field == OverrideField::Characters
                    && matches!(k.as_str(), "fontSize" | "fontFamily" | "fontWeight" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent"))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Some(ParkedOverride { path: path.to_vec(), field, value, bindings, origin_source: orig_id })
}

fn apply_parked_override_to_node(po: &ParkedOverride, to: &mut Node) {
    match &po.value {
        ParkedOverrideValue::Name(name) => to.name = name.clone(),
        ParkedOverrideValue::Visible(v) => to.visible = *v,
        ParkedOverrideValue::Locked(v) => to.locked = *v,
        ParkedOverrideValue::Opacity(v) => to.opacity = *v,
        ParkedOverrideValue::Blend(v) => to.blend = *v,
        ParkedOverrideValue::Size { size, fill_geometry, stroke_geometry, explicit_width, explicit_height } => {
            if !matches!(to.kind, NodeKind::Text(_)) && to.kind.frame_props().is_none() {
                crate::layout_constraints::apply_constrained_geometry(to, to.transform, *size);
            } else {
                to.size = *size;
                to.fill_geometry = fill_geometry.clone();
                to.stroke_geometry = stroke_geometry.clone();
            }
            if !matches!(to.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_)) && fill_geometry.is_none() && stroke_geometry.is_none() {
                to.fill_geometry = None;
                to.stroke_geometry = None;
            }
            if let Some(v) = explicit_width {
                to.plugin_data.insert("pen:explicit_width".into(), v.clone());
            }
            if let Some(v) = explicit_height {
                to.plugin_data.insert("pen:explicit_height".into(), v.clone());
            }
        }
        ParkedOverrideValue::Transform(tf) => to.transform = *tf,
        ParkedOverrideValue::Fills { fills, text_fills, style_ref } => {
            to.fills = fills.clone();
            if let (Some(tf), NodeKind::Text(b)) = (text_fills, &mut to.kind) {
                b.style.fills = tf.clone();
            }
            set_or_remove_style_ref(to, "fill", style_ref.as_deref());
        }
        ParkedOverrideValue::Stroke { stroke, style_ref, explicit_stroke_fields } => {
            to.stroke = (**stroke).clone();
            set_or_remove_style_ref(to, "stroke", style_ref.as_deref());
            if let Some(v) = explicit_stroke_fields {
                to.plugin_data.insert("pen:explicit_stroke_fields".into(), v.clone());
            }
        }
        ParkedOverrideValue::Effects { effects, style_ref } => {
            to.effects = effects.clone();
            set_or_remove_style_ref(to, "effect", style_ref.as_deref());
        }
        ParkedOverrideValue::CornerRadii { radii, smoothing } => {
            to.corner_radii = *radii;
            to.corner_smoothing = *smoothing;
        }
        ParkedOverrideValue::Characters { content, fitted_size, sizing_text } => {
            let can_restore_size = !to.component.overrides.contains(&OverrideField::Size)
                && sizing_text.as_ref().is_some_and(|a| {
                    let mut from_probe = Node::text(NodeId(0), Rect::new(0.0, 0.0, 0.0, 0.0), String::new(), a.style.clone());
                    from_probe.kind = NodeKind::Text((**a).clone());
                    from_probe.variables.bindings = po.bindings.clone();
                    same_text_sizing(&from_probe, to)
                });
            if let NodeKind::Text(b) = &mut to.kind {
                b.content = content.clone();
                if can_restore_size {
                    if let Some(sz) = fitted_size {
                        match b.auto_resize {
                            TextAutoResize::WidthAndHeight => to.size = *sz,
                            TextAutoResize::Height if (to.size.x - sz.x).abs() <= 1e-4 => to.size.y = sz.y,
                            _ => {}
                        }
                    }
                }
            }
        }
        ParkedOverrideValue::TextStyle { text, text_style_ref, text_style_overrides, fill_style_ref } => {
            if let NodeKind::Text(b) = &mut to.kind {
                b.style = text.style.clone();
                b.runs = text.runs.clone();
                b.auto_resize = text.auto_resize;
                b.max_lines = text.max_lines;
                b.paragraph = text.paragraph.clone();
                b.lists = text.lists.clone();
                b.direction = text.direction;
            }
            to.text_style_overrides = *text_style_overrides;
            set_or_remove_style_ref(to, "text", text_style_ref.as_deref());
            if matches!(to.kind, NodeKind::Text(_)) {
                set_or_remove_style_ref(to, "fill", fill_style_ref.as_deref());
            }
        }
        ParkedOverrideValue::Layout { frame, layout_child, grid_align_overrides, grid_style_ref } => {
            let target_align = (to.layout_child.grid_align_h, to.layout_child.grid_align_v);
            if let Some(b) = to.kind.frame_props_mut() {
                *b = (**frame).clone();
            }
            to.layout_child = *layout_child;
            if !grid_align_overrides[0] {
                to.layout_child.grid_align_h = target_align.0;
            }
            if !grid_align_overrides[1] {
                to.layout_child.grid_align_v = target_align.1;
            }
            to.component.grid_align_overrides = *grid_align_overrides;
            set_or_remove_style_ref(to, "grid", grid_style_ref.as_deref());
        }
        ParkedOverrideValue::LockAspect(v) => to.lock_aspect = *v,
        ParkedOverrideValue::Arc(a) => {
            if let NodeKind::Ellipse(b) = &mut to.kind {
                *b = *a;
            }
        }
    }
    let is_text = matches!(to.kind, NodeKind::Text(_));
    to.variables.bindings.retain(|k, _| binding_override_field(k, is_text) != Some(po.field));
    for (k, v) in &po.bindings {
        if binding_override_field(k, is_text) == Some(po.field) {
            to.variables.bindings.insert(k.clone(), v.clone());
        }
    }
    to.component.overrides.insert(po.field);
}

fn family_variants(d: &Document, comp_id: NodeId) -> Vec<NodeId> {
    if let Some(parent_id) = d.get(comp_id).and_then(|n| n.parent) {
        if let Some(parent) = d.get(parent_id) {
            if matches!(parent.kind, NodeKind::ComponentSet(_)) {
                return parent.children.iter().copied().filter(|&cid| matches!(d.get(cid).map(|n| &n.kind), Some(NodeKind::Component(_)))).collect();
            }
        }
    }
    vec![comp_id]
}

fn find_layer_at_path<'a>(d: &'a Document, root_comp: NodeId, path: &[NestedStep]) -> Option<&'a Node> {
    let mut cur_id = root_comp;
    let mut active_comp = Some(root_comp);
    for (step_name, step_occ) in path {
        let n = d.get(cur_id)?;
        let mut seen_names: BTreeMap<String, usize> = BTreeMap::new();
        let mut found: Option<&'a Node> = None;
        for &k in &n.children {
            let Some(kn) = d.get(k) else { continue };
            let base = base_layer(d, active_comp, kn);
            let base_name = &base.name;
            let seen = seen_names.entry(base_name.clone()).or_insert(0);
            let occ = *seen;
            *seen += 1;
            if base_name == step_name && occ == *step_occ {
                found = Some(kn);
                break;
            }
        }
        let kn = found?;
        let orig = orig_in_main(d, Some(root_comp), kn);
        let is_swapped_inst = matches!(kn.kind, NodeKind::Instance { .. })
            && (kn.component.overrides.contains(&OverrideField::MainComponent) || orig.and_then(instance_component) != instance_component(kn));
        active_comp = if is_swapped_inst || matches!(kn.kind, NodeKind::Instance { .. }) { instance_component(kn).or(active_comp) } else { active_comp };
        cur_id = kn.id;
    }
    d.get(cur_id)
}

fn family_has_compatible_layer(d: &Document, comp_id: NodeId, path: &[NestedStep], field: OverrideField) -> bool {
    family_variants(d, comp_id).into_iter().any(|var_id| find_layer_at_path(d, var_id, path).is_some_and(|node| field_applies_to_kind(field, &node.kind)))
}

fn direct_transfer_gate_allows(
    f: OverrideField,
    orig: &Node,
    new_orig: &Node,
    preserves_text_override: bool,
    has_characters_override: bool,
    same_family: bool,
    diff: &BTreeSet<OverrideField>,
) -> bool {
    let same_text_metrics = same_text_sizing(orig, new_orig);
    f == OverrideField::Name
        || (preserves_text_override && f == OverrideField::Characters)
        || (preserves_text_override && has_characters_override && same_text_metrics && f == OverrideField::Size)
        || (!same_family && f == OverrideField::TextStyle)
        || !diff.contains(&f)
}

/// Rebuilds the children of instance `i` from `new_comp`, carrying overrides
/// over to nodes with the same name path.
fn swap_main(c: &mut Ctx, i: NodeId, new_comp: NodeId) {
    let Some(inode) = c.s.get(i).cloned() else { return };
    let prior_parked = inode.component.parked_overrides.clone();
    if !prior_parked.is_empty() {
        c.had_parked.insert(i);
    }
    let mut newly_parked: Vec<ParkedOverride> = Vec::new();
    let old_comp = instance_component(&inode);
    let old_comp_node = old_comp.and_then(|oc| c.s.get(oc)).cloned();
    let top_swap = is_top(&inode);
    let same_family = old_comp
        .zip(Some(new_comp))
        .and_then(|(a, b)| Some((c.s.get(a)?.parent?, c.s.get(b)?.parent?)))
        .is_some_and(|(pa, pb)| pa == pb && matches!(c.s.get(pa).map(|n| &n.kind), Some(NodeKind::ComponentSet(_))));
    // Snapshot overridden descendants by base-name path, including overrides inherited from an enclosing component slot.
    let mut carried: Vec<CarriedOverride> = Vec::new();
    let mut inner = InnerSwaps::default();
    fn collect(
        d: &Document,
        id: NodeId,
        (old_comp, active_comp, top_swap, in_nested): (Option<NodeId>, Option<NodeId>, bool, bool),
        path: &mut Vec<NestedStep>,
        out: &mut Vec<CarriedOverride>,
        inner: &mut InnerSwaps,
    ) {
        let Some(n) = d.get(id) else { return };
        let mut seen_names: BTreeMap<String, usize> = BTreeMap::new();
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            let orig = orig_in_main(d, old_comp, kn);
            let base = base_layer(d, active_comp, kn);
            let base_name = base.name.clone();
            let seen = seen_names.entry(base_name.clone()).or_insert(0);
            let occurrence = *seen;
            *seen += 1;
            path.push((base_name, occurrence));
            let is_inst = matches!(kn.kind, NodeKind::Instance { .. });
            let is_swapped_inst =
                is_inst && (kn.component.overrides.contains(&OverrideField::MainComponent) || orig.and_then(instance_component) != instance_component(kn));
            let has_parked = is_inst && !kn.component.parked_overrides.is_empty();
            if is_swapped_inst || has_parked {
                // Only a nested instance the old main itself holds has a counterpart in the new main; anything else cannot move.
                match (orig, instance_component(kn)) {
                    (Some(_), Some(shown)) => inner.movable.push(InnerSwap {
                        path: path.clone(),
                        shown,
                        swapped: is_swapped_inst,
                        picks: kn.component.variant_picks.clone(),
                        parked: kn.component.parked_overrides.clone(),
                    }),
                    _ => {
                        if is_swapped_inst {
                            inner.dropped += 1;
                        }
                        inner.dropped_parked += kn.component.parked_overrides.len();
                    }
                }
            }
            let fields = if top_swap {
                let mut f = kn.component.overrides.clone();
                f.remove(&OverrideField::MainComponent);
                f
            } else {
                effective_child_overrides(d, active_comp, kn)
            };
            if !fields.is_empty() {
                out.push(CarriedOverride {
                    path: path.clone(),
                    old: kn.clone(),
                    orig_id: orig.or(Some(base)).map(|o| o.id),
                    fields,
                    own_layer: !in_nested && !is_inst,
                });
            }
            let child_active_comp = if is_swapped_inst { instance_component(kn).or(active_comp) } else { active_comp };
            collect(d, *k, (old_comp, child_active_comp, top_swap, in_nested || is_inst), path, out, inner);
            path.pop();
        }
    }
    collect(&c.s, i, (old_comp, old_comp, top_swap, false), &mut Vec::new(), &mut carried, &mut inner);
    let mut dropped_inner_swaps = inner.dropped;
    let mut dropped_fields = inner.dropped_parked;
    let old_text_count = count_text_nodes(&c.s, i);

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
    if let NodeKind::Instance { component, component_key, frame } = &mut n.kind {
        *component = Some(new_comp);
        *component_key = None;
        if let Some(f) = comp.kind.frame_props() {
            *frame = f.clone();
        }
    }
    let auto_named = old_comp_node.as_ref().is_some_and(|oc| {
        oc.name == n.name || oc.parent.and_then(|p| c.s.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))).is_some_and(|set| set.name == n.name)
    });
    if !same_family && !n.component.overrides.contains(&OverrideField::Name) && auto_named {
        n.name = comp.name.clone();
    }
    if top_swap && same_family {
        if !n.component.overrides.contains(&OverrideField::Size) {
            n.size = comp.size;
        }
    } else {
        let keep_x = old_comp_node.as_ref().is_some_and(|oc| (n.size.x - oc.size.x).abs() > 1e-4);
        let keep_y = old_comp_node.as_ref().is_some_and(|oc| (n.size.y - oc.size.y).abs() > 1e-4);
        if !keep_x {
            n.size.x = comp.size.x;
        }
        if !keep_y {
            n.size.y = comp.size.y;
        }
    }
    let target_size = n.size;
    if top_swap {
        n.component.source = Some(new_comp);
    }
    n.component.overrides.remove(&OverrideField::MainComponent);
    c.update(n);

    if target_size != comp.size {
        apply_instance_constraints(c, i, comp.size, target_size, 64);
    }

    // Re-apply carried overrides by base-name and sibling-occurrence path.
    let mut by_path: BTreeMap<Vec<NestedStep>, NodeId> = BTreeMap::new();
    fn index(
        d: &Document,
        id: NodeId,
        new_comp: Option<NodeId>,
        active_comp: Option<NodeId>,
        path: &mut Vec<NestedStep>,
        out: &mut BTreeMap<Vec<NestedStep>, NodeId>,
    ) {
        let Some(n) = d.get(id) else { return };
        let mut seen_names: BTreeMap<String, usize> = BTreeMap::new();
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            let orig = orig_in_main(d, new_comp, kn);
            let base = base_layer(d, active_comp, kn);
            let base_name = base.name.clone();
            let seen = seen_names.entry(base_name.clone()).or_insert(0);
            let occurrence = *seen;
            *seen += 1;
            path.push((base_name, occurrence));
            out.entry(path.clone()).or_insert(*k);
            let is_swapped_inst = matches!(kn.kind, NodeKind::Instance { .. })
                && (kn.component.overrides.contains(&OverrideField::MainComponent) || orig.and_then(instance_component) != instance_component(kn));
            let child_active_comp = if is_swapped_inst { instance_component(kn).or(active_comp) } else { active_comp };
            index(d, *k, new_comp, child_active_comp, path, out);
            path.pop();
        }
    }
    index(&c.s, i, Some(new_comp), Some(new_comp), &mut Vec::new(), &mut by_path);

    // A nested instance the user swapped moves with the swap when the new main holds an instance at the same name path; the
    // overrides below it then land on the swapped content. An inner swap with no counterpart stays behind, and is counted.
    let mut transferred_any = false;
    let mut used_swap_targets: BTreeSet<NodeId> = BTreeSet::new();
    for sw in &inner.movable {
        let target = by_path.get(&sw.path).copied().and_then(|nid| c.s.get(nid).cloned());
        let moves = target.as_ref().is_some_and(|t| {
            !used_swap_targets.contains(&t.id)
                && matches!(t.kind, NodeKind::Instance { .. })
                && matches!(c.s.get(sw.shown).map(|n| &n.kind), Some(NodeKind::Component(_)))
                && !would_hold_itself(c.s, sw.shown, t.id)
        });
        match target {
            Some(t) if moves => {
                used_swap_targets.insert(t.id);
                if sw.swapped && instance_component(&t) != Some(sw.shown) {
                    swap_main(c, t.id, sw.shown);
                    transferred_any = true;
                }
                if let Some(mut tn) = c.s.get(t.id).cloned() {
                    if sw.swapped {
                        tn.component.overrides.insert(OverrideField::MainComponent);
                        tn.component.variant_picks = sw.picks.clone();
                    }
                    if !sw.parked.is_empty() {
                        c.had_parked.insert(t.id);
                    }
                    for po in &sw.parked {
                        if !tn.component.parked_overrides.iter().any(|p| p.path == po.path && p.field == po.field) {
                            tn.component.parked_overrides.push(po.clone());
                        }
                    }
                    c.update(tn);
                }
            }
            _ => {
                if sw.swapped {
                    dropped_inner_swaps += 1;
                }
                dropped_fields += sw.parked.len();
            }
        }
    }
    if transferred_any {
        by_path.clear();
        index(&c.s, i, Some(new_comp), Some(new_comp), &mut Vec::new(), &mut by_path);
    }
    let new_texts = text_nodes(&c.s, i);
    let single_text_pair = old_text_count == 1 && new_texts.len() == 1;
    let mut used_override_targets: BTreeSet<NodeId> = BTreeSet::new();
    for item in carried {
        let CarriedOverride { path, old, orig_id, mut fields, own_layer } = item;
        let matched = by_path.get(&path).copied();
        let Some(nid) = matched.or_else(|| {
            let lone = new_texts.first().copied().filter(|_| single_text_pair && own_layer)?;
            (matches!(old.kind, NodeKind::Text(_)) && fields.iter().any(|f| matches!(f, OverrideField::Characters | OverrideField::TextStyle))).then_some(lone)
        }) else {
            for &f in &fields {
                if same_family && old.component.overrides.contains(&f) {
                    if let Some(po) = extract_parked_override(&path, &old, orig_id, f) {
                        newly_parked.push(po);
                    } else {
                        dropped_fields += 1;
                    }
                } else {
                    dropped_fields += 1;
                }
            }
            continue;
        };
        if !used_override_targets.insert(nid) {
            dropped_fields += fields.len();
            continue;
        }
        let Some(mut nn) = c.s.get(nid).cloned() else {
            dropped_fields += fields.len();
            continue;
        };
        let transfers_text = text_override_transfers(&old, &nn, matched.is_some(), single_text_pair);
        let preserves_text_override = transfers_text && matched.is_some();
        if !transfers_text {
            dropped_fields += usize::from(fields.remove(&OverrideField::Characters));
            dropped_fields += usize::from(fields.remove(&OverrideField::TextStyle));
        }
        if matched.is_none() {
            // The single text rule moves the text only; any other override on the missing layer is parked for its name path.
            let mut rest = Vec::new();
            fields.retain(|f| {
                let keep = matches!(f, OverrideField::Characters | OverrideField::TextStyle);
                if !keep {
                    rest.push(*f);
                }
                keep
            });
            for f in rest {
                if same_family && old.component.overrides.contains(&f) {
                    if let Some(po) = extract_parked_override(&path, &old, orig_id, f) {
                        newly_parked.push(po);
                    } else {
                        dropped_fields += 1;
                    }
                } else {
                    dropped_fields += 1;
                }
            }
        }
        if old.kind.frame_props().is_none() || nn.kind.frame_props().is_none() {
            dropped_fields += usize::from(fields.remove(&OverrideField::Layout));
        }
        let mut custom_layout_applied = false;
        if matched.is_some() || same_family {
            if let (Some(orig), Some(new_orig)) = (orig_id.and_then(|id| c.s.get(id)).cloned(), nn.component.source.and_then(|s| c.s.get(s)).cloned()) {
                if matched.is_some() && fields.contains(&OverrideField::Layout) {
                    fields.remove(&OverrideField::Layout);
                    custom_layout_applied = apply_transferred_layout(&orig, &new_orig, &old, &mut nn);
                    if !custom_layout_applied {
                        dropped_fields += 1;
                    }
                }
                let diff = transfer_gate_diff(&orig, &new_orig);
                let has_characters_override = fields.contains(&OverrideField::Characters);
                let before = fields.len();
                fields.retain(|&f| direct_transfer_gate_allows(f, &orig, &new_orig, preserves_text_override, has_characters_override, same_family, &diff));
                dropped_fields += before - fields.len();
            }
        }
        if fields.is_empty() && !custom_layout_applied {
            continue;
        }
        let kinds_differ = std::mem::discriminant(&old.kind) != std::mem::discriminant(&nn.kind);
        copy_overridden(&old, &mut nn, &fields);
        if custom_layout_applied {
            nn.component.overrides.insert(OverrideField::Layout);
        }
        if kinds_differ && fields.contains(&OverrideField::Size) && !matches!(nn.kind, NodeKind::Vector { .. } | NodeKind::BooleanOp(_)) {
            nn.fill_geometry = None;
            nn.stroke_geometry = None;
        }
        c.update(nn);
    }

    let mut remaining_parked: Vec<ParkedOverride> = Vec::new();
    let mut to_apply_by_node: BTreeMap<NodeId, Vec<ParkedOverride>> = BTreeMap::new();
    if same_family {
        for po in &prior_parked {
            if newly_parked.iter().any(|np| np.path == po.path && np.field == po.field) {
                continue;
            }
            let Some(orig) = po.origin_source.and_then(|id| c.s.get(id)) else {
                dropped_fields += 1;
                continue;
            };
            let Some(&nid) = by_path.get(&po.path) else {
                if family_has_compatible_layer(c.s, new_comp, &po.path, po.field) {
                    remaining_parked.push(po.clone());
                } else {
                    dropped_fields += 1;
                }
                continue;
            };
            let Some(nn) = c.s.get(nid) else {
                if family_has_compatible_layer(c.s, new_comp, &po.path, po.field) {
                    remaining_parked.push(po.clone());
                } else {
                    dropped_fields += 1;
                }
                continue;
            };
            if !field_applies_to_kind(po.field, &nn.kind) {
                if family_has_compatible_layer(c.s, new_comp, &po.path, po.field) {
                    remaining_parked.push(po.clone());
                } else {
                    dropped_fields += 1;
                }
                continue;
            }
            if nn.component.overrides.contains(&po.field) {
                // Live override on the returning layer wins; parked entry is removed.
                continue;
            }
            let Some(new_orig) = nn.component.source.and_then(|s| c.s.get(s)) else {
                dropped_fields += 1;
                continue;
            };
            let path_has_chars = nn.component.overrides.contains(&OverrideField::Characters)
                || prior_parked.iter().any(|p| {
                    p.path == po.path && p.field == OverrideField::Characters && !newly_parked.iter().any(|np| np.path == p.path && np.field == p.field)
                });
            let gate_allows = if po.field == OverrideField::Layout {
                let mut old_probe = orig.clone();
                apply_parked_override_to_node(po, &mut old_probe);
                let mut nn_probe = nn.clone();
                apply_transferred_layout(orig, new_orig, &old_probe, &mut nn_probe)
            } else {
                let diff = transfer_gate_diff(orig, new_orig);
                let preserves_text_override = text_override_transfers(orig, nn, true, false);
                direct_transfer_gate_allows(po.field, orig, new_orig, preserves_text_override, path_has_chars, true, &diff)
            };
            if gate_allows {
                to_apply_by_node.entry(nid).or_default().push(po.clone());
            } else if family_has_compatible_layer(c.s, new_comp, &po.path, po.field) {
                remaining_parked.push(po.clone());
            } else {
                dropped_fields += 1;
            }
        }
        for (nid, mut pos) in to_apply_by_node {
            let Some(mut nn) = c.s.get(nid).cloned() else { continue };
            pos.sort_by_key(|po| match po.field {
                OverrideField::TextStyle => 0u8,
                OverrideField::Characters => 1u8,
                _ => 2u8,
            });
            let mut changed = false;
            for po in &pos {
                if !nn.component.overrides.contains(&po.field) {
                    if po.field == OverrideField::Layout {
                        if let (Some(orig), Some(new_orig)) =
                            (po.origin_source.and_then(|id| c.s.get(id)).cloned(), nn.component.source.and_then(|s| c.s.get(s)).cloned())
                        {
                            let mut old_probe = orig.clone();
                            apply_parked_override_to_node(po, &mut old_probe);
                            if apply_transferred_layout(&orig, &new_orig, &old_probe, &mut nn) {
                                nn.component.overrides.insert(OverrideField::Layout);
                                changed = true;
                            }
                        }
                    } else {
                        apply_parked_override_to_node(po, &mut nn);
                        changed = true;
                    }
                }
            }
            if changed {
                c.update(nn);
            }
        }
        for po in newly_parked {
            if family_has_compatible_layer(c.s, new_comp, &po.path, po.field) {
                if let Some(existing) = remaining_parked.iter_mut().find(|p| p.path == po.path && p.field == po.field) {
                    *existing = po;
                } else {
                    remaining_parked.push(po);
                }
            } else {
                dropped_fields += 1;
            }
        }
    } else {
        dropped_fields += prior_parked.len();
    }
    let kept_for_return = remaining_parked.len();
    if !(prior_parked.is_empty() && remaining_parked.is_empty()) && c.s.get(i).is_some_and(|cur| cur.component.parked_overrides != remaining_parked) {
        if let Some(mut cur_inst) = c.s.get(i).cloned() {
            cur_inst.component.parked_overrides = remaining_parked;
            c.update(cur_inst);
        }
    }

    if dropped_inner_swaps > 0 || dropped_fields > 0 || kept_for_return > 0 {
        eprintln!(
            "designos-doc: swapping instance {i:?} dropped {dropped_inner_swaps} inner instance swap(s), {dropped_fields} field override(s), and kept {kept_for_return} override(s) for return"
        );
    }
    c.dropped_changes += dropped_inner_swaps + dropped_fields;
    c.kept_for_return += kept_for_return;
}

struct CarriedOverride {
    path: Vec<NestedStep>,
    old: Node,
    orig_id: Option<NodeId>,
    fields: BTreeSet<OverrideField>,
    own_layer: bool,
}

/// Base layer for a node inside an instance: the counterpart in `active_comp` (or the immediate linked source when
/// inside a swapped nested instance), falling back to `kn` itself.
fn base_layer<'a>(d: &'a Document, active_comp: Option<NodeId>, kn: &'a Node) -> &'a Node {
    orig_in_main(d, active_comp, kn).or_else(|| kn.component.source.and_then(|s| d.get(s))).unwrap_or(kn)
}

fn same_field_bindings(a: &Node, b: &Node, field: OverrideField) -> bool {
    let is_text = matches!(b.kind, NodeKind::Text(_));
    a.variables
        .bindings
        .keys()
        .chain(b.variables.bindings.keys())
        .filter(|k| binding_override_field(k, is_text) == Some(field))
        .all(|k| a.variables.bindings.get(k) == b.variables.bindings.get(k))
}

/// Diffs two main-side counterpart nodes for override transfer gating, comparing resolved paint and text-style values
/// while ignoring `style_refs`-only differences.
fn transfer_gate_diff(before: &Node, after: &Node) -> BTreeSet<OverrideField> {
    let mut diff = record_override(before, after);
    if before.fills == after.fills && same_field_bindings(before, after, OverrideField::Fills) {
        diff.remove(&OverrideField::Fills);
    }
    if before.stroke == after.stroke && same_field_bindings(before, after, OverrideField::Stroke) {
        diff.remove(&OverrideField::Stroke);
    }
    if before.effects == after.effects {
        diff.remove(&OverrideField::Effects);
    }
    if let (NodeKind::Text(a), NodeKind::Text(b)) = (&before.kind, &after.kind) {
        let same_text_style = a.style == b.style
            && a.runs == b.runs
            && a.auto_resize == b.auto_resize
            && a.max_lines == b.max_lines
            && a.paragraph == b.paragraph
            && a.lists == b.lists
            && a.direction == b.direction
            && same_field_bindings(before, after, OverrideField::TextStyle);
        if same_text_style {
            diff.remove(&OverrideField::TextStyle);
        }
    }
    diff
}

fn same_binding_keys(a: &Node, b: &Node, keys: &[&str]) -> bool {
    keys.iter().all(|k| a.variables.bindings.get(*k) == b.variables.bindings.get(*k))
}

fn copy_binding_keys(from: &Node, to: &mut Node, keys: &[&str]) {
    for &k in keys {
        match from.variables.bindings.get(k) {
            Some(v) => {
                to.variables.bindings.insert(k.to_string(), v.clone());
            }
            None => {
                to.variables.bindings.remove(k);
            }
        }
    }
}

/// Applies the sub-values of [`OverrideField::Layout`] (`padding`, `gap`, the rest of `auto_layout`, `clips_content`,
/// `grids`, and `layout_child`) that were equal between `orig` and `new_orig`. Returns `true` when at least one
/// overridden sub-value transferred.
fn apply_transferred_layout(orig: &Node, new_orig: &Node, old: &Node, nn: &mut Node) -> bool {
    const PAD_KEYS: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    const GAP_KEYS: [&str; 2] = ["itemSpacing", "counterAxisSpacing"];
    let (Some(orig_fp), Some(new_orig_fp), Some(old_fp)) =
        (orig.kind.frame_props().cloned(), new_orig.kind.frame_props().cloned(), old.kind.frame_props().cloned())
    else {
        return false;
    };
    let mut kept_any = false;
    let mut any_user_diff = false;

    if old_fp.clips_content != orig_fp.clips_content {
        any_user_diff = true;
        if orig_fp.clips_content == new_orig_fp.clips_content {
            if let Some(nn_fp) = nn.kind.frame_props_mut() {
                nn_fp.clips_content = old_fp.clips_content;
            }
            kept_any = true;
        }
    }

    if old_fp.grids != orig_fp.grids || old.style_refs.get("grid") != orig.style_refs.get("grid") {
        any_user_diff = true;
        if orig_fp.grids == new_orig_fp.grids {
            if let Some(nn_fp) = nn.kind.frame_props_mut() {
                nn_fp.grids = old_fp.grids.clone();
            }
            copy_style_ref_key("grid", old, nn);
            kept_any = true;
        }
    }

    match (&orig_fp.auto_layout, &new_orig_fp.auto_layout, &old_fp.auto_layout) {
        (Some(oa), Some(na), Some(fa)) => {
            let pad_changed = fa.padding != oa.padding || !same_binding_keys(old, orig, &PAD_KEYS);
            if pad_changed {
                any_user_diff = true;
                if oa.padding == na.padding && same_binding_keys(orig, new_orig, &PAD_KEYS) {
                    if let Some(Some(nna)) = nn.kind.frame_props_mut().map(|fp| &mut fp.auto_layout) {
                        nna.padding = fa.padding;
                    }
                    copy_binding_keys(old, nn, &PAD_KEYS);
                    kept_any = true;
                }
            }
            let gap_changed = fa.item_spacing != oa.item_spacing || fa.counter_spacing != oa.counter_spacing || !same_binding_keys(old, orig, &GAP_KEYS);
            if gap_changed {
                any_user_diff = true;
                if oa.item_spacing == na.item_spacing && oa.counter_spacing == na.counter_spacing && same_binding_keys(orig, new_orig, &GAP_KEYS) {
                    if let Some(Some(nna)) = nn.kind.frame_props_mut().map(|fp| &mut fp.auto_layout) {
                        nna.item_spacing = fa.item_spacing;
                        nna.counter_spacing = fa.counter_spacing;
                    }
                    copy_binding_keys(old, nn, &GAP_KEYS);
                    kept_any = true;
                }
            }
            let strip = |al: &AutoLayout| AutoLayout { padding: [0.0; 4], item_spacing: 0.0, counter_spacing: 0.0, ..al.clone() };
            if strip(fa) != strip(oa) {
                any_user_diff = true;
                if strip(oa) == strip(na) {
                    if let Some(Some(nna)) = nn.kind.frame_props_mut().map(|fp| &mut fp.auto_layout) {
                        let keep_pad = nna.padding;
                        let keep_item = nna.item_spacing;
                        let keep_counter = nna.counter_spacing;
                        *nna = fa.clone();
                        nna.padding = keep_pad;
                        nna.item_spacing = keep_item;
                        nna.counter_spacing = keep_counter;
                    }
                    kept_any = true;
                }
            }
        }
        _ => {
            let al_changed = old_fp.auto_layout != orig_fp.auto_layout || !same_binding_keys(old, orig, &PAD_KEYS) || !same_binding_keys(old, orig, &GAP_KEYS);
            if al_changed {
                any_user_diff = true;
                if orig_fp.auto_layout == new_orig_fp.auto_layout
                    && same_binding_keys(orig, new_orig, &PAD_KEYS)
                    && same_binding_keys(orig, new_orig, &GAP_KEYS)
                {
                    if let Some(nn_fp) = nn.kind.frame_props_mut() {
                        nn_fp.auto_layout = old_fp.auto_layout.clone();
                    }
                    copy_binding_keys(old, nn, &PAD_KEYS);
                    copy_binding_keys(old, nn, &GAP_KEYS);
                    kept_any = true;
                }
            }
        }
    }

    if old.layout_child != orig.layout_child || old.component.grid_align_overrides != [false, false] {
        any_user_diff = true;
        if orig.layout_child == new_orig.layout_child {
            let target_align = (nn.layout_child.grid_align_h, nn.layout_child.grid_align_v);
            nn.layout_child = old.layout_child;
            if !old.component.grid_align_overrides[0] {
                nn.layout_child.grid_align_h = target_align.0;
            }
            if !old.component.grid_align_overrides[1] {
                nn.layout_child.grid_align_v = target_align.1;
            }
            nn.component.grid_align_overrides = old.component.grid_align_overrides;
            kept_any = true;
        }
    }

    if !any_user_diff && orig_fp == new_orig_fp && orig.layout_child == new_orig.layout_child {
        kept_any = true;
    }

    kept_any
}

/// A nested instance the user swapped inside the instance being swapped: where it sits (by base-name path) and the main it
/// shows.
/// The inner swaps of one swapped instance: those a counterpart may take, and the count of those that cannot move at all.
#[derive(Default)]
struct InnerSwaps {
    movable: Vec<InnerSwap>,
    dropped: usize,
    dropped_parked: usize,
}

struct InnerSwap {
    path: Vec<NestedStep>,
    shown: NodeId,
    swapped: bool,
    /// The bound variant properties picked by hand on it, which travel with the swap.
    picks: BTreeSet<String>,
    parked: Vec<ParkedOverride>,
}

fn text_nodes(d: &Document, root: NodeId) -> Vec<NodeId> {
    fn walk(d: &Document, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(n) = d.get(id) else { return };
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            if matches!(kn.kind, NodeKind::Instance { .. }) {
                continue;
            }
            if matches!(kn.kind, NodeKind::Text(_)) {
                out.push(*k);
            }
            walk(d, *k, out);
        }
    }
    let mut out = Vec::new();
    walk(d, root, &mut out);
    out
}

fn count_text_nodes(d: &Document, root: NodeId) -> usize {
    fn walk(d: &Document, id: NodeId) -> usize {
        let Some(n) = d.get(id) else { return 0 };
        let mut count = 0;
        for k in &n.children {
            let Some(kn) = d.get(*k) else { continue };
            if matches!(kn.kind, NodeKind::Instance { .. }) {
                continue;
            }
            if matches!(kn.kind, NodeKind::Text(_)) {
                count += 1;
            }
            count += walk(d, *k);
        }
        count
    }
    walk(d, root)
}

/// The one rule that decides whether a text override crosses a swap, a variant switch or a property swap: both layers are
/// text, and either the layers match by name path (the way Figma matches layers) or each side holds exactly one text
/// layer and the text is not driven by a component property (a hoisted override stays with its property).
fn text_override_transfers(old: &Node, new: &Node, matched_by_path: bool, single_text_pair: bool) -> bool {
    matches!((&old.kind, &new.kind), (NodeKind::Text(_), NodeKind::Text(_)))
        && !old.component.bindings.contains_key(&OverrideField::Characters)
        && (matched_by_path || single_text_pair)
}

/// Brings instance node `i` in line with its reference `r`. `seen` holds the mains whose content `r` sits in or
/// mirrors, so that a main that holds an instance of itself (a file from an older build or an agent can carry one)
/// is mirrored once instead of without end: an instance of a main already in `seen` is a repeat, and keeps the
/// subtree it has.
fn sync_pair(c: &mut Ctx, r: NodeId, i: NodeId, top: bool, sync_root: bool, holder: NodeId, seen: &mut Vec<NodeId>) {
    let (Some(mut rn), Some(mut inode)) = (c.s.get(r).cloned(), c.s.get(i).cloned()) else { return };
    let slot_comp = instance_component(&rn);
    let slot_constraints = rn.constraints;
    let slot_layout_child = rn.layout_child;
    let values = c.s.get(holder).map(|h| h.component.values.clone()).unwrap_or_default();

    // The variable bound to a variant property of a nested instance is the main's: it follows it, as a binding on any
    // field the copy has no override of does.
    if !top && matches!(inode.kind, NodeKind::Instance { .. }) && matches!(rn.kind, NodeKind::Instance { .. }) {
        if let Some(followed) = follow_reference_variant_bindings(c.s, &inode, &rn) {
            c.update(followed.clone());
            inode = followed;
        }
    }

    // --- which component should this instance show? ---
    let mut swapped_here = false;
    if let Some(cur) = instance_component(&inode) {
        let mut desired = cur;
        if !inode.component.overrides.contains(&OverrideField::MainComponent) {
            if let Some(rc) = instance_component(&rn).filter(|_| !top) {
                desired = rc;
            }
        }
        let bindings: BTreeMap<_, _> = rn.component.bindings.iter().chain(inode.component.bindings.iter()).map(|(k, v)| (*k, v.clone())).collect();
        let mut prop_target = false;
        if let Some(PropValue::Instance(sw)) = bindings.get(&OverrideField::MainComponent).and_then(|p| values.get(p)) {
            if matches!(c.s.get(*sw).map(|n| &n.kind), Some(NodeKind::Component(_))) {
                desired = *sw;
                prop_target = true;
            }
        }
        let mut bound_switch = false;
        if top {
            if let Some(v) = variant_target(&c.s, &inode) {
                desired = v;
            }
        } else {
            // A bound variant property of a nested instance resolves its variable where this copy sits (the modes of
            // its ancestors), and the switch below is the one a person's pick makes. A property picked by hand keeps the
            // variant the copy shows; the others lie over it.
            let mut choices = bound_variant_choices(c.s, i, &inode, cur);
            choices.retain(|name, _| !inode.component.variant_picks.contains(name));
            // The bindings belong to the component the copy shows, not to the one an instance-swap property chose for it.
            if !choices.is_empty() && !prop_target {
                if let Some(v) = variant_target_from(&c.s, desired, &choices) {
                    desired = v;
                    bound_switch = true;
                }
            }
        }
        // not a main that would hold itself: the instance stays on the main it has
        if desired != cur && would_hold_itself(c.s, desired, i) {
            c.refused_swap.push((cur, desired));
        } else if desired != cur {
            // The switch a bound variant property makes on a copy holding a pick leaves the pick recorded (the swap drops
            // the record); a switch any other way (an instance-swap property) takes the record with it, as it always did.
            let holds_pick = bound_switch && inode.component.overrides.contains(&OverrideField::MainComponent);
            swap_main(c, i, desired);
            swapped_here = true;
            inode = match c.s.get(i) {
                Some(n) => n.clone(),
                None => return,
            };
            if holds_pick {
                inode.component.overrides.insert(OverrideField::MainComponent);
                c.update(inode.clone());
            }
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
    let eff_constraints = if inode.component.overrides.contains(&OverrideField::Layout) { inode.constraints } else { slot_constraints };
    let eff_layout_child = if inode.component.overrides.contains(&OverrideField::Layout) { inode.layout_child } else { slot_layout_child };
    let in_auto_layout_flow =
        !eff_layout_child.absolute && inode.parent.and_then(|ip| c.s.get(ip)).and_then(|p| p.kind.frame_props()).is_some_and(|fp| fp.auto_layout.is_some());
    if !top && instance_component(&inode) != slot_comp {
        if let Some(mut node_ref) = c.s.get(r).cloned() {
            let orig_slot_comp = slot_comp.and_then(|oc| c.s.get(oc));
            let slot_ov = orig_slot_comp.map(|oc| record_override(oc, &node_ref)).unwrap_or_default();
            // A direct swap at the sync root (or inside auto-layout flow) keeps what swap_main decided per axis.
            // Otherwise keep an axis where the main's own slot was resized away from its component, and let the
            // enclosing instance's constraint pass lay out the slot for the outer instance's current size.
            if swapped_here && (sync_root || in_auto_layout_flow) {
                node_ref.size = inode.size;
            } else {
                let keep_x = orig_slot_comp.is_some_and(|oc| (node_ref.size.x - oc.size.x).abs() > 1e-4);
                let keep_y = orig_slot_comp.is_some_and(|oc| (node_ref.size.y - oc.size.y).abs() > 1e-4);
                if !keep_x {
                    node_ref.size.x = rn.size.x;
                }
                if !keep_y {
                    node_ref.size.y = rn.size.y;
                }
            }
            for f in [
                OverrideField::Opacity,
                OverrideField::Blend,
                OverrideField::Fills,
                OverrideField::Stroke,
                OverrideField::Effects,
                OverrideField::CornerRadii,
                OverrideField::Layout,
            ] {
                if !node_ref.component.overrides.contains(&f) && !slot_ov.contains(&f) {
                    copy_field(f, &rn, &mut node_ref);
                }
            }
            apply_reference(&node_ref, &mut n, top, &values);
        } else {
            apply_reference(&rn, &mut n, top, &values);
        }
    } else {
        apply_reference(&rn, &mut n, top, &values);
    }
    if sync_root && !top && !swapped_here {
        if let Some((instance_parent_size, main_parent_size)) =
            c.s.get(i)
                .and_then(|node| node.parent)
                .zip(c.s.get(r).and_then(|node| node.parent))
                .and_then(|(ip, rp)| Some((c.s.get(ip)?.size, c.s.get(rp)?.size)))
        {
            let dx = (instance_parent_size.x - main_parent_size.x).abs() > 1e-4;
            let dy = (instance_parent_size.y - main_parent_size.y).abs() > 1e-4;
            if in_auto_layout_flow {
                if dx && eff_layout_child.horizontal == Sizing::Fill && !n.component.overrides.contains(&OverrideField::Size) {
                    n.size.x = inode.size.x;
                }
                if dy && eff_layout_child.vertical == Sizing::Fill && !n.component.overrides.contains(&OverrideField::Size) {
                    n.size.y = inode.size.y;
                }
            } else if dx || dy {
                let mut probe = n.clone();
                probe.constraints = eff_constraints;
                if let Some((new_tf, new_sz)) = crate::layout_constraints::constrain_child(&probe, main_parent_size, instance_parent_size) {
                    let tf = if n.component.overrides.contains(&OverrideField::Transform) { n.transform } else { new_tf };
                    let sz = if n.component.overrides.contains(&OverrideField::Size) { n.size } else { new_sz };
                    if tf != n.transform || sz != n.size {
                        crate::layout_constraints::apply_constrained_geometry(&mut n, tf, sz);
                    }
                }
            }
        }
    }
    apply_bindings(&mut n, &values);
    c.update(n);

    // An instance on an unopened page, or one the loader left empty, has no children yet, and the page loader holds what
    // the file recorded for them (its overrides and derived data). Filling it from the main would make the save write it
    // from those nodes instead of from the record, so only its own properties follow the main until the page is opened.
    let holds_record = |d: &Document| d.pending_instance_overrides(i).is_some() || d.pending_instance_derived(i).is_some();
    if c.s.get(i).is_some_and(|x| matches!(x.kind, NodeKind::Instance { .. }) && x.children.is_empty()) && holds_record(c.s) {
        return;
    }

    // --- children ---
    // A slot the user has edited keeps its content; once the override is cleared (Reset) the slot
    // mirrors the main component's children again, including dropping anything the user added.
    if inode.component.overrides.contains(&OverrideField::SlotContent) {
        return;
    }
    // A main that holds an instance of itself, met again inside its own mirror, is a repeat: the instance keeps the
    // subtree it has. A main that holds nothing of itself can sit in an instance of itself (a swap does that) and
    // is mirrored as usual.
    let own = main_of_reference(&rn);
    if let Some(m) = own.filter(|m| seen.contains(m)) {
        let holds_itself = *c.holds_itself.entry(m).or_insert_with(|| mains_held_by(c.s, &[m]).contains(&m));
        if holds_itself {
            return;
        }
    }
    let seen_before = seen.len();
    seen.extend(own);
    if swapped_here && !matches!(rn.kind, NodeKind::Component(_)) {
        relink_swapped_slot_children(c, ref_id, i);
    }
    // A nested instance whose bound variant switched while its reference still showed another main mirrors its own main's
    // layers. When the reference shows that main too (it followed the same variable), the copy is relinked to the
    // reference's layers instead of being rebuilt from them, which would drop its overrides.
    let has_parked = !inode.component.parked_overrides.is_empty() || c.had_parked.contains(&i);
    if !top
        && !swapped_here
        && (has_prop_binding(&inode) || (has_parked && inode.component.overrides.contains(&OverrideField::MainComponent)))
        && matches!(rn.kind, NodeKind::Instance { .. })
        && instance_component(&inode) == instance_component(&rn)
    {
        relink_swapped_slot_children(c, ref_id, i);
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
            sync_pair(c, *rc, ic, false, false, child_holder, seen);
            desired.push(ic);
        } else {
            let nodes = copy_tree(&mut c.s, *rc, true);
            let Some(nid) = nodes.first().map(|n| n.id) else { continue };
            let at = idx.min(c.s.get(i).map(|x| x.children.len()).unwrap_or(0));
            c.push(Command::Insert { parent: i, index: at, nodes });
            sync_pair(c, *rc, nid, false, false, child_holder, seen);
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
    let kept_size_override = c.s.get(i).is_some_and(|x| x.component.overrides.contains(&OverrideField::Size));
    if (!(top && swapped_here) || kept_size_override) && c.s.get(i).is_some_and(|x| x.kind.is_container()) {
        if let Some(slot_after) = c.s.get(i).map(|x| x.size).filter(|&sz| sz != rn.size) {
            apply_instance_constraints(c, i, rn.size, slot_after, 64);
        }
    }
    seen.truncate(seen_before);
}

fn apply_instance_constraints(c: &mut Ctx, parent: NodeId, old: Vec2, new: Vec2, depth: usize) {
    if depth == 0 || ((new.x - old.x).abs() <= 1e-4 && (new.y - old.y).abs() <= 1e-4) {
        return;
    }
    let Some(pnode) = c.s.get(parent) else { return };
    if matches!(pnode.kind, NodeKind::BooleanOp(_)) {
        for (cid, new_tf, new_sz) in crate::layout_constraints::plan_constraint_pass(c.s, parent, old, new, depth) {
            if cid == parent {
                continue;
            }
            let Some(mut child) = c.s.get(cid).cloned() else { continue };
            let tf = if child.component.overrides.contains(&OverrideField::Transform) { child.transform } else { new_tf };
            let sz = if child.component.overrides.contains(&OverrideField::Size) { child.size } else { new_sz };
            if tf != child.transform || sz != child.size {
                crate::layout_constraints::apply_constrained_geometry(&mut child, tf, sz);
                c.update(child);
            }
        }
        return;
    }
    let has_al = pnode.kind.frame_props().is_some_and(|f| f.auto_layout.is_some());
    constrain_direct_children(c, parent, old, new, depth);
    if has_al {
        solve_instance_auto_layout(c, parent, old, new, depth);
    }
}

fn constrain_direct_children(c: &mut Ctx, parent: NodeId, old: Vec2, new: Vec2, depth: usize) {
    if depth == 0 || ((new.x - old.x).abs() <= 1e-4 && (new.y - old.y).abs() <= 1e-4) {
        return;
    }
    for (cid, new_tf, new_sz) in crate::layout_constraints::plan_constraint_pass(c.s, parent, old, new, 1) {
        if cid == parent {
            continue;
        }
        let Some(mut child) = c.s.get(cid).cloned() else { continue };
        let old_child = child.size;
        let is_container = child.kind.is_container();
        let tf = if child.component.overrides.contains(&OverrideField::Transform) { child.transform } else { new_tf };
        let sz = if child.component.overrides.contains(&OverrideField::Size) { child.size } else { new_sz };
        if tf != child.transform || sz != child.size {
            crate::layout_constraints::apply_constrained_geometry(&mut child, tf, sz);
            c.update(child);
        }
        let size_changed = (sz.x - old_child.x).abs() > 1e-4 || (sz.y - old_child.y).abs() > 1e-4;
        if size_changed && is_container {
            apply_instance_constraints(c, cid, old_child, sz, depth - 1);
        }
    }
}

fn solve_instance_auto_layout(c: &mut Ctx, root: NodeId, old: Vec2, new: Vec2, depth: usize) {
    let Some(root_node) = c.s.get(root).cloned() else { return };
    let resolved = crate::resolve_node(c.s, &root_node);
    let Some(mut al) = resolved.kind.frame_props().and_then(|f| f.auto_layout.clone()) else { return };
    let stroke = resolved.stroke.clone();
    let horiz = al.direction != LayoutDirection::Vertical;
    if (new.x - old.x).abs() > 1e-4 {
        let target = if al.primary_is_horizontal() { &mut al.primary_sizing } else { &mut al.counter_sizing };
        if *target == Sizing::Hug {
            *target = Sizing::Fixed;
        }
    }
    if (new.y - old.y).abs() > 1e-4 {
        let target = if al.primary_is_horizontal() { &mut al.counter_sizing } else { &mut al.primary_sizing };
        if *target == Sizing::Hug {
            *target = Sizing::Fixed;
        }
    }
    let children = root_node.children.clone();
    for cid in &children {
        if let Some(mut cnode) = c.s.get(*cid).cloned() {
            if let Some(old_horiz) = cnode.layout_child.stretched.last_parent_horiz {
                if old_horiz != horiz {
                    cnode.layout_child.apply_parent_direction_change(old_horiz, horiz);
                    c.update(cnode);
                }
            }
        }
    }
    let measure = crate::text_measure::layout_measurer();
    let own_limits = crate::layout_solver::SizeLimits::from(&root_node.layout_child);
    let mut second_pass = false;
    while let Some(frame_node) = c.s.get(root).cloned() {
        let frame_size = frame_node.size;
        let (flow_ids, mut kids): (Vec<NodeId>, Vec<crate::layout_solver::ChildIn>) =
            children.iter().filter_map(|cid| c.s.flow_child_input(*cid, horiz).map(|k| (*cid, k))).collect::<Vec<_>>().into_iter().unzip();
        let out = {
            let doc = &*c.s;
            crate::layout_solver::compute_then_refit_wrapped(&al, frame_size, own_limits, &stroke, &flow_ids, &mut kids, |id, w| {
                let measure = measure.as_deref()?;
                crate::text_measure::height_resize_text_height(doc.get(id)?, w, measure)
            })
        };
        if ((out.frame.x - frame_size.x).abs() > 1e-3 || (out.frame.y - frame_size.y).abs() > 1e-3)
            && !frame_node.component.overrides.contains(&OverrideField::Size)
        {
            let mut fnode = frame_node;
            fnode.size = out.frame;
            c.update(fnode);
            constrain_direct_children(c, root, frame_size, out.frame, depth);
        }
        for ((cid, kid), r) in flow_ids.iter().zip(&kids).zip(&out.rects) {
            let Some(mut cnode) = c.s.get(*cid).cloned() else { continue };
            let old_child = cnode.size;
            let is_container = cnode.kind.is_container();
            let write = crate::layout_rotated::slot_write(&cnode.transform, cnode.size, *r);
            let (new_tf, new_sz) = (write.transform, write.size);
            let tf = if cnode.component.overrides.contains(&OverrideField::Transform) { cnode.transform } else { new_tf };
            let sz = if cnode.component.overrides.contains(&OverrideField::Size) { cnode.size } else { new_sz };
            let size_changed = (sz.x - old_child.x).abs() > 1e-3 || (sz.y - old_child.y).abs() > 1e-3;
            if size_changed
                && matches!(
                    cnode.kind,
                    NodeKind::Vector { .. }
                        | NodeKind::BooleanOp(_)
                        | NodeKind::Rectangle
                        | NodeKind::Ellipse(_)
                        | NodeKind::Star { .. }
                        | NodeKind::Polygon { .. }
                        | NodeKind::Line
                )
            {
                crate::layout_constraints::apply_constrained_geometry(&mut cnode, tf, sz);
            } else {
                cnode.transform = tf;
                cnode.size = sz;
            }
            cnode.layout_child.record_stretched(kid.size, crate::layout_rotated::extent(&tf, sz), horiz, kid.stretches_cross());
            c.update(cnode);
            if size_changed && is_container {
                apply_instance_constraints(c, *cid, old_child, sz, depth - 1);
            }
        }
        let drifted =
            flow_ids.iter().zip(out.rects.iter()).any(|(cid, r)| c.s.get(*cid).is_some_and(|n| crate::layout_rotated::slot_drifted(&n.transform, n.size, *r)));
        if second_pass || !drifted {
            break;
        }
        second_pass = true;
    }
}

/// The layer at the end of the `source` chain of `id`: the layer that a copy at any depth stands for. The chain stops at
/// an instance root, whose `source` is its main component: two instances of the same inner component (a button's leading
/// and trailing icon) are two layers, not one.
fn source_root(d: &Document, id: NodeId) -> NodeId {
    let mut cur = id;
    for _ in 0..64 {
        match d.get(cur).and_then(|n| n.component.source).filter(|s| d.get(*s).is_some_and(|x| !matches!(x.kind, NodeKind::Component(_)))) {
            Some(next) => cur = next,
            None => break,
        }
    }
    cur
}

/// Links the layers of `inst_node` to the layers of `slot_node` that stand for the same main layer. A layer of a copy that
/// shows its own main links to that main's layer, while a layer of the reference one level up links to the reference's
/// own source; both reach the same main layer at the end of their `source` chain, which is how they are matched at any depth.
fn relink_swapped_slot_children(c: &mut Ctx, slot_node: NodeId, inst_node: NodeId) {
    let rkids = c.s.get(slot_node).map(|x| x.children.clone()).unwrap_or_default();
    let ikids = c.s.get(inst_node).map(|x| x.children.clone()).unwrap_or_default();
    let mut slot_by_src: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    for rc in rkids {
        if let Some(src) = c.s.get(rc).and_then(|n| n.component.source) {
            slot_by_src.entry(source_root(c.s, src)).or_insert(rc);
        }
    }
    for ic in ikids {
        let Some(mut ic_node) = c.s.get(ic).cloned() else { continue };
        let Some(rc) = ic_node.component.source.and_then(|src| slot_by_src.get(&source_root(c.s, src)).copied()) else { continue };
        let same_sub_comp = instance_component(&ic_node) == c.s.get(rc).and_then(instance_component);
        ic_node.component.source = Some(rc);
        c.update(ic_node);
        if same_sub_comp {
            relink_swapped_slot_children(c, rc, ic);
        }
    }
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Swaps an instance to another component (manual "Swap instance"). The
/// instance keeps its position and records a `MainComponent` override.
pub fn swap_instance(doc: &mut Document, instance: NodeId, new_comp: NodeId) -> Command {
    swap_instance_inner(doc, instance, new_comp).0
}

pub(crate) fn swap_instance_inner(doc: &mut Document, instance: NodeId, new_comp: NodeId) -> (Command, usize) {
    let (cmd, dropped, _) = swap_instance_reporting(doc, instance, new_comp);
    (cmd, dropped)
}

pub(crate) fn swap_instance_reporting(doc: &mut Document, instance: NodeId, new_comp: NodeId) -> (Command, usize, usize) {
    let mut c = Ctx::new(doc);
    if !swap_recorded(&mut c, instance, new_comp) {
        return (nothing(), 0, 0);
    }
    let dropped = c.dropped_changes;
    let kept = c.kept_for_return;
    (c.finish(), dropped, kept)
}

/// Swaps `instance` to `new_comp` and records the swap as a `MainComponent` override. False when nothing changed: the
/// instance already shows `new_comp`, `new_comp` is no main, or it would hold itself.
fn swap_recorded(c: &mut Ctx, instance: NodeId, new_comp: NodeId) -> bool {
    let Some(n) = c.s.get(instance) else { return false };
    if instance_component(n) == Some(new_comp) || !matches!(c.s.get(new_comp).map(|x| &x.kind), Some(NodeKind::Component(_))) {
        return false;
    }
    // not inside the new component, nor inside a main it holds an instance of: that main would hold itself
    if new_comp == instance || would_hold_itself(c.s, new_comp, instance) {
        return false;
    }
    let old_comp = c.s.get(instance).and_then(instance_component);
    swap_main(c, instance, new_comp);
    if let Some(mut n) = c.s.get(instance).cloned() {
        n.component.overrides.insert(OverrideField::MainComponent);
        record_variant_swap(c.s, &mut n, old_comp, new_comp);
        c.update(n);
    }
    true
}

/// Sets (or with `None` clears) a component property value on an instance
/// root and re-syncs it: toggles visibility, swaps text, instance or variant.
pub fn set_instance_prop(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> Command {
    set_instance_prop_inner(doc, instance, name, value).0
}

/// [`set_instance_prop`], also why the edit was refused when the guard refused it: the sentence of
/// [`crate::cycle_refusal_message`] for the swap that the value would make. `None` when the edit went through.
pub fn set_instance_prop_explained(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> (Command, Option<String>) {
    let (cmd, _, _, refused) = set_instance_prop_reporting(doc, instance, name, value);
    let reason = refused.first().map(|swap| crate::refused_swap_message(doc, instance, *swap));
    (cmd, reason)
}

fn set_instance_prop_inner(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> (Command, usize) {
    let (cmd, dropped, _, _) = set_instance_prop_reporting(doc, instance, name, value);
    (cmd, dropped)
}

#[cfg(test)]
fn set_instance_prop_counts(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> (Command, usize, usize) {
    let (cmd, dropped, kept, _) = set_instance_prop_reporting(doc, instance, name, value);
    (cmd, dropped, kept)
}

fn set_instance_prop_reporting(doc: &mut Document, instance: NodeId, name: &str, value: Option<PropValue>) -> (Command, usize, usize, Vec<(NodeId, NodeId)>) {
    set_instance_prop_edit_counts_reporting(doc, instance, |n| match value {
        Some(v) => {
            if matches!(v, PropValue::Variant(_)) {
                record_variant_pick(n, name);
            }
            n.component.values.insert(name.to_string(), v);
        }
        None => {
            n.component.values.remove(name);
            n.component.variant_picks.remove(name);
        }
    })
}

/// [`set_instance_prop`] for any edit of the instance root (its stored values, its variable bindings): the edit is
/// applied, then the instance is synced, so a changed value reaches the layers (or the variant) it drives in one command.
pub(crate) fn set_instance_prop_edit(doc: &mut Document, instance: NodeId, edit: impl FnOnce(&mut Node)) -> (Command, usize) {
    let (cmd, dropped, _) = set_instance_prop_edit_reporting(doc, instance, edit);
    (cmd, dropped)
}

/// [`set_instance_prop_edit`], also the swaps the guard refused that stopped the edit, as `(main shown, main wanted)`.
/// The list is empty when the edit went through, and when it changed nothing for another reason.
pub(crate) fn set_instance_prop_edit_reporting(doc: &mut Document, instance: NodeId, edit: impl FnOnce(&mut Node)) -> (Command, usize, Vec<(NodeId, NodeId)>) {
    let (cmd, dropped, _, refused) = set_instance_prop_edit_counts_reporting(doc, instance, edit);
    (cmd, dropped, refused)
}

fn set_instance_prop_edit_counts_reporting(
    doc: &mut Document,
    instance: NodeId,
    edit: impl FnOnce(&mut Node),
) -> (Command, usize, usize, Vec<(NodeId, NodeId)>) {
    let mut c = Ctx::new(doc);
    if let Some(mut n) = c.s.get(instance).cloned() {
        edit(&mut n);
        c.update(n);
    }
    sync_one(&mut c, instance);
    let refused = std::mem::take(&mut c.refused_swap);
    let dropped = c.dropped_changes;
    let kept = c.kept_for_return;
    let cmd = c.finish();
    if refused.is_empty() {
        return (cmd, dropped, kept, Vec::new());
    }
    // A value that would swap the instance to a main that holds itself in it is not stored: nothing changes. A swap
    // the instance already refuses without this value (a stored variant that a rename or an older build left pointing
    // at such a main) is not this value's doing, so it does not stop the edit.
    let stored = refused_swaps(doc, instance);
    let blocking: Vec<(NodeId, NodeId)> = refused.into_iter().filter(|r| !stored.contains(r)).collect();
    if blocking.is_empty() {
        (cmd, dropped, kept, Vec::new())
    } else {
        (nothing(), 0, 0, blocking)
    }
}

/// Sets (or with `None` clears) a component property on an instance nested in an outer instance, the way the outer
/// instance's panel edits one of the properties a nested instance exposes. A variant property switches the nested
/// instance to the sibling variant that the choice selects, as a swap of that nested layer does: the switch is recorded
/// on the nested instance as a `MainComponent` override, so a sync of the outer main keeps it and "Reset all changes"
/// takes it back. Any other property is stored as a value, as [`set_instance_prop`] does.
pub fn set_nested_instance_prop(doc: &mut Document, nested: NodeId, name: &str, value: Option<PropValue>) -> Command {
    let Some(PropValue::Variant(choice)) = &value else { return set_instance_prop(doc, nested, name, value) };
    let Some(current) = doc.get(nested).and_then(instance_component) else { return nothing() };
    let chosen = BTreeMap::from([(name.to_string(), PropValue::Variant(choice.clone()))]);
    let Some(target) = variant_target_from(doc, current, &chosen) else { return pin_shown_variant(doc, nested, name, choice, current) };
    let mut c = Ctx::new(doc);
    if !swap_recorded(&mut c, nested, target) {
        return nothing();
    }
    // The component the nested instance shows now names its variant; a value kept for it would read as the old choice.
    if let Some(mut n) = c.s.get(nested).cloned() {
        if n.component.values.remove(name).is_some() {
            c.update(n);
        }
    }
    // The layers of the new variant start from the variant itself: the other values of the nested instance (an instance
    // swap, a boolean, a text) are applied to them in this command. The variant is not resolved or swapped again, as the
    // nested instance is not a top instance and holds its swap.
    sync_one(&mut c, nested);
    c.finish()
}

/// A pick of the variant a nested copy shows already: nothing switches, but a bound property is held on it from now on (the
/// copy keeps the main it shows, and `name` stays out of what the binding moves). Nothing changes for a property that is
/// not bound, or when `choice` is not what the copy shows.
fn pin_shown_variant(doc: &mut Document, nested: NodeId, name: &str, choice: &str, shown: NodeId) -> Command {
    let shows_choice = doc.get(shown).is_some_and(|c| parse_variant_name(&c.name).iter().any(|(k, v)| k == name && v == choice));
    let Some(mut n) = doc.get(nested).cloned().filter(|n| shows_choice && n.variables.bindings.contains_key(&prop_binding_key(name))) else {
        return nothing();
    };
    let before = n.clone();
    n.component.overrides.insert(OverrideField::MainComponent);
    n.component.variant_picks.insert(name.to_string());
    n.component.values.remove(name);
    if n == before {
        return nothing();
    }
    let mut c = Ctx::new(doc);
    c.update(n);
    c.finish()
}

/// The swaps a sync of `instance`, as it stands, refuses. The sync's writes are taken back.
fn refused_swaps(doc: &mut Document, instance: NodeId) -> Vec<(NodeId, NodeId)> {
    let mut c = Ctx::new(doc);
    sync_one(&mut c, instance);
    let refused = std::mem::take(&mut c.refused_swap);
    c.finish();
    refused
}

/// A primary nested instance inside a main `Component` or `ComponentSet` that can have its
/// properties exposed on outer instances (`propsAreBubbled` / `expose_nested_instances`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExposeNestedCandidate {
    /// Representative primary nested instance node id.
    pub id: NodeId,
    /// All connected primary nested instances across sibling variants of a `ComponentSet`.
    pub connected_ids: Vec<NodeId>,
    /// Layer name of the nested instance.
    pub name: String,
    /// Whether `expose_nested_instances` (`propsAreBubbled`) is enabled on this nested instance.
    pub exposed: bool,
    /// Names of deeper exposed nested instances inside this nested instance's backing component.
    pub deeper_exposed_names: Vec<String>,
}

fn component_family_root(doc: &Document, comp: NodeId) -> NodeId {
    doc.get(comp).and_then(|n| n.parent).filter(|&p| matches!(doc.get(p).map(|pn| &pn.kind), Some(NodeKind::ComponentSet(_)))).unwrap_or(comp)
}

fn instance_family_root(doc: &Document, inst: NodeId) -> Option<NodeId> {
    let comp = doc.get(inst).and_then(instance_component)?;
    Some(component_family_root(doc, comp))
}

fn instance_has_exposable_props(doc: &Document, inst: NodeId) -> bool {
    let Some(comp) = doc.get(inst).and_then(instance_component) else { return false };
    let has_own = prop_defs(doc, comp).iter().any(|d| !matches!(d.kind, PropKind::Slot));
    has_own || !exposed_nested_instances(doc, inst).is_empty()
}

/// One step of the path to a nested instance: the layer name, and which of the parent's same-named children it is
/// (the first `Icon` is 0, the second 1), so two layers that share a name are two places.
type NestedStep = (String, usize);

/// The primary nested instances of `comp` that have properties to expose, each with its path from `comp`.
fn primary_instances_in_component(doc: &Document, comp: NodeId) -> Vec<(Vec<NestedStep>, NodeId)> {
    fn walk(doc: &Document, id: NodeId, path: &mut Vec<NestedStep>, out: &mut Vec<(Vec<NestedStep>, NodeId)>) {
        let Some(n) = doc.get(id) else { return };
        let mut seen_names: BTreeMap<&str, usize> = BTreeMap::new();
        for &cid in &n.children {
            let Some(cn) = doc.get(cid) else { continue };
            // Every child counts, the slots too, so the same layout reads the same in every variant.
            let seen = seen_names.entry(cn.name.as_str()).or_insert(0);
            let occurrence = *seen;
            *seen += 1;
            if is_slot_container(doc, cid) {
                continue;
            }
            path.push((cn.name.clone(), occurrence));
            if matches!(cn.kind, NodeKind::Instance { .. }) {
                if instance_has_exposable_props(doc, cid) {
                    out.push((path.clone(), cid));
                }
            } else if cn.kind.is_container() {
                walk(doc, cid, path, out);
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    let mut path = Vec::new();
    walk(doc, comp, &mut path, &mut out);
    out
}

/// Lists the primary nested instances inside `comp_or_set` (a `Component` or `ComponentSet`) that
/// can be chosen under "Expose properties from" -> "Nested instances".
pub fn expose_nested_candidates(doc: &Document, comp_or_set: NodeId) -> Vec<ExposeNestedCandidate> {
    let Some(node) = doc.get(comp_or_set) else { return Vec::new() };
    let variants: Vec<NodeId> = match &node.kind {
        NodeKind::ComponentSet(_) => {
            node.children.iter().copied().filter(|&cid| matches!(doc.get(cid).map(|c| &c.kind), Some(NodeKind::Component(_)))).collect()
        }
        NodeKind::Component(_) => {
            if let Some(set_id) = node.parent.filter(|&p| matches!(doc.get(p).map(|pn| &pn.kind), Some(NodeKind::ComponentSet(_)))) {
                let mut v = vec![comp_or_set];
                if let Some(set) = doc.get(set_id) {
                    for &cid in &set.children {
                        if cid != comp_or_set && matches!(doc.get(cid).map(|c| &c.kind), Some(NodeKind::Component(_))) {
                            v.push(cid);
                        }
                    }
                }
                v
            } else {
                vec![comp_or_set]
            }
        }
        _ => return Vec::new(),
    };

    // Within one variant every nested instance is a row of its own. Across the variants of a set, the nested instances at
    // the same path and of the same component family are one row: they are exposed together.
    type ExposeGroupKey = (Vec<NestedStep>, Option<NodeId>);
    let mut groups: Vec<(ExposeGroupKey, ExposeNestedCandidate)> = Vec::new();
    for v in variants {
        for (rel_path, nid) in primary_instances_in_component(doc, v) {
            let fam = instance_family_root(doc, nid);
            let key = (rel_path, fam);
            let is_bub = doc.get(nid).is_some_and(|n| n.component.expose_nested_instances);
            let row = match groups.iter().position(|(k, _)| *k == key) {
                Some(row) => row,
                None => {
                    let name = doc.get(nid).map(|n| n.name.clone()).unwrap_or_default();
                    groups.push((key, ExposeNestedCandidate { id: nid, connected_ids: Vec::new(), name, exposed: false, deeper_exposed_names: Vec::new() }));
                    groups.len() - 1
                }
            };
            let cand = &mut groups[row].1;
            if !cand.connected_ids.contains(&nid) {
                cand.connected_ids.push(nid);
            }
            cand.exposed |= is_bub;
            // The names below a row come from every variant it connects: one variant may hold a kind of the nested
            // component that exposes something another does not.
            for did in exposed_nested_instances(doc, nid) {
                let Some(deeper) = doc.get(did).map(|dn| dn.name.clone()) else { continue };
                if !cand.deeper_exposed_names.contains(&deeper) {
                    cand.deeper_exposed_names.push(deeper);
                }
            }
        }
    }
    groups.into_iter().map(|(_, cand)| cand).collect()
}

/// Sets `expose_nested_instances` (`propsAreBubbled`) on a primary nested instance inside a main
/// `Component` (and on all connected primary nested instances across sibling variants when the
/// component belongs to a `ComponentSet`). Turning expose off leaves any existing overrides on
/// outer instances intact.
pub fn set_expose_nested_instance(doc: &mut Document, nested_inst: NodeId, expose: bool) -> Command {
    let Some(n) = doc.get(nested_inst) else { return nothing() };
    if !matches!(n.kind, NodeKind::Instance { .. }) || n.component.is_slot_content {
        return nothing();
    }
    let Some(comp) = enclosing_component(doc, nested_inst) else { return nothing() };
    // Must be a primary nested instance of `comp` (not inside another nested instance or slot inside `comp`).
    let mut cur = n.parent;
    while let Some(p) = cur {
        if p == comp {
            break;
        }
        let Some(pn) = doc.get(p) else { return nothing() };
        if matches!(pn.kind, NodeKind::Instance { .. }) || is_slot_container(doc, p) || pn.component.is_slot_content {
            return nothing();
        }
        cur = pn.parent;
    }

    let cands = expose_nested_candidates(doc, comp);
    let targets: Vec<NodeId> = cands.into_iter().find(|c| c.connected_ids.contains(&nested_inst)).map(|c| c.connected_ids).unwrap_or_else(|| vec![nested_inst]);

    let mut c = Ctx::new(doc);
    for tid in targets {
        if let Some(mut tn) = c.s.get(tid).cloned() {
            if tn.component.expose_nested_instances != expose {
                tn.component.expose_nested_instances = expose;
                c.update(tn);
            }
        }
    }
    c.finish()
}

fn is_nested_instance_exposed_in(doc: &Document, nested_inst: NodeId, enclosing_inst: NodeId) -> bool {
    let Some(n) = doc.get(nested_inst) else { return false };
    if let Some(enclosing_main) = doc.get(enclosing_inst).and_then(instance_component) {
        let enclosing_family = component_family_root(doc, enclosing_main);
        let mut cur_src = n.component.source;
        let mut hops = 0usize;
        while let Some(sid) = cur_src {
            hops += 1;
            if hops > 64 {
                break;
            }
            let Some(sn) = doc.get(sid) else { break };
            if let Some(sc) = enclosing_component(doc, sid) {
                if component_family_root(doc, sc) == enclosing_family {
                    return sn.component.expose_nested_instances;
                }
            }
            cur_src = sn.component.source;
        }
    }
    n.component.expose_nested_instances
}

/// Returns the exposed nested `Instance` nodes inside `outer_inst` in pre-order (Figma's
/// `bubbledNestedInstanceGUIDs` order). Deeper nested instances (an exposed instance inside an
/// exposed instance) are included immediately after their parent exposed instance; nested instances
/// inside an unexposed instance are not included.
pub fn exposed_nested_instances(doc: &Document, outer_inst: NodeId) -> Vec<NodeId> {
    fn walk(doc: &Document, container: NodeId, enclosing_inst: NodeId, out: &mut Vec<NodeId>, depth: usize) {
        if depth > 64 {
            return;
        }
        let Some(n) = doc.get(container) else { return };
        for &cid in &n.children {
            if is_slot_container(doc, cid) {
                continue;
            }
            let Some(cn) = doc.get(cid) else { continue };
            if matches!(cn.kind, NodeKind::Instance { .. }) {
                if is_nested_instance_exposed_in(doc, cid, enclosing_inst) {
                    out.push(cid);
                    walk(doc, cid, cid, out, depth + 1);
                }
            } else if cn.kind.is_container() {
                walk(doc, cid, enclosing_inst, out, depth + 1);
            }
        }
    }
    let mut out = Vec::new();
    walk(doc, outer_inst, outer_inst, &mut out, 0);
    out
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
    if let Some(var) = o.component.prop_default_variables.remove(old) {
        o.component.prop_default_variables.insert(new.to_string(), var);
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
        if n.component.variant_picks.remove(old) {
            n.component.variant_picks.insert(new.to_string());
        }
        if let Some(var) = n.variables.bindings.remove(&prop_binding_key(old)) {
            n.variables.bindings.insert(prop_binding_key(new), var);
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
    o.component.prop_default_variables.remove(name);
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
        n.component.variant_picks.remove(name);
        n.component.slot_content_ids.remove(name);
        n.variables.bindings.remove(&prop_binding_key(name));
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

fn unique_slot_prop_name(doc: &Document, comp: NodeId, base: &str) -> String {
    let base = if base.trim().is_empty() { "Slot" } else { base.trim() };
    let taken: Vec<String> = prop_defs(doc, comp).into_iter().map(|p| p.name).collect();
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
/// to (the component set unless the variant defines that property itself or the slot property is freshly
/// invented for an unbound instance region) and how it migrates.
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

    let mut invented_prop = false;
    let mode = match &n.kind {
        NodeKind::Instance { .. } => {
            let main_prop = n.component.bindings.get(&OverrideField::MainComponent).cloned();
            let vis_prop = n.component.bindings.get(&OverrideField::Visible).cloned();
            let prop_name = if let Some(p) = main_prop {
                p
            } else if let Some(p) = vis_prop.clone() {
                p
            } else {
                invented_prop = true;
                unique_slot_prop_name(doc, comp, &n.name)
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
    let owner = if defined_on_variant || invented_prop { comp } else { name_owner };
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
    o.component.prop_default_variables.remove(prop_name);
    if !extracted_preferred.is_empty() {
        o.component.slot_configs.entry(prop_name.to_string()).or_default().preferred = extracted_preferred;
    }
    for vp in drop_vis_props {
        if vp != prop_name {
            let still_bound = subtree_ids(c.s, owner).into_iter().any(|id| c.s.get(id).is_some_and(|n| n.component.bindings.values().any(|b| b == vp)));
            if !still_bound {
                o.component.props.retain(|p| p.name != *vp);
                o.component.prop_default_variables.remove(vp);
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

    fn is_nothing(c: &Command) -> bool {
        matches!(c, Command::Batch(v) if v.is_empty())
    }

    /// A main on `parent`.
    fn main_on(f: &mut Fx, parent: NodeId) -> NodeId {
        let id = f.d.alloc_id();
        let n = f.d.insert(parent, Node::frame(id, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        let c = create_component(&mut f.d, n);
        apply(&mut f.d, c);
        n
    }

    /// `f.comp` is main B, with a frame `host` inside it. Main A holds an instance of B, and main C one of A.
    /// Returns (A, C, host).
    fn mains_holding_each_other(f: &mut Fx) -> (NodeId, NodeId, NodeId) {
        let hid = f.d.alloc_id();
        let host = f.d.insert(f.comp, Node::frame(hid, Rect::new(5.0, 5.0, 30.0, 30.0))).unwrap();
        let page = f.page;
        let (a, c) = (main_on(f, page), main_on(f, page));
        let ins = create_instance(&mut f.d, f.comp, a, Vec2::ZERO);
        apply(&mut f.d, ins);
        let ins = create_instance(&mut f.d, a, c, Vec2::ZERO);
        apply(&mut f.d, ins);
        (a, c, host)
    }

    #[test]
    fn an_instance_cannot_go_into_a_main_that_its_component_holds_directly_or_through_other_mains() {
        let mut f = fx();
        let (a, c, host) = mains_holding_each_other(&mut f);
        let b = f.comp;
        // A holds B: an instance of A in B or in a layer of B would make B hold itself.
        assert!(is_nothing(&create_instance(&mut f.d, a, b, Vec2::ZERO)), "an instance of A in B");
        assert!(is_nothing(&create_instance(&mut f.d, a, host, Vec2::ZERO)), "an instance of A in a layer of B");
        // C holds A, which holds B.
        assert!(is_nothing(&create_instance(&mut f.d, c, a, Vec2::ZERO)), "an instance of C in A");
        assert!(is_nothing(&create_instance(&mut f.d, c, b, Vec2::ZERO)), "an instance of C in B, through A");
        // Everywhere else it goes: on the page, and an instance of B inside the main that holds it already.
        assert!(matches!(create_instance(&mut f.d, a, f.page, Vec2::ZERO), Command::Insert { .. }));
        assert!(matches!(create_instance(&mut f.d, b, c, Vec2::ZERO), Command::Insert { .. }));
    }

    #[test]
    fn an_instance_is_not_swapped_to_a_component_that_holds_the_main_it_sits_in() {
        let mut f = fx();
        let (a, _c, _host) = mains_holding_each_other(&mut f);
        let page = f.page;
        let z = main_on(&mut f, page);
        // an instance of Z inside B: swapping it to A would put an instance of A in B, and A holds B
        let ins = create_instance(&mut f.d, z, f.comp, Vec2::ZERO);
        apply(&mut f.d, ins);
        let inside_b = *f.d.get(f.comp).unwrap().children.last().unwrap();
        assert_eq!(instance_component(f.d.get(inside_b).unwrap()), Some(z));
        assert!(is_nothing(&swap_instance(&mut f.d, inside_b, a)), "no swap inside B");
        // the same swap on the page is fine
        let ins = create_instance(&mut f.d, z, f.page, Vec2::new(300.0, 0.0));
        apply(&mut f.d, ins);
        let on_page = *f.d.get(f.page).unwrap().children.last().unwrap();
        assert!(!is_nothing(&swap_instance(&mut f.d, on_page, a)), "swap on the page");
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
        let mut t = Node::text(id, Rect::new(10.0, 10.0, 100.0, 30.0), "Label", TextStyle::default());
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
    fn variant_switch_refits_hug_frame_through_group() {
        let (mut f, _set, small, big) = variant_fixture();
        let outer_id = f.d.alloc_id();
        let mut outer_node = Node::frame(outer_id, Rect::new(0.0, 0.0, 400.0, 100.0));
        outer_node.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                ..AutoLayout::default()
            }),
            ..FrameProps::default()
        });
        let outer = f.d.insert(f.page, outer_node).unwrap();
        let group = f.d.alloc_id();
        let mut group_node = Node::new(group, NodeKind::Group);
        group_node.size = Vec2::new(100.0, 40.0);
        let group = f.d.insert(outer, group_node).unwrap();
        let c = create_instance(&mut f.d, small, group, Vec2::ZERO);
        apply(&mut f.d, c);
        let inst = *f.d.get(group).unwrap().children.last().unwrap();
        f.d.relayout_subtree(outer);
        let old = f.d.get(outer).unwrap().size.x;
        swap_instance_to_component(&mut f.d, inst, big).unwrap();
        assert!(f.d.get(inst).unwrap().size.x > 100.0);
        assert!(f.d.get(outer).unwrap().size.x > old, "hug frame stayed at {old}, group was not refit");
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

    fn set_aspect_lock(d: &mut Document, id: NodeId, locked: bool) {
        let mut n = d.get(id).unwrap().clone();
        n.lock_aspect = locked;
        apply(d, Command::Update { node: Box::new(n) });
    }

    #[test]
    fn record_override_flags_an_aspect_lock_change_and_nothing_else() {
        let f = fx();
        let a = f.d.get(f.bg).unwrap().clone();
        let mut b = a.clone();
        b.lock_aspect = true;
        assert_eq!(record_override(&a, &b), BTreeSet::from([OverrideField::LockAspect]));
        assert!(record_override(&b, &b).is_empty());
    }

    #[test]
    fn sync_gives_every_mirrored_node_the_main_aspect_lock_unless_it_overrides_it() {
        let mut f = fx();
        let i = inst(&mut f, (200.0, 0.0));
        let bg = child_named(&f.d, i, "bg");

        // The root of the instance and a node inside it follow the main.
        set_aspect_lock(&mut f.d, f.comp, true);
        set_aspect_lock(&mut f.d, f.bg, true);
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert!(f.d.get(i).unwrap().lock_aspect, "the instance root follows its main");
        assert!(f.d.get(bg).unwrap().lock_aspect, "a node inside the instance follows its main node");

        // Once a node overrides the lock, the main no longer decides it.
        let mut own = f.d.get(bg).unwrap().clone();
        own.lock_aspect = false;
        own.component.overrides.insert(OverrideField::LockAspect);
        apply(&mut f.d, Command::Update { node: Box::new(own) });
        set_aspect_lock(&mut f.d, f.bg, false);
        set_aspect_lock(&mut f.d, f.bg, true);
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        assert!(!f.d.get(bg).unwrap().lock_aspect, "a node that overrides the lock keeps its own value");
        assert!(f.d.get(i).unwrap().lock_aspect, "the instance root keeps following the main");

        // Resetting the override hands the decision back to the main.
        let c = reset_overrides(&mut f.d, bg, &[OverrideField::LockAspect]);
        apply(&mut f.d, c);
        assert!(f.d.get(bg).unwrap().lock_aspect, "reset restores the main's lock");
        assert!(!f.d.get(bg).unwrap().component.overrides.contains(&OverrideField::LockAspect));
    }

    #[test]
    fn an_aspect_lock_set_on_the_main_before_an_instance_is_made_reaches_the_new_instance() {
        let mut f = fx();
        set_aspect_lock(&mut f.d, f.comp, true);
        let i = inst(&mut f, (200.0, 0.0));
        assert!(f.d.get(i).unwrap().lock_aspect);
        assert!(f.d.get(i).unwrap().component.overrides.is_empty(), "a copied lock is not an override");
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
    fn an_instance_takes_the_text_style_overrides_of_its_main_through_sync_style_edits_and_reset() {
        let mut f = fx();
        f.d.styles.insert("Heading".into(), Style::Text(TextStyle { font_size: 16.0, ..Default::default() }));
        f.d.style_meta.insert("Heading".into(), StyleMeta { name: "Heading".into(), property: StyleProperty::Text, ..Default::default() });
        let mut main_label = f.d.get(f.label).unwrap().clone();
        if let NodeKind::Text(t) = &mut main_label.kind {
            t.style.font_size = 16.0;
        }
        main_label.style_refs.insert("text".into(), "Heading".into());
        apply(&mut f.d, Command::Update { node: Box::new(main_label) });
        let i = inst(&mut f, (200.0, 0.0));
        let ilabel = child_named(&f.d, i, "label");

        // The main overrides its weight while linked, then the style is edited.
        let mut main_label = f.d.get(f.label).unwrap().clone();
        let before = main_label.kind.any_text().unwrap().clone();
        main_label.kind.any_text_mut().unwrap().style.set_font_weight(700);
        assert_eq!(apply_text_link_rule(&mut main_label, &before), LinkOutcome::Kept);
        apply(&mut f.d, Command::Update { node: Box::new(main_label) });
        let c = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, c);
        let cmd = update_style(&f.d, "Heading", Style::Text(TextStyle { font_size: 18.0, font_weight: 500, ..Default::default() })).unwrap();
        apply(&mut f.d, cmd);

        let look = |d: &Document, id: NodeId| {
            let n = d.get(id).unwrap();
            let s = &n.kind.any_text().unwrap().style;
            (s.font_size, s.font_weight, n.text_style_overrides.labels())
        };
        assert_eq!(look(&f.d, f.label), (18.0, 700, vec!["Weight"]), "the main keeps its weight and takes the new size");
        assert_eq!(look(&f.d, ilabel), look(&f.d, f.label), "the instance matches the main");

        // The instance overrides it on its own, then resets: nothing stale is left.
        let before_i = f.d.get(ilabel).unwrap().clone();
        let mut after_i = before_i.clone();
        after_i.kind.any_text_mut().unwrap().style.set_font_weight(300);
        after_i.component.overrides = record_override(&before_i, &after_i);
        apply(&mut f.d, Command::Update { node: Box::new(after_i) });
        let c = reset_overrides(&mut f.d, ilabel, &[OverrideField::TextStyle]);
        apply(&mut f.d, c);
        assert_eq!(look(&f.d, ilabel), look(&f.d, f.label));
    }

    #[test]
    fn a_reset_instance_text_keeps_no_stale_overrides_of_a_main_without_any() {
        let mut f = fx();
        f.d.styles.insert("Heading".into(), Style::Text(TextStyle { font_size: 16.0, ..Default::default() }));
        f.d.style_meta.insert("Heading".into(), StyleMeta { name: "Heading".into(), property: StyleProperty::Text, ..Default::default() });
        let mut main_label = f.d.get(f.label).unwrap().clone();
        main_label.style_refs.insert("text".into(), "Heading".into());
        apply(&mut f.d, Command::Update { node: Box::new(main_label) });
        let i = inst(&mut f, (200.0, 0.0));
        let ilabel = child_named(&f.d, i, "label");
        let before = f.d.get(ilabel).unwrap().clone();
        let mut after = before.clone();
        after.kind.any_text_mut().unwrap().style.set_font_weight(700);
        let outcome = apply_text_link_rule(&mut after, before.kind.any_text().unwrap());
        assert_eq!((outcome, after.text_style_overrides.labels()), (LinkOutcome::Kept, vec!["Weight"]));
        after.component.overrides = record_override(&before, &after);
        apply(&mut f.d, Command::Update { node: Box::new(after) });
        let c = reset_overrides(&mut f.d, ilabel, &[OverrideField::TextStyle]);
        apply(&mut f.d, c);
        assert!(f.d.get(ilabel).unwrap().text_style_overrides.is_empty(), "no stale set after Reset overrides");
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

    fn set_with_unbound_icon_in_each_variant() -> SetFx {
        let mut f = fx();
        let alt = f.d.alloc_id();
        let mut alt_node = Node::frame(alt, Rect::new(0.0, 200.0, 200.0, 48.0));
        alt_node.name = "Alt icon".into();
        let alt = f.d.insert(f.page, alt_node).unwrap();
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

        let c = create_instance(&mut f.d, small, f.page, Vec2::new(900.0, 0.0));
        apply(&mut f.d, c);
        let inst_small = *f.d.get(f.page).unwrap().children.last().unwrap();

        let c = create_instance(&mut f.d, large, f.page, Vec2::new(900.0, 200.0));
        apply(&mut f.d, c);
        let inst_large = *f.d.get(f.page).unwrap().children.last().unwrap();

        SetFx { f, alt, set, small_icon, large_icon, inst_small, inst_large }
    }

    #[test]
    fn unbound_region_in_one_variant_creates_no_set_level_slot() {
        let SetFx { mut f, set, small_icon, large_icon, inst_large, .. } = set_with_unbound_icon_in_each_variant();
        let small = f.d.get(small_icon).unwrap().parent.unwrap();
        let large_before = f.d.get(large_icon).unwrap().clone();
        let inst_large_before = f.d.get(inst_large).unwrap().clone();

        assert!(can_migrate_region_to_slot(&f.d, small_icon));
        let cmd = migrate_region_to_slot(&mut f.d, small_icon).expect("unbound instance in a variant should migrate");
        apply(&mut f.d, cmd);

        assert!(
            f.d.get(set).unwrap().component.props.iter().all(|p| p.kind != PropKind::Slot),
            "no set-level slot property may be created when migrating an unbound region in one variant"
        );
        let slot_prop_name =
            f.d.get(small)
                .unwrap()
                .component
                .props
                .iter()
                .find(|p| p.kind == PropKind::Slot)
                .expect("small variant must own the migrated slot property")
                .name
                .clone();
        assert_eq!(f.d.get(small).unwrap().component.props.iter().filter(|p| p.kind == PropKind::Slot).count(), 1);
        assert!(f.d.get(small_icon).unwrap().component.is_slot);
        assert!(same_ignoring_tree(f.d.get(large_icon).unwrap(), &large_before));
        assert_eq!(f.d.get(inst_large).unwrap().component.values, inst_large_before.component.values);
        assert_eq!(f.d.get(inst_large).unwrap().children, inst_large_before.children);

        // `set_slot_config` on the component set changes nothing; on the variant it updates what `slot_config_of` returns.
        let cfg = SlotPropConfig {
            min_children: Some(1),
            max_children: Some(3),
            stretch_child_on_insert: Some(true),
            display_by_default: Some(false),
            ..SlotPropConfig::default()
        };
        let set_before = f.d.get(set).unwrap().clone();
        let noop_cmd = set_slot_config(&mut f.d, set, &slot_prop_name, cfg.clone());
        assert!(matches!(noop_cmd, Command::Batch(ref v) if v.is_empty()));
        apply(&mut f.d, noop_cmd);
        assert_eq!(f.d.get(set).unwrap(), &set_before);
        assert_eq!(slot_config_of(&f.d, small_icon), None);

        let var_cmd = set_slot_config(&mut f.d, small, &slot_prop_name, cfg.clone());
        apply(&mut f.d, var_cmd);
        assert_eq!(slot_config_of(&f.d, small_icon), Some(cfg));
    }

    #[test]
    fn unbound_region_in_a_standalone_component_unchanged() {
        let mut f = fx();
        let id = f.d.alloc_id();
        let card = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 500.0, 300.0, 100.0))).unwrap();
        let c = create_component(&mut f.d, card);
        apply(&mut f.d, c);
        let c = create_instance(&mut f.d, f.comp, card, Vec2::new(12.0, 16.0));
        apply(&mut f.d, c);
        let region = *f.d.get(card).unwrap().children.last().unwrap();

        let cmd = migrate_region_to_slot(&mut f.d, region).expect("unbound instance in standalone component should migrate");
        apply(&mut f.d, cmd);

        assert_eq!(f.d.get(card).unwrap().component.props.iter().filter(|p| p.kind == PropKind::Slot).count(), 1);
        assert!(f.d.get(region).unwrap().component.is_slot);
    }

    #[test]
    fn undo_restores_everything() {
        let SetFx { mut f, small_icon, .. } = set_with_unbound_icon_in_each_variant();
        let before_nodes = f.d.nodes.clone();
        let cmd = migrate_region_to_slot(&mut f.d, small_icon).expect("unbound instance in a variant should migrate");
        let inv = apply(&mut f.d, cmd);
        apply(&mut f.d, inv);
        assert!(f.d.nodes == before_nodes, "one undo restores every variant, prop, and instance");
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

    #[derive(Debug)]
    struct DeferredPageTestHydrator {
        unloaded_page: NodeId,
        unloaded_inst: NodeId,
        main_comp: NodeId,
        hydrated: bool,
    }

    impl crate::PageHydrator for DeferredPageTestHydrator {
        fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
            if page != self.unloaded_page || self.hydrated {
                return false;
            }
            self.hydrated = true;
            if doc.get(self.unloaded_inst).is_some_and(|n| n.children.is_empty()) {
                let kids = doc.get(self.main_comp).map(|n| n.children.clone()).unwrap_or_default();
                for (idx, k) in kids.into_iter().enumerate() {
                    let nodes = copy_tree(doc, k, true);
                    if !nodes.is_empty() {
                        let _ = Command::Insert { parent: self.unloaded_inst, index: idx, nodes }.apply(doc);
                    }
                }
            }
            true
        }

        fn hydrate_all(&mut self, doc: &mut Document) -> bool {
            self.hydrate_page(doc, self.unloaded_page)
        }

        fn is_hydrated(&self, page: NodeId) -> bool {
            page != self.unloaded_page || self.hydrated
        }
    }

    struct InstanceNameSyncFx {
        f: Fx,
        unloaded_page: NodeId,
        inst_same_page: NodeId,
        inst_loaded_page: NodeId,
        inst_unloaded_page: NodeId,
    }

    fn instance_name_sync_fixture() -> InstanceNameSyncFx {
        let mut f = fx();
        set_text(&mut f.d, f.label, "Satellite");

        // Build a second variant and combine into a ComponentSet so `f.comp` has a variant name
        // ("Breakpoint=Desktop, Type=Simple") while instances carry their own layer name ("Contact page header").
        let id = f.d.alloc_id();
        let mut mobile = Node::frame(id, Rect::new(0.0, 200.0, 100.0, 40.0));
        mobile.name = "Breakpoint=Mobile, Type=Simple".into();
        let mobile = f.d.insert(f.page, mobile).unwrap();
        let id = f.d.alloc_id();
        let mut mobile_label = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Satellite", TextStyle::default());
        mobile_label.name = "label".into();
        f.d.insert(mobile, mobile_label).unwrap();
        let c = create_component(&mut f.d, mobile);
        apply(&mut f.d, c);

        let mut desktop = f.d.get(f.comp).unwrap().clone();
        desktop.name = "Breakpoint=Desktop, Type=Simple".into();
        apply(&mut f.d, Command::Update { node: Box::new(desktop) });

        let c = combine_as_variants(&mut f.d, &[f.comp, mobile]);
        apply(&mut f.d, c);
        let set = f.d.get(f.comp).unwrap().parent.unwrap();
        let mut set_node = f.d.get(set).unwrap().clone();
        set_node.name = "Header section".into();
        apply(&mut f.d, Command::Update { node: Box::new(set_node) });

        // 1. Instance on the same page as the main component.
        let c = create_instance(&mut f.d, f.comp, f.page, Vec2::new(300.0, 0.0));
        apply(&mut f.d, c);
        let inst_same_page = *f.d.get(f.page).unwrap().children.last().unwrap();
        let mut n = f.d.get(inst_same_page).unwrap().clone();
        n.name = "Contact page header".into();
        assert!(!n.component.overrides.contains(&OverrideField::Name));
        apply(&mut f.d, Command::Update { node: Box::new(n) });

        // 2. Instance on another loaded page.
        let loaded_page = f.d.add_page("About page");
        let c = create_instance(&mut f.d, f.comp, loaded_page, Vec2::new(0.0, 0.0));
        apply(&mut f.d, c);
        let inst_loaded_page = *f.d.get(loaded_page).unwrap().children.last().unwrap();
        let mut n = f.d.get(inst_loaded_page).unwrap().clone();
        n.name = "Contact page header".into();
        assert!(!n.component.overrides.contains(&OverrideField::Name));
        apply(&mut f.d, Command::Update { node: Box::new(n) });

        // 3. Instance on a page not loaded yet (deferred expansion via PageHydrator: instance shell is on the page,
        // its children are not expanded yet, and `is_page_hydrated(unloaded_page)` is false).
        let unloaded_page = f.d.add_page("Header section");
        let inst_unloaded_id = f.d.alloc_id();
        let comp_fp = f.d.get(f.comp).unwrap().kind.frame_props().cloned().unwrap_or_default();
        let mut unloaded_inst = Node::new(inst_unloaded_id, NodeKind::Instance { component: Some(f.comp), component_key: None, frame: comp_fp });
        unloaded_inst.name = "Contact page header".into();
        unloaded_inst.size = f.d.get(f.comp).unwrap().size;
        unloaded_inst.component.source = Some(f.comp);
        let inst_unloaded_page = f.d.insert(unloaded_page, unloaded_inst).unwrap();
        f.d.page_hydrator = Some(std::sync::Arc::new(std::sync::Mutex::new(DeferredPageTestHydrator {
            unloaded_page,
            unloaded_inst: inst_unloaded_page,
            main_comp: f.comp,
            hydrated: false,
        })));

        let is_hydrated = |d: &Document, page: NodeId| d.page_hydrator.as_ref().unwrap().lock().unwrap().is_hydrated(page);
        assert!(is_hydrated(&f.d, f.page));
        assert!(is_hydrated(&f.d, loaded_page));
        assert!(!is_hydrated(&f.d, unloaded_page));

        InstanceNameSyncFx { f, unloaded_page, inst_same_page, inst_loaded_page, inst_unloaded_page }
    }

    #[test]
    fn sync_main_text_edit_keeps_instance_layer_names_across_same_loaded_and_unloaded_pages() {
        let InstanceNameSyncFx { mut f, unloaded_page, inst_same_page, inst_loaded_page, inst_unloaded_page, .. } = instance_name_sync_fixture();

        set_text(&mut f.d, f.label, "Satellitex");
        set_fill(&mut f.d, f.bg, Color::rgb(12, 34, 56));
        let cmd = sync_instances(&mut f.d, f.comp);
        let inv = apply(&mut f.d, cmd);

        let names = [
            ("same page", f.d.get(inst_same_page).unwrap().name.as_str()),
            ("other loaded page", f.d.get(inst_loaded_page).unwrap().name.as_str()),
            ("unloaded page", f.d.get(inst_unloaded_page).unwrap().name.as_str()),
        ];
        assert_eq!(
            names,
            [("same page", "Contact page header"), ("other loaded page", "Contact page header"), ("unloaded page", "Contact page header"),],
            "instances on same page, other loaded page, and unloaded page must keep their own layer names instead of taking the main variant name"
        );
        for (label, inst_id) in [("same page", inst_same_page), ("other loaded page", inst_loaded_page), ("unloaded page", inst_unloaded_page)] {
            assert_eq!(text_of(&f.d, child_named(&f.d, inst_id, "label")), "Satellitex", "main text edit synced to instance on {label}");
            assert_eq!(f.d.get(child_named(&f.d, inst_id, "bg")).unwrap().fills, f.d.get(f.bg).unwrap().fills, "main fill edit synced to instance on {label}");
        }

        // Hydrating the previously unloaded page keeps the instance's own layer name and synced text.
        assert!(f.d.ensure_page_hydrated(unloaded_page));
        assert!(f.d.page_hydrator.as_ref().unwrap().lock().unwrap().is_hydrated(unloaded_page));
        assert_eq!(f.d.get(inst_unloaded_page).unwrap().name, "Contact page header");
        assert_eq!(text_of(&f.d, child_named(&f.d, inst_unloaded_page, "label")), "Satellitex");

        // Undo restores the pre-sync state while preserving the instance layer names.
        apply(&mut f.d, inv);
        assert_eq!(f.d.get(inst_same_page).unwrap().name, "Contact page header");
        assert_eq!(f.d.get(inst_loaded_page).unwrap().name, "Contact page header");
        assert_eq!(f.d.get(inst_unloaded_page).unwrap().name, "Contact page header");
        assert_eq!(text_of(&f.d, child_named(&f.d, inst_same_page, "label")), "Satellite");
    }

    #[test]
    fn sync_main_text_edit_preserves_instance_layer_names_across_designos_round_trip() {
        let InstanceNameSyncFx { mut f, inst_same_page, inst_loaded_page, inst_unloaded_page, .. } = instance_name_sync_fixture();

        set_text(&mut f.d, f.label, "Satellitex");
        let cmd = sync_instances(&mut f.d, f.comp);
        apply(&mut f.d, cmd);

        // A write loads every page first, in place, so the file holds what the window shows.
        assert_eq!(f.d.load_pending_pages(), Ok(0));
        let saved = f.d.to_json().expect("serialize .designos");
        let loaded = Document::from_json(&saved).expect("deserialize .designos");

        let names = [
            ("same page", loaded.get(inst_same_page).unwrap().name.as_str()),
            ("other loaded page", loaded.get(inst_loaded_page).unwrap().name.as_str()),
            ("unloaded page", loaded.get(inst_unloaded_page).unwrap().name.as_str()),
        ];
        assert_eq!(
            names,
            [("same page", "Contact page header"), ("other loaded page", "Contact page header"), ("unloaded page", "Contact page header"),],
            "saved and reloaded .designos must keep instance layer names across all pages"
        );
        for inst_id in [inst_same_page, inst_loaded_page, inst_unloaded_page] {
            assert_eq!(text_of(&loaded, child_named(&loaded, inst_id, "label")), "Satellitex");
        }
    }

    #[test]
    fn swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo() {
        let mut d = Document::new("SwapOverrides");
        let page = d.add_page("Page 1");

        // 1. Icon/Plus with child "Glyph": default white fill bound to "s.color.icon", opacity 1.0, 16x16 vector.
        let icon_plus = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 16.0, 16.0));
            f.name = "Icon/Plus".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(
                gid,
                NodeKind::Vector {
                    paths: vec![crate::Path {
                        cmds: vec![crate::PathCmd::MoveTo(Vec2::new(2.0, 8.0)), crate::PathCmd::LineTo(Vec2::new(14.0, 8.0))],
                        ..Default::default()
                    }],
                    network: None,
                },
            );
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.variables.bindings.insert("fills/0".into(), "s.color.icon".into());
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // 2. Icon/Check with child "Glyph": default white fill bound to "s.color.icon", opacity 1.0, 16x16 vector.
        let icon_check = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(40.0, 0.0, 16.0, 16.0));
            f.name = "Icon/Check".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(
                gid,
                NodeKind::Vector {
                    paths: vec![crate::Path {
                        cmds: vec![crate::PathCmd::MoveTo(Vec2::new(4.0, 8.0)), crate::PathCmd::LineTo(Vec2::new(12.0, 12.0))],
                        ..Default::default()
                    }],
                    network: None,
                },
            );
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.variables.bindings.insert("fills/0".into(), "s.color.icon".into());
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // 3. Icon/Mismatch with child "OtherPath" (not "Glyph"): default white fill bound to "s.color.icon".
        let icon_mismatch = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(80.0, 0.0, 16.0, 16.0));
            f.name = "Icon/Mismatch".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 16.0, 16.0));
            g.name = "OtherPath".into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.variables.bindings.insert("fills/0".into(), "s.color.icon".into());
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // 4. Button component with nested "Leading icon" slot (instance of Icon/Plus):
        //    - slot named "Leading icon", visible = false, resized to 20x20, bound to Boolean "Has Leading Icon" (default false)
        //      and InstanceSwap "Icon" (default Icon/Plus).
        //    - slot's "Glyph" child is resized to 20x20 and overrides fill to #0A0A0A with paint opacity 0.5, node opacity 0.5,
        //      and variable binding "fills/0" -> "s.color.text-on-brand".
        let button = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 100.0, 120.0, 40.0));
            f.name = "Button/Primary".into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);

            let c = create_instance(&mut d, icon_plus, comp, Vec2::new(8.0, 10.0));
            apply(&mut d, c);
            let slot = d.get(comp).unwrap().children[0];
            {
                let mut sn = d.get(slot).unwrap().clone();
                sn.name = "Leading icon".into();
                sn.visible = false;
                sn.size = Vec2::new(20.0, 20.0);
                sn.component.overrides.insert(OverrideField::Size);
                apply(&mut d, Command::Update { node: Box::new(sn) });
            }
            let slot_glyph = child_named(&d, slot, "Glyph");
            let mut paint = Paint::solid(Color::rgb(10, 10, 10));
            paint.opacity = 0.5;
            {
                let mut gn = d.get(slot_glyph).unwrap().clone();
                let tf = gn.transform;
                crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
                gn.fills = vec![paint.clone()];
                gn.opacity = 0.5;
                gn.variables.bindings.insert("fills/0".into(), "s.color.text-on-brand".into());
                gn.component.overrides.insert(OverrideField::Size);
                gn.component.overrides.insert(OverrideField::Fills);
                gn.component.overrides.insert(OverrideField::Opacity);
                apply(&mut d, Command::Update { node: Box::new(gn) });
            }

            let c = add_prop(&mut d, comp, ComponentPropDef { name: "Has Leading Icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) });
            apply(&mut d, c);
            let c = add_prop(
                &mut d,
                comp,
                ComponentPropDef {
                    name: "Icon".into(),
                    kind: PropKind::InstanceSwap { preferred: vec![icon_plus, icon_check, icon_mismatch] },
                    default: PropValue::Instance(icon_plus),
                },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::Visible, Some("Has Leading Icon"));
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            comp
        };

        let expected_fill = Color::rgb(10, 10, 10);
        let expected_scaled_check_cmds = vec![crate::PathCmd::MoveTo(Vec2::new(5.0, 10.0)), crate::PathCmd::LineTo(Vec2::new(15.0, 15.0))];

        // Create an instance of Button/Primary on the page.
        let c = create_instance(&mut d, button, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let btn_inst = *d.get(page).unwrap().children.last().unwrap();
        let slot_inst = d.get(btn_inst).unwrap().children[0];
        let before_swap_nodes = d.nodes.clone();

        // Swap the slot to Icon/Check via the InstanceSwap property ("Icon").
        let swap_cmd = set_instance_prop(&mut d, btn_inst, "Icon", Some(PropValue::Instance(icon_check)));
        let inv = apply(&mut d, swap_cmd);

        let slot_after = d.get(slot_inst).unwrap();
        assert_eq!(instance_component(slot_after), Some(icon_check));
        // Name kept ("Leading icon", not "Icon/Check").
        assert_eq!(slot_after.name, "Leading icon");
        // Boolean-bound visibility kept (stays hidden, and keeps Visible + MainComponent bindings).
        assert!(!slot_after.visible, "slot bound to Has Leading Icon = false must stay hidden after swap");
        assert_eq!(slot_after.component.bindings.get(&OverrideField::Visible).map(String::as_str), Some("Has Leading Icon"));
        assert_eq!(slot_after.component.bindings.get(&OverrideField::MainComponent).map(String::as_str), Some("Icon"));
        assert_eq!(slot_after.size, Vec2::new(20.0, 20.0), "slot size from variant kept");

        // Glyph size, scaled vector path, fill override, variable binding, and opacity kept.
        let glyph_after = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
        assert_eq!(glyph_after.size, Vec2::new(20.0, 20.0), "glyph size from resized slot kept");
        match &glyph_after.kind {
            NodeKind::Vector { paths, .. } => assert_eq!(paths[0].cmds, expected_scaled_check_cmds, "glyph vector path scaled to 20x20"),
            other => panic!("expected Vector, got {other:?}"),
        }
        assert_eq!(glyph_after.fills.len(), 1);
        assert_eq!(glyph_after.fills[0].kind, crate::PaintKind::Solid(expected_fill), "glyph fill override kept");
        assert_eq!(glyph_after.fills[0].opacity, 0.5, "glyph paint opacity kept");
        assert_eq!(glyph_after.opacity, 0.5, "glyph node opacity kept");
        assert_eq!(glyph_after.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.text-on-brand"), "glyph fill variable binding kept");

        // Undo restores the old main (Icon/Plus) and its overrides in one step.
        apply(&mut d, inv);
        assert_eq!(d.nodes, before_swap_nodes, "undo restores the old main and its overrides in one step");

        // Mismatched child name drops that override: swap to Icon/Mismatch (child named "OtherPath").
        let mismatch_cmd = set_instance_prop(&mut d, btn_inst, "Icon", Some(PropValue::Instance(icon_mismatch)));
        let mismatch_inv = apply(&mut d, mismatch_cmd);
        let other_path = d.get(child_named(&d, slot_inst, "OtherPath")).unwrap();
        assert_eq!(other_path.fills[0].kind, crate::PaintKind::Solid(Color::rgb(255, 255, 255)), "mismatched child name drops fill override");
        assert_eq!(other_path.fills[0].opacity, 1.0);
        assert_eq!(other_path.opacity, 1.0);
        assert_eq!(other_path.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.icon"));
        apply(&mut d, mismatch_inv);
        assert_eq!(d.nodes, before_swap_nodes);

        // Direct swap_instance on the slot also keeps size, scaled vector path, overrides, binding, opacity, visibility, name, and undoes in one step.
        let direct_cmd = swap_instance(&mut d, slot_inst, icon_check);
        let direct_inv = apply(&mut d, direct_cmd);
        let slot_direct = d.get(slot_inst).unwrap();
        assert_eq!(instance_component(slot_direct), Some(icon_check));
        assert_eq!(slot_direct.name, "Leading icon");
        assert_eq!(slot_direct.size, Vec2::new(20.0, 20.0));
        assert!(!slot_direct.visible);
        let glyph_direct = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
        assert_eq!(glyph_direct.size, Vec2::new(20.0, 20.0));
        match &glyph_direct.kind {
            NodeKind::Vector { paths, .. } => assert_eq!(paths[0].cmds, expected_scaled_check_cmds),
            other => panic!("expected Vector, got {other:?}"),
        }
        assert_eq!(glyph_direct.fills[0].kind, crate::PaintKind::Solid(expected_fill));
        assert_eq!(glyph_direct.fills[0].opacity, 0.5);
        assert_eq!(glyph_direct.opacity, 0.5);
        assert_eq!(glyph_direct.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.text-on-brand"));
        apply(&mut d, direct_inv);
        assert_eq!(d.nodes, before_swap_nodes);
    }

    #[test]
    fn sync_instances_does_not_double_scale_fill_geometry() {
        let mut d = Document::new("SyncScale");
        let page = d.add_page("Page");

        let make_icon = |d: &mut Document, name: &str, x: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let path =
                crate::Path { cmds: vec![crate::PathCmd::MoveTo(Vec2::new(4.0, 4.0)), crate::PathCmd::LineTo(Vec2::new(12.0, 12.0))], ..Default::default() };
            let mut g = Node::new(gid, NodeKind::Vector { paths: vec![path.clone()], network: None });
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.fill_geometry = Some(vec![path]);
            d.insert(comp, g).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };

        let icon_a = make_icon(&mut d, "Icon/A", 0.0);
        let icon_b = make_icon(&mut d, "Icon/B", 40.0);

        let btn = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 100.0, 120.0, 40.0));
            f.name = "Button".into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            let c = create_instance(&mut d, icon_a, comp, Vec2::new(8.0, 10.0));
            apply(&mut d, c);
            let slot = d.get(comp).unwrap().children[0];
            let mut sn = d.get(slot).unwrap().clone();
            sn.size = Vec2::new(20.0, 20.0);
            sn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(sn) });
            let glyph = child_named(&d, slot, "Glyph");
            let mut gn = d.get(glyph).unwrap().clone();
            let tf = gn.transform;
            crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
            gn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(gn) });

            let c = add_prop(
                &mut d,
                comp,
                ComponentPropDef {
                    name: "Icon".into(),
                    kind: PropKind::InstanceSwap { preferred: vec![icon_a, icon_b] },
                    default: PropValue::Instance(icon_a),
                },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            comp
        };

        let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let btn_inst = *d.get(page).unwrap().children.last().unwrap();
        let slot_inst = d.get(btn_inst).unwrap().children[0];

        let c = set_instance_prop(&mut d, btn_inst, "Icon", Some(PropValue::Instance(icon_b)));
        apply(&mut d, c);

        let glyph_after_swap = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
        let fg_after_swap = glyph_after_swap.fill_geometry.as_ref().unwrap()[0].cmds.clone();
        assert_eq!(fg_after_swap, vec![crate::PathCmd::MoveTo(Vec2::new(5.0, 5.0)), crate::PathCmd::LineTo(Vec2::new(15.0, 15.0))]);

        for sync_round in 1..=3 {
            let sync = sync_instances(&mut d, btn);
            apply(&mut d, sync);
            let glyph_after_sync = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
            let fg_after_sync = glyph_after_sync.fill_geometry.as_ref().unwrap()[0].cmds.clone();
            assert_eq!(
                fg_after_sync,
                vec![crate::PathCmd::MoveTo(Vec2::new(5.0, 5.0)), crate::PathCmd::LineTo(Vec2::new(15.0, 15.0))],
                "fill_geometry must NOT be scaled again on sync_instances round {sync_round}"
            );
        }
    }

    #[test]
    fn swap_instance_same_family_preserves_custom_name() {
        let mut d = Document::new("CustomName");
        let page = d.add_page("Page");
        let id1 = d.alloc_id();
        let mut v1 = Node::frame(id1, Rect::new(0.0, 0.0, 100.0, 40.0));
        v1.name = "Size=SM".into();
        let v1 = d.insert(page, v1).unwrap();
        let c = create_component(&mut d, v1);
        apply(&mut d, c);

        let id2 = d.alloc_id();
        let mut v2 = Node::frame(id2, Rect::new(120.0, 0.0, 140.0, 48.0));
        v2.name = "Size=LG".into();
        let v2 = d.insert(page, v2).unwrap();
        let c = create_component(&mut d, v2);
        apply(&mut d, c);

        let c = combine_as_variants(&mut d, &[v1, v2]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v1, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();

        let mut n = d.get(inst).unwrap().clone();
        n.name = "Primary CTA".into();
        apply(&mut d, Command::Update { node: Box::new(n) });

        let c = swap_instance(&mut d, inst, v2);
        apply(&mut d, c);
        assert_eq!(d.get(inst).unwrap().name, "Primary CTA");
    }

    #[test]
    fn swap_instance_preserves_overridden_size() {
        let mut d = Document::new("OverriddenSize");
        let page = d.add_page("Page");
        let id1 = d.alloc_id();
        let mut c1 = Node::frame(id1, Rect::new(0.0, 0.0, 100.0, 40.0));
        c1.name = "CompA".into();
        let c1 = d.insert(page, c1).unwrap();
        let c = create_component(&mut d, c1);
        apply(&mut d, c);

        let id2 = d.alloc_id();
        let mut c2 = Node::frame(id2, Rect::new(120.0, 0.0, 160.0, 60.0));
        c2.name = "CompB".into();
        let c2 = d.insert(page, c2).unwrap();
        let c = create_component(&mut d, c2);
        apply(&mut d, c);

        let c = create_instance(&mut d, c1, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();

        let mut n = d.get(inst).unwrap().clone();
        n.size = Vec2::new(240.0, 80.0);
        n.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(n) });

        let c = swap_instance(&mut d, inst, c2);
        apply(&mut d, c);
        assert_eq!(d.get(inst).unwrap().size, Vec2::new(240.0, 80.0), "top-level swap_instance must preserve explicit Size override");

        // Per-axis rule: when only width was resized (240x40 vs CompA's 100x40), width keeps 240 and height takes CompB's 60.
        let c = create_instance(&mut d, c1, page, Vec2::new(300.0, 200.0));
        apply(&mut d, c);
        let inst_w = *d.get(page).unwrap().children.last().unwrap();
        let mut nw = d.get(inst_w).unwrap().clone();
        nw.size = Vec2::new(240.0, 40.0);
        nw.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(nw) });
        let c = swap_instance(&mut d, inst_w, c2);
        apply(&mut d, c);
        assert_eq!(d.get(inst_w).unwrap().size, Vec2::new(240.0, 60.0), "per-axis size rule keeps custom width and takes new main height");
    }

    #[test]
    fn variant_switch_refits_new_variant_children_to_kept_instance_size() {
        let mut d = Document::new("VariantOffsets");
        let page = d.add_page("Page");

        let id1 = d.alloc_id();
        let mut v1 = Node::frame(id1, Rect::new(0.0, 0.0, 100.0, 40.0));
        v1.name = "Size=SM".into();
        let v1 = d.insert(page, v1).unwrap();
        let ch1 = d.alloc_id();
        let mut r1 = Node::rectangle(ch1, Rect::new(8.0, 8.0, 84.0, 24.0));
        r1.name = "Dot".into();
        r1.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        d.insert(v1, r1).unwrap();
        let st1 = d.alloc_id();
        let mut b1 = Node::rectangle(st1, Rect::new(8.0, 8.0, 84.0, 24.0));
        b1.name = "Bar".into();
        b1.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        d.insert(v1, b1).unwrap();
        let c = create_component(&mut d, v1);
        apply(&mut d, c);

        let id2 = d.alloc_id();
        let mut v2 = Node::frame(id2, Rect::new(120.0, 0.0, 200.0, 60.0));
        v2.name = "Size=LG".into();
        let v2 = d.insert(page, v2).unwrap();
        let ch2 = d.alloc_id();
        let mut r2 = Node::rectangle(ch2, Rect::new(20.0, 16.0, 160.0, 28.0));
        r2.name = "Dot".into();
        r2.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        d.insert(v2, r2).unwrap();
        let st2 = d.alloc_id();
        let mut b2 = Node::rectangle(st2, Rect::new(20.0, 16.0, 160.0, 28.0));
        b2.name = "Bar".into();
        b2.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        d.insert(v2, b2).unwrap();
        let c = create_component(&mut d, v2);
        apply(&mut d, c);

        let c = combine_as_variants(&mut d, &[v1, v2]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v1, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let mut n = d.get(inst).unwrap().clone();
        n.size = Vec2::new(120.0, 40.0);
        n.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(n) });

        let c = set_instance_prop(&mut d, inst, "Size", Some(PropValue::Variant("LG".into())));
        apply(&mut d, c);

        let dot_after = d.get(child_named(&d, inst, "Dot")).unwrap();
        assert_eq!(dot_after.transform.tx, 12.0, "resized instance must refit the switched variant Scale child to the kept width");
        assert_eq!(dot_after.size.x, 96.0, "resized instance must refit the switched variant Scale child to the kept width");
        assert!(!dot_after.component.overrides.contains(&OverrideField::Size), "variant switch must not manufacture child Size override");

        let bar_after = d.get(child_named(&d, inst, "Bar")).unwrap();
        assert_eq!((bar_after.transform.tx, bar_after.size.x), (20.0, 80.0), "Stretch child keeps LG's 20px left/right margins inside the kept 120px width");
        assert_eq!((bar_after.transform.ty, bar_after.size.y), (16.0, 8.0), "Stretch child keeps LG's 16px top/bottom margins inside the kept 40px height");
    }

    #[test]
    fn manual_swap_refits_resized_instance_and_preserves_auto_layout_result() {
        let mut d = Document::new("ManualSwapRefit");
        let page = d.add_page("Page");

        let id1 = d.alloc_id();
        let mut v1 = Node::frame(id1, Rect::new(0.0, 0.0, 100.0, 40.0));
        v1.name = "Size=SM".into();
        let v1 = d.insert(page, v1).unwrap();
        let ch1 = d.alloc_id();
        let mut r1 = Node::rectangle(ch1, Rect::new(8.0, 8.0, 84.0, 24.0));
        r1.name = "Dot".into();
        r1.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        d.insert(v1, r1).unwrap();
        let c = create_component(&mut d, v1);
        apply(&mut d, c);

        let id2 = d.alloc_id();
        let mut v2 = Node::frame(id2, Rect::new(120.0, 0.0, 200.0, 60.0));
        v2.name = "Size=LG".into();
        let v2 = d.insert(page, v2).unwrap();
        let ch2 = d.alloc_id();
        let mut r2 = Node::rectangle(ch2, Rect::new(20.0, 16.0, 160.0, 28.0));
        r2.name = "Dot".into();
        r2.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        d.insert(v2, r2).unwrap();
        let c = create_component(&mut d, v2);
        apply(&mut d, c);

        let c = combine_as_variants(&mut d, &[v1, v2]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v1, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let mut n = d.get(inst).unwrap().clone();
        n.size = Vec2::new(120.0, 40.0);
        n.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(n) });

        let c = swap_instance(&mut d, inst, v2);
        apply(&mut d, c);

        let dot = d.get(child_named(&d, inst, "Dot")).unwrap();
        assert_eq!(
            (dot.transform.tx, dot.size.x),
            (12.0, 96.0),
            "manual swap_instance on a size-overridden variant must refit Scale children to the kept width"
        );

        // Auto-layout instance case: a resized auto-layout instance keeps its layout result after variant switch.
        let mk_al = |d: &mut Document, name: &str, x: f32, w: f32, pad: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 300.0, w, 48.0));
            f.name = name.into();
            let mut al = crate::AutoLayout::v1();
            al.direction = crate::LayoutDirection::Horizontal;
            al.primary_sizing = crate::Sizing::Fixed;
            al.counter_sizing = crate::Sizing::Fixed;
            al.padding = [pad, pad, pad, pad];
            if let Some(fp) = f.kind.frame_props_mut() {
                fp.auto_layout = Some(al);
            }
            let comp = d.insert(page, f).unwrap();
            let cid = d.alloc_id();
            let mut pill = Node::rectangle(cid, Rect::new(pad, pad, w - 2.0 * pad, 48.0 - 2.0 * pad));
            pill.name = "FillBar".into();
            pill.layout_child.grow = 1.0;
            d.insert(comp, pill).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let al_sm = mk_al(&mut d, "Density=Compact", 0.0, 120.0, 8.0);
        let al_lg = mk_al(&mut d, "Density=Spacious", 200.0, 200.0, 16.0);
        let c = combine_as_variants(&mut d, &[al_sm, al_lg]);
        apply(&mut d, c);

        let c = create_instance(&mut d, al_sm, page, Vec2::new(0.0, 420.0));
        apply(&mut d, c);
        let al_inst = *d.get(page).unwrap().children.last().unwrap();
        let mut al_n = d.get(al_inst).unwrap().clone();
        al_n.size = Vec2::new(240.0, 48.0);
        al_n.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(al_n) });

        let c = set_instance_prop(&mut d, al_inst, "Density", Some(PropValue::Variant("Spacious".into())));
        apply(&mut d, c);
        let fill_bar = d.get(child_named(&d, al_inst, "FillBar")).unwrap();
        assert_eq!(d.get(al_inst).unwrap().size, Vec2::new(240.0, 48.0));
        assert_eq!(
            (fill_bar.transform.tx, fill_bar.size.x),
            (16.0, 208.0),
            "auto-layout instance keeps its layout result inside the kept 240px width with Spacious 16px padding"
        );
    }

    #[test]
    fn swap_main_matches_renamed_child_by_source_name() {
        let mut d = Document::new("RenamedChild");
        let page = d.add_page("Page");

        let make_icon = |d: &mut Document, name: &str, x: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 16.0, 16.0));
            g.name = "Glyph".into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            d.insert(comp, g).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };

        let icon_a = make_icon(&mut d, "Icon/A", 0.0);
        let icon_b = make_icon(&mut d, "Icon/B", 40.0);

        let c = create_instance(&mut d, icon_a, page, Vec2::new(0.0, 100.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let glyph = child_named(&d, inst, "Glyph");

        let mut gn = d.get(glyph).unwrap().clone();
        gn.name = "Custom Glyph Name".into();
        gn.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        gn.component.overrides.insert(OverrideField::Name);
        gn.component.overrides.insert(OverrideField::Fills);
        apply(&mut d, Command::Update { node: Box::new(gn) });

        let c = swap_instance(&mut d, inst, icon_b);
        apply(&mut d, c);

        let glyph_after = d.get(d.get(inst).unwrap().children[0]).unwrap();
        assert_eq!(
            glyph_after.fills[0].kind,
            crate::PaintKind::Solid(Color::rgb(255, 0, 0)),
            "renamed child must still match new main's 'Glyph' via its source name in old main"
        );
    }

    #[test]
    fn variant_switch_without_size_override_takes_new_variant_size() {
        let mut d = Document::new("VariantSize");
        let page = d.add_page("Page");

        let id1 = d.alloc_id();
        let mut v1 = Node::frame(id1, Rect::new(0.0, 0.0, 100.0, 40.0));
        v1.name = "Size=SM".into();
        let v1 = d.insert(page, v1).unwrap();
        let c = create_component(&mut d, v1);
        apply(&mut d, c);

        let id2 = d.alloc_id();
        let mut v2 = Node::frame(id2, Rect::new(120.0, 0.0, 200.0, 60.0));
        v2.name = "Size=LG".into();
        let v2 = d.insert(page, v2).unwrap();
        let c = create_component(&mut d, v2);
        apply(&mut d, c);

        let c = combine_as_variants(&mut d, &[v1, v2]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v1, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();

        let mut n = d.get(inst).unwrap().clone();
        n.size = Vec2::new(120.0, 40.0);
        apply(&mut d, Command::Update { node: Box::new(n) });

        let c = swap_instance(&mut d, inst, v2);
        apply(&mut d, c);

        let inst_after = d.get(inst).unwrap();
        assert_eq!(inst_after.size, Vec2::new(200.0, 60.0), "variant switch without OverrideField::Size must take new variant's size");
        assert!(!inst_after.component.overrides.contains(&OverrideField::Size), "variant switch must not manufacture OverrideField::Size");
    }

    #[test]
    fn slot_swap_scales_inset_glyph_by_scale_not_slot_size() {
        let mut d = Document::new("InsetGlyph");
        let page = d.add_page("Page");

        // Icon/A has a 16x16 full-bleed vector.
        let icon_a = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 16.0, 16.0));
            f.name = "Icon/A".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(
                gid,
                NodeKind::Vector {
                    paths: vec![crate::Path {
                        cmds: vec![crate::PathCmd::MoveTo(Vec2::new(0.0, 0.0)), crate::PathCmd::LineTo(Vec2::new(16.0, 16.0))],
                        ..Default::default()
                    }],
                    network: None,
                },
            );
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // Icon/B has a 12x12 inset vector at (2, 2) with (Scale, Scale) constraints.
        let icon_b = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(40.0, 0.0, 16.0, 16.0));
            f.name = "Icon/B".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(
                gid,
                NodeKind::Vector {
                    paths: vec![crate::Path {
                        cmds: vec![crate::PathCmd::MoveTo(Vec2::new(0.0, 0.0)), crate::PathCmd::LineTo(Vec2::new(12.0, 12.0))],
                        ..Default::default()
                    }],
                    network: None,
                },
            );
            g.transform.tx = 2.0;
            g.transform.ty = 2.0;
            g.size = Vec2::new(12.0, 12.0);
            g.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
            g.name = "Glyph".into();
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // Button has a 20x20 slot where Icon/A's glyph was scaled to 20x20.
        let btn = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 100.0, 120.0, 40.0));
            f.name = "Button".into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            let c = create_instance(&mut d, icon_a, comp, Vec2::new(8.0, 10.0));
            apply(&mut d, c);
            let slot = d.get(comp).unwrap().children[0];
            let mut sn = d.get(slot).unwrap().clone();
            sn.size = Vec2::new(20.0, 20.0);
            sn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(sn) });
            let glyph = child_named(&d, slot, "Glyph");
            let mut gn = d.get(glyph).unwrap().clone();
            let tf = gn.transform;
            crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
            apply(&mut d, Command::Update { node: Box::new(gn) });

            let c = add_prop(
                &mut d,
                comp,
                ComponentPropDef {
                    name: "Icon".into(),
                    kind: PropKind::InstanceSwap { preferred: vec![icon_a, icon_b] },
                    default: PropValue::Instance(icon_a),
                },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            comp
        };

        let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let btn_inst = *d.get(page).unwrap().children.last().unwrap();
        let slot_inst = d.get(btn_inst).unwrap().children[0];

        let c = set_instance_prop(&mut d, btn_inst, "Icon", Some(PropValue::Instance(icon_b)));
        apply(&mut d, c);

        let glyph_after = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
        assert_eq!(glyph_after.size, Vec2::new(15.0, 15.0), "inset 12x12 Scale glyph in 16x16 icon scaled into 20x20 slot must be 15x15, not 20x20");
        assert_eq!(glyph_after.transform.tx, 2.5, "inset glyph x offset (2.0) scaled by 20/16 must be 2.5");
        assert!(glyph_after.component.overrides.is_empty(), "constraint-scaled child must not manufacture Size/Transform overrides");

        // Syncing Icon/B must keep the 15x15 size and (2.5, 2.5) position via the slot constraint pass.
        let sync_b = sync_instances(&mut d, icon_b);
        apply(&mut d, sync_b);
        let glyph_after_sync = d.get(child_named(&d, slot_inst, "Glyph")).unwrap();
        assert_eq!(glyph_after_sync.size, Vec2::new(15.0, 15.0), "sync_instances on Icon/B must keep 15x15 glyph size");
        assert_eq!(glyph_after_sync.transform.tx, 2.5, "sync_instances on Icon/B must keep 2.5 glyph tx");
    }

    #[test]
    fn swap_transfers_matching_overrides_across_node_types_and_preserves_slot_bindings() {
        let mut d = Document::new("CrossTypeOverrides");
        let page = d.add_page("Page");

        // 1. Field-exists and originally-matched override transfer across node types:
        // CompRect has a Rectangle "Shape" (white fill) + Text "Item" + Frame "Box".
        let comp_rect = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            f.name = "Comp/Rect".into();
            let comp = d.insert(page, f).unwrap();
            let sid = d.alloc_id();
            let mut s = Node::rectangle(sid, Rect::new(0.0, 0.0, 16.0, 16.0));
            s.name = "Shape".into();
            s.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            d.insert(comp, s).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(0.0, 20.0, 40.0, 16.0), "Default", crate::TextStyle::default());
            t.name = "Item".into();
            d.insert(comp, t).unwrap();
            let bid = d.alloc_id();
            let mut b = Node::frame(bid, Rect::new(20.0, 0.0, 16.0, 16.0));
            b.name = "Box".into();
            d.insert(comp, b).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // CompEllipse has an Ellipse "Shape" with the SAME default white fill (so originally-matched passes),
        // a Frame "Item" (not Text, so text overrides are dropped), and a Frame "Box" (same default FrameProps).
        let comp_ellipse_same_fill = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(60.0, 0.0, 40.0, 40.0));
            f.name = "Comp/EllipseSame".into();
            let comp = d.insert(page, f).unwrap();
            let sid = d.alloc_id();
            let mut s = Node::ellipse(sid, Rect::new(0.0, 0.0, 16.0, 16.0));
            s.name = "Shape".into();
            s.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            d.insert(comp, s).unwrap();
            let iid = d.alloc_id();
            let mut item_frame = Node::frame(iid, Rect::new(0.0, 20.0, 20.0, 12.0));
            item_frame.name = "Item".into();
            d.insert(comp, item_frame).unwrap();
            let bid = d.alloc_id();
            let mut b = Node::frame(bid, Rect::new(20.0, 0.0, 16.0, 16.0));
            b.name = "Box".into();
            d.insert(comp, b).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // CompEllipseDiffFill has an Ellipse "Shape" with a DIFFERENT default fill (green #00ff00) + Text "Item" with different default text.
        let comp_ellipse_diff_fill = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(120.0, 0.0, 40.0, 40.0));
            f.name = "Comp/EllipseDiff".into();
            let comp = d.insert(page, f).unwrap();
            let sid = d.alloc_id();
            let mut s = Node::ellipse(sid, Rect::new(0.0, 0.0, 16.0, 16.0));
            s.name = "Shape".into();
            s.fills = vec![Paint::solid(Color::rgb(0, 255, 0))];
            d.insert(comp, s).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(0.0, 20.0, 40.0, 16.0), "OtherDefault", crate::TextStyle::default());
            t.name = "Item".into();
            d.insert(comp, t).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        // Create an auto-named instance of Comp/Rect ("Comp/Rect").
        let c = create_instance(&mut d, comp_rect, page, Vec2::new(0.0, 100.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        assert_eq!(d.get(inst).unwrap().name, "Comp/Rect");
        {
            let mut in_node = d.get(inst).unwrap().clone();
            if let NodeKind::Instance { ref mut frame, .. } = in_node.kind {
                frame.clips_content = true;
            }
            in_node.component.overrides.insert(OverrideField::Layout);
            apply(&mut d, Command::Update { node: Box::new(in_node) });
        }

        // Override "Shape" fill to red, "Item" text + text style, and "Box" frame layout (clip_content = true).
        let shape_id = child_named(&d, inst, "Shape");
        {
            let mut sn = d.get(shape_id).unwrap().clone();
            sn.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
            sn.component.overrides.insert(OverrideField::Fills);
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }
        let item_id = child_named(&d, inst, "Item");
        {
            let mut tn = d.get(item_id).unwrap().clone();
            if let NodeKind::Text(ref mut tp) = tn.kind {
                tp.content = "OverriddenText".into();
                tp.style.font_size = 22.0;
            }
            tn.component.overrides.insert(OverrideField::Characters);
            tn.component.overrides.insert(OverrideField::TextStyle);
            apply(&mut d, Command::Update { node: Box::new(tn) });
        }
        let box_id = child_named(&d, inst, "Box");
        {
            let mut bn = d.get(box_id).unwrap().clone();
            if let NodeKind::Frame(ref mut fp) = bn.kind {
                fp.clips_content = true;
            }
            bn.component.overrides.insert(OverrideField::Layout);
            apply(&mut d, Command::Update { node: Box::new(bn) });
        }

        // Swap to Comp/EllipseSame:
        // - auto-named instance ("Comp/Rect") takes new main's name ("Comp/EllipseSame").
        // - "Shape" fill override transfers from Rectangle onto Ellipse because old & new mains share default white fill.
        // - "Item" in Comp/EllipseSame is a Frame, so Characters/TextStyle do NOT transfer onto it.
        // - "Box" in Comp/EllipseSame is a Frame with same default FrameProps, so Layout (clips_content = true) transfers.
        let swap_same = swap_instance(&mut d, inst, comp_ellipse_same_fill);
        let inv_same = apply(&mut d, swap_same);
        assert_eq!(d.get(inst).unwrap().name, "Comp/EllipseSame", "auto-named instance takes new main's name on swap");
        assert!(
            !d.get(inst).unwrap().kind.frame_props().unwrap().clips_content,
            "swap_main copies the new main's FrameProps onto the instance root (base behaviour)"
        );
        let shape_after = d.get(child_named(&d, inst, "Shape")).unwrap();
        assert!(matches!(shape_after.kind, NodeKind::Ellipse { .. }));
        assert_eq!(
            shape_after.fills[0].kind,
            crate::PaintKind::Solid(Color::rgb(255, 0, 0)),
            "fill override transfers from Rectangle to Ellipse when originally matched"
        );
        let item_after = d.get(child_named(&d, inst, "Item")).unwrap();
        assert!(!item_after.component.overrides.contains(&OverrideField::Characters), "Characters override must not mark a non-Text target");
        assert!(!item_after.component.overrides.contains(&OverrideField::TextStyle), "TextStyle override must not mark a non-Text target");
        let box_after = d.get(child_named(&d, inst, "Box")).unwrap();
        assert!(box_after.component.overrides.contains(&OverrideField::Layout));
        assert!(box_after.kind.frame_props().unwrap().clips_content, "Layout override must copy FrameProps onto target Frame");

        apply(&mut d, inv_same);

        // Now swap to Comp/EllipseDiff:
        // - "Originally matched" gate: "Shape" default fill in Comp/Rect (white) != Comp/EllipseDiff (green), so red fill override is dropped!
        // - Text exception: "Item" Characters ("OverriddenText") and TextStyle (font_size 22.0) survive even though default text differed!
        let swap_diff = swap_instance(&mut d, inst, comp_ellipse_diff_fill);
        let inv_diff = apply(&mut d, swap_diff);
        let shape_diff = d.get(child_named(&d, inst, "Shape")).unwrap();
        assert_eq!(
            shape_diff.fills[0].kind,
            crate::PaintKind::Solid(Color::rgb(0, 255, 0)),
            "originally-matched gate drops fill override when old and new mains differ"
        );
        assert!(!shape_diff.component.overrides.contains(&OverrideField::Fills));
        let item_diff = d.get(child_named(&d, inst, "Item")).unwrap();
        match &item_diff.kind {
            NodeKind::Text(tp) => {
                assert_eq!(tp.content, "OverriddenText", "text characters override survives even when mains differ");
                assert_eq!(tp.style.font_size, 22.0, "text style override survives even when mains differ");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        apply(&mut d, inv_diff);

        // 2. Variant switch (`top_swap && same_family`) where a Frame child with `OverrideField::Layout`
        // matches a Rectangle child in the target variant -> `OverrideField::Layout` must not mark the Rectangle.
        let v_frame = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(200.0, 0.0, 60.0, 40.0));
            f.name = "Kind=Frame".into();
            let comp = d.insert(page, f).unwrap();
            let bid = d.alloc_id();
            let mut b = Node::frame(bid, Rect::new(0.0, 0.0, 20.0, 20.0));
            b.name = "Box".into();
            d.insert(comp, b).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let v_rect = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(280.0, 0.0, 60.0, 40.0));
            f.name = "Kind=Rect".into();
            let comp = d.insert(page, f).unwrap();
            let bid = d.alloc_id();
            let mut b = Node::rectangle(bid, Rect::new(0.0, 0.0, 20.0, 20.0));
            b.name = "Box".into();
            d.insert(comp, b).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let c = combine_as_variants(&mut d, &[v_frame, v_rect]);
        apply(&mut d, c);
        let c = create_instance(&mut d, v_frame, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let v_inst = *d.get(page).unwrap().children.last().unwrap();
        let vbox = child_named(&d, v_inst, "Box");
        {
            let mut bn = d.get(vbox).unwrap().clone();
            if let NodeKind::Frame(ref mut fp) = bn.kind {
                fp.clips_content = true;
            }
            bn.component.overrides.insert(OverrideField::Layout);
            apply(&mut d, Command::Update { node: Box::new(bn) });
        }
        let c = set_instance_prop(&mut d, v_inst, "Kind", Some(PropValue::Variant("Rect".into())));
        apply(&mut d, c);
        let vbox_after = d.get(child_named(&d, v_inst, "Box")).unwrap();
        assert!(!vbox_after.component.overrides.contains(&OverrideField::Layout), "Layout override must not be inserted onto a Rectangle target");

        // 3. Outer component slot child has:
        // - a "width" variable binding on "Glyph" (with no other override on "Glyph")
        // - an explicit `OverrideField::Layout` (`clips_content = true`) on a child frame "Wrap" recorded in the outer component's slot
        let icon_first = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(400.0, 0.0, 16.0, 16.0));
            f.name = "Icon/First".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 16.0, 16.0));
            g.name = "Glyph".into();
            d.insert(comp, g).unwrap();
            let wid = d.alloc_id();
            let mut w = Node::frame(wid, Rect::new(0.0, 0.0, 16.0, 16.0));
            w.name = "Wrap".into();
            d.insert(comp, w).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let icon_second = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(440.0, 0.0, 16.0, 16.0));
            f.name = "Icon/Second".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 16.0, 16.0));
            g.name = "Glyph".into();
            d.insert(comp, g).unwrap();
            let wid = d.alloc_id();
            let mut w = Node::frame(wid, Rect::new(0.0, 0.0, 16.0, 16.0));
            w.name = "Wrap".into();
            d.insert(comp, w).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let holder = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(400.0, 60.0, 80.0, 40.0));
            f.name = "Holder".into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            let c = create_instance(&mut d, icon_first, comp, Vec2::new(4.0, 4.0));
            apply(&mut d, c);
            let slot = d.get(comp).unwrap().children[0];
            let slot_glyph = child_named(&d, slot, "Glyph");
            {
                let mut gn = d.get(slot_glyph).unwrap().clone();
                gn.variables.bindings.insert("width".into(), "size.icon.w".into());
                apply(&mut d, Command::Update { node: Box::new(gn) });
            }
            let slot_wrap = child_named(&d, slot, "Wrap");
            {
                let mut wn = d.get(slot_wrap).unwrap().clone();
                if let NodeKind::Frame(ref mut fp) = wn.kind {
                    fp.clips_content = true;
                }
                wn.component.overrides.insert(OverrideField::Layout);
                apply(&mut d, Command::Update { node: Box::new(wn) });
            }
            let c = add_prop(
                &mut d,
                comp,
                ComponentPropDef {
                    name: "Icon".into(),
                    kind: PropKind::InstanceSwap { preferred: vec![icon_first, icon_second] },
                    default: PropValue::Instance(icon_first),
                },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            comp
        };
        let c = create_instance(&mut d, holder, page, Vec2::new(400.0, 140.0));
        apply(&mut d, c);
        let holder_inst = *d.get(page).unwrap().children.last().unwrap();
        let holder_slot = d.get(holder_inst).unwrap().children[0];
        let c = set_instance_prop(&mut d, holder_inst, "Icon", Some(PropValue::Instance(icon_second)));
        apply(&mut d, c);
        let glyph_after_swap = d.get(child_named(&d, holder_slot, "Glyph")).unwrap();
        assert_eq!(
            glyph_after_swap.variables.bindings.get("width").map(String::as_str),
            Some("size.icon.w"),
            "width variable binding on slot child must survive swap"
        );
        let wrap_after_swap = d.get(child_named(&d, holder_slot, "Wrap")).unwrap();
        assert!(wrap_after_swap.kind.frame_props().unwrap().clips_content, "explicit Layout override on outer slot child must survive swap");
    }

    fn path_bbox(paths: &[crate::Path]) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in paths {
            for c in &p.cmds {
                let pts: Vec<Vec2> = match c {
                    crate::PathCmd::MoveTo(p) | crate::PathCmd::LineTo(p) => vec![*p],
                    crate::PathCmd::QuadTo(a, b) => vec![*a, *b],
                    crate::PathCmd::CubicTo(a, b, e) => vec![*a, *b, *e],
                    crate::PathCmd::Close => vec![],
                };
                for q in pts {
                    x0 = x0.min(q.x);
                    y0 = y0.min(q.y);
                    x1 = x1.max(q.x);
                    y1 = y1.max(q.y);
                }
            }
        }
        (x0, y0, x1, y1)
    }

    fn box_outline(w: f32) -> Vec<crate::Path> {
        vec![crate::Path {
            cmds: vec![
                crate::PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
                crate::PathCmd::LineTo(Vec2::new(w, 0.0)),
                crate::PathCmd::LineTo(Vec2::new(w, w)),
                crate::PathCmd::LineTo(Vec2::new(0.0, w)),
                crate::PathCmd::Close,
            ],
            ..Default::default()
        }]
    }

    #[test]
    fn resized_vector_in_instance_keeps_scaled_paths_through_main_sync() {
        let mut d = Document::new("ResizedVector");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        f.name = "Mark".into();
        let comp = d.insert(page, f).unwrap();
        let gid = d.alloc_id();
        let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(100.0), network: None });
        g.name = "Glyph".into();
        g.size = Vec2::new(100.0, 100.0);
        g.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        g.fill_geometry = Some(box_outline(100.0));
        d.insert(comp, g).unwrap();
        let c = create_component(&mut d, comp);
        apply(&mut d, c);
        let c = create_instance(&mut d, comp, page, Vec2::new(200.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let glyph = child_named(&d, inst, "Glyph");

        // Type W = 250 on the vector inside the instance.
        let mut gn = d.get(glyph).unwrap().clone();
        let before = gn.clone();
        let tf = gn.transform;
        crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(250.0, 100.0));
        gn.component.overrides = record_override(&before, &gn);
        apply(&mut d, Command::Update { node: Box::new(gn) });
        let width = |d: &Document| match &d.get(glyph).unwrap().kind {
            NodeKind::Vector { paths, .. } => path_bbox(paths).2,
            other => panic!("expected Vector, got {other:?}"),
        };
        assert_eq!(d.get(glyph).unwrap().size.x, 250.0);
        assert_eq!(width(&d), 250.0, "the resize scales the path");

        // Change only the main's opacity, then sync.
        let mut m = d.get(gid).unwrap().clone();
        m.opacity = 0.5;
        apply(&mut d, Command::Update { node: Box::new(m) });
        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        let g = d.get(glyph).unwrap();
        assert_eq!(g.size.x, 250.0);
        assert_eq!(width(&d), 250.0, "the path keeps the resized geometry after a main sync");
        let fg = g.fill_geometry.as_ref().map(|p| path_bbox(p).2);
        assert_eq!(fg, Some(250.0), "the fill bake keeps the resized geometry too");
    }

    #[test]
    fn resized_boolean_operand_in_instance_keeps_scaled_paths_through_main_sync() {
        let mut d = Document::new("ResizedOperand");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        f.name = "Mark".into();
        let comp = d.insert(page, f).unwrap();
        let bid = d.alloc_id();
        let mut b = Node::new(bid, NodeKind::BooleanOp(BooleanOp::Union));
        b.name = "Bool".into();
        b.size = Vec2::new(100.0, 100.0);
        let bool_main = d.insert(comp, b).unwrap();
        let gid = d.alloc_id();
        let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(100.0), network: None });
        g.name = "Operand".into();
        g.size = Vec2::new(100.0, 100.0);
        g.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        d.insert(bool_main, g).unwrap();
        let c = create_component(&mut d, comp);
        apply(&mut d, c);
        let c = create_instance(&mut d, comp, page, Vec2::new(200.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let operand = child_named(&d, child_named(&d, inst, "Bool"), "Operand");

        let mut on = d.get(operand).unwrap().clone();
        let before = on.clone();
        let tf = on.transform;
        crate::layout_constraints::apply_constrained_geometry(&mut on, tf, Vec2::new(250.0, 100.0));
        on.component.overrides = record_override(&before, &on);
        apply(&mut d, Command::Update { node: Box::new(on) });

        let mut m = d.get(gid).unwrap().clone();
        m.opacity = 0.5;
        apply(&mut d, Command::Update { node: Box::new(m) });
        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        let n = d.get(operand).unwrap();
        assert_eq!(n.size.x, 250.0);
        match &n.kind {
            NodeKind::Vector { paths, .. } => assert_eq!(path_bbox(paths).2, 250.0, "the operand keeps the resized path after a main sync"),
            other => panic!("expected Vector, got {other:?}"),
        }
    }

    #[test]
    fn copy_field_size_on_imported_shapes_after_main_resize() {
        let mut stale = Vec::new();
        for (label, kind) in [
            ("rect", NodeKind::Rectangle),
            ("ellipse", NodeKind::Ellipse(crate::ArcData::default())),
            ("star", NodeKind::Star { count: 5, inner_ratio: 0.5 }),
            ("vector", NodeKind::Vector { paths: box_outline(10.0), network: None }),
        ] {
            let mut d = Document::new("cf");
            let page = d.add_page("P");
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 24.0, 24.0));
            f.name = "Badge".into();
            let comp = d.insert(page, f).unwrap();
            let sid = d.alloc_id();
            let mut s = Node::new(sid, kind);
            s.name = "Dot".into();
            s.size = Vec2::new(10.0, 10.0);
            s.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
            s.fill_geometry = Some(box_outline(10.0));
            s.stroke_geometry = Some(box_outline(10.0));
            d.insert(comp, s).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            let c = create_instance(&mut d, comp, page, Vec2::new(100.0, 0.0));
            apply(&mut d, c);
            let inst = *d.get(page).unwrap().children.last().unwrap();
            {
                let mut m = d.get(sid).unwrap().clone();
                let tf = m.transform;
                crate::layout_constraints::apply_constrained_geometry(&mut m, tf, Vec2::new(24.0, 24.0));
                apply(&mut d, Command::Update { node: Box::new(m) });
            }
            let c = sync_instances(&mut d, comp);
            apply(&mut d, c);
            let m = d.get(sid).unwrap();
            let ic = d.get(d.get(inst).unwrap().children[0]).unwrap();
            let m_fg = m.fill_geometry.as_ref().map(|p| path_bbox(p));
            let m_sg = m.stroke_geometry.as_ref().map(|p| path_bbox(p));
            let i_fg = ic.fill_geometry.as_ref().map(|p| path_bbox(p));
            let i_sg = ic.stroke_geometry.as_ref().map(|p| path_bbox(p));
            if i_fg != m_fg || i_sg != m_sg {
                stale.push((label, i_fg, m_fg));
            }
        }
        assert!(
            stale.is_empty(),
            "after resizing the main from 10 to 24 and syncing, instance fill_geometry / stroke_geometry must match the main (24 px, not 10 px); stale: {stale:?}"
        );
    }

    #[test]
    fn a_corner_radius_override_drops_the_main_baked_outline_on_sync() {
        let mut d = Document::new("corner-sync");
        let page = d.add_page("P");
        let main_id = d.alloc_id();
        let main = d.insert(page, Node::frame(main_id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let child_id = d.alloc_id();
        let mut child = Node::rectangle(child_id, Rect::new(0.0, 0.0, 40.0, 40.0));
        child.name = "Box".into();
        child.corner_radii = [10.0; 4];
        child.fill_geometry = Some(box_outline(10.0));
        child.stroke_geometry = Some(box_outline(10.0));
        d.insert(main, child).unwrap();
        let create = create_component(&mut d, main);
        apply(&mut d, create);

        let instance_cmd = create_instance(&mut d, main, page, Vec2::new(100.0, 0.0));
        apply(&mut d, instance_cmd);
        let instance = *d.get(page).unwrap().children.last().unwrap();
        let instance_child = child_named(&d, instance, "Box");
        let mut own = d.get(instance_child).unwrap().clone();
        own.corner_radii = [24.0; 4];
        own.fill_geometry = Some(box_outline(24.0));
        own.stroke_geometry = Some(box_outline(24.0));
        own.component.overrides.insert(OverrideField::CornerRadii);
        apply(&mut d, Command::Update { node: Box::new(own) });

        let mut main_node = d.get(main).unwrap().clone();
        main_node.opacity = 0.5;
        apply(&mut d, Command::Update { node: Box::new(main_node) });
        let sync = sync_instances(&mut d, main);
        apply(&mut d, sync);

        let synced = d.get(instance_child).unwrap();
        assert_eq!(synced.corner_radii, [24.0; 4]);
        assert!(synced.fill_geometry.is_none() && synced.stroke_geometry.is_none(), "sync must not restore the main's baked corners over an own radius");
    }

    #[test]
    fn own_corner_radius_and_size_keep_own_baked_outline_on_unrelated_sync() {
        let mut d = Document::new("corner-sync-size");
        let page = d.add_page("P");
        let main_id = d.alloc_id();
        let main = d.insert(page, Node::frame(main_id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let child_id = d.alloc_id();
        let mut child = Node::rectangle(child_id, Rect::new(0.0, 0.0, 40.0, 40.0));
        child.name = "Box".into();
        child.corner_radii = [10.0; 4];
        child.fill_geometry = Some(box_outline(40.0));
        child.stroke_geometry = Some(box_outline(40.0));
        d.insert(main, child).unwrap();
        let create = create_component(&mut d, main);
        apply(&mut d, create);

        let instance_cmd = create_instance(&mut d, main, page, Vec2::new(100.0, 0.0));
        apply(&mut d, instance_cmd);
        let instance = *d.get(page).unwrap().children.last().unwrap();
        let instance_child = child_named(&d, instance, "Box");
        let mut own = d.get(instance_child).unwrap().clone();
        own.corner_radii = [24.0; 4];
        own.size = Vec2::new(60.0, 60.0);
        own.fill_geometry = Some(box_outline(60.0));
        own.stroke_geometry = Some(box_outline(60.0));
        own.component.overrides.insert(OverrideField::CornerRadii);
        own.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(own) });

        let mut main_node = d.get(main).unwrap().clone();
        main_node.opacity = 0.5;
        apply(&mut d, Command::Update { node: Box::new(main_node) });
        let sync = sync_instances(&mut d, main);
        apply(&mut d, sync);

        let synced = d.get(instance_child).unwrap();
        assert_eq!(synced.corner_radii, [24.0; 4]);
        assert_eq!(synced.size, Vec2::new(60.0, 60.0));
        assert!(synced.fill_geometry.is_some() && synced.stroke_geometry.is_some(), "own outline for own radius and size must survive an unrelated main sync");
    }

    #[test]
    fn main_corner_radius_change_drops_size_overridden_instance_baked_outline_on_sync() {
        let mut d = Document::new("corner-sync-main-radius");
        let page = d.add_page("P");
        let main_id = d.alloc_id();
        let main = d.insert(page, Node::frame(main_id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let child_id = d.alloc_id();
        let mut child = Node::rectangle(child_id, Rect::new(0.0, 0.0, 40.0, 40.0));
        child.name = "Box".into();
        child.corner_radii = [10.0; 4];
        child.fill_geometry = Some(box_outline(40.0));
        child.stroke_geometry = Some(box_outline(40.0));
        let main_child = d.insert(main, child).unwrap();
        let create = create_component(&mut d, main);
        apply(&mut d, create);

        let instance_cmd = create_instance(&mut d, main, page, Vec2::new(100.0, 0.0));
        apply(&mut d, instance_cmd);
        let instance = *d.get(page).unwrap().children.last().unwrap();
        let instance_child = child_named(&d, instance, "Box");
        let mut own = d.get(instance_child).unwrap().clone();
        own.size = Vec2::new(60.0, 60.0);
        own.fill_geometry = Some(box_outline(60.0));
        own.stroke_geometry = Some(box_outline(60.0));
        own.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(own) });

        let mut main_box = d.get(main_child).unwrap().clone();
        main_box.corner_radii = [24.0; 4];
        apply(&mut d, Command::Update { node: Box::new(main_box) });
        let sync = sync_instances(&mut d, main);
        apply(&mut d, sync);

        let synced = d.get(instance_child).unwrap();
        assert_eq!(synced.corner_radii, [24.0; 4]);
        assert_eq!(synced.size, Vec2::new(60.0, 60.0));
        assert!(
            synced.fill_geometry.is_none() && synced.stroke_geometry.is_none(),
            "a main corner-radius edit must drop the stale baked outline on a size-overridden instance"
        );
    }

    #[test]
    fn swap_drops_override_when_old_and_new_mains_differ() {
        let mut d = Document::new("DiffMains");
        let page = d.add_page("P");
        let a = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, 0.0, 16.0, 16.0));
            f.name = "Icon/A".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(16.0), network: None });
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.fill_geometry = Some(box_outline(16.0));
            d.insert(comp, g).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let b = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(40.0, 0.0, 16.0, 16.0));
            f.name = "Icon/B".into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut gf = Node::frame(gid, Rect::new(0.0, 0.0, 16.0, 16.0));
            gf.name = "Glyph".into();
            gf.fills = vec![Paint::solid(Color::rgb(0, 128, 0))];
            d.insert(comp, gf).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };

        let c = create_instance(&mut d, a, page, Vec2::new(100.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let g = child_named(&d, inst, "Glyph");
        let mut gn = d.get(g).unwrap().clone();
        let before = gn.clone();
        gn.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        let tf = gn.transform;
        crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
        gn.component.overrides = record_override(&before, &gn);
        apply(&mut d, Command::Update { node: Box::new(gn) });

        let c = swap_instance(&mut d, inst, b);
        apply(&mut d, c);
        let ng = d.get(child_named(&d, inst, "Glyph")).unwrap();
        assert_eq!(
            ng.fills[0].kind,
            crate::PaintKind::Solid(Color::rgb(0, 128, 0)),
            "originally-matched gate drops fill override when old and new mains differ"
        );
        assert!(ng.fill_geometry.is_none(), "a frame must not take the vector's outline as its fill_geometry");
    }

    #[test]
    fn nested_slot_widened_by_outer_instance_keeps_width_on_swap() {
        let mut d = Document::new("WidenedSlot");
        let page = d.add_page("P");
        let mkbtn = |d: &mut Document, name: &str, x: f32, h: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 300.0, 80.0, h));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 80.0, h));
            r.name = "bg".into();
            r.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
            d.insert(comp, r).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let ba = mkbtn(&mut d, "Btn/A", 0.0, 32.0);
        let bb = mkbtn(&mut d, "Btn/B", 100.0, 40.0);
        let id = d.alloc_id();
        let mut card = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        card.name = "Card".into();
        let card = d.insert(page, card).unwrap();
        let c = create_component(&mut d, card);
        apply(&mut d, c);
        let c = create_instance(&mut d, ba, card, Vec2::new(10.0, 10.0));
        apply(&mut d, c);
        let slot = d.get(card).unwrap().children[0];
        {
            let mut sn = d.get(slot).unwrap().clone();
            sn.name = "Action".into();
            sn.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }
        let c = add_prop(
            &mut d,
            card,
            ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred: vec![ba, bb] }, default: PropValue::Instance(ba) },
        );
        apply(&mut d, c);
        let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Btn"));
        apply(&mut d, c);
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(ci).unwrap().children[0];
        {
            let mut cn = d.get(ci).unwrap().clone();
            cn.size = Vec2::new(300.0, 100.0);
            cn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(cn) });
            for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
                let mut n = d.get(cid).unwrap().clone();
                crate::layout_constraints::apply_constrained_geometry(&mut n, tf, sz);
                apply(&mut d, Command::Update { node: Box::new(n) });
            }
        }
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let s = d.get(si).unwrap();
        let bg = d.get(s.children[0]).unwrap();
        assert_eq!(s.size, Vec2::new(280.0, 40.0), "nested slot widened by outer instance keeps width 280 and takes new main height 40");
        assert_eq!(bg.size, Vec2::new(280.0, 40.0), "Stretch bg must fill the 280x40 slot after the swap");
        assert!(bg.component.overrides.is_empty(), "constraint-stretched child must not have stuck Size/Transform overrides: {:?}", bg.component.overrides);
    }

    #[test]
    fn nested_slot_size_rule_is_per_axis_across_swap_and_sync() {
        let mut d = Document::new("PerAxisSize");
        let page = d.add_page("P");
        let mkbtn = |d: &mut Document, name: &str, x: f32, h: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 300.0, 80.0, h));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 80.0, h));
            r.name = "bg".into();
            d.insert(comp, r).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let ba = mkbtn(&mut d, "Btn/A", 0.0, 32.0);
        let bb = mkbtn(&mut d, "Btn/B", 100.0, 40.0);
        let id = d.alloc_id();
        let mut card = Node::frame(id, Rect::new(0.0, 0.0, 300.0, 100.0));
        card.name = "Card".into();
        let card = d.insert(page, card).unwrap();
        let c = create_component(&mut d, card);
        apply(&mut d, c);
        let c = create_instance(&mut d, ba, card, Vec2::new(10.0, 10.0));
        apply(&mut d, c);
        let slot = d.get(card).unwrap().children[0];
        {
            let mut sn = d.get(slot).unwrap().clone();
            sn.name = "Action".into();
            sn.size = Vec2::new(280.0, 32.0);
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }
        let c = add_prop(
            &mut d,
            card,
            ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred: vec![ba, bb] }, default: PropValue::Instance(ba) },
        );
        apply(&mut d, c);
        let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Btn"));
        apply(&mut d, c);
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(ci).unwrap().children[0];
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let s = d.get(si).unwrap();
        assert_eq!(s.size, Vec2::new(280.0, 40.0), "width was stretched (keep 280); height was never stretched (take new main 40)");
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);
        let s_after = d.get(si).unwrap();
        assert_eq!(s_after.size, Vec2::new(280.0, 40.0), "later sync keeps stretched width 280 and new main height 40 per axis");
    }

    #[test]
    fn pinned_and_scale_glyphs_follow_constraint_solver_on_slot_swap() {
        const MINMIN: Constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Min };
        const SCALE: Constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        let mut bad = Vec::new();
        for (label, cons) in [("minmin", MINMIN), ("scale", SCALE)] {
            let mut d = Document::new("Solver");
            let page = d.add_page("P");
            let mk_icon = |d: &mut Document, name: &str, x: f32, at: (f32, f32), sz: f32| -> NodeId {
                let id = d.alloc_id();
                let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
                f.name = name.into();
                let comp = d.insert(page, f).unwrap();
                let gid = d.alloc_id();
                let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(sz), network: None });
                g.size = Vec2::new(sz, sz);
                g.transform = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: at.0, ty: at.1 };
                g.name = "Glyph".into();
                g.constraints = cons;
                g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
                g.fill_geometry = Some(box_outline(sz));
                d.insert(comp, g).unwrap();
                let c = create_component(d, comp);
                apply(d, c);
                comp
            };
            let a = mk_icon(&mut d, "Icon/A", 0.0, (0.0, 0.0), 16.0);
            let b = mk_icon(&mut d, "Icon/B", 40.0, (2.0, 2.0), 12.0);
            let btn = {
                let id = d.alloc_id();
                let mut btn = Node::frame(id, Rect::new(0.0, 100.0, 32.0, 32.0));
                btn.name = "Tool".into();
                let btn = d.insert(page, btn).unwrap();
                let c = create_component(&mut d, btn);
                apply(&mut d, c);
                let c = create_instance(&mut d, a, btn, Vec2::new(6.0, 6.0));
                apply(&mut d, c);
                let slot = d.get(btn).unwrap().children[0];
                let mut sn = d.get(slot).unwrap().clone();
                sn.name = "Icon".into();
                sn.size = Vec2::new(20.0, 20.0);
                sn.component.overrides.insert(OverrideField::Name);
                sn.component.overrides.insert(OverrideField::Size);
                apply(&mut d, Command::Update { node: Box::new(sn) });
                let c = add_prop(
                    &mut d,
                    btn,
                    ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![a, b] }, default: PropValue::Instance(a) },
                );
                apply(&mut d, c);
                let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
                apply(&mut d, c);
                btn
            };
            let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
            apply(&mut d, c);
            let bi = *d.get(page).unwrap().children.last().unwrap();
            let si = d.get(bi).unwrap().children[0];
            let c = swap_instance(&mut d, si, b);
            let inv = apply(&mut d, c);
            let g_direct = d.get(child_named(&d, si, "Glyph")).unwrap();
            let ok_direct = if label == "minmin" {
                (g_direct.transform.tx, g_direct.transform.ty, g_direct.size) == (2.0, 2.0, Vec2::new(12.0, 12.0))
            } else {
                (g_direct.transform.tx - 2.5).abs() < 1e-3 && (g_direct.size.x - 15.0).abs() < 1e-3
            };
            if !ok_direct {
                bad.push((label, g_direct.transform.tx, g_direct.transform.ty, g_direct.size));
            }
            apply(&mut d, inv);
            let c = set_instance_prop(&mut d, bi, "Icon", Some(PropValue::Instance(b)));
            apply(&mut d, c);
            let g = d.get(child_named(&d, si, "Glyph")).unwrap();
            let ok = if label == "minmin" {
                (g.transform.tx, g.transform.ty, g.size) == (2.0, 2.0, Vec2::new(12.0, 12.0))
            } else {
                (g.transform.tx - 2.5).abs() < 1e-3 && (g.size.x - 15.0).abs() < 1e-3
            };
            if !ok {
                bad.push((label, g.transform.tx, g.transform.ty, g.size));
            }
        }
        assert!(bad.is_empty(), "Scale scales 1.25x, pinned (Min/Min) glyph keeps (2,2) 12x12; off: {bad:?}");
    }

    #[test]
    fn pinned_logo_keeps_size_when_outer_width_is_kept_per_axis() {
        let mut d = Document::new("PinnedLogoWidth");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 200.0, 100.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let lid = d.alloc_id();
            let mut l = Node::new(lid, NodeKind::Vector { paths: box_outline(24.0), network: None });
            l.size = Vec2::new(24.0, 24.0);
            l.transform = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 8.0, ty: 8.0 };
            l.name = "Logo".into();
            l.fill_geometry = Some(box_outline(24.0));
            d.insert(comp, l).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "Card/A", 0.0);
        let b = mk(&mut d, "Card/B", 200.0);
        let c = create_instance(&mut d, a, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        {
            let mut n = d.get(inst).unwrap().clone();
            n.size = Vec2::new(300.0, 100.0);
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        let c = swap_instance(&mut d, inst, b);
        apply(&mut d, c);
        let l = d.get(child_named(&d, inst, "Logo")).unwrap();
        assert_eq!(l.size, Vec2::new(24.0, 24.0), "a pinned (Min/Min) logo keeps its 24x24 size; got {:?}", l.size);
        assert_eq!((l.transform.tx, l.transform.ty), (8.0, 8.0));
    }

    #[test]
    fn pinned_logo_keeps_size_on_user_resized_card_after_swap_and_sync() {
        let mut d = Document::new("PinnedLogoCard");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 200.0, 100.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let lid = d.alloc_id();
            let mut l = Node::new(lid, NodeKind::Vector { paths: box_outline(24.0), network: None });
            l.size = Vec2::new(24.0, 24.0);
            l.transform = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 8.0, ty: 8.0 };
            l.name = "Logo".into();
            l.fill_geometry = Some(box_outline(24.0));
            d.insert(comp, l).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "Card/A", 0.0);
        let b = mk(&mut d, "Card/B", 200.0);
        let c = create_instance(&mut d, a, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        {
            let mut n = d.get(inst).unwrap().clone();
            n.size = Vec2::new(300.0, 100.0);
            n.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        let c = swap_instance(&mut d, inst, b);
        apply(&mut d, c);
        let l_swap = d.get(child_named(&d, inst, "Logo")).unwrap();
        assert_eq!(l_swap.size, Vec2::new(24.0, 24.0), "a pinned (Min/Min) logo keeps its 24x24 size after swap; got {:?}", l_swap.size);
        let c = sync_instances(&mut d, b);
        apply(&mut d, c);
        let l = d.get(child_named(&d, inst, "Logo")).unwrap();
        assert_eq!(l.size, Vec2::new(24.0, 24.0), "a pinned (Min/Min) logo keeps its 24x24 size after swap and sync; got {:?}", l.size);
        assert_eq!((l.transform.tx, l.transform.ty), (8.0, 8.0));
    }

    #[test]
    fn set_named_variant_instance_takes_new_main_name_on_cross_family_swap() {
        let mut d = Document::new("nm");
        let page = d.add_page("P");
        let mut variants = Vec::new();
        for (i, v) in ["Size=Sm", "Size=Md"].iter().enumerate() {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(i as f32 * 120.0, 0.0, 100.0, 40.0));
            f.name = (*v).into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            variants.push(comp);
        }
        let c = combine_as_variants(&mut d, &variants);
        apply(&mut d, c);
        let set = d.get(variants[0]).unwrap().parent.unwrap();
        let mut sn = d.get(set).unwrap().clone();
        sn.name = "Button".into();
        apply(&mut d, Command::Update { node: Box::new(sn) });
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 200.0, 60.0, 24.0));
        f.name = "Badge".into();
        let badge = d.insert(page, f).unwrap();
        let c = create_component(&mut d, badge);
        apply(&mut d, c);
        let c = create_instance(&mut d, variants[0], page, Vec2::new(0.0, 400.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        assert_eq!(d.get(inst).unwrap().name, "Button");
        let c = swap_instance(&mut d, inst, badge);
        apply(&mut d, c);
        assert_eq!(d.get(inst).unwrap().name, "Badge", "an auto-named (set-named) instance takes the new main's name");
    }

    #[test]
    fn inner_swap_below_swapped_slot_follows_the_slot_swap() {
        const MINMIN: Constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Min };
        let mut d = Document::new("InnerSwap");
        let page = d.add_page("P");
        let badge = |d: &mut Document, name: &str, x: f32, rgb: (u8, u8, u8)| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 500.0, 6.0, 6.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let did = d.alloc_id();
            let mut dot = Node::ellipse(did, Rect::new(0.0, 0.0, 6.0, 6.0));
            dot.name = "Dot".into();
            dot.fills = vec![Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2))];
            d.insert(comp, dot).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let mk_icon = |d: &mut Document, name: &str, x: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(16.0), network: None });
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.constraints = MINMIN;
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.fill_geometry = Some(box_outline(16.0));
            d.insert(comp, g).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let red = badge(&mut d, "Badge/Red", 0.0, (255, 0, 0));
        let blue = badge(&mut d, "Badge/Blue", 20.0, (0, 0, 255));
        let green = badge(&mut d, "Badge/Green", 30.0, (0, 200, 0));
        let a = mk_icon(&mut d, "Icon/A", 0.0);
        let b = mk_icon(&mut d, "Icon/B", 40.0);
        for (m, placed) in [(a, red), (b, green)] {
            let c = create_instance(&mut d, placed, m, Vec2::new(10.0, 0.0));
            apply(&mut d, c);
            let bi = *d.get(m).unwrap().children.last().unwrap();
            let mut bn = d.get(bi).unwrap().clone();
            bn.name = "Badge".into();
            bn.component.overrides.insert(OverrideField::Name);
            apply(&mut d, Command::Update { node: Box::new(bn) });
        }
        let badge_of = |d: &Document, slot: NodeId| -> NodeId {
            *d.get(slot).unwrap().children.iter().find(|c| matches!(d.get(**c).unwrap().kind, NodeKind::Instance { .. })).unwrap()
        };
        let (btn, slot) = {
            let id = d.alloc_id();
            let mut btn = Node::frame(id, Rect::new(0.0, 100.0, 32.0, 32.0));
            btn.name = "Tool".into();
            let btn = d.insert(page, btn).unwrap();
            let c = create_component(&mut d, btn);
            apply(&mut d, c);
            let c = create_instance(&mut d, a, btn, Vec2::new(6.0, 6.0));
            apply(&mut d, c);
            let slot = d.get(btn).unwrap().children[0];
            let mut sn = d.get(slot).unwrap().clone();
            sn.name = "Icon".into();
            sn.size = Vec2::new(20.0, 20.0);
            sn.component.overrides.insert(OverrideField::Name);
            sn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(sn) });
            let c = add_prop(
                &mut d,
                btn,
                ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![a, b] }, default: PropValue::Instance(a) },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            (btn, slot)
        };
        {
            let bslot = badge_of(&d, slot);
            let c = swap_instance(&mut d, bslot, blue);
            apply(&mut d, c);
            let dot = d.get(bslot).unwrap().children[0];
            let mut dn = d.get(dot).unwrap().clone();
            dn.fills = vec![Paint::solid(Color::rgb(255, 200, 0))];
            dn.component.overrides.insert(OverrideField::Fills);
            apply(&mut d, Command::Update { node: Box::new(dn) });
        }
        let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let bi = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(bi).unwrap().children[0];
        assert_eq!(instance_component(d.get(badge_of(&d, si)).unwrap()), Some(blue));
        let (c, dropped) = set_instance_prop_inner(&mut d, bi, "Icon", Some(PropValue::Instance(b)));
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(badge_of(&d, si)).unwrap()), Some(blue), "the inner swap below the swapped slot follows it");
        assert_eq!(
            paint_of(&d, d.get(badge_of(&d, si)).unwrap().children[0]),
            crate::PaintKind::Solid(Color::rgb(255, 200, 0)),
            "the gold fill on the descendant below the swapped slot follows it"
        );
        assert_eq!(dropped, 0, "nothing was left behind below the swapped slot");
    }

    fn frame_main_with_dot(d: &mut Document, page: NodeId, name: &str, x: f32, y: f32, w: f32, h: f32) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, y, w, h));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 4.0, 4.0));
        r.name = "Dot".into();
        d.insert(comp, r).unwrap();
        let c = create_component(d, comp);
        apply(d, c);
        comp
    }

    fn put_named_instance(d: &mut Document, comp: NodeId, into: NodeId, name: &str) -> NodeId {
        let c = create_instance(d, comp, into, Vec2::new(0.0, 0.0));
        apply(d, c);
        let k = *d.get(into).unwrap().children.last().unwrap();
        let mut n = d.get(k).unwrap().clone();
        n.name = name.into();
        n.component.overrides.insert(OverrideField::Name);
        apply(d, Command::Update { node: Box::new(n) });
        k
    }

    #[test]
    fn swap_reports_zero_dropped_inner_swaps_on_unswapped_nested_chain() {
        let mut d = Document::new("ZeroDropped");
        let page = d.add_page("P");
        let badge = frame_main_with_dot(&mut d, page, "Badge", 0.0, 500.0, 8.0, 8.0);
        let btn = frame_main_with_dot(&mut d, page, "Btn", 0.0, 400.0, 40.0, 20.0);
        put_named_instance(&mut d, badge, btn, "Badge");
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, btn, card, "Btn");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, btn, card2, "Btn");
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let (c, dropped) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);
        assert_eq!(dropped, 0, "nothing was swapped below the top instance");
    }

    #[test]
    fn swap_reports_dropped_inner_swaps_per_call() {
        let mut d = Document::new("TwoTransferred");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        put_named_instance(&mut d, red, card, "B2");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card2, "B1");
        put_named_instance(&mut d, red, card2, "B2");
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        for k in d.get(ci).unwrap().children.clone() {
            if matches!(d.get(k).unwrap().kind, NodeKind::Instance { .. }) {
                let c = swap_instance(&mut d, k, blue);
                apply(&mut d, c);
                let dot = child_named(&d, k, "Dot");
                override_fill(&mut d, dot, Color::rgb(0, 255, 0));
            }
        }
        let (c, after_outer) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);
        for name in ["B1", "B2"] {
            let b = child_named(&d, ci, name);
            assert_eq!(instance_component(d.get(b).unwrap()), Some(blue), "{name} transfers to the counterpart");
            assert_eq!(paint_of(&d, child_named(&d, b, "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)), "{name} keeps its descendant fill override");
        }
        let c = create_instance(&mut d, red, page, Vec2::new(900.0, 0.0));
        apply(&mut d, c);
        let lone = *d.get(page).unwrap().children.last().unwrap();
        let (c, after_unrelated) = swap_instance_inner(&mut d, lone, blue);
        apply(&mut d, c);
        assert_eq!(after_outer, 0, "counterparts in Card2 receive both inner swaps");
        assert_eq!(after_unrelated, 0, "subsequent unrelated swap drops 0");
    }

    #[test]
    fn property_swap_accumulates_dropped_inner_swaps_across_bound_slots() {
        let build = |slots: usize| -> (Document, NodeId, NodeId, NodeId, NodeId) {
            let mut d = Document::new("cnt");
            let page = d.add_page("P");
            let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
            let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
            let ba = frame_main_with_dot(&mut d, page, "Btn/A", 0.0, 400.0, 40.0, 20.0);
            put_named_instance(&mut d, red, ba, "Badge");
            let bb = frame_main_with_dot(&mut d, page, "Btn/B", 60.0, 400.0, 40.0, 20.0);
            put_named_instance(&mut d, red, bb, "Badge");
            let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 40.0, 20.0);
            let mut ss = Vec::new();
            for k in 0..slots {
                ss.push(put_named_instance(&mut d, ba, card, &format!("S{k}")));
            }
            let c = add_prop(
                &mut d,
                card,
                ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred: vec![ba, bb] }, default: PropValue::Instance(ba) },
            );
            apply(&mut d, c);
            for s in &ss {
                let c = bind_prop(&mut d, *s, OverrideField::MainComponent, Some("Btn"));
                apply(&mut d, c);
            }
            let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
            apply(&mut d, c);
            let ci = *d.get(page).unwrap().children.last().unwrap();
            for s in d.get(ci).unwrap().children.clone() {
                if !d.get(s).unwrap().name.starts_with('S') {
                    continue;
                }
                let badge = child_named(&d, s, "Badge");
                let c = swap_instance(&mut d, badge, blue);
                apply(&mut d, c);
                let dot = child_named(&d, badge, "Dot");
                override_fill(&mut d, dot, Color::rgb(0, 255, 0));
            }
            (d, ci, ba, bb, blue)
        };
        let (mut d, ci, ba, bb, blue) = build(1);
        let (c, one) = set_instance_prop_inner(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let s0 = child_named(&d, ci, "S0");
        let b0 = child_named(&d, s0, "Badge");
        assert_eq!(instance_component(d.get(b0).unwrap()), Some(blue));
        assert_eq!(paint_of(&d, child_named(&d, b0, "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)));
        let (c, back) = set_instance_prop_inner(&mut d, ci, "Btn", Some(PropValue::Instance(ba)));
        apply(&mut d, c);
        let s0_back = child_named(&d, ci, "S0");
        let b0_back = child_named(&d, s0_back, "Badge");
        assert_eq!(instance_component(d.get(b0_back).unwrap()), Some(blue));
        assert_eq!(paint_of(&d, child_named(&d, b0_back, "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)));

        let (mut d2, ci2, _, bb2, blue2) = build(2);
        let (c, two_slots) = set_instance_prop_inner(&mut d2, ci2, "Btn", Some(PropValue::Instance(bb2)));
        apply(&mut d2, c);
        for slot_name in ["S0", "S1"] {
            let slot = child_named(&d2, ci2, slot_name);
            let badge = child_named(&d2, slot, "Badge");
            assert_eq!(instance_component(d2.get(badge).unwrap()), Some(blue2), "{slot_name} transfers inner swap");
            assert_eq!(paint_of(&d2, child_named(&d2, badge, "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)), "{slot_name} keeps descendant fill");
        }
        let (c, again) = set_instance_prop_inner(&mut d2, ci2, "Btn", Some(PropValue::Instance(bb2)));
        apply(&mut d2, c);
        assert_eq!((one, back, two_slots, again), (0, 0, 0, 0));
    }

    #[test]
    fn inner_swaps_with_no_layer_at_path_count_drops_per_call_and_across_bound_slots() {
        let mut d = Document::new("TwoDroppedNoMatch");
        let page = d.add_page("P");
        let red = frame_main_with_dot(&mut d, page, "Badge/Red", 0.0, 500.0, 8.0, 8.0);
        let blue = frame_main_with_dot(&mut d, page, "Badge/Blue", 20.0, 500.0, 8.0, 8.0);
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        put_named_instance(&mut d, red, card, "B2");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        for k in d.get(ci).unwrap().children.clone() {
            if matches!(d.get(k).unwrap().kind, NodeKind::Instance { .. }) {
                let c = swap_instance(&mut d, k, blue);
                apply(&mut d, c);
            }
        }
        let (c, after_outer) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);
        let c = create_instance(&mut d, red, page, Vec2::new(900.0, 0.0));
        apply(&mut d, c);
        let lone = *d.get(page).unwrap().children.last().unwrap();
        let (c, after_unrelated) = swap_instance_inner(&mut d, lone, blue);
        apply(&mut d, c);
        assert_eq!(after_outer, 2, "two inner swaps dropped when no layer exists at that path");
        assert_eq!(after_unrelated, 0, "subsequent unrelated swap drops 0");

        let build = |slots: usize| -> (Document, NodeId, NodeId, NodeId) {
            let mut d = Document::new("cnt");
            let page = d.add_page("P");
            let red = frame_main_with_dot(&mut d, page, "Badge/Red", 0.0, 500.0, 40.0, 20.0);
            let blue = frame_main_with_dot(&mut d, page, "Badge/Blue", 60.0, 500.0, 40.0, 20.0);
            let ba = frame_main_with_dot(&mut d, page, "Btn/A", 0.0, 400.0, 40.0, 20.0);
            put_named_instance(&mut d, red, ba, "Badge");
            let bb = frame_main_with_dot(&mut d, page, "Btn/B", 60.0, 400.0, 40.0, 20.0);
            let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 40.0, 20.0);
            let mut ss = Vec::new();
            for k in 0..slots {
                ss.push(put_named_instance(&mut d, ba, card, &format!("S{k}")));
            }
            let c = add_prop(
                &mut d,
                card,
                ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred: vec![ba, bb] }, default: PropValue::Instance(ba) },
            );
            apply(&mut d, c);
            for s in &ss {
                let c = bind_prop(&mut d, *s, OverrideField::MainComponent, Some("Btn"));
                apply(&mut d, c);
            }
            let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
            apply(&mut d, c);
            let ci = *d.get(page).unwrap().children.last().unwrap();
            for s in d.get(ci).unwrap().children.clone() {
                if !d.get(s).unwrap().name.starts_with('S') {
                    continue;
                }
                let badge = child_named(&d, s, "Badge");
                let c = swap_instance(&mut d, badge, blue);
                apply(&mut d, c);
            }
            (d, ci, ba, bb)
        };
        let (mut d, ci, ba, bb) = build(1);
        let (c, one) = set_instance_prop_inner(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let (c, back) = set_instance_prop_inner(&mut d, ci, "Btn", Some(PropValue::Instance(ba)));
        apply(&mut d, c);
        let (mut d2, ci2, _, bb2) = build(2);
        let (c, two_slots) = set_instance_prop_inner(&mut d2, ci2, "Btn", Some(PropValue::Instance(bb2)));
        apply(&mut d2, c);
        let (c, again) = set_instance_prop_inner(&mut d2, ci2, "Btn", Some(PropValue::Instance(bb2)));
        apply(&mut d2, c);
        assert_eq!((one, back, two_slots, again), (1, 0, 2, 0));
    }

    #[test]
    fn variant_switch_does_not_turn_refit_text_size_into_override() {
        let mut d = Document::new("RefitText");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, h: f32, fs: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 100.0, h + 8.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let style = crate::TextStyle { font_size: fs, ..crate::TextStyle::default() };
            let mut t = Node::text(tid, Rect::new(4.0, 4.0, 50.0, h), "Label", style);
            t.name = "label".into();
            if let NodeKind::Text(tp) = &mut t.kind {
                tp.auto_resize = TextAutoResize::WidthAndHeight;
            }
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let sm = mk(&mut d, "Size=Sm", 0.0, 20.0, 14.0);
        let md = mk(&mut d, "Size=Md", 100.0, 24.0, 16.0);
        let c = combine_as_variants(&mut d, &[sm, md]);
        apply(&mut d, c);
        let c = create_instance(&mut d, sm, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let l = child_named(&d, inst, "label");
        let mut ln = d.get(l).unwrap().clone();
        let before = ln.clone();
        if let NodeKind::Text(tp) = &mut ln.kind {
            tp.content = "Continue to checkout".into();
        }
        ln.component.overrides = record_override(&before, &ln);
        ln.size = Vec2::new(140.0, 20.0);
        apply(&mut d, Command::Update { node: Box::new(ln) });
        let c = set_instance_prop(&mut d, inst, "Size", Some(PropValue::Variant("Md".into())));
        apply(&mut d, c);
        let nl = d.get(child_named(&d, inst, "label")).unwrap();
        assert!(!nl.component.overrides.contains(&OverrideField::Size), "a re-fit text box is not a user Size override");
    }

    #[test]
    fn variant_switch_round_trip_keeps_long_text_size_override() {
        let mut d = Document::new("LongTextRoundTrip");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, text: &str, w: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 100.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(4.0, 4.0, w, 20.0), text, crate::TextStyle::default());
            t.name = "label".into();
            if let NodeKind::Text(tp) = &mut t.kind {
                tp.auto_resize = TextAutoResize::WidthAndHeight;
            }
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "State=A", 0.0, "A", 40.0);
        let b = mk(&mut d, "State=B", 100.0, "Longer B", 80.0);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let label = child_named(&d, inst, "label");
        let before = d.get(label).unwrap().clone();
        let mut edited = before.clone();
        if let NodeKind::Text(tp) = &mut edited.kind {
            tp.content = "A much longer overridden label".into();
            tp.auto_resize = TextAutoResize::WidthAndHeight;
        }
        edited.size.x = 240.0;
        edited.component.overrides = record_override(&before, &edited);
        assert!(edited.component.overrides.contains(&OverrideField::Characters));
        assert!(edited.component.overrides.contains(&OverrideField::Size));
        apply(&mut d, Command::Update { node: Box::new(edited) });
        let kept_width = d.get(child_named(&d, inst, "label")).unwrap().size.x;
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        let after = d.get(child_named(&d, inst, "label")).unwrap();
        assert_eq!(after.size.x, kept_width, "switching B then A must preserve the overridden text width");
        assert_eq!(text_of(&d, after.id), "A much longer overridden label");

        // Also cover derived auto-resize text width when only Characters is in overrides (no Size override flag).
        let c = create_instance(&mut d, a, page, Vec2::new(450.0, 0.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        let label2 = child_named(&d, inst2, "label");
        override_text(&mut d, label2, "A much longer overridden label");
        let mut refit = d.get(label2).unwrap().clone();
        refit.size.x = 180.0;
        apply(&mut d, Command::Update { node: Box::new(refit) });
        assert!(!d.get(label2).unwrap().component.overrides.contains(&OverrideField::Size));
        let c = set_instance_prop(&mut d, inst2, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, inst2, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        let after2 = d.get(child_named(&d, inst2, "label")).unwrap();
        assert_eq!(after2.size.x, 180.0, "derived auto-resize text width must round-trip when text style matches");
        assert!(!after2.component.overrides.contains(&OverrideField::Size));
    }

    #[test]
    fn variant_switch_drops_size_only_text_override_when_variant_boxes_differ() {
        let mut d = Document::new("SizeOnlyText");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, w: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 200.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(4.0, 4.0, w, 20.0), "Same", crate::TextStyle::default());
            t.name = "label".into();
            if let NodeKind::Text(tp) = &mut t.kind {
                tp.auto_resize = TextAutoResize::None;
            }
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "State=A", 0.0, 60.0);
        let b = mk(&mut d, "State=B", 100.0, 120.0);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let label = child_named(&d, inst, "label");
        let before = d.get(label).unwrap().clone();
        let mut edited = before.clone();
        edited.size.x = 80.0;
        edited.component.overrides = record_override(&before, &edited);
        apply(&mut d, Command::Update { node: Box::new(edited) });
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let after = d.get(child_named(&d, inst, "label")).unwrap();
        assert_eq!(after.size.x, 120.0, "a size-only override without a Characters override is gated when variant boxes differ");
    }

    #[test]
    fn variant_switch_does_not_carry_text_size_when_variant_font_sizes_differ() {
        let mut d = Document::new("DiffFontTextSize");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, text: &str, w: f32, fs: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 200.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let style = crate::TextStyle { font_size: fs, ..crate::TextStyle::default() };
            let mut t = Node::text(tid, Rect::new(4.0, 4.0, w, 20.0), text, style);
            t.name = "label".into();
            if let NodeKind::Text(tp) = &mut t.kind {
                tp.auto_resize = TextAutoResize::WidthAndHeight;
            }
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "Size=Sm", 0.0, "A", 40.0, 14.0);
        let b = mk(&mut d, "Size=Lg", 100.0, "B", 90.0, 20.0);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        // Case 1: explicit Characters + Size overrides when font_size differs across variants.
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let inst1 = *d.get(page).unwrap().children.last().unwrap();
        let label1 = child_named(&d, inst1, "label");
        let before1 = d.get(label1).unwrap().clone();
        let mut edited1 = before1.clone();
        if let NodeKind::Text(tp) = &mut edited1.kind {
            tp.content = "Overridden label".into();
        }
        edited1.size.x = 240.0;
        edited1.component.overrides = record_override(&before1, &edited1);
        apply(&mut d, Command::Update { node: Box::new(edited1) });
        let c = set_instance_prop(&mut d, inst1, "Size", Some(PropValue::Variant("Lg".into())));
        apply(&mut d, c);
        let after1 = d.get(child_named(&d, inst1, "label")).unwrap();
        assert_eq!(after1.size.x, 90.0, "explicit Size override on an overridden text must not carry across variants with different font sizes");

        // Case 2: Characters override with derived box (no Size override flag) when font_size differs across variants.
        let c = create_instance(&mut d, a, page, Vec2::new(520.0, 0.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        let label2 = child_named(&d, inst2, "label");
        override_text(&mut d, label2, "Overridden label");
        let mut refit2 = d.get(label2).unwrap().clone();
        refit2.size.x = 180.0;
        apply(&mut d, Command::Update { node: Box::new(refit2) });
        let c = set_instance_prop(&mut d, inst2, "Size", Some(PropValue::Variant("Lg".into())));
        apply(&mut d, c);
        let after2 = d.get(child_named(&d, inst2, "label")).unwrap();
        assert_eq!(after2.size.x, 90.0, "derived auto-resize text box must not carry across variants with different font sizes");
    }

    #[test]
    fn variant_switch_transfers_styled_run_on_text_crossing_variants_with_different_defaults() {
        let mut d = Document::new("RichTextVariantSwitch");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, text: &str, runs: Vec<crate::TextRun>| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 160.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(4.0, 4.0, 120.0, 20.0), text, crate::TextStyle::default());
            t.name = "Label".into();
            if let NodeKind::Text(tp) = &mut t.kind {
                tp.runs = runs;
            }
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };

        let bold = crate::TextStyle { font_weight: 700, ..crate::TextStyle::default() };
        let user_run = crate::TextRun { start: 4, end: 7, style: bold };

        let idle = mk(&mut d, "State=Idle", 0.0, "Save", Vec::new());
        let done = mk(&mut d, "State=Done", 80.0, "Saved", Vec::new());
        let c = combine_as_variants(&mut d, &[idle, done]);
        apply(&mut d, c);

        let c = create_instance(&mut d, idle, page, Vec2::new(240.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let lbl = child_named(&d, inst, "Label");
        let before = d.get(lbl).unwrap().clone();
        let mut edited = before.clone();
        if let NodeKind::Text(tp) = &mut edited.kind {
            tp.content = "Buy Now".into();
            tp.runs = vec![user_run.clone()];
        }
        edited.component.overrides = record_override(&before, &edited);
        apply(&mut d, Command::Update { node: Box::new(edited) });

        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("Done".into())));
        apply(&mut d, c);
        let done_lbl = child_named(&d, inst, "Label");
        let NodeKind::Text(done_tp) = &d.get(done_lbl).unwrap().kind else { panic!("expected text") };
        assert_eq!(done_tp.content, "Buy Now");
        assert_eq!(done_tp.runs, vec![user_run], "user styled run transfers with Characters when variant default runs match");
    }

    #[test]
    fn top_level_variant_switch_children_match_root_size() {
        let mut d = Document::new("VariantRoot");
        let page = d.add_page("P");
        let mk = |d: &mut Document, name: &str, y: f32, h: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(0.0, y, 100.0, h));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, h));
            r.name = "bg".into();
            r.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
            d.insert(comp, r).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk(&mut d, "Size=A", 0.0, 40.0);
        let b = mk(&mut d, "Size=B", 100.0, 60.0);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        {
            let mut n = d.get(inst).unwrap().clone();
            n.size = Vec2::new(300.0, 40.0);
            apply(&mut d, Command::Update { node: Box::new(n) });
            let bgid = child_named(&d, inst, "bg");
            let mut bg = d.get(bgid).unwrap().clone();
            bg.size = Vec2::new(300.0, 40.0);
            apply(&mut d, Command::Update { node: Box::new(bg) });
        }
        let c = set_instance_prop(&mut d, inst, "Size", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let n = d.get(inst).unwrap();
        let bg = d.get(child_named(&d, inst, "bg")).unwrap();
        assert_eq!(n.size, Vec2::new(100.0, 60.0));
        assert_eq!(bg.size, n.size, "Stretch child must match the root after a variant switch");
        assert!(!bg.component.overrides.contains(&OverrideField::Size));
    }

    #[test]
    fn swapped_slot_glyph_after_three_syncs_matches_direct_20px_glyph() {
        const MINMIN: Constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Min };
        let mut d = Document::new("DirectGlyph");
        let page = d.add_page("P");
        let mk_icon = |d: &mut Document, name: &str, x: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let gid = d.alloc_id();
            let mut g = Node::new(gid, NodeKind::Vector { paths: box_outline(16.0), network: None });
            g.size = Vec2::new(16.0, 16.0);
            g.name = "Glyph".into();
            g.constraints = MINMIN;
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.fill_geometry = Some(box_outline(16.0));
            d.insert(comp, g).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let a = mk_icon(&mut d, "Icon/A", 0.0);
        let b = mk_icon(&mut d, "Icon/B", 40.0);
        {
            let gb = d.get(b).unwrap().children[0];
            let mut g = d.get(gb).unwrap().clone();
            let p = vec![crate::Path {
                cmds: vec![
                    crate::PathCmd::MoveTo(Vec2::new(3.0, 2.0)),
                    crate::PathCmd::LineTo(Vec2::new(13.0, 4.0)),
                    crate::PathCmd::LineTo(Vec2::new(8.0, 14.0)),
                    crate::PathCmd::Close,
                ],
                ..Default::default()
            }];
            g.kind = NodeKind::Vector { paths: p.clone(), network: None };
            g.fill_geometry = Some(p);
            apply(&mut d, Command::Update { node: Box::new(g) });
        }
        let btn = {
            let id = d.alloc_id();
            let mut btn = Node::frame(id, Rect::new(0.0, 100.0, 32.0, 32.0));
            btn.name = "Tool".into();
            let btn = d.insert(page, btn).unwrap();
            let c = create_component(&mut d, btn);
            apply(&mut d, c);
            let c = create_instance(&mut d, a, btn, Vec2::new(6.0, 6.0));
            apply(&mut d, c);
            let slot = d.get(btn).unwrap().children[0];
            let mut sn = d.get(slot).unwrap().clone();
            sn.name = "Icon".into();
            sn.size = Vec2::new(20.0, 20.0);
            sn.component.overrides.insert(OverrideField::Name);
            sn.component.overrides.insert(OverrideField::Size);
            apply(&mut d, Command::Update { node: Box::new(sn) });
            let gl = child_named(&d, slot, "Glyph");
            let mut gn = d.get(gl).unwrap().clone();
            let before = gn.clone();
            let tf = gn.transform;
            crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
            gn.component.overrides.extend(record_override(&before, &gn));
            apply(&mut d, Command::Update { node: Box::new(gn) });
            let c = add_prop(
                &mut d,
                btn,
                ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![a, b] }, default: PropValue::Instance(a) },
            );
            apply(&mut d, c);
            let c = bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon"));
            apply(&mut d, c);
            btn
        };
        let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let bi = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(bi).unwrap().children[0];
        let c = set_instance_prop(&mut d, bi, "Icon", Some(PropValue::Instance(b)));
        apply(&mut d, c);

        let mut direct = d.get(d.get(b).unwrap().children[0]).unwrap().clone();
        let tf = direct.transform;
        crate::layout_constraints::apply_constrained_geometry(&mut direct, tf, Vec2::new(20.0, 20.0));
        let want_b = path_bbox(direct.fill_geometry.as_ref().unwrap());
        let want = (want_b.0 + direct.transform.tx, want_b.1 + direct.transform.ty, want_b.2 + direct.transform.tx, want_b.3 + direct.transform.ty);

        for k in 0..=3 {
            if k > 0 {
                for m in [btn, b] {
                    let c = sync_instances(&mut d, m);
                    apply(&mut d, c);
                }
            }
            let g = d.get(child_named(&d, si, "Glyph")).unwrap();
            let gb = path_bbox(g.fill_geometry.as_ref().expect("fill_geometry kept"));
            let got = (gb.0 + g.transform.tx, gb.1 + g.transform.ty, gb.2 + g.transform.tx, gb.3 + g.transform.ty);
            let dmax = [got.0 - want.0, got.1 - want.1, got.2 - want.2, got.3 - want.3].iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!(dmax <= 0.5, "after {k} syncs the drawn glyph {got:?} is {dmax} px from a direct 20 px glyph {want:?}");
            assert!(got.0 >= -1e-3 && got.1 >= -1e-3 && got.2 <= 20.0 + 1e-3 && got.3 <= 20.0 + 1e-3, "glyph drawn outside the 20 px slot: {got:?}");
        }
    }

    #[test]
    fn top_level_swap_does_not_carry_diff_derived_overrides() {
        let mut d = Document::new("TopSwapDiff");
        let page = d.add_page("P");
        let mk_comp = |d: &mut Document, name: &str, x: f32| -> (NodeId, NodeId) {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 80.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let bid = d.alloc_id();
            let mut b = Node::rectangle(bid, Rect::new(4.0, 4.0, 12.0, 12.0));
            b.name = "Badge".into();
            b.visible = true;
            d.insert(comp, b).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            (comp, bid)
        };
        let (comp_a, badge_a) = mk_comp(&mut d, "Comp/A", 0.0);
        let (comp_b, _) = mk_comp(&mut d, "Comp/B", 100.0);
        let c = add_prop(&mut d, comp_a, ComponentPropDef { name: "Show".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) });
        apply(&mut d, c);
        let c = bind_prop(&mut d, badge_a, OverrideField::Visible, Some("Show"));
        apply(&mut d, c);

        let c = create_instance(&mut d, comp_a, page, Vec2::new(0.0, 100.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let c = set_instance_prop(&mut d, inst, "Show", Some(PropValue::Bool(false)));
        apply(&mut d, c);
        let inst_badge = child_named(&d, inst, "Badge");
        assert!(!d.get(inst_badge).unwrap().visible, "boolean prop value hides Badge in Comp/A instance without an explicit Visible override");
        assert!(!d.get(inst_badge).unwrap().component.overrides.contains(&OverrideField::Visible));

        let c = swap_instance(&mut d, inst, comp_b);
        apply(&mut d, c);
        let badge_after = d.get(child_named(&d, inst, "Badge")).unwrap();
        assert!(badge_after.visible, "top-level swap must not carry diff-derived overrides onto Comp/B");
        assert!(!badge_after.component.overrides.contains(&OverrideField::Visible));
    }

    fn mkbtn_stretch(d: &mut Document, page: NodeId, name: &str, x: f32, h: f32) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, 300.0, 80.0, h));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 80.0, h));
        r.name = "bg".into();
        r.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        d.insert(comp, r).unwrap();
        let c = create_component(d, comp);
        apply(d, c);
        comp
    }

    fn card_with_btn_slot(d: &mut Document, page: NodeId, preferred: Vec<NodeId>, widen: bool) -> (NodeId, NodeId, NodeId) {
        let ba = preferred[0];
        let id = d.alloc_id();
        let mut card = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        card.name = "Card".into();
        let card = d.insert(page, card).unwrap();
        let c = create_component(d, card);
        apply(d, c);
        let c = create_instance(d, ba, card, Vec2::new(10.0, 10.0));
        apply(d, c);
        let slot = d.get(card).unwrap().children[0];
        {
            let mut sn = d.get(slot).unwrap().clone();
            sn.name = "Action".into();
            sn.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
            apply(d, Command::Update { node: Box::new(sn) });
        }
        let c = add_prop(d, card, ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred }, default: PropValue::Instance(ba) });
        apply(d, c);
        let c = bind_prop(d, slot, OverrideField::MainComponent, Some("Btn"));
        apply(d, c);
        let c = create_instance(d, card, page, Vec2::new(400.0, 0.0));
        apply(d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(ci).unwrap().children[0];
        if widen {
            let mut cn = d.get(ci).unwrap().clone();
            cn.size = Vec2::new(300.0, 100.0);
            cn.component.overrides.insert(OverrideField::Size);
            apply(d, Command::Update { node: Box::new(cn) });
            for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
                let mut n = d.get(cid).unwrap().clone();
                crate::layout_constraints::apply_constrained_geometry(&mut n, tf, sz);
                apply(d, Command::Update { node: Box::new(n) });
            }
        }
        (card, ci, si)
    }

    fn resize_main_with_constraints(d: &mut Document, comp: NodeId, new: Vec2) {
        let mut n = d.get(comp).unwrap().clone();
        let old = n.size;
        n.size = new;
        apply(d, Command::Update { node: Box::new(n) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(d, comp, old, new, 8) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(d, Command::Update { node: Box::new(k) });
        }
    }

    #[test]
    fn swapped_nested_slot_follows_new_main_resize_by_property() {
        let mut d = Document::new("follow");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb], false);
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        resize_main_with_constraints(&mut d, bb, Vec2::new(80.0, 48.0));
        for _ in 1..=3 {
            for m in [bb, card] {
                let c = sync_instances(&mut d, m);
                apply(&mut d, c);
            }
        }
        let s = d.get(si).unwrap();
        let bg = d.get(s.children[0]).unwrap();
        assert_eq!((s.size, bg.size), (Vec2::new(80.0, 48.0), Vec2::new(80.0, 48.0)), "an unresized swapped slot follows its new main's resize");
    }

    #[test]
    fn nested_stretch_instance_keeps_resized_width_when_parent_syncs_unrelated_opacity() {
        let mut d = Document::new("nested-stretch-sync");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![btn], false);

        // Resize the Card instance. Its nested Stretch button is fitted to the
        // widened instance by the same constraint pass used by the editor.
        let mut card_instance = d.get(ci).unwrap().clone();
        card_instance.size = Vec2::new(300.0, 100.0);
        card_instance.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(card_instance) });
        for (id, tf, size) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
            let mut n = d.get(id).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut n, tf, size);
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        let widened = d.get(si).unwrap().size.x;
        let bg = d.get(si).unwrap().children[0];
        assert!(widened > 200.0, "fixture must widen the nested Stretch instance, got {widened}");
        assert_eq!(d.get(bg).unwrap().size.x, widened);

        // An unrelated edit to the Card main still triggers a full sync.
        let mut card_main = d.get(card).unwrap().clone();
        card_main.opacity = 0.5;
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let before_sync = d.nodes.clone();
        let c = sync_instances(&mut d, card);
        let mut history = History::default();
        history.exec(&mut d, c).unwrap();

        assert_eq!(d.get(ci).unwrap().opacity, 0.5);
        assert_eq!(d.get(si).unwrap().size.x, widened, "sync must preserve the resized nested Stretch width");
        assert_eq!(d.get(bg).unwrap().size.x, widened, "sync must preserve stretched child width inside nested Stretch instance");

        // Undo of the sync restores the exact pre-sync state.
        assert!(history.undo(&mut d).unwrap());
        assert_eq!(d.nodes, before_sync);
        assert!(history.redo(&mut d).unwrap());
        assert_eq!(d.get(ci).unwrap().opacity, 0.5);
        assert_eq!(d.get(si).unwrap().size.x, widened);
        assert_eq!(d.get(bg).unwrap().size.x, widened);

        // Repeating the same sync must be idempotent.
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);
        assert_eq!(d.get(si).unwrap().size.x, widened);
        assert_eq!(d.get(bg).unwrap().size.x, widened);
    }

    fn plain_card_with_stretch_btn(d: &mut Document, page: NodeId, btn: NodeId) -> (NodeId, NodeId, NodeId) {
        let id = d.alloc_id();
        let card = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        d.get_mut(card).unwrap().name = "Card".into();
        let c = create_component(d, card);
        apply(d, c);
        let c = create_instance(d, btn, card, Vec2::new(10.0, 10.0));
        apply(d, c);
        let slot = d.get(card).unwrap().children[0];
        let mut sn = d.get(slot).unwrap().clone();
        sn.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        apply(d, Command::Update { node: Box::new(sn) });
        let c = create_instance(d, card, page, Vec2::new(400.0, 0.0));
        apply(d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(ci).unwrap().children[0];
        let mut cn = d.get(ci).unwrap().clone();
        cn.size = Vec2::new(300.0, 100.0);
        cn.component.overrides.insert(OverrideField::Size);
        apply(d, Command::Update { node: Box::new(cn) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
            let mut n = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut n, tf, sz);
            apply(d, Command::Update { node: Box::new(n) });
        }
        (card, ci, si)
    }

    #[test]
    fn plain_nested_stretch_instance_keeps_width_when_card_main_syncs() {
        let mut d = Document::new("plain-nested");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = plain_card_with_stretch_btn(&mut d, page, btn);
        let widened = d.get(si).unwrap().size.x;
        assert!(widened > 200.0, "fixture must widen the nested instance, got {widened}");
        d.get_mut(card).unwrap().opacity = 0.5;
        for _ in 0..2 {
            let c = sync_instances(&mut d, card);
            apply(&mut d, c);
        }
        assert_eq!(d.get(ci).unwrap().opacity, 0.5);
        assert_eq!(d.get(si).unwrap().size.x, widened);
    }

    #[test]
    fn plain_nested_stretch_instance_keeps_width_when_btn_main_syncs() {
        let mut d = Document::new("plain-nested-btn");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, _ci, si) = plain_card_with_stretch_btn(&mut d, page, btn);
        let widened = d.get(si).unwrap().size.x;
        d.get_mut(btn).unwrap().opacity = 0.5;
        for m in [btn, card, btn] {
            let c = sync_instances(&mut d, m);
            apply(&mut d, c);
        }
        assert_eq!(d.get(si).unwrap().opacity, 0.5);
        assert_eq!(d.get(si).unwrap().size.x, widened, "btn main sync");
    }

    #[test]
    fn swapped_nested_stretch_instance_keeps_width_across_syncs_like_an_unswapped_one() {
        let mut d = Document::new("swap-sync");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb], true);
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let widened = d.get(si).unwrap().size.x;
        assert!(widened > 200.0, "fixture must widen the swapped nested instance, got {widened}");
        d.get_mut(card).unwrap().opacity = 0.5;
        for m in [card, bb, card] {
            let c = sync_instances(&mut d, m);
            apply(&mut d, c);
        }
        let bg = d.get(si).unwrap().children[0];
        assert_eq!((d.get(si).unwrap().size.x, d.get(bg).unwrap().size.x), (widened, widened));
    }

    #[test]
    fn nested_instance_geometry_is_stable_across_syncs_for_every_horizontal_constraint() {
        for h in [Constraint::Stretch, Constraint::Scale, Constraint::Max, Constraint::Center, Constraint::Min] {
            let mut d = Document::new("constraint-sweep");
            let page = d.add_page("P");
            let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
            let (card, ci, si) = plain_card_with_stretch_btn(&mut d, page, btn);
            let slot_main = d.get(card).unwrap().children[0];
            let mut sn = d.get(slot_main).unwrap().clone();
            sn.constraints = Constraints { horizontal: h, vertical: Constraint::Min };
            apply(&mut d, Command::Update { node: Box::new(sn) });
            // Re-run the outer resize from the original size so the new constraint decides the nested geometry.
            let mut cn = d.get(ci).unwrap().clone();
            cn.size = Vec2::new(100.0, 100.0);
            apply(&mut d, Command::Update { node: Box::new(cn) });
            let c = sync_instances(&mut d, card);
            apply(&mut d, c);
            let mut cn = d.get(ci).unwrap().clone();
            cn.size = Vec2::new(300.0, 100.0);
            apply(&mut d, Command::Update { node: Box::new(cn) });
            for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
                let mut n = d.get(cid).unwrap().clone();
                crate::layout_constraints::apply_constrained_geometry(&mut n, tf, sz);
                apply(&mut d, Command::Update { node: Box::new(n) });
            }
            let bg = d.get(si).unwrap().children[0];
            let before = (d.get(si).unwrap().size, d.get(si).unwrap().transform, d.get(bg).unwrap().size, d.get(bg).unwrap().transform);
            d.get_mut(card).unwrap().opacity = 0.5;
            d.get_mut(btn).unwrap().opacity = 0.5;
            for m in [card, btn, card, btn] {
                let c = sync_instances(&mut d, m);
                apply(&mut d, c);
            }
            let after = (d.get(si).unwrap().size, d.get(si).unwrap().transform, d.get(bg).unwrap().size, d.get(bg).unwrap().transform);
            assert_eq!(before, after, "{h:?}: syncs must not move or resize the nested instance");
        }
    }

    #[test]
    fn nested_fixed_instance_takes_main_size_when_parent_syncs() {
        let mut d = Document::new("nested-fixed-sync");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![btn], false);
        let slot = d.get(card).unwrap().children[0];

        // Configure the nested slot as Fixed (Constraint::Min) rather than Stretch.
        for id in [slot, si] {
            let mut n = d.get(id).unwrap().clone();
            n.constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Min };
            apply(&mut d, Command::Update { node: Box::new(n) });
        }

        // Widen the Card instance and give the nested instance a transient non-overridden width.
        let mut card_instance = d.get(ci).unwrap().clone();
        card_instance.size = Vec2::new(300.0, 100.0);
        card_instance.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(card_instance) });
        let mut nested = d.get(si).unwrap().clone();
        nested.size.x = 280.0;
        apply(&mut d, Command::Update { node: Box::new(nested) });

        // Syncing the Card main snaps the Fixed nested instance back to the main slot's width (80.0).
        let before_sync = d.nodes.clone();
        let c = sync_instances(&mut d, card);
        let mut history = History::default();
        history.exec(&mut d, c).unwrap();
        assert_eq!(d.get(si).unwrap().size, Vec2::new(80.0, 40.0), "Fixed nested instance must take the main's size");

        // Undoing the sync restores the pre-sync state.
        assert!(history.undo(&mut d).unwrap());
        assert_eq!(d.nodes, before_sync);
    }

    #[test]
    fn fixed_nested_instance_with_a_leftover_stretch_constraint_follows_its_main() {
        let mut d = Document::new("al-fixed-leftover-stretch");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![btn], false);
        let slot = d.get(card).unwrap().children[0];

        // Configure Card as an auto-layout container, and the nested slot as Fixed sizing
        // with a leftover Stretch constraint.
        for id in [card, ci] {
            let mut n = d.get(id).unwrap().clone();
            if let Some(fp) = n.kind.frame_props_mut() {
                fp.auto_layout =
                    Some(crate::AutoLayout { direction: crate::LayoutDirection::Horizontal, padding: [10.0, 10.0, 10.0, 10.0], ..Default::default() });
            }
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        for id in [slot, si] {
            let mut n = d.get(id).unwrap().clone();
            n.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
            n.layout_child.horizontal = crate::Sizing::Fixed;
            apply(&mut d, Command::Update { node: Box::new(n) });
        }

        // Widen the Card instance and relayout.
        let mut card_instance = d.get(ci).unwrap().clone();
        card_instance.size = Vec2::new(300.0, 100.0);
        card_instance.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(card_instance) });
        d.relayout_subtree(ci);
        assert_eq!(d.get(si).unwrap().size.x, 80.0);

        // Resize the Btn main from 80 to 120 and sync.
        let mut btn_main = d.get(btn).unwrap().clone();
        let old_sz = btn_main.size;
        btn_main.size.x = 120.0;
        let new_sz = btn_main.size;
        apply(&mut d, Command::Update { node: Box::new(btn_main) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, btn, old_sz, new_sz, 8) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(&mut d, Command::Update { node: Box::new(k) });
        }
        let c = sync_instances(&mut d, btn);
        apply(&mut d, c);

        assert_eq!(d.get(slot).unwrap().size.x, 120.0);
        assert_eq!(d.get(si).unwrap().size.x, 120.0, "Fixed child in auto-layout parent must follow its main despite leftover Stretch constraint");
    }

    #[test]
    fn nested_stretch_instance_follows_main_slot_move_and_resize() {
        let mut d = Document::new("nested-stretch-slot-move");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![btn], false);
        let slot = d.get(card).unwrap().children[0];

        // Widen the Card instance from 100 to 300.
        let mut card_instance = d.get(ci).unwrap().clone();
        card_instance.size = Vec2::new(300.0, 100.0);
        card_instance.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(card_instance) });
        for (id, tf, size) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 8) {
            let mut n = d.get(id).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut n, tf, size);
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        assert_eq!(d.get(si).unwrap().size.x, 280.0);

        // Move the slot in the Card main from x=10, w=80 to x=20, w=70 (keeping a 10 px right margin).
        let mut slot_node = d.get(slot).unwrap().clone();
        let old_sz = slot_node.size;
        slot_node.transform.tx = 20.0;
        slot_node.size.x = 70.0;
        let new_sz = slot_node.size;
        apply(&mut d, Command::Update { node: Box::new(slot_node) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, slot, old_sz, new_sz, 8) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(&mut d, Command::Update { node: Box::new(k) });
        }

        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        let bg = d.get(si).unwrap().children[0];
        assert_eq!(d.get(si).unwrap().transform.tx, 20.0);
        assert_eq!(d.get(si).unwrap().size.x, 270.0, "nested Stretch width must follow the main slot's new margins");
        assert_eq!(d.get(bg).unwrap().size.x, 270.0);
    }

    #[test]
    fn nested_auto_layout_fill_instance_keeps_resized_width_when_parent_syncs() {
        let mut d = Document::new("nested-al-fill-keeps-width");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![btn], false);
        let slot = d.get(card).unwrap().children[0];

        for id in [card, ci] {
            let mut n = d.get(id).unwrap().clone();
            if let Some(fp) = n.kind.frame_props_mut() {
                fp.auto_layout =
                    Some(crate::AutoLayout { direction: crate::LayoutDirection::Vertical, padding: [10.0, 10.0, 10.0, 10.0], ..Default::default() });
            }
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        for id in [slot, si] {
            let mut n = d.get(id).unwrap().clone();
            n.constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Min };
            n.layout_child.horizontal = crate::Sizing::Fill;
            apply(&mut d, Command::Update { node: Box::new(n) });
        }

        let mut card_instance = d.get(ci).unwrap().clone();
        card_instance.size = Vec2::new(300.0, 100.0);
        card_instance.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(card_instance) });
        d.relayout_subtree(ci);
        assert_eq!(d.get(si).unwrap().size.x, 280.0);

        let mut card_main = d.get(card).unwrap().clone();
        card_main.opacity = 0.75;
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        let bg_inst = d.get(si).unwrap().children[0];
        assert_eq!(d.get(si).unwrap().size.x, 280.0, "auto-layout Fill child must keep its parent-given width after sync");
        assert_eq!(d.get(bg_inst).unwrap().size.x, 280.0);
    }

    #[test]
    fn nested_stretch_instance_under_swapped_parent_keeps_parent_delta_on_sync() {
        let mut d = Document::new("nested-under-swapped-parent");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);

        // CardA (width 100) holds slot (width 80, Stretch).
        let (card_a, _ci_a, _) = card_with_btn_slot(&mut d, page, vec![btn], false);
        // CardB (width 200) holds slot_b (width 160, Stretch).
        let card_b = d.alloc_id();
        let mut cb = Node::new(card_b, NodeKind::Component(FrameProps::default()));
        cb.name = "CardB".into();
        cb.size = Vec2::new(200.0, 100.0);
        d.insert(page, cb).unwrap();
        let c = create_instance(&mut d, btn, card_b, Vec2::new(20.0, 10.0));
        apply(&mut d, c);
        let slot_b = d.get(card_b).unwrap().children[0];
        let mut sb = d.get(slot_b).unwrap().clone();
        sb.size = Vec2::new(160.0, 40.0);
        sb.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        sb.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(sb) });

        // Outer holds an instance of CardA (`inner_card`), which we swap to CardB in an outer instance.
        let outer = d.alloc_id();
        let mut om = Node::new(outer, NodeKind::Component(FrameProps::default()));
        om.name = "Outer".into();
        om.size = Vec2::new(220.0, 120.0);
        d.insert(page, om).unwrap();
        let c = create_instance(&mut d, card_a, outer, Vec2::new(10.0, 10.0));
        apply(&mut d, c);

        let c = create_instance(&mut d, outer, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let outer_inst = *d.get(page).unwrap().children.last().unwrap();
        let inner_card = d.get(outer_inst).unwrap().children[0];
        let c = swap_instance(&mut d, inner_card, card_b);
        apply(&mut d, c);
        let inner_btn = d.get(inner_card).unwrap().children[0];
        assert_eq!(d.get(inner_btn).unwrap().size.x, 160.0);

        // Widen inner_card to 240 (+40 over CardB's 200), resize slot_b in CardB to 140, and sync Btn:
        // inner_btn stretches against inner_card's current 240 width to 180 (140 + 40).
        let mut ic = d.get(inner_card).unwrap().clone();
        ic.size.x = 240.0;
        ic.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(ic) });
        let mut sb2 = d.get(slot_b).unwrap().clone();
        sb2.size.x = 140.0;
        sb2.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(sb2) });
        let c = sync_instances(&mut d, btn);
        apply(&mut d, c);

        assert_eq!(d.get(inner_btn).unwrap().size.x, 180.0);
    }

    #[test]
    fn unresized_swapped_nested_stretch_slot_follows_new_main_width_change() {
        let mut d = Document::new("unresized-swapped-stretch-width");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb], false);
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        resize_main_with_constraints(&mut d, bb, Vec2::new(120.0, 48.0));
        for m in [bb, card] {
            let c = sync_instances(&mut d, m);
            apply(&mut d, c);
        }
        let s = d.get(si).unwrap();
        let bg = d.get(s.children[0]).unwrap();
        assert_eq!((s.size, bg.size), (Vec2::new(120.0, 48.0), Vec2::new(120.0, 48.0)));
    }

    #[test]
    fn swapped_nested_slot_follows_new_main_resize_by_direct_swap() {
        let mut d = Document::new("follow2");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let (card, _ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb], false);
        let c = swap_instance(&mut d, si, bb);
        apply(&mut d, c);
        resize_main_with_constraints(&mut d, bb, Vec2::new(80.0, 48.0));
        for _ in 1..=3 {
            for m in [bb, card] {
                let c = sync_instances(&mut d, m);
                apply(&mut d, c);
            }
        }
        let s = d.get(si).unwrap();
        let bg = d.get(s.children[0]).unwrap();
        assert_eq!((s.size, bg.size), (Vec2::new(80.0, 48.0), Vec2::new(80.0, 48.0)), "an unresized swapped slot follows its new main's resize");
    }

    #[test]
    fn reswapped_nested_slot_takes_new_main_size() {
        let mut d = Document::new("reswap");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let bc = mkbtn_stretch(&mut d, page, "Btn/C", 200.0, 48.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb, bc], false);
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bb)));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, ci, "Btn", Some(PropValue::Instance(bc)));
        apply(&mut d, c);
        let s_swap = d.get(si).unwrap().size;
        let bg_swap = d.get(d.get(si).unwrap().children[0]).unwrap().size;
        for m in [bc, card] {
            let c = sync_instances(&mut d, m);
            apply(&mut d, c);
        }
        assert_eq!((s_swap, bg_swap), (Vec2::new(80.0, 48.0), Vec2::new(80.0, 48.0)), "B -> C on an unresized slot takes C's size");
    }

    fn tri_outline(w: f32) -> Vec<crate::Path> {
        vec![crate::Path {
            cmds: vec![
                crate::PathCmd::MoveTo(Vec2::new(0.15 * w, 0.1 * w)),
                crate::PathCmd::LineTo(Vec2::new(0.9 * w, 0.5 * w)),
                crate::PathCmd::LineTo(Vec2::new(0.2 * w, 0.85 * w)),
                crate::PathCmd::Close,
            ],
            ..Default::default()
        }]
    }

    fn glyph_node_of_kind(d: &mut Document, kind: &str, size: f32) -> Node {
        let gid = d.alloc_id();
        let mut g = match kind {
            "vector" => Node::new(gid, NodeKind::Vector { paths: tri_outline(size), network: None }),
            "boolean" => Node::new(gid, NodeKind::BooleanOp(BooleanOp::Union)),
            "ellipse" => Node::ellipse(gid, Rect::new(0.0, 0.0, size, size)),
            "frame" => Node::frame(gid, Rect::new(0.0, 0.0, size, size)),
            _ => unreachable!(),
        };
        g.size = Vec2::new(size, size);
        g.transform = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };
        g.name = "Glyph".into();
        g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
        g.fill_geometry = Some(tri_outline(size));
        g
    }

    fn icon_of_kind(d: &mut Document, page: NodeId, name: &str, x: f32, kind: &str) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, 0.0, 16.0, 16.0));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();
        let g = glyph_node_of_kind(d, kind, 16.0);
        d.insert(comp, g).unwrap();
        let c = create_component(d, comp);
        apply(d, c);
        comp
    }

    fn tool_with_icon_slot(d: &mut Document, page: NodeId, a: NodeId, b: NodeId) -> (NodeId, NodeId) {
        let id = d.alloc_id();
        let mut btn = Node::frame(id, Rect::new(0.0, 100.0, 32.0, 32.0));
        btn.name = "Tool".into();
        let btn = d.insert(page, btn).unwrap();
        let c = create_component(d, btn);
        apply(d, c);
        let c = create_instance(d, a, btn, Vec2::new(6.0, 6.0));
        apply(d, c);
        let slot = d.get(btn).unwrap().children[0];
        let mut sn = d.get(slot).unwrap().clone();
        sn.name = "Icon".into();
        sn.size = Vec2::new(20.0, 20.0);
        sn.component.overrides.insert(OverrideField::Name);
        sn.component.overrides.insert(OverrideField::Size);
        apply(d, Command::Update { node: Box::new(sn) });
        let gl = child_named(d, slot, "Glyph");
        let mut gn = d.get(gl).unwrap().clone();
        let before = gn.clone();
        let tf = gn.transform;
        crate::layout_constraints::apply_constrained_geometry(&mut gn, tf, Vec2::new(20.0, 20.0));
        gn.component.overrides.extend(record_override(&before, &gn));
        apply(d, Command::Update { node: Box::new(gn) });
        let c =
            add_prop(d, btn, ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![a, b] }, default: PropValue::Instance(a) });
        apply(d, c);
        let c = bind_prop(d, slot, OverrideField::MainComponent, Some("Icon"));
        apply(d, c);
        (btn, slot)
    }

    #[test]
    fn glyph_type_change_with_carried_size_draws_new_outline() {
        let placed = |b: (f32, f32, f32, f32), t: Affine| (b.0 + t.tx, b.1 + t.ty, b.2 + t.tx, b.3 + t.ty);
        let mut bad = Vec::new();
        for (ka, kb) in [("vector", "vector"), ("vector", "boolean"), ("boolean", "vector"), ("vector", "ellipse"), ("vector", "frame"), ("ellipse", "vector")]
        {
            let mut d = Document::new("xk");
            let page = d.add_page("P");
            let a = icon_of_kind(&mut d, page, "Icon/A", 0.0, ka);
            let b = icon_of_kind(&mut d, page, "Icon/B", 40.0, kb);
            {
                let gb = d.get(b).unwrap().children[0];
                let mut g = d.get(gb).unwrap().clone();
                let p = vec![crate::Path {
                    cmds: vec![
                        crate::PathCmd::MoveTo(Vec2::new(3.0, 2.0)),
                        crate::PathCmd::LineTo(Vec2::new(13.0, 4.0)),
                        crate::PathCmd::LineTo(Vec2::new(8.0, 14.0)),
                        crate::PathCmd::Close,
                    ],
                    ..Default::default()
                }];
                if let NodeKind::Vector { paths, .. } = &mut g.kind {
                    *paths = p.clone();
                }
                g.fill_geometry = Some(p);
                apply(&mut d, Command::Update { node: Box::new(g) });
            }
            let (btn, _) = tool_with_icon_slot(&mut d, page, a, b);
            let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
            apply(&mut d, c);
            let bi = *d.get(page).unwrap().children.last().unwrap();
            let si = d.get(bi).unwrap().children[0];
            let c = set_instance_prop(&mut d, bi, "Icon", Some(PropValue::Instance(b)));
            apply(&mut d, c);
            let mut direct = d.get(d.get(b).unwrap().children[0]).unwrap().clone();
            let tf = direct.transform;
            crate::layout_constraints::apply_constrained_geometry(&mut direct, tf, Vec2::new(20.0, 20.0));
            let want = direct.fill_geometry.as_ref().map(|p| placed(path_bbox(p), direct.transform));
            for k in 0..=3 {
                if k > 0 {
                    for m in [btn, b, a] {
                        let c = sync_instances(&mut d, m);
                        apply(&mut d, c);
                    }
                }
                if k == 0 || k == 1 || k == 3 {
                    let g = d.get(child_named(&d, si, "Glyph")).unwrap();
                    let got = g.fill_geometry.as_ref().map(|p| placed(path_bbox(p), g.transform));
                    let ok = g.size == Vec2::new(20.0, 20.0)
                        && match (got, want) {
                            (None, None) => true,
                            (Some(x), Some(y)) => [x.0 - y.0, x.1 - y.1, x.2 - y.2, x.3 - y.3].iter().all(|v| v.abs() <= 0.5),
                            _ => false,
                        };
                    if !ok {
                        bad.push(format!("{ka}->{kb} sync#{k}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "cross-kind Size carry draws the wrong outline: {bad:?}");
    }

    #[test]
    fn direct_cross_type_swap_with_fills_only_keeps_imported_outline_before_sync() {
        for kb in ["frame", "ellipse"] {
            let mut d = Document::new("dc");
            let page = d.add_page("P");
            let a = icon_of_kind(&mut d, page, "Icon/A", 0.0, "vector");
            let b = icon_of_kind(&mut d, page, "Icon/B", 40.0, kb);
            {
                let gb = d.get(b).unwrap().children[0];
                let mut g = d.get(gb).unwrap().clone();
                let p = vec![crate::Path {
                    cmds: vec![
                        crate::PathCmd::MoveTo(Vec2::new(3.0, 2.0)),
                        crate::PathCmd::LineTo(Vec2::new(13.0, 4.0)),
                        crate::PathCmd::LineTo(Vec2::new(8.0, 14.0)),
                        crate::PathCmd::Close,
                    ],
                    ..Default::default()
                }];
                g.fill_geometry = Some(p);
                apply(&mut d, Command::Update { node: Box::new(g) });
            }
            let id = d.alloc_id();
            let mut btn = Node::frame(id, Rect::new(0.0, 100.0, 32.0, 32.0));
            btn.name = "Tool".into();
            let btn = d.insert(page, btn).unwrap();
            let c = create_component(&mut d, btn);
            apply(&mut d, c);
            let c = create_instance(&mut d, a, btn, Vec2::new(6.0, 6.0));
            apply(&mut d, c);
            let slot = d.get(btn).unwrap().children[0];
            let gl = child_named(&d, slot, "Glyph");
            {
                let mut gn = d.get(gl).unwrap().clone();
                let before = gn.clone();
                gn.fills = vec![Paint::solid(Color::rgb(200, 30, 30))];
                gn.component.overrides.extend(record_override(&before, &gn));
                apply(&mut d, Command::Update { node: Box::new(gn) });
            }
            let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
            apply(&mut d, c);
            let bi = *d.get(page).unwrap().children.last().unwrap();
            let si = d.get(bi).unwrap().children[0];
            let c = swap_instance(&mut d, si, b);
            apply(&mut d, c);
            let want = Some((3.0, 2.0, 13.0, 14.0));
            let g0 = d.get(child_named(&d, si, "Glyph")).unwrap();
            assert_eq!(
                g0.fill_geometry.as_ref().map(|p| path_bbox(p)),
                want,
                "vector->{kb} Fills-only direct swap must keep new main's imported outline before sync"
            );
            for m in [btn, b, a] {
                let c = sync_instances(&mut d, m);
                apply(&mut d, c);
            }
            let g1 = d.get(child_named(&d, si, "Glyph")).unwrap();
            assert_eq!(
                g1.fill_geometry.as_ref().map(|p| path_bbox(p)),
                want,
                "vector->{kb} Fills-only direct swap keeps new main's imported outline after sync"
            );
        }
    }

    #[test]
    fn cross_type_swap_with_carried_size_clears_stroke_geometry_on_frame_glyph() {
        let mut d = Document::new("sg");
        let page = d.add_page("P");
        let a = icon_of_kind(&mut d, page, "Icon/A", 0.0, "vector");
        let b = icon_of_kind(&mut d, page, "Icon/B", 40.0, "frame");
        for comp in [a, b] {
            let gid = d.get(comp).unwrap().children[0];
            let mut g = d.get(gid).unwrap().clone();
            g.stroke_geometry = Some(box_outline(16.0));
            apply(&mut d, Command::Update { node: Box::new(g) });
        }
        let (btn, _) = tool_with_icon_slot(&mut d, page, a, b);
        let c = create_instance(&mut d, btn, page, Vec2::new(200.0, 100.0));
        apply(&mut d, c);
        let bi = *d.get(page).unwrap().children.last().unwrap();
        let si = d.get(bi).unwrap().children[0];
        let c = set_instance_prop(&mut d, bi, "Icon", Some(PropValue::Instance(b)));
        apply(&mut d, c);
        let g0 = d.get(child_named(&d, si, "Glyph")).unwrap();
        assert_eq!(g0.stroke_geometry, None, "vector->frame with carried Size must clear stroke_geometry at swap");
        for m in [btn, b, a] {
            let c = sync_instances(&mut d, m);
            apply(&mut d, c);
        }
        let g1 = d.get(child_named(&d, si, "Glyph")).unwrap();
        assert_eq!(g1.stroke_geometry, None, "vector->frame with carried Size keeps stroke_geometry == None after 1 sync");
    }

    #[test]
    fn expose_nested_instances_toggles_bubbles_edits_survives_sync_and_preserves_overrides_when_unexposed() {
        let mut d = Document::new("expose-nested");
        let page = d.add_page("P");

        // 1. Build a component set `Badge` with two variants (`Size=Sm`, `Size=Lg`), a Boolean prop `Show dot`, and a Text prop `Label`.
        let mut make_badge_variant = |name: &str, x: f32, w: f32| {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, w, 24.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let dot_id = d.alloc_id();
            let mut dot = Node::ellipse(dot_id, Rect::new(4.0, 8.0, 8.0, 8.0));
            dot.name = "Dot".into();
            let dot = d.insert(comp, dot).unwrap();
            let txt_id = d.alloc_id();
            let mut txt = Node::text(txt_id, Rect::new(16.0, 4.0, 40.0, 16.0), "Badge", TextStyle::default());
            txt.name = "Text".into();
            let txt = d.insert(comp, txt).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            (comp, dot, txt)
        };
        let (v_sm, dot_sm, txt_sm) = make_badge_variant("Size=Sm", 0.0, 60.0);
        let (v_lg, dot_lg, txt_lg) = make_badge_variant("Size=Lg", 80.0, 90.0);
        let c = combine_as_variants(&mut d, &[v_sm, v_lg]);
        apply(&mut d, c);
        let badge_set = d.get(v_sm).unwrap().parent.unwrap();
        d.get_mut(badge_set).unwrap().name = "Badge".into();

        let c = add_prop(&mut d, badge_set, ComponentPropDef { name: "Show dot".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut d, c);
        let c = add_prop(&mut d, badge_set, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Badge".into()) });
        apply(&mut d, c);
        for (dot, txt) in [(dot_sm, txt_sm), (dot_lg, txt_lg)] {
            let c = bind_prop(&mut d, dot, OverrideField::Visible, Some("Show dot"));
            apply(&mut d, c);
            let c = bind_prop(&mut d, txt, OverrideField::Characters, Some("Label"));
            apply(&mut d, c);
        }

        // Plain component without any properties (should not appear in expose_nested_candidates).
        let plain_id = d.alloc_id();
        let mut plain = Node::frame(plain_id, Rect::new(200.0, 0.0, 20.0, 20.0));
        plain.name = "PlainDivider".into();
        let plain = d.insert(page, plain).unwrap();
        let c = create_component(&mut d, plain);
        apply(&mut d, c);

        // 2. Build outer main component `Card` with two nested Badge instances (`Leading badge`, `Trailing badge`) and one `PlainDivider` instance.
        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(0.0, 100.0, 240.0, 60.0));
        card.name = "Card".into();
        let card = d.insert(page, card).unwrap();
        let c = create_component(&mut d, card);
        apply(&mut d, c);

        let c = create_instance(&mut d, v_sm, card, Vec2::new(8.0, 8.0));
        apply(&mut d, c);
        let leading = *d.get(card).unwrap().children.last().unwrap();
        d.get_mut(leading).unwrap().name = "Leading badge".into();

        let c = create_instance(&mut d, v_sm, card, Vec2::new(80.0, 8.0));
        apply(&mut d, c);
        let trailing = *d.get(card).unwrap().children.last().unwrap();
        d.get_mut(trailing).unwrap().name = "Trailing badge".into();

        let c = create_instance(&mut d, plain, card, Vec2::new(160.0, 8.0));
        apply(&mut d, c);

        // Candidates list only nested instances whose component has properties.
        let cands = expose_nested_candidates(&d, card);
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].id, leading);
        assert_eq!(cands[0].name, "Leading badge");
        assert!(!cands[0].exposed);
        assert_eq!(cands[1].id, trailing);
        assert_eq!(cands[1].name, "Trailing badge");
        assert!(!cands[1].exposed);

        // 3. Create two outer instances of `Card`.
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let card_inst_1 = *d.get(page).unwrap().children.last().unwrap();
        let c = create_instance(&mut d, card, page, Vec2::new(260.0, 200.0));
        apply(&mut d, c);
        let card_inst_2 = *d.get(page).unwrap().children.last().unwrap();

        assert!(exposed_nested_instances(&d, card_inst_1).is_empty(), "nothing exposed before toggling");

        // 4. Toggle `Leading badge` exposed on `Card` in one undoable command.
        let toggle_on = set_expose_nested_instance(&mut d, leading, true);
        let toggle_inv = apply(&mut d, toggle_on);
        assert!(d.get(leading).unwrap().component.expose_nested_instances);
        assert!(!d.get(trailing).unwrap().component.expose_nested_instances);

        let inst1_leading = child_named(&d, card_inst_1, "Leading badge");
        let inst2_leading = child_named(&d, card_inst_2, "Leading badge");
        assert_eq!(exposed_nested_instances(&d, card_inst_1), vec![inst1_leading]);

        // Undo and redo the expose toggle in one step.
        let toggle_redo = apply(&mut d, toggle_inv);
        assert!(!d.get(leading).unwrap().component.expose_nested_instances);
        assert!(exposed_nested_instances(&d, card_inst_1).is_empty());
        apply(&mut d, toggle_redo);
        assert!(d.get(leading).unwrap().component.expose_nested_instances);
        assert_eq!(exposed_nested_instances(&d, card_inst_1), vec![inst1_leading]);

        // 5. Edit Variant, Boolean, and Text properties on the exposed nested instance inside `card_inst_1`.
        let c = set_nested_instance_prop(&mut d, inst1_leading, "Size", Some(PropValue::Variant("Lg".into())));
        apply(&mut d, c);
        assert!(
            d.get(inst1_leading).unwrap().component.overrides.contains(&OverrideField::MainComponent),
            "the variant edit is recorded as a swap on the nested instance"
        );
        let c = set_instance_prop(&mut d, inst1_leading, "Show dot", Some(PropValue::Bool(false)));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, inst1_leading, "Label", Some(PropValue::Text("Featured".into())));
        apply(&mut d, c);

        assert_eq!(instance_component(d.get(inst1_leading).unwrap()), Some(v_lg), "nested instance in card_inst_1 switched to Lg variant");
        assert!(!d.get(child_named(&d, inst1_leading, "Dot")).unwrap().visible, "nested instance in card_inst_1 hid Dot");
        assert_eq!(text_of(&d, child_named(&d, inst1_leading, "Text")), "Featured");

        // Main component's nested instance and sibling outer instance are untouched.
        assert_eq!(instance_component(d.get(leading).unwrap()), Some(v_sm));
        assert!(d.get(child_named(&d, leading, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, leading, "Text")), "Badge");
        assert_eq!(instance_component(d.get(inst2_leading).unwrap()), Some(v_sm));
        assert!(d.get(child_named(&d, inst2_leading, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, inst2_leading, "Text")), "Badge");

        // 6. Syncing the outer main component preserves all nested instance overrides.
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(inst1_leading).unwrap()), Some(v_lg), "variant override on nested instance survives sync_instances");
        assert!(!d.get(child_named(&d, inst1_leading, "Dot")).unwrap().visible, "boolean override on nested instance survives sync_instances");
        assert_eq!(text_of(&d, child_named(&d, inst1_leading, "Text")), "Featured", "text override on nested instance survives sync_instances");

        // 7. Turning expose off hides the nested instance from `exposed_nested_instances` but preserves existing overrides on instances.
        let toggle_off = set_expose_nested_instance(&mut d, leading, false);
        apply(&mut d, toggle_off);
        assert!(exposed_nested_instances(&d, card_inst_1).is_empty());
        assert_eq!(instance_component(d.get(inst1_leading).unwrap()), Some(v_lg));
        assert!(!d.get(child_named(&d, inst1_leading, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, inst1_leading, "Text")), "Featured");

        // 8. While the nested instance is exposed, resetting the outer instance takes back its variant, boolean and text.
        let toggle_on = set_expose_nested_instance(&mut d, leading, true);
        apply(&mut d, toggle_on);
        let c = reset_instance(&mut d, card_inst_1);
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(inst1_leading).unwrap()), Some(v_sm));
        assert!(d.get(child_named(&d, inst1_leading, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, inst1_leading, "Text")), "Badge");
    }

    #[test]
    fn exposed_nested_instances_handles_multi_level_hierarchy_and_variant_sets() {
        let mut d = Document::new("expose-multi-level");
        let page = d.add_page("P");

        // Inner component with a Boolean property.
        let inner_id = d.alloc_id();
        let mut inner = Node::frame(inner_id, Rect::new(0.0, 0.0, 16.0, 16.0));
        inner.name = "Icon".into();
        let inner = d.insert(page, inner).unwrap();
        let glyph_id = d.alloc_id();
        let mut glyph = Node::rectangle(glyph_id, Rect::new(2.0, 2.0, 12.0, 12.0));
        glyph.name = "Glyph".into();
        let glyph = d.insert(inner, glyph).unwrap();
        let c = create_component(&mut d, inner);
        apply(&mut d, c);
        let c = add_prop(&mut d, inner, ComponentPropDef { name: "Show glyph".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(&mut d, c);
        let c = bind_prop(&mut d, glyph, OverrideField::Visible, Some("Show glyph"));
        apply(&mut d, c);

        // Middle component containing an instance of `Icon` (exposed).
        let mid_id = d.alloc_id();
        let mut mid = Node::frame(mid_id, Rect::new(40.0, 0.0, 48.0, 24.0));
        mid.name = "Chip".into();
        let mid = d.insert(page, mid).unwrap();
        let c = create_component(&mut d, mid);
        apply(&mut d, c);
        let c = create_instance(&mut d, inner, mid, Vec2::new(4.0, 4.0));
        apply(&mut d, c);
        let icon_in_mid = d.get(mid).unwrap().children[0];
        d.get_mut(icon_in_mid).unwrap().name = "Chip icon".into();
        let c = set_expose_nested_instance(&mut d, icon_in_mid, true);
        apply(&mut d, c);

        // Outer component set with two variants, each containing a `Chip` instance.
        let mut make_outer_var = |name: &str, y: f32| {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(120.0, y, 120.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            let c = create_instance(&mut d, mid, comp, Vec2::new(8.0, 8.0));
            apply(&mut d, c);
            let chip = d.get(comp).unwrap().children[0];
            d.get_mut(chip).unwrap().name = "Status chip".into();
            (comp, chip)
        };
        let (ov1, chip1) = make_outer_var("State=Default", 0.0);
        let (ov2, chip2) = make_outer_var("State=Active", 60.0);
        let c = combine_as_variants(&mut d, &[ov1, ov2]);
        apply(&mut d, c);
        let outer_set = d.get(ov1).unwrap().parent.unwrap();

        let c = create_instance(&mut d, ov1, page, Vec2::new(300.0, 0.0));
        apply(&mut d, c);
        let outer_inst = *d.get(page).unwrap().children.last().unwrap();

        // While `Status chip` is NOT exposed in `outer_set`, the deeper `Chip icon` (even though exposed in `Chip`) must NOT bubble to `outer_inst`.
        assert!(exposed_nested_instances(&d, outer_inst).is_empty());

        // Exposing `Status chip` on `ov1` syncs across sibling variants of `outer_set` and bubbles both `Status chip` and `Chip icon`.
        let c = set_expose_nested_instance(&mut d, chip1, true);
        apply(&mut d, c);
        assert!(d.get(chip1).unwrap().component.expose_nested_instances);
        assert!(d.get(chip2).unwrap().component.expose_nested_instances, "toggling expose in a variant set updates matching nested instances across variants");

        let cands = expose_nested_candidates(&d, outer_set);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].name, "Status chip");
        assert!(cands[0].exposed);
        assert_eq!(cands[0].deeper_exposed_names, vec!["Chip icon".to_string()]);

        let exposed_in_outer = exposed_nested_instances(&d, outer_inst);
        let chip_in_outer = child_named(&d, outer_inst, "Status chip");
        let icon_in_outer = child_named(&d, chip_in_outer, "Chip icon");
        assert_eq!(exposed_in_outer, vec![chip_in_outer, icon_in_outer]);
    }

    fn expose_badge_variant(d: &mut Document, page: NodeId, name: &str, x: f32, w: f32) -> (NodeId, NodeId, NodeId) {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, 0.0, w, 24.0));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();
        let dot_id = d.alloc_id();
        let mut dot = Node::ellipse(dot_id, Rect::new(4.0, 8.0, 8.0, 8.0));
        dot.name = "Dot".into();
        let dot = d.insert(comp, dot).unwrap();
        let txt_id = d.alloc_id();
        let mut txt = Node::text(txt_id, Rect::new(16.0, 4.0, 40.0, 16.0), "Badge", TextStyle::default());
        txt.name = "Text".into();
        let txt = d.insert(comp, txt).unwrap();
        let c = create_component(d, comp);
        apply(d, c);
        (comp, dot, txt)
    }

    /// A `Badge` set (`Size=Sm`, `Size=Lg`) with a Boolean `Show dot` and a Text `Label`, each bound to a layer of both
    /// variants. Returns the two variants and the set.
    fn expose_badge_set(d: &mut Document, page: NodeId) -> (NodeId, NodeId, NodeId) {
        let (v_sm, dot_sm, txt_sm) = expose_badge_variant(d, page, "Size=Sm", 0.0, 60.0);
        let (v_lg, dot_lg, txt_lg) = expose_badge_variant(d, page, "Size=Lg", 80.0, 90.0);
        let c = combine_as_variants(d, &[v_sm, v_lg]);
        apply(d, c);
        let set = d.get(v_sm).unwrap().parent.unwrap();
        d.get_mut(set).unwrap().name = "Badge".into();
        let c = add_prop(d, set, ComponentPropDef { name: "Show dot".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
        apply(d, c);
        let c = add_prop(d, set, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Badge".into()) });
        apply(d, c);
        for (dot, txt) in [(dot_sm, txt_sm), (dot_lg, txt_lg)] {
            let c = bind_prop(d, dot, OverrideField::Visible, Some("Show dot"));
            apply(d, c);
            let c = bind_prop(d, txt, OverrideField::Characters, Some("Label"));
            apply(d, c);
        }
        (v_sm, v_lg, set)
    }

    /// A main component `name` holding one instance of `of` per entry of `children`, each named as given.
    fn expose_main_holding(d: &mut Document, page: NodeId, name: &str, of: NodeId, children: &[&str]) -> (NodeId, Vec<NodeId>) {
        let id = d.alloc_id();
        let mut main = Node::frame(id, Rect::new(0.0, 100.0, 240.0, 60.0));
        main.name = name.into();
        let main = d.insert(page, main).unwrap();
        let c = create_component(d, main);
        apply(d, c);
        let mut ids = Vec::new();
        for (i, child) in children.iter().enumerate() {
            let c = create_instance(d, of, main, Vec2::new(8.0 + 60.0 * i as f32, 8.0));
            apply(d, c);
            let inst = *d.get(main).unwrap().children.last().unwrap();
            d.get_mut(inst).unwrap().name = (*child).into();
            ids.push(inst);
        }
        (main, ids)
    }

    fn expose_flag(d: &Document, id: NodeId) -> bool {
        d.get(id).unwrap().component.expose_nested_instances
    }

    #[test]
    fn same_named_nested_instances_in_one_component_are_exposed_one_by_one() {
        let mut d = Document::new("expose-same-name");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (card, icons) = expose_main_holding(&mut d, page, "Card", v_sm, &["Icon", "Icon"]);
        let (first, second) = (icons[0], icons[1]);

        let cands = expose_nested_candidates(&d, card);
        assert_eq!(cands.iter().map(|c| c.id).collect::<Vec<_>>(), vec![first, second], "each same-named nested instance is its own row");
        assert!(cands.iter().all(|c| c.connected_ids.len() == 1), "a row sets one nested instance within a single variant");

        let c = set_expose_nested_instance(&mut d, second, true);
        apply(&mut d, c);
        assert!(!expose_flag(&d, first) && expose_flag(&d, second), "exposing the second Icon leaves the first alone");
        let c = set_expose_nested_instance(&mut d, first, true);
        apply(&mut d, c);
        let c = set_expose_nested_instance(&mut d, first, false);
        apply(&mut d, c);
        assert!(!expose_flag(&d, first) && expose_flag(&d, second), "turning the first Icon off keeps the second exposed");

        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let listed = exposed_nested_instances(&d, outer);
        assert_eq!(listed.len(), 1, "the outer instance lists the one exposed Icon only");
        assert_eq!(d.get(listed[0]).unwrap().component.source, Some(second));
    }

    #[test]
    fn one_nested_instance_per_variant_stays_one_row_that_sets_every_variant() {
        let mut d = Document::new("expose-per-variant");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (a, a_kids) = expose_main_holding(&mut d, page, "State=Default", v_sm, &["Icon"]);
        let (b, b_kids) = expose_main_holding(&mut d, page, "State=Active", v_sm, &["Icon"]);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let outer_set = d.get(a).unwrap().parent.unwrap();

        let cands = expose_nested_candidates(&d, outer_set);
        assert_eq!(cands.len(), 1, "the Icon of each variant is one row");
        assert_eq!(cands[0].connected_ids.len(), 2);

        let c = set_expose_nested_instance(&mut d, a_kids[0], true);
        apply(&mut d, c);
        assert!(expose_flag(&d, a_kids[0]) && expose_flag(&d, b_kids[0]), "the row sets the Icon of both variants");
    }

    #[test]
    fn same_named_nested_instances_connect_across_variants_by_their_position_among_same_names() {
        let mut d = Document::new("expose-same-name-variants");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (a, a_kids) = expose_main_holding(&mut d, page, "State=Default", v_sm, &["Icon", "Icon"]);
        let (b, b_kids) = expose_main_holding(&mut d, page, "State=Active", v_sm, &["Icon", "Icon"]);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let outer_set = d.get(a).unwrap().parent.unwrap();

        let cands = expose_nested_candidates(&d, outer_set);
        assert_eq!(cands.len(), 2, "two Icons per variant make two rows");
        let ids = |c: &ExposeNestedCandidate| c.connected_ids.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(ids(&cands[0]), BTreeSet::from([a_kids[0], b_kids[0]]), "the first Icon of each variant is connected");
        assert_eq!(ids(&cands[1]), BTreeSet::from([a_kids[1], b_kids[1]]), "the second Icon of each variant is connected");

        let c = set_expose_nested_instance(&mut d, b_kids[1], true);
        apply(&mut d, c);
        assert!(expose_flag(&d, a_kids[1]) && expose_flag(&d, b_kids[1]));
        assert!(!expose_flag(&d, a_kids[0]) && !expose_flag(&d, b_kids[0]), "the first Icons are not touched");
    }

    #[test]
    fn nested_variant_values_do_not_switch_a_nested_instance_while_expose_is_off() {
        let mut d = Document::new("expose-off-variant-values");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        assert!(!expose_flag(&d, kids[0]));

        // A value an import or an older build left on the nested copy.
        d.get_mut(nested).unwrap().component.values.insert("Size".into(), PropValue::Variant("Lg".into()));
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_sm), "a nested variant value is inert until the nested instance is swapped");
    }

    #[test]
    fn nested_values_are_overrides_only_while_the_nested_instance_is_exposed() {
        let mut d = Document::new("expose-off-reset");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        let c = set_instance_prop(&mut d, nested, "Show dot", Some(PropValue::Bool(false)));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, nested, "Label", Some(PropValue::Text("Pro".into())));
        apply(&mut d, c);
        assert!(!d.get(child_named(&d, nested, "Dot")).unwrap().visible);

        // Expose off: Reset leaves the nested values, as it always did.
        let c = reset_instance(&mut d, outer);
        apply(&mut d, c);
        assert_eq!(d.get(nested).unwrap().component.values.len(), 2, "Reset all changes leaves the values of an unexposed nested instance");
        assert!(!d.get(child_named(&d, nested, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, nested, "Text")), "Pro");

        // Expose on: the same values are overrides and Reset takes them back.
        let c = set_expose_nested_instance(&mut d, kids[0], true);
        apply(&mut d, c);
        let c = reset_instance(&mut d, outer);
        apply(&mut d, c);
        assert!(d.get(nested).unwrap().component.values.is_empty(), "Reset all changes reverts the values of an exposed nested instance");
        assert!(d.get(child_named(&d, nested, "Dot")).unwrap().visible);
        assert_eq!(text_of(&d, child_named(&d, nested, "Text")), "Badge");
    }

    #[test]
    fn deeper_exposed_names_of_a_row_are_the_union_over_its_variants() {
        let mut d = Document::new("expose-deeper-union");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);

        // `Chip` set: variant A exposes `Alpha badge`, variant B exposes `Beta badge`.
        let (chip_a, alpha) = expose_main_holding(&mut d, page, "Kind=A", v_sm, &["Alpha badge"]);
        let (chip_b, beta) = expose_main_holding(&mut d, page, "Kind=B", v_sm, &["Beta badge"]);
        let c = combine_as_variants(&mut d, &[chip_a, chip_b]);
        apply(&mut d, c);
        for nested in [alpha[0], beta[0]] {
            let c = set_expose_nested_instance(&mut d, nested, true);
            apply(&mut d, c);
        }

        // `Panel` set: both variants hold a `Chip` at the same place, one of each kind.
        let (panel_one, _) = expose_main_holding(&mut d, page, "View=One", chip_a, &["Chip"]);
        let (panel_two, _) = expose_main_holding(&mut d, page, "View=Two", chip_b, &["Chip"]);
        let c = combine_as_variants(&mut d, &[panel_one, panel_two]);
        apply(&mut d, c);
        let panel_set = d.get(panel_one).unwrap().parent.unwrap();

        let cands = expose_nested_candidates(&d, panel_set);
        assert_eq!(cands.len(), 1, "the Chip of both variants is one row");
        assert_eq!(cands[0].connected_ids.len(), 2);
        assert_eq!(
            cands[0].deeper_exposed_names,
            vec!["Alpha badge".to_string(), "Beta badge".to_string()],
            "the names below a row come from every variant it connects"
        );
    }

    #[test]
    fn a_variant_edit_on_an_exposed_nested_instance_is_a_swap_that_a_sync_keeps_and_reset_takes_back() {
        let mut d = Document::new("expose-variant-edit");
        let page = d.add_page("P");
        let (v_sm, v_lg, badge_set) = expose_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        let c = set_expose_nested_instance(&mut d, kids[0], true);
        apply(&mut d, c);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        let size_def = prop_defs(&d, v_sm).into_iter().find(|p| p.name == "Size").expect("the Size property");

        // A value an earlier edit or an import left for the variant property would read as the old choice.
        d.get_mut(nested).unwrap().component.values.insert("Size".into(), PropValue::Variant("Sm".into()));
        let edit = set_nested_instance_prop(&mut d, nested, "Size", Some(PropValue::Variant("Lg".into())));
        let undo = apply(&mut d, edit);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_lg), "the nested instance shows the variant that was chosen");
        assert!(d.get(nested).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the switch is recorded as a swap on the nested instance");
        assert!(!d.get(nested).unwrap().component.values.contains_key("Size"), "no stale value of the variant property is kept");
        assert_eq!(prop_value(&d, nested, &size_def), PropValue::Variant("Lg".into()), "the panel reads the chosen variant back");
        assert_eq!(instance_component(d.get(kids[0]).unwrap()), Some(v_sm), "the main component's nested instance is not touched");

        // One undo step takes the edit back; its inverse does it again.
        let redo = apply(&mut d, undo);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_sm), "undo puts the variant before the edit back");
        assert!(d.get(nested).unwrap().component.overrides.is_empty());
        apply(&mut d, redo);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_lg), "redo chooses the variant again");

        // Syncing the outer main, or the nested instance's own set, keeps the switch.
        for owner in [card, badge_set] {
            let c = sync_instances(&mut d, owner);
            apply(&mut d, c);
            assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_lg), "a sync of {owner:?} keeps the switch");
        }

        // Choosing what is already shown, or an option the set has no variant for, changes nothing.
        assert!(is_nothing(&set_nested_instance_prop(&mut d, nested, "Size", Some(PropValue::Variant("Lg".into())))));
        assert!(is_nothing(&set_nested_instance_prop(&mut d, nested, "Size", Some(PropValue::Variant("Xl".into())))));

        // Reset all changes takes the switch back.
        let c = reset_instance(&mut d, outer);
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_sm));
        assert!(d.get(nested).unwrap().component.overrides.is_empty());
    }

    #[test]
    fn a_property_other_than_a_variant_is_stored_on_the_nested_instance_as_a_value() {
        let mut d = Document::new("expose-value-edit");
        let page = d.add_page("P");
        let (v_sm, _, _) = expose_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        let c = set_expose_nested_instance(&mut d, kids[0], true);
        apply(&mut d, c);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");

        let c = set_nested_instance_prop(&mut d, nested, "Show dot", Some(PropValue::Bool(false)));
        apply(&mut d, c);
        assert_eq!(d.get(nested).unwrap().component.values.get("Show dot"), Some(&PropValue::Bool(false)));
        assert!(!d.get(child_named(&d, nested, "Dot")).unwrap().visible);
        assert!(d.get(nested).unwrap().component.overrides.is_empty(), "a value is not a swap");
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);
        assert!(!d.get(child_named(&d, nested, "Dot")).unwrap().visible, "a sync of the outer main keeps the value");
        assert!(!expose_flag(&d, nested), "the outer copy never carries the flag");
    }

    /// The outer instance and the nested badge of an outer instance of a `Card` that holds one exposed `Leading badge`.
    fn expose_outer_with_badge(d: &mut Document, page: NodeId) -> (NodeId, NodeId, NodeId, NodeId) {
        let (v_sm, _, _) = expose_badge_set(d, page);
        let (card, kids) = expose_main_holding(d, page, "Card", v_sm, &["Leading badge"]);
        let c = set_expose_nested_instance(d, kids[0], true);
        apply(d, c);
        let c = create_instance(d, card, page, Vec2::new(0.0, 200.0));
        apply(d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(d, outer, "Leading badge");
        (card, kids[0], outer, nested)
    }

    fn dot_visible(d: &Document, badge: NodeId) -> bool {
        d.get(child_named(d, badge, "Dot")).unwrap().visible
    }

    /// Sets a value on the main-side badge and syncs the card, as an edit of the main does.
    fn set_main_badge_value(d: &mut Document, card: NodeId, badge: NodeId, name: &str, value: PropValue) {
        let c = set_instance_prop(d, badge, name, Some(value));
        apply(d, c);
        let c = sync_instances(d, card);
        apply(d, c);
    }

    fn override_opacity(d: &mut Document, id: NodeId) {
        let n = d.get_mut(id).unwrap();
        n.opacity = 0.5;
        n.component.overrides.insert(OverrideField::Opacity);
    }

    #[test]
    fn a_nested_value_inherited_from_the_main_side_badge_is_not_a_change() {
        let mut d = Document::new("expose-inherited-value");
        let page = d.add_page("P");
        let (card, main_badge, outer, nested) = expose_outer_with_badge(&mut d, page);

        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(false));
        assert!(!dot_visible(&d, nested), "the outer instance's badge follows the main-side badge");
        assert!(d.get(nested).unwrap().component.values.is_empty(), "the badge in the outer instance holds no value of its own");
        assert!(exposed_nested_value_overrides(&d, outer).is_empty(), "a value that is inherited is not a change of the outer instance");
    }

    #[test]
    fn reset_leaves_an_inherited_nested_value_inheriting() {
        let mut d = Document::new("expose-inherited-reset");
        let page = d.add_page("P");
        let (card, main_badge, outer, nested) = expose_outer_with_badge(&mut d, page);
        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(false));

        // Another change of the outer instance, so that Reset has something to take back.
        override_opacity(&mut d, outer);
        let c = reset_instance(&mut d, outer);
        apply(&mut d, c);
        assert_eq!(d.get(outer).unwrap().opacity, 1.0, "Reset took the other change back");
        assert!(d.get(nested).unwrap().component.values.is_empty(), "Reset does not write the main-side value onto the badge");

        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(true));
        assert!(dot_visible(&d, nested), "the badge in the outer instance still follows the main-side badge");
        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(false));
        assert!(!dot_visible(&d, nested));
    }

    #[test]
    fn a_nested_badge_made_after_the_main_side_value_holds_an_equal_map_that_is_no_change() {
        let mut d = Document::new("expose-cloned-value");
        let page = d.add_page("P");
        let (card, main_badge, _, _) = expose_outer_with_badge(&mut d, page);
        let c = set_instance_prop(&mut d, main_badge, "Show dot", Some(PropValue::Bool(false)));
        apply(&mut d, c);

        // An outer instance made now copies the main-side badge, values included.
        let c = create_instance(&mut d, card, page, Vec2::new(300.0, 200.0));
        apply(&mut d, c);
        let later = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, later, "Leading badge");
        let main_values = d.get(main_badge).unwrap().component.values.clone();
        assert!(d.get(nested).unwrap().component.values.is_empty(), "the copy inherits values without storing a snapshot");
        assert!(exposed_nested_value_overrides(&d, later).is_empty(), "an equal map is no change");

        override_opacity(&mut d, later);
        let c = reset_instance(&mut d, later);
        apply(&mut d, c);
        assert!(d.get(nested).unwrap().component.values.is_empty(), "Reset leaves inherited values unset");
        assert_eq!(main_values, d.get(main_badge).unwrap().component.values);
    }

    #[test]
    fn nested_copy_created_after_main_value_change_follows_later_main_changes() {
        let mut d = Document::new("nested-copy-live-values");
        let page = d.add_page("P");
        let (card, main_badge, _, _) = expose_outer_with_badge(&mut d, page);
        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(false));

        // This outer instance is created after the main-side badge changed. Its
        // nested copy must inherit that value, rather than storing a snapshot.
        let c = create_instance(&mut d, card, page, Vec2::new(300.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        assert!(!dot_visible(&d, nested));
        assert!(d.get(nested).unwrap().component.values.is_empty());

        // A later edit on the main-side nested instance must flow through.
        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(true));
        assert!(dot_visible(&d, nested));
    }

    #[test]
    fn reset_takes_back_the_nested_values_that_differ_from_the_main_side_badge_and_the_badge_follows_again() {
        let mut d = Document::new("expose-overriding-value");
        let page = d.add_page("P");
        let (card, main_badge, outer, nested) = expose_outer_with_badge(&mut d, page);
        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(false));

        // The outer instance shows the dot again and has its own label.
        let c = set_instance_prop(&mut d, nested, "Show dot", Some(PropValue::Bool(true)));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, nested, "Label", Some(PropValue::Text("Pro".into())));
        apply(&mut d, c);
        assert!(dot_visible(&d, nested));
        assert_eq!(exposed_nested_value_overrides(&d, outer), vec![nested], "values that differ from the main-side badge are changes");

        let c = reset_instance(&mut d, outer);
        apply(&mut d, c);
        assert!(d.get(nested).unwrap().component.values.is_empty(), "Reset removed the values that differed");
        assert!(!dot_visible(&d, nested), "the badge shows what the main-side badge shows");
        assert_eq!(text_of(&d, child_named(&d, nested, "Text")), "Badge");
        assert!(exposed_nested_value_overrides(&d, outer).is_empty());

        set_main_badge_value(&mut d, card, main_badge, "Show dot", PropValue::Bool(true));
        assert!(dot_visible(&d, nested), "after Reset the badge follows a later change of the main-side badge");
    }

    /// The icon the `Glyph` instance in `badge` shows.
    fn glyph_of(d: &Document, badge: NodeId) -> Option<NodeId> {
        instance_component(d.get(child_named(d, badge, "Glyph")).unwrap())
    }

    /// A `Badge` set (`Size=Sm`, `Size=Lg`) whose variants each hold a `Glyph` instance of `Icon A`, bound to the
    /// instance-swap property `Glyph`. Returns the two variants, the set and the two icons.
    fn expose_glyph_badge_set(d: &mut Document, page: NodeId) -> (NodeId, NodeId, NodeId, NodeId, NodeId) {
        let mut icons = Vec::new();
        for (name, x) in [("Icon A", 200.0), ("Icon B", 240.0)] {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 12.0, 12.0));
            f.name = name.into();
            let icon = d.insert(page, f).unwrap();
            let c = create_component(d, icon);
            apply(d, c);
            icons.push(icon);
        }
        let (icon_a, icon_b) = (icons[0], icons[1]);
        let mut variants = Vec::new();
        for (name, x, w) in [("Size=Sm", 0.0, 60.0), ("Size=Lg", 80.0, 90.0)] {
            let (comp, _, _) = expose_badge_variant(d, page, name, x, w);
            let c = create_instance(d, icon_a, comp, Vec2::new(40.0, 6.0));
            apply(d, c);
            let glyph = *d.get(comp).unwrap().children.last().unwrap();
            d.get_mut(glyph).unwrap().name = "Glyph".into();
            variants.push((comp, glyph));
        }
        let c = combine_as_variants(d, &[variants[0].0, variants[1].0]);
        apply(d, c);
        let set = d.get(variants[0].0).unwrap().parent.unwrap();
        d.get_mut(set).unwrap().name = "Badge".into();
        let c = add_prop(
            d,
            set,
            ComponentPropDef { name: "Glyph".into(), kind: PropKind::InstanceSwap { preferred: vec![icon_a, icon_b] }, default: PropValue::Instance(icon_a) },
        );
        apply(d, c);
        for (_, glyph) in &variants {
            let c = bind_prop(d, *glyph, OverrideField::MainComponent, Some("Glyph"));
            apply(d, c);
        }
        (variants[0].0, variants[1].0, set, icon_a, icon_b)
    }

    #[test]
    fn a_variant_edit_on_an_exposed_nested_instance_applies_the_instance_swap_chosen_before_it_at_once() {
        let mut d = Document::new("expose-variant-after-swap");
        let page = d.add_page("P");
        let (v_sm, v_lg, _, icon_a, icon_b) = expose_glyph_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        let c = set_expose_nested_instance(&mut d, kids[0], true);
        apply(&mut d, c);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        let glyph_def = prop_defs(&d, v_sm).into_iter().find(|p| p.name == "Glyph").expect("the Glyph property");

        let c = set_nested_instance_prop(&mut d, nested, "Glyph", Some(PropValue::Instance(icon_b)));
        apply(&mut d, c);
        assert_eq!(glyph_of(&d, nested), Some(icon_b), "the instance swap shows Icon B");
        assert_eq!(glyph_of(&d, kids[0]), Some(icon_a), "the main-side badge keeps Icon A");

        // One command: the variant changes, and the badge in the new variant shows Icon B at once.
        let c = set_nested_instance_prop(&mut d, nested, "Size", Some(PropValue::Variant("Lg".into())));
        let undo = apply(&mut d, c);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_lg), "the badge is on the Lg variant");
        assert_eq!(glyph_of(&d, nested), Some(icon_b), "the glyph of the new variant shows the chosen icon without another sync");
        assert_eq!(prop_value(&d, nested, &glyph_def), PropValue::Instance(icon_b), "the dropdown shows the same icon");

        // One undo step returns to Sm, still with Icon B.
        apply(&mut d, undo);
        assert_eq!(instance_component(d.get(nested).unwrap()), Some(v_sm), "undo returns to the Sm variant");
        assert_eq!(glyph_of(&d, nested), Some(icon_b), "undo keeps the chosen icon");
        assert_eq!(prop_value(&d, nested, &glyph_def), PropValue::Instance(icon_b));
    }

    /// The `Badge` set of [`expose_glyph_badge_set`] with a Boolean `Show dot` and a Text `Label` bound to the `Dot` and
    /// `Text` of both variants, and a Boolean `Pinned` that no layer is bound to. Returns the two variants, the set and
    /// the two icons.
    fn detach_badge_set(d: &mut Document, page: NodeId) -> (NodeId, NodeId, NodeId, NodeId, NodeId) {
        let (v_sm, v_lg, set, icon_a, icon_b) = expose_glyph_badge_set(d, page);
        for def in [
            ComponentPropDef { name: "Show dot".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) },
            ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Badge".into()) },
            ComponentPropDef { name: "Pinned".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) },
        ] {
            let c = add_prop(d, set, def);
            apply(d, c);
        }
        for variant in [v_sm, v_lg] {
            let dot = child_named(d, variant, "Dot");
            let c = bind_prop(d, dot, OverrideField::Visible, Some("Show dot"));
            apply(d, c);
            let text = child_named(d, variant, "Text");
            let c = bind_prop(d, text, OverrideField::Characters, Some("Label"));
            apply(d, c);
        }
        (v_sm, v_lg, set, icon_a, icon_b)
    }

    /// Sets a boolean, a text and an instance swap on `badge`, which sits in the main `holder`, and syncs the holder.
    fn set_badge_values(d: &mut Document, badge: NodeId, holder: NodeId, icon: NodeId) {
        for (name, value) in [("Show dot", PropValue::Bool(false)), ("Label", PropValue::Text("Pro".into())), ("Glyph", PropValue::Instance(icon))] {
            let c = set_instance_prop(d, badge, name, Some(value));
            apply(d, c);
        }
        let c = sync_instances(d, holder);
        apply(d, c);
    }

    /// What `badge` shows: whether its dot is visible, its label and the icon of its glyph.
    fn badge_shows(d: &Document, badge: NodeId) -> (bool, String, Option<NodeId>) {
        (dot_visible(d, badge), text_of(d, child_named(d, badge, "Text")), glyph_of(d, badge))
    }

    /// The value of the properties `Show dot`, `Label` and `Glyph` of `badge`, as its panel reads them.
    fn badge_panel_values(d: &Document, badge: NodeId) -> Vec<PropValue> {
        let comp = instance_component(d.get(badge).unwrap()).unwrap();
        ["Show dot", "Label", "Glyph"]
            .into_iter()
            .map(|name| {
                let def = prop_defs(d, comp).into_iter().find(|p| p.name == name).expect("the property");
                prop_value(d, badge, &def)
            })
            .collect()
    }

    #[test]
    fn detaching_an_outer_instance_keeps_the_values_its_nested_badge_inherited() {
        let mut d = Document::new("detach-inherited-values");
        let page = d.add_page("P");
        let (v_sm, _, set, _, icon_b) = detach_badge_set(&mut d, page);
        let (card, kids) = expose_main_holding(&mut d, page, "Card", v_sm, &["Leading badge"]);
        set_badge_values(&mut d, kids[0], card, icon_b);
        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, outer, "Leading badge");
        let shown = (false, "Pro".to_string(), Some(icon_b));
        let panel = vec![PropValue::Bool(false), PropValue::Text("Pro".into()), PropValue::Instance(icon_b)];
        assert!(d.get(nested).unwrap().component.values.is_empty(), "the copy inherits the values instead of holding them");
        assert_eq!(badge_shows(&d, nested), shown, "the copy shows what the main-side badge shows");

        let c = detach_instance(&mut d, outer);
        apply(&mut d, c);
        assert!(matches!(d.get(outer).unwrap().kind, NodeKind::Frame(_)));
        assert_eq!(d.get(nested).unwrap().component.source, None, "the badge is an instance of the Badge main on its own");
        assert_eq!(badge_shows(&d, nested), shown, "detaching changes nothing the badge shows");
        assert_eq!(badge_panel_values(&d, nested), panel, "the panel reads the same values after the detach");

        let c = sync_instances(&mut d, set);
        apply(&mut d, c);
        assert_eq!(badge_shows(&d, nested), shown, "a sync of the Badge keeps what the badge showed");
        assert_eq!(badge_panel_values(&d, nested), panel);

        let c = set_instance_prop(&mut d, nested, "Pinned", Some(PropValue::Bool(true)));
        apply(&mut d, c);
        assert_eq!(badge_shows(&d, nested), shown, "an edit of another property keeps the rest");
        assert_eq!(badge_panel_values(&d, nested), panel);
    }

    #[test]
    fn detaching_an_outer_instance_keeps_the_values_of_a_badge_nested_two_levels_down() {
        let mut d = Document::new("detach-inherited-values-deep");
        let page = d.add_page("P");
        let (v_sm, _, set, _, icon_b) = detach_badge_set(&mut d, page);
        let (chip, chip_kids) = expose_main_holding(&mut d, page, "Chip", v_sm, &["Chip badge"]);
        let (card, card_kids) = expose_main_holding(&mut d, page, "Card", chip, &["Status chip"]);
        let badge_in_chip = chip_kids[0];
        let badge_in_card = child_named(&d, card_kids[0], "Chip badge");

        // The Chip's own badge pins `Pinned`; the badge the Card holds in its Chip instance carries the three other values.
        let c = set_instance_prop(&mut d, badge_in_chip, "Pinned", Some(PropValue::Bool(true)));
        apply(&mut d, c);
        set_badge_values(&mut d, badge_in_card, card, icon_b);

        let c = create_instance(&mut d, card, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let outer = *d.get(page).unwrap().children.last().unwrap();
        let chip_copy = child_named(&d, outer, "Status chip");
        let nested = child_named(&d, chip_copy, "Chip badge");
        let shown = (false, "Pro".to_string(), Some(icon_b));
        let panel = vec![PropValue::Bool(false), PropValue::Text("Pro".into()), PropValue::Instance(icon_b)];
        assert_eq!(badge_shows(&d, nested), shown, "the copy shows what the badge in the Card shows");
        assert_eq!(badge_panel_values(&d, nested), panel);

        let c = detach_instance(&mut d, outer);
        apply(&mut d, c);
        assert_eq!(d.get(chip_copy).unwrap().component.source, None, "the Chip instance is an instance of the Chip main on its own");
        assert_eq!(d.get(nested).unwrap().component.source, Some(badge_in_chip), "the badge keeps mirroring the badge of the Chip main");
        assert_eq!(badge_shows(&d, nested), shown, "detaching changes nothing the badge shows");
        assert_eq!(badge_panel_values(&d, nested), panel, "the panel reads the same values after the detach");

        for owner in [set, chip] {
            let c = sync_instances(&mut d, owner);
            apply(&mut d, c);
            assert_eq!(badge_shows(&d, nested), shown, "a sync of {owner:?} keeps what the badge showed");
            assert_eq!(badge_panel_values(&d, nested), panel);
        }

        let c = set_instance_prop(&mut d, nested, "Label", Some(PropValue::Text("Other".into())));
        apply(&mut d, c);
        assert_eq!(badge_shows(&d, nested), (false, "Other".to_string(), Some(icon_b)), "an edit of one property keeps the rest");

        // What the badge of the Chip main still gives it is not copied onto it: it keeps following that badge.
        let pinned = prop_defs(&d, v_sm).into_iter().find(|p| p.name == "Pinned").expect("the Pinned property");
        assert!(!d.get(nested).unwrap().component.values.contains_key("Pinned"));
        assert_eq!(prop_value(&d, nested, &pinned), PropValue::Bool(true));
        let c = set_instance_prop(&mut d, badge_in_chip, "Pinned", Some(PropValue::Bool(false)));
        apply(&mut d, c);
        let c = sync_instances(&mut d, chip);
        apply(&mut d, c);
        assert_eq!(prop_value(&d, nested, &pinned), PropValue::Bool(false));
    }

    fn paint_of(d: &Document, id: NodeId) -> crate::PaintKind {
        d.get(id).unwrap().fills[0].kind.clone()
    }

    fn override_fill(d: &mut Document, id: NodeId, color: Color) {
        let mut n = d.get(id).unwrap().clone();
        n.fills = vec![Paint::solid(color)];
        n.component.overrides.insert(OverrideField::Fills);
        apply(d, Command::Update { node: Box::new(n) });
    }

    fn override_text(d: &mut Document, id: NodeId, s: &str) {
        let mut n = d.get(id).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = s.into();
        }
        n.component.overrides.insert(OverrideField::Characters);
        apply(d, Command::Update { node: Box::new(n) });
    }

    /// A main whose only children are texts with the given layer names.
    fn main_with_texts(d: &mut Document, page: NodeId, name: &str, x: f32, texts: &[&str]) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, 700.0, 100.0, 40.0));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();
        for t in texts {
            let tid = d.alloc_id();
            let mut tn = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 20.0), "Default", TextStyle::default());
            tn.name = (*t).into();
            d.insert(comp, tn).unwrap();
        }
        let c = create_component(d, comp);
        apply(d, c);
        comp
    }

    /// A main holding one filled "Dot" rectangle.
    fn main_with_filled_dot(d: &mut Document, page: NodeId, name: &str, x: f32, fill: Color) -> NodeId {
        let comp = frame_main_with_dot(d, page, name, x, 800.0, 40.0, 20.0);
        let dot = child_named(d, comp, "Dot");
        set_fill(d, dot, fill);
        let c = sync_instances(d, comp);
        apply(d, c);
        comp
    }

    #[test]
    fn inner_swap_moves_with_the_swap_when_the_new_main_places_the_same_original() {
        let mut d = Document::new("InnerRemap");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card2, "B1");
        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let b1 = child_named(&d, ci, "B1");
        let c = swap_instance(&mut d, b1, blue);
        apply(&mut d, c);
        let dot = child_named(&d, b1, "Dot");
        override_fill(&mut d, dot, Color::rgb(0, 255, 0));

        let (c, dropped) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);

        let nb1 = child_named(&d, ci, "B1");
        assert_eq!(instance_component(d.get(nb1).unwrap()), Some(blue), "the inner swap follows the outer swap");
        assert!(d.get(nb1).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the carried swap stays a recorded override");
        assert_eq!(
            paint_of(&d, child_named(&d, nb1, "Dot")),
            crate::PaintKind::Solid(Color::rgb(0, 255, 0)),
            "the override below the inner swap lands on the swapped content"
        );
        assert_eq!(dropped, 0, "nothing was left behind");
    }

    #[test]
    fn inner_swap_follows_the_swap_even_when_the_new_main_places_another_original() {
        let mut d = Document::new("InnerOtherOriginal");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let green = main_with_filled_dot(&mut d, page, "Badge/Green", 120.0, Color::rgb(0, 200, 0));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        let card3 = frame_main_with_dot(&mut d, page, "Card3", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, green, card3, "B1");
        let c = create_instance(&mut d, card, page, Vec2::new(600.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let b1 = child_named(&d, ci, "B1");
        let c = swap_instance(&mut d, b1, blue);
        apply(&mut d, c);
        let (c, dropped) = swap_instance_inner(&mut d, ci, card3);
        apply(&mut d, c);
        assert_eq!(instance_component(d.get(child_named(&d, ci, "B1")).unwrap()), Some(blue));
        assert_eq!(dropped, 0);
    }

    #[test]
    fn inner_swap_stays_behind_and_is_counted_when_the_new_main_has_no_counterpart() {
        let mut d = Document::new("InnerNoMatch");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        let empty = frame_main_with_dot(&mut d, page, "Empty", 400.0, 0.0, 100.0, 100.0);
        let again = frame_main_with_dot(&mut d, page, "Again", 600.0, 0.0, 100.0, 100.0);
        let c = create_instance(&mut d, card, page, Vec2::new(800.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let b1 = child_named(&d, ci, "B1");
        let c = swap_instance(&mut d, b1, blue);
        apply(&mut d, c);

        let (c, dropped) = swap_instance_inner(&mut d, ci, empty);
        apply(&mut d, c);
        assert!(d.get(ci).unwrap().children.iter().all(|k| d.get(*k).unwrap().name != "B1"), "the new main has no B1");
        assert_eq!(dropped, 1, "the swap that could not move is counted");

        let (c, dropped) = swap_instance_inner(&mut d, ci, again);
        apply(&mut d, c);
        assert_eq!(dropped, 0, "an instance that already lost its swap has nothing left to drop");
    }

    #[test]
    fn single_text_rule_moves_a_text_override_between_differently_named_layers() {
        let mut d = Document::new("SingleText");
        let page = d.add_page("P");
        let a = main_with_texts(&mut d, page, "Card/A", 0.0, &["Label"]);
        let b = main_with_texts(&mut d, page, "Card/B", 200.0, &["Title"]);
        let two = main_with_texts(&mut d, page, "Card/Two", 400.0, &["Title", "Sub"]);
        for (target, expect) in [(b, "Hello"), (two, "Default")] {
            let c = create_instance(&mut d, a, page, Vec2::new(0.0, 900.0));
            apply(&mut d, c);
            let inst = *d.get(page).unwrap().children.last().unwrap();
            let lbl = child_named(&d, inst, "Label");
            override_text(&mut d, lbl, "Hello");
            let c = swap_instance(&mut d, inst, target);
            apply(&mut d, c);
            let first_text = d.get(inst).unwrap().children[0];
            assert_eq!(text_of(&d, first_text), expect, "single text rule for {target:?}");
        }
    }

    #[test]
    fn single_text_rule_does_not_move_a_text_driven_by_a_component_property() {
        let mut d = Document::new("HoistedText");
        let page = d.add_page("P");
        let a = main_with_texts(&mut d, page, "Card/A", 0.0, &["Label"]);
        let b = main_with_texts(&mut d, page, "Card/B", 200.0, &["Title"]);
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 900.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let label = child_named(&d, inst, "Label");
        override_text(&mut d, label, "Hello");
        let mut ln = d.get(label).unwrap().clone();
        ln.component.bindings.insert(OverrideField::Characters, "Label".into());
        apply(&mut d, Command::Update { node: Box::new(ln) });
        let c = swap_instance(&mut d, inst, b);
        apply(&mut d, c);
        assert_eq!(text_of(&d, d.get(inst).unwrap().children[0]), "Default");
    }

    #[test]
    fn single_text_rule_applies_on_a_variant_switch_too() {
        let mut d = Document::new("VariantText");
        let page = d.add_page("P");
        let a = main_with_texts(&mut d, page, "Size=A", 0.0, &["Label"]);
        let b = main_with_texts(&mut d, page, "Size=B", 200.0, &["Title"]);
        let c_var = main_with_texts(&mut d, page, "Size=C", 400.0, &["Title"]);
        {
            let title_c = child_named(&d, c_var, "Title");
            let mut tn = d.get(title_c).unwrap().clone();
            if let NodeKind::Text(tp) = &mut tn.kind {
                tp.content = "Saved".into();
            }
            apply(&mut d, Command::Update { node: Box::new(tn) });
        }
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);
        for (value, target, expect) in [("B", b, "Hello"), ("C", c_var, "Saved")] {
            let c = create_instance(&mut d, a, page, Vec2::new(0.0, 900.0));
            apply(&mut d, c);
            let inst = *d.get(page).unwrap().children.last().unwrap();
            let lbl = child_named(&d, inst, "Label");
            override_text(&mut d, lbl, "Hello");
            let c = set_instance_prop(&mut d, inst, "Size", Some(PropValue::Variant(value.into())));
            apply(&mut d, c);
            assert_eq!(instance_component(d.get(inst).unwrap()), Some(target));
            assert_eq!(text_of(&d, d.get(inst).unwrap().children[0]), expect, "switch to {value}");
        }
    }

    #[test]
    fn variant_switch_reports_override_on_child_missing_from_other_variant() {
        let mut d = Document::new("MissingChildOverride");
        let page = d.add_page("P");
        let a = main_with_texts(&mut d, page, "Card/A", 0.0, &["Title", "Subtitle"]);
        let b = main_with_texts(&mut d, page, "Card/B", 200.0, &["Title"]);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 900.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let subtitle = child_named(&d, inst, "Subtitle");
        override_text(&mut d, subtitle, "Custom");
        let (c, dropped) = swap_instance_inner(&mut d, inst, b);
        let undo = apply(&mut d, c);
        assert_eq!(dropped, 0, "the missing Subtitle Characters override is parked rather than reported as lost");
        let (c_back, dropped_back) = swap_instance_inner(&mut d, inst, a);
        let undo_back = apply(&mut d, c_back);
        assert_eq!(dropped_back, 0);
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "Custom", "switching back to Card/A restores the parked Subtitle override");
        apply(&mut d, undo_back);
        apply(&mut d, undo);
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "Custom", "undoing the variant switches restores the Subtitle override");

        // Count field overrides that truly cannot return: dropped by the transfer gate (e.g. Fills when variants' default fills differ)
        // or when the counterpart layer at the same name path has an incompatible kind.
        let red = main_with_filled_dot(&mut d, page, "State=Red", 400.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "State=Blue", 600.0, Color::rgb(0, 0, 255));
        let c = combine_as_variants(&mut d, &[red, blue]);
        apply(&mut d, c);
        let c = create_instance(&mut d, red, page, Vec2::new(400.0, 900.0));
        apply(&mut d, c);
        let fill_inst = *d.get(page).unwrap().children.last().unwrap();
        let dot = child_named(&d, fill_inst, "Dot");
        override_fill(&mut d, dot, Color::rgb(0, 255, 0));
        let (c, gated_dropped) = swap_instance_inner(&mut d, fill_inst, blue);
        let undo_fill = apply(&mut d, c);
        assert_eq!(gated_dropped, 1, "a fill override dropped by the variant transfer gate must be counted");
        apply(&mut d, undo_fill);
        assert_eq!(paint_of(&d, child_named(&d, fill_inst, "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)), "undo restores the gated fill override");
    }

    #[test]
    fn variant_switch_resets_single_text_override_when_variants_originally_differed() {
        let mut d = Document::new("VariantSingleTextGate");
        let page = d.add_page("P");
        let mk_variant = |d: &mut Document, name: &str, x: f32, layer_name: &str, text: &str, font_size: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 100.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let style = TextStyle { font_size, ..TextStyle::default() };
            let mut tn = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 20.0), text, style);
            tn.name = layer_name.into();
            d.insert(comp, tn).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let v_default = mk_variant(&mut d, "State=Default", 0.0, "Label", "Save", 14.0);
        let v_done = mk_variant(&mut d, "State=Done", 120.0, "Title", "Saved", 14.0);
        let v_loading = mk_variant(&mut d, "State=Loading", 240.0, "Heading", "Save", 16.0);
        let c = combine_as_variants(&mut d, &[v_default, v_done, v_loading]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v_default, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        {
            let lbl = child_named(&d, inst, "Label");
            let mut ln = d.get(lbl).unwrap().clone();
            if let NodeKind::Text(tp) = &mut ln.kind {
                tp.content = "Submit".into();
                tp.style.font_size = 20.0;
            }
            ln.component.overrides.insert(OverrideField::Characters);
            ln.component.overrides.insert(OverrideField::TextStyle);
            apply(&mut d, Command::Update { node: Box::new(ln) });
        }

        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("Done".into())));
        apply(&mut d, c);
        let done_title = child_named(&d, inst, "Title");
        assert_eq!(text_of(&d, done_title), "Saved", "switching to Done resets single-text override Submit to Saved because Save != Saved");
        if let NodeKind::Text(tp) = &d.get(done_title).unwrap().kind {
            assert_eq!(tp.style.font_size, 20.0, "matching 14px original font_size keeps user 20px override across single-text fallback");
        }

        let c = create_instance(&mut d, v_default, page, Vec2::new(200.0, 200.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        {
            let lbl = child_named(&d, inst2, "Label");
            let mut ln = d.get(lbl).unwrap().clone();
            if let NodeKind::Text(tp) = &mut ln.kind {
                tp.content = "Submit".into();
                tp.style.font_size = 20.0;
            }
            ln.component.overrides.insert(OverrideField::Characters);
            ln.component.overrides.insert(OverrideField::TextStyle);
            apply(&mut d, Command::Update { node: Box::new(ln) });
        }

        let c = set_instance_prop(&mut d, inst2, "State", Some(PropValue::Variant("Loading".into())));
        apply(&mut d, c);
        let loading_heading = child_named(&d, inst2, "Heading");
        assert_eq!(text_of(&d, loading_heading), "Submit", "matching Save original text keeps user text Submit across single-text fallback");
        if let NodeKind::Text(tp) = &d.get(loading_heading).unwrap().kind {
            assert_eq!(tp.style.font_size, 16.0, "differing 14px vs 16px original font_size resets to Loading's 16px across single-text fallback");
        }
    }

    #[test]
    fn variant_switch_keeps_a_changed_field_only_when_the_variants_originally_matched() {
        let mut d = Document::new("VariantGate");
        let page = d.add_page("P");
        let white = Color::rgb(255, 255, 255);
        let a = main_with_filled_dot(&mut d, page, "State=Default", 0.0, white);
        let same = main_with_filled_dot(&mut d, page, "State=Focus", 100.0, white);
        let different = main_with_filled_dot(&mut d, page, "State=Hover", 200.0, Color::rgb(0, 0, 0));
        let c = combine_as_variants(&mut d, &[a, same, different]);
        apply(&mut d, c);
        for (value, expect) in [("Focus", Color::rgb(255, 0, 0)), ("Hover", Color::rgb(0, 0, 0))] {
            let c = create_instance(&mut d, a, page, Vec2::new(0.0, 900.0));
            apply(&mut d, c);
            let inst = *d.get(page).unwrap().children.last().unwrap();
            let dot = child_named(&d, inst, "Dot");
            override_fill(&mut d, dot, Color::rgb(255, 0, 0));
            let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant(value.into())));
            apply(&mut d, c);
            assert_eq!(paint_of(&d, child_named(&d, inst, "Dot")), crate::PaintKind::Solid(expect), "switch to {value}");
        }
    }

    #[test]
    fn duplicate_sibling_instances_transfer_swap_and_overrides_to_matching_occurrence_only() {
        let mut d = Document::new("DuplicateSiblingIcons");
        let page = d.add_page("P");
        let icon_a = main_with_filled_dot(&mut d, page, "Icon/A", 0.0, Color::rgb(255, 0, 0));
        let icon_b = main_with_filled_dot(&mut d, page, "Icon/B", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, icon_a, card, "Icon");
        put_named_instance(&mut d, icon_a, card, "Icon");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, icon_a, card2, "Icon");
        put_named_instance(&mut d, icon_a, card2, "Icon");

        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let icons: Vec<NodeId> = d.get(ci).unwrap().children.iter().copied().filter(|&k| d.get(k).unwrap().name == "Icon").collect();
        assert_eq!(icons.len(), 2);

        let c = swap_instance(&mut d, icons[1], icon_b);
        apply(&mut d, c);
        let dot2 = child_named(&d, icons[1], "Dot");
        override_fill(&mut d, dot2, Color::rgb(0, 255, 0));

        let (c, dropped) = swap_instance_inner(&mut d, ci, card2);
        let undo = apply(&mut d, c);
        assert_eq!(dropped, 0);

        let new_icons: Vec<NodeId> = d.get(ci).unwrap().children.iter().copied().filter(|&k| d.get(k).unwrap().name == "Icon").collect();
        assert_eq!(new_icons.len(), 2);
        assert_eq!(instance_component(d.get(new_icons[0]).unwrap()), Some(icon_a), "first Icon keeps the default component");
        assert_eq!(instance_component(d.get(new_icons[1]).unwrap()), Some(icon_b), "only the second Icon shows Icon/B");
        assert_eq!(paint_of(&d, child_named(&d, new_icons[0], "Dot")), crate::PaintKind::Solid(Color::rgb(255, 0, 0)));
        assert_eq!(paint_of(&d, child_named(&d, new_icons[1], "Dot")), crate::PaintKind::Solid(Color::rgb(0, 255, 0)));

        apply(&mut d, undo);
        let restored_icons: Vec<NodeId> = d.get(ci).unwrap().children.iter().copied().filter(|&k| d.get(k).unwrap().name == "Icon").collect();
        assert_eq!(instance_component(d.get(restored_icons[0]).unwrap()), Some(icon_a));
        assert_eq!(instance_component(d.get(restored_icons[1]).unwrap()), Some(icon_b));
    }

    #[test]
    fn renamed_descendant_under_inner_swap_keeps_fill_after_outer_swap() {
        let mut d = Document::new("RenamedDescendantInnerSwap");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card, "B1");
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        put_named_instance(&mut d, red, card2, "B1");

        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let b1 = child_named(&d, ci, "B1");
        let c = swap_instance(&mut d, b1, blue);
        apply(&mut d, c);

        let dot = child_named(&d, b1, "Dot");
        let mut dn = d.get(dot).unwrap().clone();
        dn.name = "Mark".into();
        dn.fills = vec![Paint::solid(Color::rgb(0, 255, 0))];
        dn.component.overrides.insert(OverrideField::Name);
        dn.component.overrides.insert(OverrideField::Fills);
        apply(&mut d, Command::Update { node: Box::new(dn) });

        let (c, dropped) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);
        assert_eq!(dropped, 0);

        let nb1 = child_named(&d, ci, "B1");
        assert_eq!(instance_component(d.get(nb1).unwrap()), Some(blue));
        let mark = child_named(&d, nb1, "Mark");
        assert_eq!(
            paint_of(&d, mark),
            crate::PaintKind::Solid(Color::rgb(0, 255, 0)),
            "renamed descendant Dot -> Mark under inner swap keeps its fill after outer swap"
        );
    }

    #[test]
    fn auto_named_nested_instance_keeps_descendant_fill_after_outer_swap() {
        let mut d = Document::new("AutoNamedInnerSwap");
        let page = d.add_page("P");
        let red = main_with_filled_dot(&mut d, page, "Badge/Red", 0.0, Color::rgb(255, 0, 0));
        let blue = main_with_filled_dot(&mut d, page, "Badge/Blue", 60.0, Color::rgb(0, 0, 255));
        let card = frame_main_with_dot(&mut d, page, "Card", 0.0, 0.0, 100.0, 100.0);
        let c = create_instance(&mut d, red, card, Vec2::new(0.0, 0.0));
        apply(&mut d, c);
        let card2 = frame_main_with_dot(&mut d, page, "Card2", 200.0, 0.0, 100.0, 100.0);
        let c = create_instance(&mut d, red, card2, Vec2::new(0.0, 0.0));
        apply(&mut d, c);

        let c = create_instance(&mut d, card, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let nested = child_named(&d, ci, "Badge/Red");
        let c = swap_instance(&mut d, nested, blue);
        apply(&mut d, c);
        assert_eq!(d.get(nested).unwrap().name, "Badge/Blue", "inner swap auto-renames the un-overridden nested instance");
        let dot = child_named(&d, nested, "Dot");
        override_fill(&mut d, dot, Color::rgb(0, 255, 0));

        let (c, dropped) = swap_instance_inner(&mut d, ci, card2);
        apply(&mut d, c);
        assert_eq!(dropped, 0);

        let new_nested = d.get(ci).unwrap().children.iter().copied().find(|&k| matches!(d.get(k).unwrap().kind, NodeKind::Instance { .. })).unwrap();
        assert_eq!(instance_component(d.get(new_nested).unwrap()), Some(blue));
        assert_eq!(
            paint_of(&d, child_named(&d, new_nested, "Dot")),
            crate::PaintKind::Solid(Color::rgb(0, 255, 0)),
            "auto-named nested instance keeps its child fill after outer swap"
        );
    }

    #[test]
    fn single_text_rule_ignores_text_inside_nested_instances_when_neither_main_has_own_text() {
        let mut d = Document::new("SingleTextNestedOnly");
        let page = d.add_page("P");
        let btn_a = main_with_texts(&mut d, page, "Btn/A", 0.0, &["Label"]);
        let btn_b = main_with_texts(&mut d, page, "Btn/B", 120.0, &["Title"]);

        let mk_card = |d: &mut Document, name: &str, x: f32, btn: NodeId, slot_name: &str| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 100.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            put_named_instance(d, btn, comp, slot_name);
            comp
        };
        let card_a = mk_card(&mut d, "Card/A", 0.0, btn_a, "BtnA");
        let card_b = mk_card(&mut d, "Card/B", 200.0, btn_b, "BtnB");

        let c = create_instance(&mut d, card_a, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let nested_a = child_named(&d, inst, "BtnA");
        let lbl = child_named(&d, nested_a, "Label");
        override_text(&mut d, lbl, "Custom");

        let c = swap_instance(&mut d, inst, card_b);
        apply(&mut d, c);
        let nested_b = child_named(&d, inst, "BtnB");
        let title = child_named(&d, nested_b, "Title");
        assert_eq!(text_of(&d, title), "Default", "text inside a nested instance must not transfer via the outer single-text rule");
    }

    #[test]
    fn single_text_rule_transfers_own_text_even_when_nested_instance_has_text_and_skips_non_text_path_match() {
        let mut d = Document::new("SingleTextWithNestedAndNonTextMatch");
        let page = d.add_page("P");
        let badge = main_with_texts(&mut d, page, "Badge", 0.0, &["Tag"]);
        let card_a = main_with_texts(&mut d, page, "Card/A", 120.0, &["Label"]);
        put_named_instance(&mut d, badge, card_a, "Badge");
        let card_b = main_with_texts(&mut d, page, "Card/B", 260.0, &["Title"]);
        put_named_instance(&mut d, badge, card_b, "Badge");

        let c = create_instance(&mut d, card_a, page, Vec2::new(400.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let lbl = child_named(&d, inst, "Label");
        override_text(&mut d, lbl, "Hello");

        let c = swap_instance(&mut d, inst, card_b);
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, inst, "Title")), "Hello", "nested instance text must not block the component's own single text transfer");

        // When the old text's name path matches a non-text node in the target main, do not fall back to the target's lone text.
        let card_c = {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(400.0, 700.0, 100.0, 40.0));
            f.name = "Card/C".into();
            let comp = d.insert(page, f).unwrap();
            let fid = d.alloc_id();
            let mut non_text = Node::frame(fid, Rect::new(0.0, 0.0, 20.0, 20.0));
            non_text.name = "Label".into();
            d.insert(comp, non_text).unwrap();
            let tid = d.alloc_id();
            let mut lone_text = Node::text(tid, Rect::new(24.0, 0.0, 60.0, 20.0), "Default", TextStyle::default());
            lone_text.name = "Other".into();
            d.insert(comp, lone_text).unwrap();
            let c = create_component(&mut d, comp);
            apply(&mut d, c);
            comp
        };
        let c = create_instance(&mut d, card_a, page, Vec2::new(600.0, 0.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        let lbl2 = child_named(&d, inst2, "Label");
        override_text(&mut d, lbl2, "Hello");
        let c = swap_instance(&mut d, inst2, card_c);
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, inst2, "Other")), "Default", "name-path match to a non-text must not fall back to the lone text");
    }

    #[test]
    fn variant_switch_transfers_matching_text_and_gates_style_when_variants_differ() {
        let mut d = Document::new("VariantTextGate");
        let page = d.add_page("P");
        let mk_variant = |d: &mut Document, name: &str, x: f32, label_text: &str, font_size: f32| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 100.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let style = TextStyle { font_size, ..TextStyle::default() };
            let mut tn = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 20.0), label_text, style);
            tn.name = "Label".into();
            d.insert(comp, tn).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let v_default = mk_variant(&mut d, "State=Default", 0.0, "Save", 14.0);
        let v_done = mk_variant(&mut d, "State=Done", 120.0, "Saved", 14.0);
        let v_loading = mk_variant(&mut d, "State=Loading", 240.0, "Save", 16.0);
        let c = combine_as_variants(&mut d, &[v_default, v_done, v_loading]);
        apply(&mut d, c);

        // Switching Default ("Save", 14px) -> Done ("Saved", 14px): same-name user text survives, while 20px style override stays.
        let c = create_instance(&mut d, v_default, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        {
            let lbl = child_named(&d, inst, "Label");
            let mut ln = d.get(lbl).unwrap().clone();
            if let NodeKind::Text(tp) = &mut ln.kind {
                tp.content = "Submit".into();
                tp.style.font_size = 20.0;
            }
            ln.component.overrides.insert(OverrideField::Characters);
            ln.component.overrides.insert(OverrideField::TextStyle);
            apply(&mut d, Command::Update { node: Box::new(ln) });
        }
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("Done".into())));
        apply(&mut d, c);
        let done_lbl = child_named(&d, inst, "Label");
        assert_eq!(text_of(&d, done_lbl), "Submit", "same-name text override survives differing variant defaults");
        if let NodeKind::Text(tp) = &d.get(done_lbl).unwrap().kind {
            assert_eq!(tp.style.font_size, 20.0, "matching 14px original font_size keeps user 20px override");
        }

        // Switching Default ("Save", 14px) -> Loading ("Save", 16px): user text "Submit" stays, while 14px/20px style resets to 16px.
        let c = create_instance(&mut d, v_default, page, Vec2::new(200.0, 200.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        {
            let lbl = child_named(&d, inst2, "Label");
            let mut ln = d.get(lbl).unwrap().clone();
            if let NodeKind::Text(tp) = &mut ln.kind {
                tp.content = "Submit".into();
                tp.style.font_size = 20.0;
            }
            ln.component.overrides.insert(OverrideField::Characters);
            ln.component.overrides.insert(OverrideField::TextStyle);
            apply(&mut d, Command::Update { node: Box::new(ln) });
        }
        let c = set_instance_prop(&mut d, inst2, "State", Some(PropValue::Variant("Loading".into())));
        apply(&mut d, c);
        let loading_lbl = child_named(&d, inst2, "Label");
        assert_eq!(text_of(&d, loading_lbl), "Submit", "matching Save original text keeps user text Submit");
        if let NodeKind::Text(tp) = &d.get(loading_lbl).unwrap().kind {
            assert_eq!(tp.style.font_size, 16.0, "differing 14px vs 16px original font_size resets to Loading's 16px");
        }

        // Property-bound text exclusion applies on same-name path matches too.
        let card_a = main_with_texts(&mut d, page, "Card/A", 0.0, &["Label"]);
        let card_b = main_with_texts(&mut d, page, "Card/B", 200.0, &["Label"]);
        let c = create_instance(&mut d, card_a, page, Vec2::new(400.0, 200.0));
        apply(&mut d, c);
        let bound_inst = *d.get(page).unwrap().children.last().unwrap();
        let bound_lbl = child_named(&d, bound_inst, "Label");
        override_text(&mut d, bound_lbl, "Hoisted");
        let mut bln = d.get(bound_lbl).unwrap().clone();
        bln.component.bindings.insert(OverrideField::Characters, "Label".into());
        apply(&mut d, Command::Update { node: Box::new(bln) });
        let c = swap_instance(&mut d, bound_inst, card_b);
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, bound_inst, "Label")), "Default", "bound text does not transfer on same-named path match");
    }

    #[test]
    fn variant_switch_compares_resolved_fills_ignoring_style_refs_and_layout_subvalues() {
        let mut d = Document::new("VariantSubvalues");
        let page = d.add_page("P");
        let white = Color::rgb(255, 255, 255);
        let mk = |d: &mut Document, name: &str, x: f32, style_ref: Option<&str>, clips: bool| -> NodeId {
            let comp = main_with_filled_dot(d, page, name, x, white);
            let dot = child_named(d, comp, "Dot");
            if let Some(sr) = style_ref {
                let mut dn = d.get(dot).unwrap().clone();
                dn.style_refs.insert("fill".into(), sr.into());
                apply(d, Command::Update { node: Box::new(dn) });
            }
            let bid = d.alloc_id();
            let mut box_node = Node::frame(bid, Rect::new(0.0, 0.0, 40.0, 20.0));
            box_node.name = "Box".into();
            if let NodeKind::Frame(fp) = &mut box_node.kind {
                fp.clips_content = clips;
                fp.auto_layout = Some(AutoLayout { padding: [8.0, 8.0, 8.0, 8.0], item_spacing: 4.0, ..AutoLayout::default() });
            }
            d.insert(comp, box_node).unwrap();
            comp
        };
        let v_a = mk(&mut d, "State=Default", 0.0, Some("S:fill_a"), false);
        let v_b = mk(&mut d, "State=Hover", 120.0, Some("S:fill_b"), true);
        let c = combine_as_variants(&mut d, &[v_a, v_b]);
        apply(&mut d, c);

        let c = create_instance(&mut d, v_a, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let dot = child_named(&d, inst, "Dot");
        override_fill(&mut d, dot, Color::rgb(255, 0, 0));

        let box_id = child_named(&d, inst, "Box");
        let mut bn = d.get(box_id).unwrap().clone();
        if let NodeKind::Frame(fp) = &mut bn.kind {
            if let Some(al) = &mut fp.auto_layout {
                al.padding = [16.0, 16.0, 16.0, 16.0];
            }
        }
        bn.component.overrides.insert(OverrideField::Layout);
        apply(&mut d, Command::Update { node: Box::new(bn) });

        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("Hover".into())));
        apply(&mut d, c);

        assert_eq!(
            paint_of(&d, child_named(&d, inst, "Dot")),
            crate::PaintKind::Solid(Color::rgb(255, 0, 0)),
            "equal resolved fill with different style_refs keeps the user's red fill"
        );
        let box_after = d.get(child_named(&d, inst, "Box")).unwrap();
        let fp_after = box_after.kind.frame_props().unwrap();
        assert_eq!(fp_after.auto_layout.as_ref().unwrap().padding, [16.0, 16.0, 16.0, 16.0], "equal original padding keeps user 16px padding override");
        assert!(fp_after.clips_content, "differing clips_content takes Hover's clips_content = true");
    }

    fn main_with_constrained_rect(d: &mut Document, page: NodeId, horizontal: Constraint) -> (NodeId, NodeId, NodeId) {
        let comp_id = d.alloc_id();
        let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 100.0, 100.0));
        comp.name = "Main".into();
        comp.fills = vec![Paint::solid(Color::rgb(20, 30, 40))];
        let comp = d.insert(page, comp).unwrap();
        let rect_id = d.alloc_id();
        let mut rect = Node::rectangle(rect_id, Rect::new(10.0, 10.0, 80.0, 80.0));
        rect.name = "Bar".into();
        rect.constraints = Constraints { horizontal, vertical: Constraint::Min };
        rect.fills = vec![Paint::solid(Color::rgb(60, 120, 220))];
        d.insert(comp, rect).unwrap();
        let c = create_component(d, comp);
        apply(d, c);

        let c = create_instance(d, comp, page, Vec2::new(200.0, 0.0));
        apply(d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let mut inst_node = d.get(inst).unwrap().clone();
        inst_node.size = Vec2::new(300.0, 100.0);
        inst_node.component.overrides.insert(OverrideField::Size);
        apply(d, Command::Update { node: Box::new(inst_node) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(d, inst, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 64) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(d, Command::Update { node: Box::new(k) });
        }
        let inst_rect = d.get(inst).unwrap().children[0];
        (comp, inst, inst_rect)
    }

    #[test]
    fn resized_instance_keeps_stretch_child_width_after_main_sync() {
        let mut d = Document::new("stretch-child-sync");
        let page = d.add_page("P");
        let (comp, inst, inst_rect) = main_with_constrained_rect(&mut d, page, Constraint::Stretch);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 280.0);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 10.0);

        let main_rect = d.get(comp).unwrap().children[0];
        let mut mr = d.get(main_rect).unwrap().clone();
        mr.fills = vec![Paint::solid(Color::rgb(220, 80, 60))];
        apply(&mut d, Command::Update { node: Box::new(mr) });

        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        assert_eq!(d.get(inst).unwrap().size.x, 300.0);
        assert_eq!(d.get(inst_rect).unwrap().fills, vec![Paint::solid(Color::rgb(220, 80, 60))]);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 10.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 280.0);
    }

    #[test]
    fn resized_instance_keeps_scale_child_geometry_after_main_sync() {
        let mut d = Document::new("scale-child-sync");
        let page = d.add_page("P");
        let (comp, inst, inst_rect) = main_with_constrained_rect(&mut d, page, Constraint::Scale);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 30.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 240.0);

        let main_rect = d.get(comp).unwrap().children[0];
        let mut mr = d.get(main_rect).unwrap().clone();
        mr.fills = vec![Paint::solid(Color::rgb(80, 200, 120))];
        apply(&mut d, Command::Update { node: Box::new(mr) });

        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        assert_eq!(d.get(inst).unwrap().size.x, 300.0);
        assert_eq!(d.get(inst_rect).unwrap().fills, vec![Paint::solid(Color::rgb(80, 200, 120))]);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 30.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 240.0);
    }

    #[test]
    fn resized_instance_keeps_max_pinned_child_position_after_main_sync() {
        let mut d = Document::new("max-child-sync");
        let page = d.add_page("P");
        let (comp, inst, inst_rect) = main_with_constrained_rect(&mut d, page, Constraint::Max);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 210.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 80.0);

        let main_rect = d.get(comp).unwrap().children[0];
        let mut mr = d.get(main_rect).unwrap().clone();
        mr.fills = vec![Paint::solid(Color::rgb(240, 180, 40))];
        apply(&mut d, Command::Update { node: Box::new(mr) });

        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        assert_eq!(d.get(inst).unwrap().size.x, 300.0);
        assert_eq!(d.get(inst_rect).unwrap().fills, vec![Paint::solid(Color::rgb(240, 180, 40))]);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 210.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 80.0);
    }

    #[test]
    fn resized_instance_keeps_centered_child_position_after_main_sync() {
        let mut d = Document::new("center-child-sync");
        let page = d.add_page("P");
        let (comp, inst, inst_rect) = main_with_constrained_rect(&mut d, page, Constraint::Center);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 110.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 80.0);

        let main_rect = d.get(comp).unwrap().children[0];
        let mut mr = d.get(main_rect).unwrap().clone();
        mr.fills = vec![Paint::solid(Color::rgb(40, 190, 210))];
        apply(&mut d, Command::Update { node: Box::new(mr) });

        let c = sync_instances(&mut d, comp);
        apply(&mut d, c);

        assert_eq!(d.get(inst).unwrap().size.x, 300.0);
        assert_eq!(d.get(inst_rect).unwrap().fills, vec![Paint::solid(Color::rgb(40, 190, 210))]);
        assert_eq!(d.get(inst_rect).unwrap().transform.tx, 110.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 80.0);
    }

    #[test]
    fn nested_instance_inside_plain_frame_of_resized_instance_keeps_stretched_width_after_main_sync() {
        let mut d = Document::new("nested-in-plain-frame-sync");
        let page = d.add_page("P");
        let btn = mkbtn_stretch(&mut d, page, "Btn", 0.0, 40.0);

        let card_id = d.alloc_id();
        let mut card_node = Node::frame(card_id, Rect::new(0.0, 100.0, 100.0, 100.0));
        card_node.name = "Card".into();
        let card = d.insert(page, card_node).unwrap();
        let c = create_component(&mut d, card);
        apply(&mut d, c);

        let wrap_id = d.alloc_id();
        let mut wrap = Node::frame(wrap_id, Rect::new(10.0, 10.0, 80.0, 80.0));
        wrap.name = "Wrap".into();
        wrap.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        d.insert(card, wrap).unwrap();

        let c = create_instance(&mut d, btn, wrap_id, Vec2::new(10.0, 5.0));
        apply(&mut d, c);
        let slot = d.get(wrap_id).unwrap().children[0];
        let mut sn = d.get(slot).unwrap().clone();
        sn.size = Vec2::new(60.0, 40.0);
        sn.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        sn.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(sn) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, slot, Vec2::new(80.0, 40.0), Vec2::new(60.0, 40.0), 64) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(&mut d, Command::Update { node: Box::new(k) });
        }

        // Also include an auto-layout row inside the plain frame so auto-layout inside the instance is exercised.
        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(10.0, 50.0, 60.0, 20.0));
        row.name = "Row".into();
        row.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Horizontal, ..AutoLayout::default() });
        }
        d.insert(wrap_id, row).unwrap();
        let pill_id = d.alloc_id();
        let mut pill = Node::rectangle(pill_id, Rect::new(0.0, 0.0, 60.0, 20.0));
        pill.layout_child.horizontal = Sizing::Fill;
        d.insert(row_id, pill).unwrap();

        let c = create_instance(&mut d, card, page, Vec2::new(300.0, 100.0));
        apply(&mut d, c);
        let ci = *d.get(page).unwrap().children.last().unwrap();
        let mut cn = d.get(ci).unwrap().clone();
        cn.size = Vec2::new(300.0, 100.0);
        cn.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(cn) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 64) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(&mut d, Command::Update { node: Box::new(k) });
        }
        d.relayout_subtree(ci);

        let wrap_inst = d.get(ci).unwrap().children[0];
        let nested_inst = d.get(wrap_inst).unwrap().children[0];
        let nested_bg = d.get(nested_inst).unwrap().children[0];
        let row_inst = d.get(wrap_inst).unwrap().children[1];
        let pill_inst = d.get(row_inst).unwrap().children[0];
        assert_eq!(d.get(wrap_inst).unwrap().size.x, 280.0);
        assert_eq!(d.get(nested_inst).unwrap().size.x, 260.0);
        assert_eq!(d.get(nested_bg).unwrap().size.x, 260.0);
        assert_eq!(d.get(row_inst).unwrap().size.x, 260.0);
        assert_eq!(d.get(pill_inst).unwrap().size.x, 260.0);

        let mut card_main = d.get(card).unwrap().clone();
        card_main.fills = vec![Paint::solid(Color::rgb(200, 90, 40))];
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        assert_eq!(d.get(wrap_inst).unwrap().size.x, 280.0);
        assert_eq!(d.get(nested_inst).unwrap().size.x, 260.0);
        assert_eq!(d.get(nested_bg).unwrap().size.x, 260.0);
        assert_eq!(d.get(row_inst).unwrap().size.x, 260.0);
        assert_eq!(d.get(pill_inst).unwrap().size.x, 260.0);
    }

    #[test]
    fn swapped_nested_instance_in_resized_outer_instance_keeps_constrained_width_after_main_sync() {
        let mut d = Document::new("swapped-nested-in-resized-sync");
        let page = d.add_page("P");
        let ba = mkbtn_stretch(&mut d, page, "Btn/A", 0.0, 32.0);
        let bb = mkbtn_stretch(&mut d, page, "Btn/B", 100.0, 40.0);
        let (card, ci, si) = card_with_btn_slot(&mut d, page, vec![ba, bb], false);

        // Configure the slot in Card as Scale (80 in 100 -> 240 in 300) and swap si to Btn/B.
        let slot = d.get(card).unwrap().children[0];
        for id in [slot, si] {
            let mut n = d.get(id).unwrap().clone();
            n.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Min };
            apply(&mut d, Command::Update { node: Box::new(n) });
        }
        let c = swap_instance(&mut d, si, bb);
        apply(&mut d, c);

        let mut cn = d.get(ci).unwrap().clone();
        cn.size = Vec2::new(300.0, 100.0);
        cn.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(cn) });
        for (cid, tf, sz) in crate::layout_constraints::plan_constraint_pass(&d, ci, Vec2::new(100.0, 100.0), Vec2::new(300.0, 100.0), 64) {
            let mut k = d.get(cid).unwrap().clone();
            crate::layout_constraints::apply_constrained_geometry(&mut k, tf, sz);
            apply(&mut d, Command::Update { node: Box::new(k) });
        }
        let bg = d.get(si).unwrap().children[0];
        assert_eq!(d.get(si).unwrap().transform.tx, 30.0);
        assert_eq!(d.get(si).unwrap().size, Vec2::new(240.0, 40.0));
        assert_eq!(d.get(bg).unwrap().size, Vec2::new(240.0, 40.0));

        // Edit Card's fill colour and sync Card.
        let mut card_main = d.get(card).unwrap().clone();
        card_main.fills = vec![Paint::solid(Color::rgb(50, 150, 250))];
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        assert_eq!(d.get(si).unwrap().transform.tx, 30.0);
        assert_eq!(d.get(si).unwrap().size, Vec2::new(240.0, 40.0));
        assert_eq!(d.get(bg).unwrap().size, Vec2::new(240.0, 40.0));

        // Edit Btn/B's fill colour and sync Btn/B directly.
        let bb_bg = d.get(bb).unwrap().children[0];
        let mut bb_bg_node = d.get(bb_bg).unwrap().clone();
        bb_bg_node.fills = vec![Paint::solid(Color::rgb(250, 120, 50))];
        apply(&mut d, Command::Update { node: Box::new(bb_bg_node) });
        let c = sync_instances(&mut d, bb);
        apply(&mut d, c);

        assert_eq!(d.get(si).unwrap().transform.tx, 30.0);
        assert_eq!(d.get(si).unwrap().size, Vec2::new(240.0, 40.0));
        assert_eq!(d.get(bg).unwrap().size, Vec2::new(240.0, 40.0));
        assert_eq!(d.get(bg).unwrap().fills, vec![Paint::solid(Color::rgb(250, 120, 50))]);
    }

    #[test]
    fn single_undo_after_main_sync_restores_resized_instance_and_children() {
        let mut d = Document::new("undo-after-sync-constraints");
        let page = d.add_page("P");
        let (comp, inst, inst_rect) = main_with_constrained_rect(&mut d, page, Constraint::Stretch);

        let main_rect = d.get(comp).unwrap().children[0];
        let mut mr = d.get(main_rect).unwrap().clone();
        mr.fills = vec![Paint::solid(Color::rgb(230, 70, 90))];
        apply(&mut d, Command::Update { node: Box::new(mr) });

        let before_sync = d.nodes.clone();
        let mut history = History::default();
        let c = sync_instances(&mut d, comp);
        history.exec(&mut d, c).unwrap();

        assert_eq!(d.get(inst).unwrap().size.x, 300.0);
        assert_eq!(d.get(inst_rect).unwrap().size.x, 280.0);
        assert_eq!(d.get(inst_rect).unwrap().fills, vec![Paint::solid(Color::rgb(230, 70, 90))]);

        assert!(history.undo(&mut d).unwrap());
        assert_eq!(d.nodes, before_sync);
    }

    #[test]
    fn wrap_row_hugging_after_main_sync_refits_siblings_and_pins_absolute_child() {
        let mut d = Document::new("wrap-row-hug-sync");
        let page = d.add_page("P");

        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 100.0, 60.0));
        card.name = "Card".into();
        card.fills = vec![Paint::solid(Color::rgb(20, 30, 40))];
        if let NodeKind::Frame(fp) = &mut card.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                padding: [0.0; 4],
                item_spacing: 0.0,
                ..Default::default()
            });
        }
        let card = d.insert(page, card).unwrap();

        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 40.0));
        row.name = "Row".into();
        row.layout_child.horizontal = Sizing::Fill;
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                wrap: true,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                padding: [0.0; 4],
                item_spacing: 0.0,
                counter_spacing: 0.0,
                ..Default::default()
            });
        }
        let row = d.insert(card, row).unwrap();

        for (name, y) in [("ChipA", 0.0), ("ChipB", 20.0)] {
            let chip_id = d.alloc_id();
            let mut chip = Node::rectangle(chip_id, Rect::new(0.0, y, 60.0, 20.0));
            chip.name = name.into();
            d.insert(row, chip).unwrap();
        }

        let pin_id = d.alloc_id();
        let mut pin = Node::rectangle(pin_id, Rect::new(0.0, 35.0, 10.0, 5.0));
        pin.name = "Pin".into();
        pin.layout_child.absolute = true;
        pin.constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Max };
        d.insert(row, pin).unwrap();

        let footer_id = d.alloc_id();
        let mut footer = Node::rectangle(footer_id, Rect::new(0.0, 40.0, 100.0, 20.0));
        footer.name = "Footer".into();
        d.insert(card, footer).unwrap();

        let c = create_component(&mut d, card);
        apply(&mut d, c);

        let c = create_instance(&mut d, card, page, Vec2::new(200.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_row = d.get(inst).unwrap().children[0];
        let inst_footer = d.get(inst).unwrap().children[1];
        let inst_pin = d.get(inst_row).unwrap().children[2];

        // Simulate editor resize of Card instance to 300x60: Row stretches to 300 and hugs to 20,
        // Footer moves up to y=20, and bottom-pinned Pin sits at y=15 inside the 20px Row.
        let mut inst_node = d.get(inst).unwrap().clone();
        inst_node.size = Vec2::new(300.0, 60.0);
        inst_node.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(inst_node) });
        let mut row_node = d.get(inst_row).unwrap().clone();
        row_node.size = Vec2::new(300.0, 20.0);
        row_node.layout_child.record_stretched(Vec2::new(100.0, 20.0), Vec2::new(300.0, 20.0), false, true);
        apply(&mut d, Command::Update { node: Box::new(row_node) });
        let mut pin_node = d.get(inst_pin).unwrap().clone();
        pin_node.transform.ty = 15.0;
        apply(&mut d, Command::Update { node: Box::new(pin_node) });
        let mut footer_node = d.get(inst_footer).unwrap().clone();
        footer_node.transform.ty = 20.0;
        apply(&mut d, Command::Update { node: Box::new(footer_node) });

        // First main edit + sync.
        let mut card_main = d.get(card).unwrap().clone();
        card_main.fills = vec![Paint::solid(Color::rgb(40, 80, 120))];
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        assert_eq!(d.get(inst_row).unwrap().size, Vec2::new(300.0, 20.0));
        assert_eq!(d.get(inst_footer).unwrap().transform.ty, 20.0);
        assert_eq!(d.get(inst_pin).unwrap().transform.ty, 15.0);
        assert_eq!(d.get(inst_pin).unwrap().size, Vec2::new(10.0, 5.0));

        // Second sync stays stable.
        let mut card_main = d.get(card).unwrap().clone();
        card_main.fills = vec![Paint::solid(Color::rgb(80, 120, 160))];
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        assert_eq!(d.get(inst_row).unwrap().size, Vec2::new(300.0, 20.0));
        assert_eq!(d.get(inst_footer).unwrap().transform.ty, 20.0);
        assert_eq!(d.get(inst_pin).unwrap().transform.ty, 15.0);
        assert_eq!(d.get(inst_pin).unwrap().size, Vec2::new(10.0, 5.0));
    }

    #[test]
    fn hug_container_shrinking_on_drift_pass_repositions_bottom_pinned_absolute_child() {
        let mut d = Document::new("hug-drift-abs-pin-sync");
        let page = d.add_page("P");

        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 100.0, 60.0));
        card.name = "Card".into();
        card.fills = vec![Paint::solid(Color::rgb(20, 30, 40))];
        if let NodeKind::Frame(fp) = &mut card.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                padding: [0.0; 4],
                item_spacing: 0.0,
                ..Default::default()
            });
        }
        let card = d.insert(page, card).unwrap();

        let body_id = d.alloc_id();
        let mut body = Node::frame(body_id, Rect::new(0.0, 0.0, 100.0, 60.0));
        body.name = "Body".into();
        body.layout_child.horizontal = Sizing::Fill;
        if let NodeKind::Frame(fp) = &mut body.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                padding: [0.0; 4],
                item_spacing: 0.0,
                ..Default::default()
            });
        }
        let body = d.insert(card, body).unwrap();

        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 40.0));
        row.name = "Row".into();
        row.layout_child.horizontal = Sizing::Fill;
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                wrap: true,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                padding: [0.0; 4],
                item_spacing: 0.0,
                counter_spacing: 0.0,
                ..Default::default()
            });
        }
        let row = d.insert(body, row).unwrap();

        for (name, y) in [("ChipA", 0.0), ("ChipB", 20.0)] {
            let chip_id = d.alloc_id();
            let mut chip = Node::rectangle(chip_id, Rect::new(0.0, y, 60.0, 20.0));
            chip.name = name.into();
            d.insert(row, chip).unwrap();
        }

        let footer_id = d.alloc_id();
        let mut footer = Node::rectangle(footer_id, Rect::new(0.0, 40.0, 100.0, 20.0));
        footer.name = "Footer".into();
        d.insert(body, footer).unwrap();

        let body_pin_id = d.alloc_id();
        let mut body_pin = Node::rectangle(body_pin_id, Rect::new(0.0, 55.0, 10.0, 5.0));
        body_pin.name = "BodyPin".into();
        body_pin.layout_child.absolute = true;
        body_pin.constraints = Constraints { horizontal: Constraint::Min, vertical: Constraint::Max };
        d.insert(body, body_pin).unwrap();

        let c = create_component(&mut d, card);
        apply(&mut d, c);

        let c = create_instance(&mut d, card, page, Vec2::new(200.0, 0.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_body = d.get(inst).unwrap().children[0];
        let inst_row = d.get(inst_body).unwrap().children[0];
        let inst_footer = d.get(inst_body).unwrap().children[1];
        let inst_body_pin = d.get(inst_body).unwrap().children[2];

        let mut inst_node = d.get(inst).unwrap().clone();
        inst_node.size = Vec2::new(300.0, 60.0);
        inst_node.component.overrides.insert(OverrideField::Size);
        apply(&mut d, Command::Update { node: Box::new(inst_node) });

        let mut card_main = d.get(card).unwrap().clone();
        card_main.fills = vec![Paint::solid(Color::rgb(40, 80, 120))];
        apply(&mut d, Command::Update { node: Box::new(card_main) });
        let c = sync_instances(&mut d, card);
        apply(&mut d, c);

        assert_eq!(d.get(inst_row).unwrap().size, Vec2::new(300.0, 20.0));
        assert_eq!(d.get(inst_footer).unwrap().transform.ty, 20.0);
        assert_eq!(d.get(inst_body).unwrap().size, Vec2::new(300.0, 40.0));
        assert_eq!(d.get(inst_body_pin).unwrap().transform.ty, 35.0);
        assert_eq!(d.get(inst_body_pin).unwrap().size, Vec2::new(10.0, 5.0));
    }

    fn build_rich_variant(d: &mut Document, page: NodeId, name: &str, x: f32, with_extra_layers: bool) -> NodeId {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(x, 0.0, 200.0, 80.0));
        f.name = name.into();
        let comp = d.insert(page, f).unwrap();

        let title_id = d.alloc_id();
        let mut title = Node::text(title_id, Rect::new(8.0, 4.0, 60.0, 14.0), "Title", TextStyle::default());
        title.name = "Title".into();
        d.insert(comp, title).unwrap();

        if with_extra_layers {
            let sub_id = d.alloc_id();
            let mut sub = Node::text(sub_id, Rect::new(8.0, 22.0, 60.0, 14.0), "Default subtitle", TextStyle::default());
            sub.name = "Subtitle".into();
            d.insert(comp, sub).unwrap();

            let badge_id = d.alloc_id();
            let mut badge = Node::rectangle(badge_id, Rect::new(8.0, 40.0, 20.0, 12.0));
            badge.name = "Badge".into();
            d.insert(comp, badge).unwrap();

            let chip_id = d.alloc_id();
            let mut chip = Node::rectangle(chip_id, Rect::new(36.0, 40.0, 24.0, 12.0));
            chip.name = "Chip".into();
            chip.fills = vec![Paint::solid(Color::rgb(200, 200, 200))];
            d.insert(comp, chip).unwrap();

            let rule_id = d.alloc_id();
            let mut rule = Node::rectangle(rule_id, Rect::new(8.0, 60.0, 120.0, 2.0));
            rule.name = "Rule".into();
            rule.stroke.paints = vec![Paint::solid(Color::rgb(120, 120, 120))];
            rule.stroke.weight = 1.0;
            d.insert(comp, rule).unwrap();
        }

        let c = create_component(d, comp);
        apply(d, c);
        comp
    }

    fn normalized_node(d: &Document, id: NodeId) -> Node {
        let mut n = d.get(id).unwrap().clone();
        n.id = NodeId(0);
        n.parent = None;
        n.children.clear();
        n.motion = None;
        n
    }

    fn subtree_nodes(d: &Document, root: NodeId) -> Vec<Node> {
        let mut out = Vec::new();
        d.walk(root, |n, _| out.push(n.clone()));
        out
    }

    fn normalized_subtree(d: &Document, root: NodeId) -> Vec<Node> {
        let mut out = Vec::new();
        d.walk(root, |n, _| {
            let mut c = n.clone();
            c.id = NodeId(0);
            c.parent = None;
            c.children.clear();
            c.motion = None;
            out.push(c);
        });
        out
    }

    #[test]
    fn override_on_layer_missing_in_other_variant_returns_on_switch_back_for_text_visibility_fill_and_stroke() {
        let mut d = Document::new("ThereAndBackMissingLayer");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();

        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Custom subtitle");
        {
            let mut sn = d.get(sub).unwrap().clone();
            sn.size = Vec2::new(112.0, 14.0);
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }

        let badge = child_named(&d, inst, "Badge");
        {
            let before = d.get(badge).unwrap().clone();
            let mut bn = before.clone();
            bn.visible = false;
            bn.component.overrides = record_override(&before, &bn);
            apply(&mut d, Command::Update { node: Box::new(bn) });
        }

        let chip = child_named(&d, inst, "Chip");
        override_fill(&mut d, chip, Color::rgb(10, 200, 90));

        let rule = child_named(&d, inst, "Rule");
        {
            let before = d.get(rule).unwrap().clone();
            let mut rn = before.clone();
            rn.stroke.paints = vec![Paint::solid(Color::rgb(220, 40, 40))];
            rn.stroke.weight = 3.0;
            rn.component.overrides = record_override(&before, &rn);
            apply(&mut d, Command::Update { node: Box::new(rn) });
        }

        let before_sub = normalized_node(&d, sub);
        let before_badge = normalized_node(&d, badge);
        let before_chip = normalized_node(&d, chip);
        let before_rule = normalized_node(&d, rule);
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());

        let (cmd_to_b, dropped_to_b, kept_to_b) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, cmd_to_b);
        assert_eq!((dropped_to_b, kept_to_b), (0, 4), "switch A -> B reports 0 dropped and 4 kept-for-return overrides");
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 4);
        assert_eq!(d.get(inst).unwrap().children.len(), 1, "B only has Title");

        let (cmd_to_a, dropped_to_a, kept_to_a) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, cmd_to_a);
        assert_eq!((dropped_to_a, kept_to_a), (0, 0), "switch B -> A restores all 4 overrides so 0 are dropped and 0 remain parked");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());

        assert_eq!(normalized_node(&d, child_named(&d, inst, "Subtitle")), before_sub, "Subtitle node returns identical");
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Badge")), before_badge, "Badge node returns identical");
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Chip")), before_chip, "Chip node returns identical");
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Rule")), before_rule, "Rule node returns identical");
    }

    #[test]
    fn parked_override_applies_to_third_variant_and_waits_when_layer_kind_differs() {
        let mut d = Document::new("ThirdVariantParked");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 220.0, false);
        let c_var = build_rich_variant(&mut d, page, "State=C", 440.0, true);

        let d_id = d.alloc_id();
        let mut d_frame = Node::frame(d_id, Rect::new(660.0, 0.0, 200.0, 80.0));
        d_frame.name = "State=D".into();
        let d_var = d.insert(page, d_frame).unwrap();
        let dt_id = d.alloc_id();
        let mut dt = Node::text(dt_id, Rect::new(8.0, 4.0, 60.0, 14.0), "Title", TextStyle::default());
        dt.name = "Title".into();
        d.insert(d_var, dt).unwrap();
        let ds_id = d.alloc_id();
        let mut ds = Node::rectangle(ds_id, Rect::new(8.0, 22.0, 60.0, 14.0));
        ds.name = "Subtitle".into();
        d.insert(d_var, ds).unwrap();
        let c = create_component(&mut d, d_var);
        apply(&mut d, c);

        let c = combine_as_variants(&mut d, &[a, b, c_var, d_var]);
        apply(&mut d, c);

        // 1. A -> B -> C: C has Text "Subtitle", so the parked override applies on C and is consumed.
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 160.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "For C too");

        let (c, dropped_ab, kept_ab) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!((dropped_ab, kept_ab), (0, 1));
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);
        let (c, dropped_bc, kept_bc) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!((dropped_bc, kept_bc), (0, 0));
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "For C too", "parked Subtitle override applies when switching from B to third variant C");

        // 2. A -> B -> D -> A: D has a Rectangle named "Subtitle", so Characters stays parked on D and applies when switched to A.
        let c = create_instance(&mut d, a, page, Vec2::new(260.0, 160.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        let sub2 = child_named(&d, inst2, "Subtitle");
        override_text(&mut d, sub2, "Kept across D");
        let before_sub2 = normalized_node(&d, sub2);

        let (c, dropped1, kept1) = set_instance_prop_counts(&mut d, inst2, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!((dropped1, kept1), (0, 1));
        let (c, dropped2, kept2) = set_instance_prop_counts(&mut d, inst2, "State", Some(PropValue::Variant("D".into())));
        apply(&mut d, c);
        assert_eq!((dropped2, kept2), (0, 1), "on D (Rectangle Subtitle), Characters remains parked because Variants A and C have Text Subtitle");
        assert_eq!(d.get(inst2).unwrap().component.parked_overrides.len(), 1);
        assert!(matches!(d.get(child_named(&d, inst2, "Subtitle")).unwrap().kind, NodeKind::Rectangle));
        let (c, dropped3, kept3) = set_instance_prop_counts(&mut d, inst2, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!((dropped3, kept3), (0, 0));
        assert!(d.get(inst2).unwrap().component.parked_overrides.is_empty());
        assert_eq!(normalized_node(&d, child_named(&d, inst2, "Subtitle")), before_sub2);
    }

    #[test]
    fn parked_override_survives_undo_redo_save_reopen_copy_paste_and_main_sync() {
        let mut d = Document::new("ParkedLifecycle");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);

        let clean_json = d.to_json().unwrap();
        assert!(!clean_json.contains("parked_overrides"), "a document without parked overrides omits the field in JSON");

        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Persisted subtitle");

        // Also bind Chip's fill to a variable on A so variable bindings on a parked field are tested end-to-end.
        let chip = child_named(&d, inst, "Chip");
        {
            let mut cn = d.get(chip).unwrap().clone();
            cn.fills = vec![Paint::solid(Color::rgb(30, 144, 255))];
            cn.variables.bindings.insert("fills/0".into(), "var:brand-blue".into());
            cn.component.overrides.insert(OverrideField::Fills);
            apply(&mut d, Command::Update { node: Box::new(cn) });
        }

        let state_on_a_exact = subtree_nodes(&d, inst);
        let state_on_a_norm = normalized_subtree(&d, inst);
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());

        // Switch A -> B (step 1).
        let (cmd1, dropped1, kept1) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        let undo1 = apply(&mut d, cmd1);
        assert_eq!((dropped1, kept1), (0, 2));
        let parked_on_b = d.get(inst).unwrap().component.parked_overrides.clone();
        assert_eq!(parked_on_b.len(), 2);
        assert!(parked_on_b.iter().any(|p| p.field == OverrideField::Fills && p.bindings.get("fills/0").map(String::as_str) == Some("var:brand-blue")));
        let state_on_b_exact = subtree_nodes(&d, inst);

        // Save and reopen while on B: the parked overrides survive round-trip JSON serialization.
        let mid_json = d.to_json().unwrap();
        let mut reopened = Document::from_json(&mid_json).unwrap();
        assert_eq!(reopened.get(inst).unwrap().component.parked_overrides, parked_on_b);
        let c = set_instance_prop(&mut reopened, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut reopened, c);
        assert_eq!(
            normalized_subtree(&reopened, inst),
            state_on_a_norm,
            "saving and reopening on B preserves parked overrides and variable bindings across B -> A"
        );
        assert!(
            !reopened.to_json().unwrap().contains("parked_overrides"),
            "once the parked override is applied on A, the list is empty and omitted from JSON again"
        );

        // Copy and paste (duplicate_subtree) while on B: the copy carries the parked override.
        let dup_nodes = duplicate_subtree(&mut d, inst);
        let dup_root = dup_nodes[0].id;
        let insert_idx = d.get(page).unwrap().children.len();
        apply(&mut d, Command::Insert { parent: page, index: insert_idx, nodes: dup_nodes });
        assert_eq!(d.get(dup_root).unwrap().component.parked_overrides, parked_on_b);
        let c = set_instance_prop(&mut d, dup_root, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, dup_root, "Subtitle")), "Persisted subtitle");
        assert_eq!(
            d.get(child_named(&d, dup_root, "Chip")).unwrap().variables.bindings.get("fills/0").map(String::as_str),
            Some("var:brand-blue"),
            "pasted instance restores the bound variable on Chip"
        );
        assert!(d.get(dup_root).unwrap().component.parked_overrides.is_empty());

        // Main sync while on B: syncing B's main preserves the parked override on inst.
        let c = sync_instances(&mut d, b);
        apply(&mut d, c);
        assert_eq!(d.get(inst).unwrap().component.parked_overrides, parked_on_b);

        // Switch B -> A (step 2).
        let (cmd2, dropped2, kept2) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        let undo2 = apply(&mut d, cmd2);
        assert_eq!((dropped2, kept2), (0, 0));
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        assert_eq!(normalized_subtree(&d, inst), state_on_a_norm, "B -> A restores every node and binding");
        let state_after_ba_exact = subtree_nodes(&d, inst);

        // Undo B -> A, then undo A -> B; redo A -> B, then redo B -> A (comparing whole nodes and parked_overrides at every step).
        let redo2 = apply(&mut d, undo2);
        assert_eq!(subtree_nodes(&d, inst), state_on_b_exact, "undoing B -> A restores the exact B subtree and parked_overrides");
        assert_eq!(d.get(inst).unwrap().component.parked_overrides, parked_on_b);

        let redo1 = apply(&mut d, undo1);
        assert_eq!(subtree_nodes(&d, inst), state_on_a_exact, "undoing A -> B restores the exact A subtree and empty parked_overrides");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());

        let _ = apply(&mut d, redo1);
        assert_eq!(subtree_nodes(&d, inst), state_on_b_exact, "redoing A -> B restores the exact B subtree and parked_overrides");
        assert_eq!(d.get(inst).unwrap().component.parked_overrides, parked_on_b);

        let _ = apply(&mut d, redo2);
        assert_eq!(subtree_nodes(&d, inst), state_after_ba_exact, "redoing B -> A restores the exact post-switch A subtree and empty parked_overrides");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
    }

    #[test]
    fn reset_and_detach_clear_parked_overrides() {
        let mut d = Document::new("ResetAndDetachParked");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        // 1. reset_instance clears parked overrides.
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Temporary subtitle");
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = reset_instance(&mut d, inst);
        apply(&mut d, c);
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "Default subtitle", "reset_instance drops parked overrides for good");

        // 2. reset_overrides on the instance clears parked overrides for the specified field.
        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Second subtitle");
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = reset_overrides(&mut d, inst, &[OverrideField::Characters]);
        apply(&mut d, c);
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "Default subtitle", "reset_overrides drops parked overrides for that field");

        // 3. detach_instance clears parked overrides so the detached frame has a clean component link.
        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Third subtitle");
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = detach_instance(&mut d, inst);
        apply(&mut d, c);
        assert!(d.get(inst).unwrap().component.is_empty(), "detaching an instance with parked overrides leaves an empty ComponentLink");
    }

    #[test]
    fn swap_outside_component_family_drops_parked_overrides_and_keeps_target_text() {
        let mut d = Document::new("CrossFamilySwapParked");
        let page = d.add_page("P");
        let card_a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let card_b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c = combine_as_variants(&mut d, &[card_a, card_b]);
        apply(&mut d, c);

        // Button is an unrelated component outside Card's family that has its own "Subtitle" text layer.
        let btn_id = d.alloc_id();
        let mut btn_f = Node::frame(btn_id, Rect::new(500.0, 0.0, 160.0, 60.0));
        btn_f.name = "Button".into();
        let button_with_sub = d.insert(page, btn_f).unwrap();
        let bt_id = d.alloc_id();
        let mut bt = Node::text(bt_id, Rect::new(8.0, 4.0, 60.0, 14.0), "Title", TextStyle::default());
        bt.name = "Title".into();
        d.insert(button_with_sub, bt).unwrap();
        let bs_id = d.alloc_id();
        let mut bs = Node::text(bs_id, Rect::new(8.0, 22.0, 100.0, 14.0), "Button own subtitle", TextStyle::default());
        bs.name = "Subtitle".into();
        d.insert(button_with_sub, bs).unwrap();
        let c = create_component(&mut d, button_with_sub);
        apply(&mut d, c);

        // Button2 is an unrelated component outside Card's family without "Subtitle".
        let btn2_id = d.alloc_id();
        let mut btn2_f = Node::frame(btn2_id, Rect::new(700.0, 0.0, 160.0, 60.0));
        btn2_f.name = "ButtonPlain".into();
        let button_without_sub = d.insert(page, btn2_f).unwrap();
        let b2t_id = d.alloc_id();
        let mut b2t = Node::text(b2t_id, Rect::new(8.0, 4.0, 60.0, 14.0), "Title", TextStyle::default());
        b2t.name = "Title".into();
        d.insert(button_without_sub, b2t).unwrap();
        let c = create_component(&mut d, button_without_sub);
        apply(&mut d, c);

        // 1. Card/A "Subtitle" override -> switch to Card/B (parked) -> swap to Button with a "Subtitle" layer:
        //    Button shows its own text ("Button own subtitle"), parked list is removed, and the dropped count says 1.
        let c = create_instance(&mut d, card_a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, sub, "Card custom subtitle");

        let (c, dropped_to_b) = set_instance_prop_inner(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(dropped_to_b, 0);
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);

        let (c, dropped_to_btn, kept_to_btn) = swap_instance_reporting(&mut d, inst, button_with_sub);
        apply(&mut d, c);
        assert_eq!(
            text_of(&d, child_named(&d, inst, "Subtitle")),
            "Button own subtitle",
            "Button shows its own Subtitle text rather than the parked Card override"
        );
        assert_eq!((dropped_to_btn, kept_to_btn), (1, 0), "swapping outside the family drops the parked entry and reports 1 dropped");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty(), "parked list is removed on cross-family swap");

        // 2. Direct swap from Card/A (with "Subtitle" overridden) to ButtonPlain (which lacks "Subtitle") drops the override instead of parking it.
        let c = create_instance(&mut d, card_a, page, Vec2::new(300.0, 140.0));
        apply(&mut d, c);
        let inst2 = *d.get(page).unwrap().children.last().unwrap();
        let inst2_sub = child_named(&d, inst2, "Subtitle");
        override_text(&mut d, inst2_sub, "Card direct subtitle");
        let (c, dropped_direct, kept_direct) = swap_instance_reporting(&mut d, inst2, button_without_sub);
        apply(&mut d, c);
        assert_eq!((dropped_direct, kept_direct), (1, 0), "cross-family swap with missing layer counts 1 dropped and parks 0");
        assert!(d.get(inst2).unwrap().component.parked_overrides.is_empty());
    }

    #[test]
    fn parked_fill_override_respects_direct_transfer_gate_on_intermediate_variant_and_returns_on_origin() {
        let mut d = Document::new("ParkedTransferGateFill");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c_var = build_rich_variant(&mut d, page, "State=C", 480.0, true);
        // Variant A's Chip has grey fill (200, 200, 200); set Variant C's Chip default fill to blue (0, 80, 255) so A and C differ.
        let c_chip = child_named(&d, c_var, "Chip");
        set_fill(&mut d, c_chip, Color::rgb(0, 80, 255));
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);

        // Baseline: a direct A -> C switch drops the Chip fill override because A and C have different default Chip fills.
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let direct_inst = *d.get(page).unwrap().children.last().unwrap();
        let direct_chip = child_named(&d, direct_inst, "Chip");
        override_fill(&mut d, direct_chip, Color::rgb(10, 200, 90));
        let (c, direct_dropped, direct_kept) = set_instance_prop_counts(&mut d, direct_inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!((direct_dropped, direct_kept), (1, 0), "direct A -> C drops the Chip fill override");
        let expected_c_chip = normalized_node(&d, child_named(&d, direct_inst, "Chip"));
        assert_eq!(paint_of(&d, child_named(&d, direct_inst, "Chip")), crate::PaintKind::Solid(Color::rgb(0, 80, 255)));

        // Indirect A -> B (Chip missing, fill parked) -> C -> A:
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_chip = child_named(&d, inst, "Chip");
        override_fill(&mut d, inst_chip, Color::rgb(10, 200, 90));
        let expected_a_chip = normalized_node(&d, child_named(&d, inst, "Chip"));

        let (c, d_ab) = set_instance_prop_inner(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(d_ab, 0);
        let parked_after_b = d.get(inst).unwrap().component.parked_overrides.clone();
        assert_eq!(parked_after_b.len(), 1);

        let (c, d_bc, k_bc) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Chip")), expected_c_chip, "C's Chip shows C's own fill with no override flag");
        assert_eq!(d.get(inst).unwrap().component.parked_overrides, parked_after_b, "the parked entry stays in parked_overrides on C");
        assert_eq!((d_bc, k_bc), (0, 1), "when the transfer gate says no on C, the entry is not dropped and remains parked");

        let (c, d_ca, k_ca) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!((d_ca, k_ca), (0, 0));
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Chip")), expected_a_chip, "C -> A brings the green fill override back");
    }

    #[test]
    fn when_returning_layer_already_has_override_live_override_wins_and_parked_entry_is_removed() {
        let mut d = Document::new("LiveOverrideWinsOverParked");
        let page = d.add_page("P");
        // Variant A has two texts ("Title", "Subtitle"); Variant B has one text ("Heading"); Variant C has one text ("Subtitle").
        let a = main_with_texts(&mut d, page, "State=A", 0.0, &["Title", "Subtitle"]);
        let b = main_with_texts(&mut d, page, "State=B", 200.0, &["Heading"]);
        let c_var = main_with_texts(&mut d, page, "State=C", 400.0, &["Subtitle"]);
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);

        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 200.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, inst_sub, "Parked from A");

        // Switch A -> B: A has 2 texts and B has 1, so single-text rule does not fire and "Subtitle" is parked.
        let (c, d_ab) = set_instance_prop_inner(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(d_ab, 0);
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);

        // On B, override "Heading" to "Live from B".
        let inst_heading = child_named(&d, inst, "Heading");
        override_text(&mut d, inst_heading, "Live from B");

        // Switch B -> C: B has 1 text ("Heading") and C has 1 text ("Subtitle"), so single-text rule transfers "Live from B"
        // onto C's "Subtitle". The returning "Subtitle" layer already has a live Characters override, so "Live from B" wins
        // and the parked entry from A is removed.
        let (c, d_bc) = set_instance_prop_inner(&mut d, inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!(d_bc, 0);
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty(), "the parked entry is removed when the live override wins");
        assert_eq!(text_of(&d, child_named(&d, inst, "Subtitle")), "Live from B", "the live override on the returning layer wins");
    }

    #[test]
    fn nested_instance_keeps_parked_overrides_across_outer_variant_switch_and_returns_on_slot_variant() {
        let mut d = Document::new("NestedSwitchParked");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 220.0, false);
        let c_var = build_rich_variant(&mut d, page, "State=C", 440.0, true);
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);

        let mk_outer = |d: &mut Document, name: &str, x: f32| -> NodeId {
            let outer_id = d.alloc_id();
            let mut outer_f = Node::frame(outer_id, Rect::new(x, 200.0, 240.0, 100.0));
            outer_f.name = name.into();
            let outer_main = d.insert(page, outer_f).unwrap();
            let c = create_component(d, outer_main);
            apply(d, c);
            let c = create_instance(d, a, outer_main, Vec2::new(0.0, 0.0));
            apply(d, c);
            outer_main
        };
        let outer_one = mk_outer(&mut d, "Layout=One", 0.0);
        let outer_two = mk_outer(&mut d, "Layout=Two", 280.0);
        let c = combine_as_variants(&mut d, &[outer_one, outer_two]);
        apply(&mut d, c);

        let c = create_instance(&mut d, outer_one, page, Vec2::new(0.0, 360.0));
        apply(&mut d, c);
        let outer_inst = *d.get(page).unwrap().children.last().unwrap();
        let nested = d.get(outer_inst).unwrap().children[0];
        let sub = child_named(&d, nested, "Subtitle");
        override_text(&mut d, sub, "Brand");
        let expected_brand_sub = normalized_node(&d, sub);

        // Switch nested Card from A to B ("Brand" is parked on the nested Card).
        let c = set_nested_instance_prop(&mut d, nested, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let parked_on_nested = d.get(nested).unwrap().component.parked_overrides.clone();
        assert_eq!(parked_on_nested.len(), 1);

        // Switch nested Card to third variant C (restores "Brand"), then back to B (parks "Brand" again).
        let c = set_nested_instance_prop(&mut d, nested, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, nested, "Subtitle")), "Brand");
        assert!(d.get(nested).unwrap().component.parked_overrides.is_empty());

        let c = set_nested_instance_prop(&mut d, nested, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(d.get(nested).unwrap().component.parked_overrides.len(), 1);

        // Switch the outer variant Layout=One -> Layout=Two and back to Layout=One (rebuilding the nested Card each time).
        let c = set_instance_prop(&mut d, outer_inst, "Layout", Some(PropValue::Variant("Two".into())));
        apply(&mut d, c);
        let nested_on_two = d.get(outer_inst).unwrap().children[0];
        assert_eq!(
            d.get(nested_on_two).unwrap().component.parked_overrides.len(),
            1,
            "nested instance keeps its parked list when rebuilt on outer variant Two"
        );

        let c = set_instance_prop(&mut d, outer_inst, "Layout", Some(PropValue::Variant("One".into())));
        apply(&mut d, c);
        let nested_on_one = d.get(outer_inst).unwrap().children[0];
        assert_eq!(
            d.get(nested_on_one).unwrap().component.parked_overrides.len(),
            1,
            "nested instance keeps its parked list when rebuilt back on outer variant One"
        );

        // Switch the nested Card back to A (the variant the outer main's slot itself shows): "Subtitle" returns as "Brand".
        let c = set_nested_instance_prop(&mut d, nested_on_one, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert!(d.get(nested_on_one).unwrap().component.parked_overrides.is_empty());
        assert_eq!(
            normalized_node(&d, child_named(&d, nested_on_one, "Subtitle")),
            expected_brand_sub,
            "switching nested Card back to slot variant A restores Brand on Subtitle"
        );
    }

    #[test]
    fn new_instance_of_main_whose_nested_instance_has_parked_entries_starts_with_empty_parked_list() {
        let mut d = Document::new("NewInstanceFromMainWithParkedNested");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 220.0, false);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        let outer_id = d.alloc_id();
        let mut outer_f = Node::frame(outer_id, Rect::new(0.0, 200.0, 240.0, 100.0));
        outer_f.name = "OuterMain".into();
        let outer_main = d.insert(page, outer_f).unwrap();
        let c = create_component(&mut d, outer_main);
        apply(&mut d, c);
        let c = create_instance(&mut d, a, outer_main, Vec2::new(0.0, 0.0));
        apply(&mut d, c);

        // On the main component's own nested Card, override Subtitle to "Brand" and switch it to B so it holds a parked entry.
        let main_nested = d.get(outer_main).unwrap().children[0];
        let main_nested_sub = child_named(&d, main_nested, "Subtitle");
        override_text(&mut d, main_nested_sub, "Brand");
        let c = set_nested_instance_prop(&mut d, main_nested, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(d.get(main_nested).unwrap().component.parked_overrides.len(), 1);

        // Creating a new instance of OuterMain does not copy the main-side nested instance's parked list.
        let c = create_instance(&mut d, outer_main, page, Vec2::new(300.0, 200.0));
        apply(&mut d, c);
        let new_outer = *d.get(page).unwrap().children.last().unwrap();
        let copy_nested = d.get(new_outer).unwrap().children[0];
        assert!(
            d.get(copy_nested).unwrap().component.parked_overrides.is_empty(),
            "a new instance made from a main component does not inherit parked entries from the main's nested instance"
        );
        let c = set_nested_instance_prop(&mut d, copy_nested, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!(text_of(&d, child_named(&d, copy_nested, "Subtitle")), "Default subtitle");
    }

    #[test]
    fn parked_entry_is_removed_and_counted_as_dropped_when_deleted_from_all_variants_or_reused_with_incompatible_kind() {
        let mut d = Document::new("ParkedEntryEndsWhenLayerGoneFromFamily");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c = combine_as_variants(&mut d, &[a, b]);
        apply(&mut d, c);

        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_sub = child_named(&d, inst, "Subtitle");
        override_text(&mut d, inst_sub, "Doomed subtitle");

        // Switch A -> B: "Subtitle" is parked on inst.
        let (c, d_ab) = set_instance_prop_inner(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!(d_ab, 0);
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);

        // The main deletes "Subtitle" in Variant A (so no variant of the family has "Subtitle" anymore).
        let sub_in_a = child_named(&d, a, "Subtitle");
        apply(&mut d, Command::Remove { id: sub_in_a });

        // A later switch B -> A reports the drop and empties the parked list.
        let (c, d_ba, k_ba) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!((d_ba, k_ba), (1, 0), "when no variant in the family has the layer, a later switch reports the drop");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty(), "the parked list is now empty");

        // Second check: a new layer that reuses the name "Subtitle" with another kind (Rectangle) gets nothing,
        // and when no variant in the family has a Text "Subtitle", switching B -> A drops the parked Characters override.
        let rect_id = d.alloc_id();
        let mut rect = Node::rectangle(rect_id, Rect::new(8.0, 22.0, 60.0, 14.0));
        rect.name = "Subtitle".into();
        rect.fills = vec![Paint::solid(Color::rgb(180, 180, 180))];
        d.insert(a, rect).unwrap();
        let c = sync_instances(&mut d, a);
        apply(&mut d, c);
        let expected_rect = normalized_node(&d, child_named(&d, inst, "Subtitle"));

        // Seed a parked Characters override for ["Subtitle"] while on B and switch B -> A where "Subtitle" is a Rectangle in A (and absent in B).
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        {
            let mut in_b = d.get(inst).unwrap().clone();
            in_b.component.parked_overrides.push(ParkedOverride {
                path: vec![("Subtitle".into(), 0)],
                field: OverrideField::Characters,
                value: ParkedOverrideValue::Characters { content: "Stale text".into(), fitted_size: None, sizing_text: None },
                bindings: BTreeMap::new(),
                origin_source: Some(rect_id),
            });
            apply(&mut d, Command::Update { node: Box::new(in_b) });
        }
        let (c, d_rect, k_rect) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("A".into())));
        apply(&mut d, c);
        assert_eq!((d_rect, k_rect), (1, 0), "parked Characters is dropped when Subtitle only exists as a Rectangle across the family");
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty());
        assert_eq!(normalized_node(&d, child_named(&d, inst, "Subtitle")), expected_rect, "Rectangle reusing the name gets nothing");
    }

    #[test]
    fn parked_value_is_dropped_and_never_written_when_origin_layer_was_deleted() {
        let mut d = Document::new("ParkedOriginDeletedGate");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c_var = build_rich_variant(&mut d, page, "State=C", 480.0, true);
        let c_chip = child_named(&d, c_var, "Chip");
        set_fill(&mut d, c_chip, Color::rgb(0, 80, 255));
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);

        // Baseline: direct A -> C drops the Chip fill override because A and C have different default Chip fills.
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let direct_inst = *d.get(page).unwrap().children.last().unwrap();
        let direct_chip = child_named(&d, direct_inst, "Chip");
        override_fill(&mut d, direct_chip, Color::rgb(10, 200, 90));
        let (c, direct_dropped, direct_kept) = set_instance_prop_counts(&mut d, direct_inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!((direct_dropped, direct_kept), (1, 0));
        let expected_c_chip = normalized_node(&d, child_named(&d, direct_inst, "Chip"));

        // Chip fill overridden on A, parked on B. Delete A's Chip. Switch B -> C:
        // C shows its own fill with no override flag, and the entry is removed and counted as dropped.
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_chip = child_named(&d, inst, "Chip");
        override_fill(&mut d, inst_chip, Color::rgb(10, 200, 90));

        let (c, d_ab, k_ab) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        assert_eq!((d_ab, k_ab), (0, 1));
        assert_eq!(d.get(inst).unwrap().component.parked_overrides.len(), 1);

        let a_chip = child_named(&d, a, "Chip");
        apply(&mut d, Command::Remove { id: a_chip });

        let (c, d_bc, k_bc) = set_instance_prop_counts(&mut d, inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!(
            normalized_node(&d, child_named(&d, inst, "Chip")),
            expected_c_chip,
            "when origin layer was deleted, C shows its own fill with no override flag"
        );
        assert!(d.get(inst).unwrap().component.parked_overrides.is_empty(), "entry with deleted origin is removed");
        assert_eq!((d_bc, k_bc), (1, 0), "entry with deleted origin is counted as dropped");
    }

    #[test]
    fn main_sync_of_nested_instance_without_parked_entries_matches_main_behavior() {
        let mut d = Document::new("MainSyncNestedNoParked");
        let page = d.add_page("P");

        let mk_btn = |d: &mut Document, name: &str, x: f32, label: &str| -> NodeId {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 100.0, 40.0));
            f.name = name.into();
            let comp = d.insert(page, f).unwrap();
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(8.0, 8.0, 80.0, 16.0), label, TextStyle::default());
            t.name = "Label".into();
            d.insert(comp, t).unwrap();
            let c = create_component(d, comp);
            apply(d, c);
            comp
        };
        let btn_a = mk_btn(&mut d, "State=A", 0.0, "Submit");
        let btn_b = mk_btn(&mut d, "State=B", 140.0, "Cancel");
        let c = combine_as_variants(&mut d, &[btn_a, btn_b]);
        apply(&mut d, c);

        let card_id = d.alloc_id();
        let mut card_f = Node::frame(card_id, Rect::new(0.0, 120.0, 160.0, 80.0));
        card_f.name = "Card".into();
        let card_main = d.insert(page, card_f).unwrap();
        let c = create_component(&mut d, card_main);
        apply(&mut d, c);
        let c = create_instance(&mut d, btn_a, card_main, Vec2::new(10.0, 10.0));
        apply(&mut d, c);
        let main_btn = d.get(card_main).unwrap().children[0];

        let c = create_instance(&mut d, card_main, page, Vec2::new(220.0, 120.0));
        apply(&mut d, c);
        let card_inst = *d.get(page).unwrap().children.last().unwrap();
        let inst_btn = d.get(card_inst).unwrap().children[0];

        // Nested button on card_inst is swapped to B and its label overridden to "Pay" (no parked entries).
        let c = set_nested_instance_prop(&mut d, inst_btn, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let inst_lbl = child_named(&d, inst_btn, "Label");
        override_text(&mut d, inst_lbl, "Pay");
        assert!(d.get(inst_btn).unwrap().component.parked_overrides.is_empty());

        // Then Card main's nested button is swapped to B too, and Card's instances are synced.
        let c = set_nested_instance_prop(&mut d, main_btn, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = sync_instances(&mut d, card_main);
        apply(&mut d, c);

        // On origin/main, with an empty parked list, the sync rebuilds the slot children from the reference and replaces "Pay" with the main label "Cancel".
        let synced_lbl = child_named(&d, inst_btn, "Label");
        assert_eq!(text_of(&d, synced_lbl), "Cancel", "with an empty parked list, main sync matches main and replaces Pay with Cancel");
        assert!(d.get(synced_lbl).unwrap().component.overrides.is_empty());
    }

    #[test]
    fn returning_text_uses_same_text_sizing_predicate_including_variable_bindings() {
        let mut d = Document::new("ReturningTextFontSizeBinding");
        let page = d.add_page("P");
        let a = build_rich_variant(&mut d, page, "State=A", 0.0, true);
        let b = build_rich_variant(&mut d, page, "State=B", 240.0, false);
        let c_var = build_rich_variant(&mut d, page, "State=C", 480.0, true);
        // On Variant C, Subtitle has a different box width (140.0 vs 60.0) and a fontSize variable binding that A lacks.
        let c_sub = child_named(&d, c_var, "Subtitle");
        {
            let mut csn = d.get(c_sub).unwrap().clone();
            csn.size = Vec2::new(140.0, 18.0);
            csn.variables.bindings.insert("fontSize".into(), "var:heading-size".into());
            apply(&mut d, Command::Update { node: Box::new(csn) });
        }
        let c = combine_as_variants(&mut d, &[a, b, c_var]);
        apply(&mut d, c);

        // Direct switch A -> C: Subtitle has WidthAndHeight sizing and Characters overridden (with fitted size 95x14 on A),
        // and because fontSize binding differs on C, the box on C is C's target size (140x18).
        let c = create_instance(&mut d, a, page, Vec2::new(0.0, 140.0));
        apply(&mut d, c);
        let direct_inst = *d.get(page).unwrap().children.last().unwrap();
        let direct_sub = child_named(&d, direct_inst, "Subtitle");
        {
            let mut sn = d.get(direct_sub).unwrap().clone();
            if let NodeKind::Text(tp) = &mut sn.kind {
                tp.content = "Longer custom subtitle".into();
            }
            sn.size = Vec2::new(95.0, 14.0);
            sn.component.overrides.insert(OverrideField::Characters);
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }
        let c = set_instance_prop(&mut d, direct_inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        let expected_c_sub = normalized_node(&d, child_named(&d, direct_inst, "Subtitle"));
        assert_eq!(expected_c_sub.size, Vec2::new(140.0, 18.0), "direct switch keeps target variant's box when fontSize binding differs");

        // Parked return A -> B -> C: must produce the exact same Subtitle node and box size (140x18) as the direct switch.
        let c = create_instance(&mut d, a, page, Vec2::new(300.0, 140.0));
        apply(&mut d, c);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let sub = child_named(&d, inst, "Subtitle");
        {
            let mut sn = d.get(sub).unwrap().clone();
            if let NodeKind::Text(tp) = &mut sn.kind {
                tp.content = "Longer custom subtitle".into();
            }
            sn.size = Vec2::new(95.0, 14.0);
            sn.component.overrides.insert(OverrideField::Characters);
            apply(&mut d, Command::Update { node: Box::new(sn) });
        }
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("B".into())));
        apply(&mut d, c);
        let c = set_instance_prop(&mut d, inst, "State", Some(PropValue::Variant("C".into())));
        apply(&mut d, c);
        assert_eq!(
            normalized_node(&d, child_named(&d, inst, "Subtitle")),
            expected_c_sub,
            "returning text uses same_text_sizing and keeps target C's box when fontSize binding differs"
        );
    }
}
