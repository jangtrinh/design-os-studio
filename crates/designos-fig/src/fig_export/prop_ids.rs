//! The ids a save gives component property definitions.
//!
//! A definition imported from a `.fig` keeps the id Figma gave it, everywhere the save writes it: on the main component or set
//! that defines it, and in every assignment, reference, variant spec and binding that names it. The model holds a definition by
//! its name only (a variant property has no id in it at all), so the id comes from the record the open kept for the layer that
//! defines it. A property Studio made has no such id and takes one from a session of its own, one no id of the file uses.
//!
//! Both sides of a definition are written by the same rule: a layer the save writes from the store carries Figma's bytes, which
//! name Figma's ids; a layer Studio writes names the same ids through [`ExportCtx::resolve_prop_guid`]. A kept reference whose
//! definition this save no longer writes is dropped (see [`PropIds::check_reference`]), so a definition deleted in Studio never
//! leaves a kept assignment pointing at nothing.

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
    /// The ids this save defines: those of the properties the model holds, and the aliases a variant writes from the store.
    live: HashSet<Guid>,
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
    let mut claimed: HashSet<Guid> = HashSet::new();
    for (&id, n) in &doc.nodes {
        for p in &n.component.props {
            let from_file = by_node
                .get(&id)
                .and_then(|k| k.defs.iter().find(|d| d.parent.is_none() && d.name.as_deref() == Some(bare(&p.name)) && !claimed.contains(&d.id)))
                .map(|d| d.id);
            let g = match from_file {
                Some(g) => g,
                None => cx.alloc_guid(session),
            };
            claimed.insert(g);
            cx.prop_guids.insert((id, p.name.clone()), g);
        }
    }
    cx.prop_ids.live.extend(claimed);

    // A variant's copy of a set property is written only from the store, whole, so it is a definition of this save only when that
    // is certain: the layer is written from the store, every entry of the field is a plain alias, and every parent is defined.
    for (nid, k) in &by_node {
        let aliases_only = !k.defs.is_empty() && k.defs.iter().all(|d| d.plain_alias);
        if aliases_only
            && k.defs.iter().all(|d| d.parent.is_some_and(|p| cx.prop_ids.live.contains(&p)))
            && kept_fields::layer_is_written_from_the_store(cx, *nid)
        {
            cx.prop_ids.live.extend(k.defs.iter().map(|d| d.id));
        }
    }
}

impl PropIds {
    /// Whether a kept reference to the definition `g` may be written: a definition this save defines, or one the file's own records
    /// never defined (a library's). One the file defined and this save does not is a reference to nothing.
    pub(super) fn check_reference(&self, g: Guid) -> Result<(), &'static str> {
        if self.live.contains(&g) || !self.file_defs.contains(&g) {
            Ok(())
        } else {
            Err("prop_def_not_written")
        }
    }

    /// Whether a definition the file made is among the ones this save writes.
    #[cfg(test)]
    pub(super) fn is_live(&self, g: Guid) -> bool {
        self.live.contains(&g)
    }
}

impl ExportCtx<'_> {
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

    /// The id a save writes for the property `prop_name` of `scope`. A name no definition has gets an id of the session Studio
    /// keeps for itself, the same one each time.
    pub(super) fn resolve_prop_guid(&mut self, scope: NodeId, prop_name: &str) -> Guid {
        if let Some(g) = self.prop_guid_in_scope(scope, prop_name) {
            return g;
        }
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
