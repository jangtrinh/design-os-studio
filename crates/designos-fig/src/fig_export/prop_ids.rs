//! The ids a save gives component property definitions.
//!
//! A definition imported from a `.fig` keeps the id Figma gave it, everywhere the save writes it: on the main component or set
//! that defines it, and in every assignment, reference, variant spec and binding that names it. The model holds a definition by
//! its name only (a variant property has no id in it at all), so the id comes from the record the open kept for the layer that
//! defines it. A property Studio made has no such id and takes one from a session of its own, one no id of the file uses.
//!
//! Both sides of a definition are written by the same rule: a layer the save writes from the store carries Figma's bytes, which
//! name Figma's ids; a layer Studio writes names the same ids through [`ExportCtx::resolve_prop_guid`], and the consumption entries
//! it writes from a layer's metadata are routed through the same definitions ([`ExportCtx::route_metadata_prop_refs`]). A reference
//! is kept only if the layer can name the definition: one its own main or set (or an ancestor) defines in this save. An id is a
//! definition of a layer, not of the file, so after a delete or rename on one set the references under it stop naming an id a Figma
//! copy of the set still defines.

use super::{kept_fields, ExportCtx};
use crate::kiwi::KiwiValue as V;
use designos_doc::{KeptRecordKind, NodeId};
use std::collections::{BTreeMap, HashMap, HashSet};

type Guid = (u32, u32);

/// One entry of a kept `componentPropDefs`.
pub(super) struct KeptDef {
    id: Guid,
    name: Option<String>,
    parent: Option<Guid>,
    /// An entry that only says "this variant's copy of the set's property": no name, a parent, no value of its own.
    plain_alias: bool,
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
    /// The ids each layer defines in this save: those of the properties the model holds on it, and the aliases a variant writes from
    /// the store. An id is a definition of a layer, not of the file: a Figma copy of a set defines the ids of the set it was made from.
    live_by_owner: HashMap<NodeId, HashSet<Guid>>,
    /// Every id some layer defines in this save.
    defined_anywhere: HashSet<Guid>,
    /// The name the file gave a definition, by id.
    file_def_names: HashMap<Guid, String>,
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
            let plain_alias = name.is_none() && parent.is_some() && d.get("initialValue").is_none() && d.get("varValue").is_none();
            Some(KeptDef { id, name, parent, plain_alias })
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
            if let Some(name) = &d.name {
                cx.prop_ids.file_def_names.entry(d.id).or_insert_with(|| name.clone());
            }
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
            cx.prop_ids.live_by_owner.insert(id, taken.into_iter().collect());
        }
    }

    // A variant's copy of a set property is written only from the store, whole, so it is a definition of this save only when that
    // is certain: the layer is written from the store, every entry of the field is a plain alias, and every parent is defined by the
    // variant's own set.
    for (nid, k) in &by_node {
        let aliases_only = !k.defs.is_empty() && k.defs.iter().all(|d| d.plain_alias);
        if !aliases_only {
            continue;
        }
        let set = doc.get(*nid).and_then(|n| n.parent);
        let parents_defined = k.defs.iter().all(|d| d.parent.is_some_and(|p| set.is_some_and(|s| cx.prop_ids.defines(s, p))));
        if parents_defined && kept_fields::layer_is_written_from_the_store(cx, *nid) {
            cx.prop_ids.live_by_owner.entry(*nid).or_default().extend(k.defs.iter().map(|d| d.id));
        }
    }
    cx.prop_ids.defined_anywhere = cx.prop_ids.live_by_owner.values().flatten().copied().collect();
}

impl PropIds {
    /// Whether the layer `owner` defines the property `g` in this save.
    fn defines(&self, owner: NodeId, g: Guid) -> bool {
        self.live_by_owner.get(&owner).is_some_and(|ids| ids.contains(&g))
    }
}

