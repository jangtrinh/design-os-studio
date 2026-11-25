//! The ids a save gives component property definitions, and who writes the keys that name them.
//!
//! A definition imported from a `.fig` keeps the id Figma gave it. The model holds a property by its name only (a variant
//! property has no id in it at all), so the id comes from the record the open kept for the layer that defines it
//! ([`assign`]). A property Studio made has no such id and takes one from a session of its own, one no id of the file uses.
//! These ids are the same whichever writer writes a layer.
//!
//! Figma resolves a property id inside one owner: a component set (with its variants and everything under them), or a main
//! component that is no variant. Nothing here models liveness. Two structural rules decide who writes the keys of an owner group
//! instead:
//!
//! - **The definition side is all or nothing per group.** `componentPropDefs`, `componentPropRefs`, `variantPropSpecs`,
//!   `parameterConsumptionMap` (and the assignments of a layer that is no instance) of every layer of a group are Figma's bytes
//!   only when every layer of the group is written from the store and none was moved. Otherwise they are Studio's for the whole
//!   group, and Studio names the same ids through [`assign`] wherever a property name survives.
//! - **An instance's assignments are checked against what its main's group writes.** They are Figma's when the instance is
//!   written from the store and each id is a property that group writes (or the group is whole, or the id is a library's).
//!
//! No writer looks a property up by name outside the owner it names. A name nothing defines gets an id of Studio's session and
//! is counted in the save's report, so an unresolved reference is dangling, never bound to another component's property.

use super::{kept_fields, ExportCtx};
use crate::kiwi::KiwiValue as V;
use designos_doc::{KeptRecordKind, NodeId, NodeKind};
use std::collections::{BTreeMap, HashMap, HashSet};

type Guid = (u32, u32);

/// One entry of a kept `componentPropDefs`.
pub(super) struct KeptDef {
    id: Guid,
    name: Option<String>,
    parent: Option<Guid>,
}

/// The kept definitions of one record, as the file's layer holds them.
pub(super) struct KeptDefs {
    record: Guid,
    defs: Vec<KeptDef>,
}

/// What a save knows about the definitions it writes.
#[derive(Default)]
pub(crate) struct PropIds {
    /// Every id a record of the file defines, named or not, deleted or not.
    file_defs: HashSet<Guid>,
    /// The ids the model's properties of each layer have in this save.
    defined_by: HashMap<NodeId, HashSet<Guid>>,
    /// The named definitions of each layer's own record.
    kept_named: HashMap<NodeId, Vec<Guid>>,
    /// Whether every layer of the group that has this root is written from the store and unmoved.
    groups: HashMap<NodeId, bool>,
    /// The session Studio-made ids come from.
    session: u32,
    /// A name no definition in scope has: the id given to it, once.
    unresolved: BTreeMap<String, Guid>,
}

fn guid_value(v: &V) -> Option<Guid> {
    let n = |k: &str| match v.get(k) {
        Some(V::Uint(u)) => Some(*u),
        _ => None,
    };
    Some((n("sessionID")?, n("localID")?))
}

fn decode_defs(cx: &ExportCtx<'_>, rec: designos_doc::KeptRecordRef<'_>) -> Option<KeptDefs> {
    let no = cx.kept.field_number("componentPropDefs")?;
    let bytes = rec.field(no)?;
    let (_, value) = cx.kept.decode_unchecked(cx.schema, cx.nc_def, no, bytes)?;
    let defs = value
        .as_array()?
        .iter()
        .filter_map(|d| {
            let id = d.get("id").and_then(guid_value)?;
            let name = match d.get("name") {
                Some(V::String(s)) => Some(s.clone()),
                _ => None,
            };
            let parent = d.get("parentPropDefId").and_then(guid_value).filter(|g| *g != (0, 0));
            Some(KeptDef { id, name, parent })
        })
        .collect();
    Some(KeptDefs { record: rec.guid(), defs })
}

/// The definitions every layer record of the file holds. Their ids are spent: nothing Studio makes may take one.
pub(super) fn reserve_file_defs(cx: &mut ExportCtx<'_>) -> Vec<KeptDefs> {
    if !cx.kept.is_active() {
        return Vec::new();
    }
    let doc = cx.doc;
    let mut out = Vec::new();
    for rec in doc.fig_kept.records() {
        if rec.kind() != KeptRecordKind::Layer {
            continue;
        }
        let Some(found) = decode_defs(cx, rec) else { continue };
        for d in &found.defs {
            for g in std::iter::once(d.id).chain(d.parent) {
                cx.prop_ids.file_defs.insert(g);
                cx.reserve_guid(g);
            }
        }
        out.push(found);
    }
    out
}