impl ExportCtx<'_> {
    /// Whether the definition `g` is one the layer `scope` can name: a definition this save gives the layer, the component it is an
    /// instance of, that component's set, or a layer above it. An id the file's own records never defined (a library's) is not
    /// this save's to judge.
    pub(super) fn prop_def_is_in_scope(&self, scope: NodeId, g: Guid) -> bool {
        if !self.prop_ids.file_defs.contains(&g) {
            return true;
        }
        // A variant that sits outside any set (a file can hold one whose set is elsewhere or gone) has no owner to ask: the
        // definition has to exist somewhere in this save.
        if self.doc.get(scope).is_some_and(|n| {
            matches!(n.kind, designos_doc::NodeKind::Component(_))
                && !n.component.variant_props.is_empty()
                && !n.parent.and_then(|p| self.doc.get(p)).is_some_and(|p| matches!(p.kind, designos_doc::NodeKind::ComponentSet(_)))
        }) {
            return self.prop_ids.defined_anywhere.contains(&g);
        }
        let mut cur = Some(scope);
        while let Some(c) = cur {
            if self.prop_ids.defines(c, g) {
                return true;
            }
            let Some(n) = self.doc.get(c) else { return false };
            if let designos_doc::NodeKind::Instance { component: Some(master), .. } = n.kind {
                if self.prop_ids.defines(master, g) || self.doc.get(master).and_then(|m| m.parent).is_some_and(|set| self.prop_ids.defines(set, g)) {
                    return true;
                }
            }
            cur = n.parent;
        }
        false
    }

    /// A kept reference to the definition `g`, written on the layer `scope`: it has to be one that layer can name.
    pub(super) fn check_prop_reference(&self, scope: NodeId, g: Guid) -> Result<(), &'static str> {
        if self.prop_def_is_in_scope(scope, g) {
            Ok(())
        } else {
            Err("prop_def_not_written")
        }
    }

    /// The consumption entries Studio writes from the metadata a layer carried: an entry that binds to a component property keeps
    /// the id only if the layer can name that definition. A property the file defined and the layer cannot name any more is looked
    /// up again by the name the file gave it (a copy of a component binds to its own property), and an entry with no such property is
    /// left out.
    pub(super) fn route_metadata_prop_refs(&self, scope: NodeId, entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
        entries.into_iter().filter_map(|mut e| self.route_prop_ref(scope, &mut e).then_some(e)).collect()
    }

    fn route_prop_ref(&self, scope: NodeId, v: &mut serde_json::Value) -> bool {
        use serde_json::Value;
        match v {
            Value::Object(m) => {
                if let Some(Value::Object(d)) = m.get_mut("propRefValue").and_then(|p| p.get_mut("defId")) {
                    let g = match (d.get("sessionID").and_then(Value::as_u64), d.get("localID").and_then(Value::as_u64)) {
                        (Some(s), Some(l)) => u32::try_from(s).ok().zip(u32::try_from(l).ok()),
                        _ => None,
                    };
                    if let Some(g) = g.filter(|g| !self.prop_def_is_in_scope(scope, *g)) {
                        let Some(now) = self.prop_ids.file_def_names.get(&g).and_then(|name| self.prop_guid_in_scope(scope, name)) else { return false };
                        d.insert("sessionID".into(), now.0.into());
                        d.insert("localID".into(), now.1.into());
                    }
                }
                m.values_mut().all(|x| self.route_prop_ref(scope, x))
            }
            Value::Array(a) => a.iter_mut().all(|x| self.route_prop_ref(scope, x)),
            _ => true,
        }
    }

    /// The id of the definition `prop_name` names for the layer `scope`: the one of the layer's own properties, or of the
    /// component it is an instance of, or of that component's set, going up through the layer's parents. `None` when no
    /// definition in that chain has the name.
    pub(super) fn prop_guid_in_scope(&self, scope: NodeId, prop_name: &str) -> Option<Guid> {
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
            if let designos_doc::NodeKind::Instance { component: Some(master), .. } = n.kind {
                if let Some(&g) = own(master) {
                    return Some(g);
                }
                if let Some(&g) = self.doc.get(master).and_then(|m| m.parent).and_then(own) {
                    return Some(g);
                }
            }
            cur = n.parent;
        }
        None
    }

    /// The id a save writes for the property `prop_name` of `scope`: the one in scope, else the one of any definition of that name
    /// (an instance whose main this file does not hold, set apart from its variants), else an id of the session Studio keeps for
    /// itself, the same one each time.
    pub(super) fn resolve_prop_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        if let Some(g) = self.prop_guid_in_scope(scope, prop_name) {
            return g;
        }
        let key = bare(prop_name);
        if let Some((_, &g)) = self.prop_guids.iter().find(|((_, name), _)| name == key) {
            return g;
        }
        self.studio_prop_guid(prop_name)
    }

    /// The id a variant spec names for the property `prop_name` of `scope`. A variant of a component set that does not define the
    /// property (deleted or renamed there) gets `None`: a spec of it would name another component's definition. An instance of
    /// such a variant keeps the pick under an id of Studio's own (the name travels beside it), and any other layer resolves as
    /// [`Self::resolve_prop_guid`] does.
    pub(super) fn resolve_variant_spec_guid(&mut self, scope: NodeId, prop_name: &str) -> Option<Guid> {
        use designos_doc::NodeKind;
        if let Some(g) = self.prop_guid_in_scope(scope, prop_name) {
            return Some(g);
        }
        let mut cur = Some(scope);
        let mut through_instance = false;
        while let Some(c) = cur {
            let n = self.doc.get(c)?;
            if matches!(n.kind, NodeKind::ComponentSet(_)) {
                break;
            }
            if let NodeKind::Instance { component: Some(master), .. } = n.kind {
                if self.doc.get(master).and_then(|m| m.parent).and_then(|p| self.doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
                    through_instance = true;
                    break;
                }
            }
            cur = n.parent;
            if cur.is_none() {
                return Some(self.resolve_prop_guid(scope, prop_name));
            }
        }
        if !through_instance {
            return None;
        }
        Some(self.studio_prop_guid(prop_name))
    }

    /// The one id of Studio's own session for a property name nothing defines.
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