fn bare(name: &str) -> &str {
    name.split('#').next().unwrap_or(name)
}

/// Gives every property definition of the model its id: Figma's, where the layer that holds it has a record naming it, else one
/// from the session Studio keeps for itself.
pub(super) fn assign(cx: &mut ExportCtx<'_>, kept: Vec<KeptDefs>) {
    let doc = cx.doc;
    // a session no id of the file uses, so an id Studio makes cannot be one Figma made (counted down from the largest a signed 32-bit
    // reader still takes)
    let sessions: HashSet<u32> = cx.used_guids.iter().map(|g| g.0).collect();
    let mut session = i32::MAX as u32;
    while sessions.contains(&session) && session > 5 {
        session -= 1;
    }
    cx.prop_ids.session = session;

    // A layer's own definitions are Figma's only when the record is the layer's: it is written under the guid of that record.
    let mut by_node: HashMap<NodeId, KeptDefs> = HashMap::new();
    for k in kept {
        if let Some(nid) = cx.node_with_guid(k.record) {
            if doc.fig_kept.record_of_node(nid).is_some_and(|r| r.guid() == k.record) {
                by_node.insert(nid, k);
            }
        }
    }
    // Figma gives one id to a property that many components share (a copy of a component set keeps its properties' ids), so the same
    // id may define a property on several layers; on one layer each definition has an id of its own.
    for (&id, n) in &doc.nodes {
        let mut taken: Vec<Guid> = Vec::new();
        for p in &n.component.props {
            let from_file = by_node
                .get(&id)
                .and_then(|k| k.defs.iter().find(|d| d.parent.is_none() && d.name.as_deref() == Some(bare(&p.name)) && !taken.contains(&d.id)))
                .map(|d| d.id);
            let g = match from_file {
                Some(g) => g,
                None => cx.alloc_guid(session),
            };
            taken.push(g);
            cx.prop_guids.insert((id, p.name.clone()), g);
        }
        if !taken.is_empty() {
            cx.prop_ids.defined_by.insert(id, taken.into_iter().collect());
        }
    }
    for (nid, k) in by_node {
        let named: Vec<Guid> = k.defs.iter().filter(|d| d.name.is_some() && d.parent.is_none()).map(|d| d.id).collect();
        if !named.is_empty() {
            cx.prop_ids.kept_named.insert(nid, named);
        }
    }
}

fn is_component(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))
}

impl ExportCtx<'_> {
    /// The root of the owner group `o` belongs to: its nearest component set above (not through an instance), else `o`.
    fn owner_root(&self, o: NodeId) -> NodeId {
        let mut cur = self.doc.get(o).and_then(|n| n.parent);
        while let Some(c) = cur {
            let Some(n) = self.doc.get(c) else { break };
            match n.kind {
                NodeKind::ComponentSet(_) => return c,
                NodeKind::Instance { .. } => break,
                _ => cur = n.parent,
            }
        }
        o
    }

    /// The owner of the keys that bind `layer` to a property: the owner group of the component it sits in, or, inside an instance,
    /// of that instance's main. A component's own definitions belong to its own group.
    pub(super) fn binding_owner(&self, layer: NodeId) -> Option<NodeId> {
        let n = self.doc.get(layer)?;
        if is_component(&n.kind) {
            return Some(self.owner_root(layer));
        }
        let mut cur = n.parent;
        while let Some(c) = cur {
            let n = self.doc.get(c)?;
            match n.kind {
                NodeKind::Component(_) | NodeKind::ComponentSet(_) => return Some(self.owner_root(c)),
                NodeKind::Instance { component, .. } => return component.map(|m| self.owner_root(m)),
                _ => cur = n.parent,
            }
        }
        None
    }

    /// The owner of an instance's assignments: the group of its main.
    pub(super) fn assignment_owner(&self, instance: NodeId) -> Option<NodeId> {
        match self.doc.get(instance)?.kind {
            NodeKind::Instance { component: Some(m), .. } => Some(self.owner_root(m)),
            _ => None,
        }
    }

    /// The set a variant belongs to, or `None` for a component that is in none.
    fn spec_owner(&self, variant: NodeId) -> Option<NodeId> {
        let r = self.owner_root(variant);
        (r != variant).then_some(r)
    }

    /// Whether every layer of the group rooted at `root` (the root and what is under it, up to the instances) is written from the
    /// store and sits under the parent the file gave it.
    fn group_is_verbatim(&mut self, root: NodeId) -> bool {
        if let Some(&v) = self.prop_ids.groups.get(&root) {
            return v;
        }
        let doc = self.doc;
        let mut stack = vec![root];
        let mut whole = true;
        while let Some(id) = stack.pop() {
            if !kept_fields::node_is_verbatim(self, id) {
                whole = false;
                break;
            }
            let Some(n) = doc.get(id) else { continue };
            if !matches!(n.kind, NodeKind::Instance { .. }) {
                stack.extend(n.children.iter().copied());
            }
        }
        self.prop_ids.groups.insert(root, whole);
        whole
    }

    /// Whether the store may write the key `key` of `layer`: the definition side of a group is the store's only when the whole group
    /// is verbatim. The assignments of an instance are judged id by id (see [`Self::assignment_ref_is_written`]).
    pub(super) fn store_may_write_property_key(&mut self, layer: NodeId, key: &str) -> bool {
        if !matches!(key, "componentPropDefs" | "componentPropRefs" | "variantPropSpecs" | "parameterConsumptionMap" | "componentPropAssignments") {
            return true;
        }
        let Some(n) = self.doc.get(layer) else { return false };
        if key == "componentPropAssignments" && matches!(n.kind, NodeKind::Instance { .. }) {
            return true;
        }
        // a variant outside every set has no owner to ask: it is Figma's if it is as the file had it
        let loose = matches!(n.kind, NodeKind::Component(_)) && !n.component.variant_props.is_empty() && self.spec_owner(layer).is_none();
        if loose && matches!(key, "variantPropSpecs" | "componentPropDefs") {
            return kept_fields::node_is_verbatim(self, layer);
        }
        match self.binding_owner(layer) {
            Some(root) => self.group_is_verbatim(root),
            None => true,
        }
    }

    /// Whether the save writes the property `g` in the group rooted at `root`: the model's properties of its layers, or the named
    /// definitions of its record when the record is written as the file had it.
    fn group_writes_def(&mut self, root: NodeId, main: Option<NodeId>, g: Guid) -> bool {
        let defined = |ids: &PropIds, n: NodeId| ids.defined_by.get(&n).is_some_and(|s| s.contains(&g));
        if defined(&self.prop_ids, root) || main.is_some_and(|m| defined(&self.prop_ids, m)) {
            return true;
        }
        self.prop_ids.kept_named.get(&root).is_some_and(|ids| ids.contains(&g)) && kept_fields::node_is_verbatim(self, root)
    }

    /// Whether a kept reference to the property `g`, found in a field of the layer being written, may be written as the file had it.
    /// Assignments of an instance: the id is a library's, or the group of its main is whole, or that group writes the property.
    /// Any other reference (the field is not one of the group's keys, such as a paint bound to a property): the group is whole.
    pub(super) fn kept_prop_ref_is_written(&mut self, root_key: &str, g: Guid) -> Result<(), &'static str> {
        let Some(layer) = self.kept.scope else { return Err("prop_def_without_a_layer") };
        let is_instance = self.doc.get(layer).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. }));
        if root_key == "componentPropAssignments" && is_instance {
            let Some(owner) = self.assignment_owner(layer) else { return Ok(()) };
            let main = match self.doc.get(layer).map(|n| &n.kind) {
                Some(NodeKind::Instance { component, .. }) => *component,
                _ => None,
            };
            if !self.prop_ids.file_defs.contains(&g) || self.group_is_verbatim(owner) || self.group_writes_def(owner, main, g) {
                return Ok(());
            }
            return Err("prop_def_not_written");
        }
        match self.binding_owner(layer) {
            Some(root) if !self.group_is_verbatim(root) => Err("prop_def_group_not_verbatim"),
            _ => Ok(()),
        }
    }

    /// The consumption entries Studio writes from the metadata a layer carried. An entry that binds to a component property is
    /// kept only if the property is one the save writes for the owner of the layer, or one the file never defined (a library's).
    /// An entry that fails is left out and counted.
    pub(super) fn route_metadata_prop_refs(&mut self, layer: NodeId, entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
        let owner = self.binding_owner(layer);
        let mut out = Vec::with_capacity(entries.len());
        for e in entries {
            let mut ids = Vec::new();
            prop_ref_ids(&e, &mut ids);
            let ok = ids.into_iter().all(|g| !self.prop_ids.file_defs.contains(&g) || owner.is_some_and(|o| self.group_writes_def(o, None, g)));
            if ok {
                out.push(e);
            } else {
                self.kept.pending[kept_fields::PENDING_DROPPED_ENTRY] += 1;
            }
        }
        out
    }

    /// The id of the definition `prop_name` names for an assignment on `scope` (an instance, or a layer whose own properties are
    /// meant): the one of the layer's own properties, or of the component it is an instance of, or of that component's set, going up
    /// through the layer's parents. `None` when no definition in that chain has the name.
    pub(super) fn assignment_scope_guid(&self, scope: NodeId, prop_name: &str) -> Option<Guid> {
        self.scoped_guid(scope, prop_name, true)
    }

    fn scoped_guid(&self, scope: NodeId, prop_name: &str, own_main: bool) -> Option<Guid> {
        if let Some(rest) = prop_name.strip_prefix("propDefId:") {
            if let Some(g) = super::parse_guid_pair(rest) {
                return Some(g);
            }
        }
        let own = |node: NodeId| {
            self.prop_guids
                .get(&(node, prop_name.to_string()))
                .or_else(|| (bare(prop_name) != prop_name).then(|| self.prop_guids.get(&(node, bare(prop_name).to_string()))).flatten())
        };
        let mut cur = Some(scope);
        while let Some(c) = cur {
            if let Some(&g) = own(c) {
                return Some(g);
            }
            let n = self.doc.get(c)?;
            if let NodeKind::Instance { component: Some(master), .. } = n.kind {
                if own_main || c != scope {
                    if let Some(&g) = own(master) {
                        return Some(g);
                    }
                    if let Some(&g) = self.doc.get(master).and_then(|m| m.parent).and_then(own) {
                        return Some(g);
                    }
                }
            }
            cur = n.parent;
        }
        None
    }

    /// The id a save writes for the property `prop_name` of `scope` (see [`Self::assignment_scope_guid`]). A name no definition has
    /// gets an id of the session Studio keeps for itself, the same one each time, and is counted for the layer (see
    /// [`kept_fields::fold_unresolved`]).
    pub(super) fn resolve_prop_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        self.resolve_counted(scope, prop_name, true, kept_fields::UNRESOLVED_ASSIGNMENT)
    }

    /// [`Self::resolve_prop_guid`] for a binding (see [`Self::binding_scope_guid`]).
    pub(super) fn resolve_binding_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        self.resolve_counted(scope, prop_name, false, kept_fields::UNRESOLVED_BINDING)
    }

    /// [`Self::resolve_prop_guid`] for a variant spec.
    pub(super) fn resolve_spec_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        self.resolve_counted(scope, prop_name, true, kept_fields::UNRESOLVED_SPEC)
    }

    /// [`Self::resolve_prop_guid`] for the assignments inside the overrides of an instance, which the store never writes.
    pub(super) fn resolve_nested_assignment_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        self.resolve_counted(scope, prop_name, true, kept_fields::UNRESOLVED_NESTED)
    }

    fn resolve_counted(&mut self, scope: NodeId, prop_name: &str, own_main: bool, kind: usize) -> Guid {
        if let Some(g) = self.scoped_guid(scope, prop_name, own_main) {
            return g;
        }
        self.kept.pending[kind] += 1;
        self.studio_prop_guid(prop_name)
    }

    /// The one id of Studio's own session for a property name nothing in scope defines.
    fn studio_prop_guid(&mut self, prop_name: &str) -> Guid {
        if let Some(&g) = self.prop_ids.unresolved.get(prop_name) {
            return g;
        }
        let session = self.prop_ids.session;
        let g = self.alloc_guid(session);
        self.prop_ids.unresolved.insert(prop_name.to_string(), g);
        g
    }

    /// Spends `g`: nothing the save makes may take it.
    pub(super) fn reserve_guid(&mut self, g: Guid) {
        self.used_guids.insert(g);
    }
}

/// Every property id a consumption entry binds to.
fn prop_ref_ids(v: &serde_json::Value, out: &mut Vec<Guid>) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            if let Some(Value::Object(d)) = m.get("propRefValue").and_then(|p| p.get("defId")) {
                if let (Some(s), Some(l)) = (d.get("sessionID").and_then(Value::as_u64), d.get("localID").and_then(Value::as_u64)) {
                    if let (Ok(s), Ok(l)) = (u32::try_from(s), u32::try_from(l)) {
                        out.push((s, l));
                    }
                }
            }
            m.values().for_each(|x| prop_ref_ids(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| prop_ref_ids(x, out)),
        _ => {}
    }
}
