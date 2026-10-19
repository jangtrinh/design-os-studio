//! Shared-library operations as pure document functions.
//!
//! Publishing turns a file into a [`LibrarySnapshot`] (the components, sets,
//! styles and variables it exposes, plus the nodes they need). Consumers copy
//! assets out of a snapshot with [`import_asset`] into the hidden
//! [`Document::library_page`] / `styles` / `variables`, and later compare their
//! copies with a newer snapshot using [`pending_updates`] and [`apply_updates`].
//!
//! Keys. Asset keys are derived from stable ids so they survive republishing:
//! `c<node id>` (component), `s<node id>` (component set), `style:<style key>`,
//! `col:<collection id>` and `var:<variable id>`. In a consumer file every
//! imported asset is stored under `<library id>/<asset key>`: that is the key
//! in [`LibraryState::external`], the key in `styles` and the id in
//! `variables` / `variable_collections`.
//!
//! Like [`crate::components`], functions return a [`Command`] and leave the
//! document alone, with these additive exceptions that are not part of undo:
//! creating the empty hidden page, adding styles (there is no style command)
//! and adding embedded images.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// `LibraryRef::id` / `ExternalAsset::library_id` prefix of placeholder
/// libraries created by importers for components of other (unavailable) files.
pub const REMOTE_LIBRARY_PREFIX: &str = "remote:";

/// One published asset inside a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotAsset {
    pub key: String,
    pub kind: LibraryAssetKind,
    pub name: String,
    /// Root node in [`LibrarySnapshot::nodes`] (components and sets).
    pub node: Option<NodeId>,
    /// Content hash, comparable with the same hash of a consumer's copy.
    pub fingerprint: u64,
    /// Keys of other component assets whose instances this asset nests.
    pub deps: Vec<String>,
}

/// Everything a consumer needs from a published library version.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub library_id: String,
    pub name: String,
    pub version: u32,
    pub assets: Vec<SnapshotAsset>,
    pub nodes: BTreeMap<NodeId, Node>,
    pub styles: BTreeMap<String, Style>,
    pub style_meta: BTreeMap<String, StyleMeta>,
    pub variables: BTreeMap<String, Variable>,
    pub variable_collections: BTreeMap<String, VariableCollection>,
    pub images: BTreeMap<String, Vec<u8>>,
}

impl LibrarySnapshot {
    pub fn asset(&self, key: &str) -> Option<&SnapshotAsset> {
        self.assets.iter().find(|a| a.key == key)
    }

    pub fn count(&self, kind: LibraryAssetKind) -> usize {
        self.assets.iter().filter(|a| a.kind == kind).count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateKind {
    /// The library's version differs from the local copy.
    Changed,
    /// The library no longer has the asset; the local copy is kept.
    Removed,
    /// New in the library (a variable added to an imported collection).
    Added,
}

/// A difference between a library snapshot and the local copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    /// Asset key inside the library (without the library prefix).
    pub key: String,
    pub kind: LibraryAssetKind,
    pub name: String,
    pub change: UpdateKind,
}

/// Namespaced local key of an asset or style / variable id.
pub fn local_key(library_id: &str, key: &str) -> String {
    format!("{library_id}/{key}")
}

fn strip<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

fn nothing() -> Command {
    Command::Batch(Vec::new())
}

// ---------------------------------------------------------------------------
// Fingerprints
// ---------------------------------------------------------------------------

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }
    fn write(&mut self, s: &str) {
        for b in s.bytes().chain(std::iter::once(0xff)) {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

fn fp_tree(nodes: &BTreeMap<NodeId, Node>, id: NodeId, refs: &BTreeMap<NodeId, String>, prefix: &str, h: &mut Fnv) {
    let Some(n) = nodes.get(&id) else { return };
    let mut c = n.clone();
    c.id = NodeId(0);
    c.parent = None;
    let kids = std::mem::take(&mut c.children);
    c.source_id = None;
    c.component.source = None;
    let mut target = String::new();
    if let NodeKind::Instance { component, .. } = &mut c.kind {
        if let Some(k) = component.and_then(|x| refs.get(&x)) {
            target = k.clone();
        }
        *component = None;
    }
    for v in c.style_refs.values_mut() {
        *v = strip(v, prefix).to_string();
    }
    for v in c.variables.bindings.values_mut() {
        *v = strip(v, prefix).to_string();
    }
    c.variables.modes = std::mem::take(&mut c.variables.modes).into_iter().map(|(k, v)| (strip(&k, prefix).to_string(), v)).collect();
    h.write(&serde_json::to_string(&c).unwrap_or_default());
    h.write(&target);
    h.write(&kids.len().to_string());
    for k in kids {
        fp_tree(nodes, k, refs, prefix, h);
    }
}

fn tree_fingerprint(nodes: &BTreeMap<NodeId, Node>, root: NodeId, refs: &BTreeMap<NodeId, String>, prefix: &str) -> u64 {
    let mut h = Fnv::new();
    fp_tree(nodes, root, refs, prefix, &mut h);
    h.0
}

fn style_fingerprint(style: &Style, meta: Option<&StyleMeta>) -> u64 {
    let mut h = Fnv::new();
    h.write(&serde_json::to_string(&(style, meta.map(|m| (&m.name, &m.description, m.property)))).unwrap_or_default());
    h.0
}

fn variable_fingerprint(v: &Variable, prefix: &str) -> u64 {
    let mut c = v.clone();
    c.id.clear();
    c.key = None;
    c.collection = strip(&c.collection, prefix).to_string();
    for val in c.values.values_mut() {
        if let VariableValue::Alias(a) = val {
            *a = strip(a, prefix).to_string();
        }
    }
    let mut h = Fnv::new();
    h.write(&serde_json::to_string(&c).unwrap_or_default());
    h.0
}

fn collection_fingerprint(c: &VariableCollection) -> u64 {
    let mut h = Fnv::new();
    h.write(&serde_json::to_string(&(&c.name, &c.modes, &c.default_mode)).unwrap_or_default());
    h.0
}

/// Component node -> reference key (`key` for components, `key#i` for the
/// i-th variant of a set), used so fingerprints do not depend on node ids.
fn ref_map(nodes: &BTreeMap<NodeId, Node>, entries: &[(String, NodeId)]) -> BTreeMap<NodeId, String> {
    let mut out = BTreeMap::new();
    for (key, root) in entries {
        match nodes.get(root).map(|n| &n.kind) {
            Some(NodeKind::Component(_)) => {
                out.insert(*root, key.clone());
            }
            Some(NodeKind::ComponentSet(_)) => {
                for (i, c) in nodes[root].children.iter().enumerate() {
                    out.insert(*c, format!("{key}#{i}"));
                }
            }
            _ => {}
        }
    }
    out
}

fn subtree(nodes: &BTreeMap<NodeId, Node>, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(n) = nodes.get(&id) {
            out.push(id);
            stack.extend(n.children.iter().rev().copied());
        }
    }
    out
}

fn image_refs_of(n: &Node, out: &mut BTreeSet<String>) {
    let mut lists: Vec<&Vec<Paint>> = vec![&n.fills, &n.stroke.paints];
    if let NodeKind::Text(t) = &n.kind {
        lists.push(&t.style.fills);
        lists.extend(t.runs.iter().map(|r| &r.style.fills));
    }
    for list in lists {
        for p in list {
            if let PaintKind::Image { image_ref, .. } = &p.kind {
                out.insert(image_ref.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Publishing side
// ---------------------------------------------------------------------------

fn component_key(id: NodeId) -> String {
    format!("c{}", id.0)
}

fn set_key(id: NodeId) -> String {
    format!("s{}", id.0)
}

/// Assets this file can publish, as `(key, kind, name)`: components outside
/// sets, component sets, styles, variable collections and variables (those not
/// hidden from publishing). Assets imported from other libraries are excluded.
/// Keys are derived from ids, so they stay stable across publishes.
pub fn publishable_assets(doc: &Document) -> Vec<(String, LibraryAssetKind, String)> {
    let mut out = Vec::new();
    for &page in &doc.pages {
        doc.walk(page, |n, _| match &n.kind {
            NodeKind::Component(_) if !n.parent.and_then(|p| doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_))) => {
                out.push((component_key(n.id), LibraryAssetKind::Component, n.name.clone()))
            }
            NodeKind::ComponentSet(_) => out.push((set_key(n.id), LibraryAssetKind::ComponentSet, n.name.clone())),
            _ => {}
        });
    }
    let imported: BTreeSet<&str> = doc.library.external.values().filter_map(|e| e.local_key.as_deref()).collect();
    for k in doc.styles.keys().filter(|k| !imported.contains(k.as_str())) {
        out.push((format!("style:{k}"), LibraryAssetKind::Style, doc.style_name(k).to_string()));
    }
    for (id, c) in doc.variable_collections.iter().filter(|(id, _)| !imported.contains(id.as_str())) {
        out.push((format!("col:{id}"), LibraryAssetKind::VariableCollection, c.name.clone()));
    }
    for (id, v) in doc.variables.iter().filter(|(id, v)| !imported.contains(id.as_str()) && v.is_publishable()) {
        out.push((format!("var:{id}"), LibraryAssetKind::Variable, v.name.clone()));
    }
    out
}

/// Captures the publishable assets of `doc` (and what they reference) as a
/// snapshot of `version` of library `library_id`.
pub fn snapshot(doc: &Document, library_id: &str, name: &str, version: u32) -> LibrarySnapshot {
    let assets = publishable_assets(doc);
    let mut snap = LibrarySnapshot { library_id: library_id.into(), name: name.into(), version, ..Default::default() };

    let comp_entries: Vec<(String, NodeId)> = assets
        .iter()
        .filter_map(|(k, kind, _)| {
            let id = k.strip_prefix('c').or_else(|| k.strip_prefix('s')).and_then(|s| s.parse::<u64>().ok()).map(NodeId)?;
            matches!(kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet).then(|| (k.clone(), id))
        })
        .collect();
    for (_, root) in &comp_entries {
        for id in subtree(&doc.nodes, *root) {
            snap.nodes.insert(id, doc.nodes[&id].clone());
        }
    }
    let refs = ref_map(&doc.nodes, &comp_entries);

    // Styles and variables: all published ones plus whatever the nodes use.
    let mut style_keys: BTreeSet<String> = assets.iter().filter_map(|(k, _, _)| k.strip_prefix("style:").map(String::from)).collect();
    let mut var_ids: BTreeSet<String> = assets.iter().filter_map(|(k, _, _)| k.strip_prefix("var:").map(String::from)).collect();
    let mut images = BTreeSet::new();
    for n in snap.nodes.values() {
        style_keys.extend(n.style_refs.values().cloned());
        var_ids.extend(n.variables.bindings.values().cloned());
        image_refs_of(n, &mut images);
    }
    let mut queue: Vec<String> = var_ids.iter().cloned().collect();
    while let Some(id) = queue.pop() {
        let Some(v) = doc.variables.get(&id) else { continue };
        for val in v.values.values() {
            if let VariableValue::Alias(a) = val {
                if var_ids.insert(a.clone()) {
                    queue.push(a.clone());
                }
            }
        }
    }
    for k in &style_keys {
        if let Some(s) = doc.styles.get(k) {
            snap.styles.insert(k.clone(), s.clone());
            if let Some(m) = doc.style_meta.get(k) {
                snap.style_meta.insert(k.clone(), m.clone());
            }
        }
    }
    for id in &var_ids {
        if let Some(v) = doc.variables.get(id) {
            snap.variables.insert(id.clone(), v.clone());
            if let Some(c) = doc.variable_collections.get(&v.collection) {
                snap.variable_collections.insert(c.id.clone(), c.clone());
            }
        }
    }
    // Published collections are listed even when empty.
    for (k, _, _) in &assets {
        if let Some(c) = k.strip_prefix("col:").and_then(|id| doc.variable_collections.get(id)) {
            snap.variable_collections.insert(c.id.clone(), c.clone());
        }
    }
    // Drop dangling references so copies and fingerprints agree.
    let (styles, vars, colls) = (
        snap.styles.keys().cloned().collect::<BTreeSet<_>>(),
        snap.variables.keys().cloned().collect::<BTreeSet<_>>(),
        snap.variable_collections.keys().cloned().collect::<BTreeSet<_>>(),
    );
    for n in snap.nodes.values_mut() {
        n.style_refs.retain(|_, v| styles.contains(v));
        n.variables.bindings.retain(|_, v| vars.contains(v));
        n.variables.modes.retain(|k, _| colls.contains(k));
    }
    for img in images {
        if let Some(b) = doc.images.get(&img) {
            snap.images.insert(img, b.clone());
        }
    }

    for (key, kind, aname) in assets {
        let (node, fingerprint, deps) = match kind {
            LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => {
                let root = comp_entries.iter().find(|(k, _)| *k == key).map(|(_, id)| *id);
                let mut deps: Vec<String> = Vec::new();
                if let Some(root) = root {
                    for id in subtree(&snap.nodes, root) {
                        if let NodeKind::Instance { component: Some(c), .. } = &snap.nodes[&id].kind {
                            if let Some(r) = refs.get(c) {
                                let dep = r.split('#').next().unwrap_or(r).to_string();
                                if dep != key && !deps.contains(&dep) {
                                    deps.push(dep);
                                }
                            }
                        }
                    }
                }
                (root, root.map(|r| tree_fingerprint(&snap.nodes, r, &refs, "")).unwrap_or(0), deps)
            }
            LibraryAssetKind::Style => {
                let k = &key["style:".len()..];
                (None, snap.styles.get(k).map(|s| style_fingerprint(s, snap.style_meta.get(k))).unwrap_or(0), Vec::new())
            }
            LibraryAssetKind::Variable => (None, snap.variables.get(&key["var:".len()..]).map(|v| variable_fingerprint(v, "")).unwrap_or(0), Vec::new()),
            LibraryAssetKind::VariableCollection => {
                (None, snap.variable_collections.get(&key["col:".len()..]).map(collection_fingerprint).unwrap_or(0), Vec::new())
            }
        };
        snap.assets.push(SnapshotAsset { key, kind, name: aname, node, fingerprint, deps });
    }
    snap
}

/// What changed between two snapshots of the same library (publish changelog).
pub fn diff_snapshots(old: Option<&LibrarySnapshot>, new: &LibrarySnapshot) -> Vec<Update> {
    let mut out = Vec::new();
    for a in &new.assets {
        match old.and_then(|o| o.asset(&a.key)) {
            None => out.push(Update { key: a.key.clone(), kind: a.kind, name: a.name.clone(), change: UpdateKind::Added }),
            Some(o) if o.fingerprint != a.fingerprint || o.name != a.name => {
                out.push(Update { key: a.key.clone(), kind: a.kind, name: a.name.clone(), change: UpdateKind::Changed })
            }
            Some(_) => {}
        }
    }
    if let Some(old) = old {
        for o in &old.assets {
            if new.asset(&o.key).is_none() {
                out.push(Update { key: o.key.clone(), kind: o.kind, name: o.name.clone(), change: UpdateKind::Removed });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Consumer side: queries
// ---------------------------------------------------------------------------

/// True for nodes living on the hidden library page (copies of library
/// components); they must not be selectable or editable on the canvas.
pub fn is_library_node(doc: &Document, id: NodeId) -> bool {
    let Some(lp) = doc.library_page else { return false };
    id == lp || doc.is_ancestor(lp, id)
}

/// True when style `key` is the local copy of a style published by a shared library. Such a
/// style is edited in its library file, never in a file that consumes it.
pub fn is_library_style(doc: &Document, key: &str) -> bool {
    doc.library.external.values().any(|e| e.kind == LibraryAssetKind::Style && e.local_key.as_deref() == Some(key))
}

/// The library asset a component node belongs to: `(external key, entry)`. A
/// variant resolves to its set.
pub fn library_asset_of_component(doc: &Document, comp: NodeId) -> Option<(&String, &ExternalAsset)> {
    let parent_set = doc.get(comp).and_then(|n| n.parent).filter(|p| doc.get(*p).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet(_))));
    doc.library.external.iter().find(|(_, e)| e.node.is_some() && (e.node == Some(comp) || e.node == parent_set))
}

/// For an instance of a library component: its asset and the library's name.
pub struct LibraryInstanceInfo {
    pub library_id: String,
    pub library_name: String,
    /// Asset key inside the library.
    pub asset_key: String,
    pub asset_name: String,
    /// The library is a placeholder for components of another, unavailable file.
    pub remote: bool,
}

pub fn library_instance_info(doc: &Document, instance: NodeId) -> Option<LibraryInstanceInfo> {
    let comp = instance_component(doc.get(instance)?)?;
    if !is_library_node(doc, comp) {
        return None;
    }
    let (ekey, e) = library_asset_of_component(doc, comp)?;
    let library_name = doc
        .library
        .enabled
        .iter()
        .find(|l| l.id == e.library_id)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| strip(&e.library_id, REMOTE_LIBRARY_PREFIX).to_string());
    Some(LibraryInstanceInfo {
        library_id: e.library_id.clone(),
        library_name,
        asset_key: strip(ekey, &format!("{}/", e.library_id)).to_string(),
        asset_name: e.node.and_then(|n| doc.get(n)).map(|n| n.name.clone()).unwrap_or_default(),
        remote: e.library_id.starts_with(REMOTE_LIBRARY_PREFIX),
    })
}

/// Local components outside the library page: main components (and sets) on
/// pages, as `(root node, name)`, in page order.
pub fn local_component_roots(doc: &Document) -> Vec<(NodeId, String)> {
    let mut out = Vec::new();
    for &p in &doc.pages {
        doc.walk(p, |n, _| match &n.kind {
            NodeKind::ComponentSet(_) => out.push((n.id, n.name.clone())),
            NodeKind::Component(_) if !n.parent.and_then(|x| doc.get(x)).is_some_and(|x| matches!(x.kind, NodeKind::ComponentSet(_))) => {
                out.push((n.id, n.name.clone()))
            }
            _ => {}
        });
    }
    out
}

/// Library component assets already copied into this file, as
/// `(library id, root node on the library page, name)`.
pub fn library_component_copies(doc: &Document) -> Vec<(String, NodeId, String)> {
    doc.library
        .external
        .values()
        .filter(|e| matches!(e.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet))
        .filter_map(|e| e.node.and_then(|n| doc.get(n)).map(|n| (e.library_id.clone(), n.id, n.name.clone())))
        .collect()
}

// ---------------------------------------------------------------------------
// Consumer side: pending updates
// ---------------------------------------------------------------------------

fn local_refs(doc: &Document, lib: &str) -> BTreeMap<NodeId, String> {
    let prefix = format!("{lib}/");
    let entries: Vec<(String, NodeId)> = doc
        .library
        .external
        .iter()
        .filter(|(_, e)| e.library_id == lib && matches!(e.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet))
        .filter_map(|(k, e)| e.node.map(|n| (strip(k, &prefix).to_string(), n)))
        .collect();
    ref_map(&doc.nodes, &entries)
}

fn local_fingerprint(doc: &Document, lib: &str, raw_key: &str, e: &ExternalAsset, refs: &BTreeMap<NodeId, String>) -> Option<u64> {
    let prefix = format!("{lib}/");
    Some(match e.kind {
        LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => tree_fingerprint(&doc.nodes, e.node?, refs, &prefix),
        LibraryAssetKind::Style => {
            let k = e.local_key.as_deref()?;
            style_fingerprint(doc.styles.get(k)?, doc.style_meta.get(k))
        }
        LibraryAssetKind::Variable => variable_fingerprint(doc.variables.get(e.local_key.as_deref()?)?, &prefix),
        LibraryAssetKind::VariableCollection => collection_fingerprint(doc.variable_collections.get(e.local_key.as_deref()?)?),
    })
    .filter(|_| !raw_key.is_empty())
}

fn local_name(doc: &Document, e: &ExternalAsset) -> String {
    match e.kind {
        LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => e.node.and_then(|n| doc.get(n)).map(|n| n.name.clone()).unwrap_or_default(),
        LibraryAssetKind::Style => e.local_key.as_deref().map(|k| doc.style_name(k).to_string()).unwrap_or_default(),
        LibraryAssetKind::Variable => e.local_key.as_deref().and_then(|k| doc.variables.get(k)).map(|v| v.name.clone()).unwrap_or_default(),
        LibraryAssetKind::VariableCollection => {
            e.local_key.as_deref().and_then(|k| doc.variable_collections.get(k)).map(|c| c.name.clone()).unwrap_or_default()
        }
    }
}

/// Differences between this file's copies of the library's assets and
/// `snap`: changed assets, assets the library dropped (the local copy stays,
/// flagged "missing from library") and variables added to imported collections.
pub fn pending_updates(doc: &Document, snap: &LibrarySnapshot) -> Vec<Update> {
    let lib = snap.library_id.as_str();
    let prefix = format!("{lib}/");
    let refs = local_refs(doc, lib);
    let mut out = Vec::new();
    for (ekey, e) in doc.library.external.iter().filter(|(_, e)| e.library_id == lib) {
        let raw = strip(ekey, &prefix);
        match snap.asset(raw) {
            None => {
                // Dependencies pulled in by reference are not assets of their own.
                if snap.styles.contains_key(strip(raw, "style:")) && raw.starts_with("style:")
                    || snap.variables.contains_key(strip(raw, "var:")) && raw.starts_with("var:")
                    || snap.variable_collections.contains_key(strip(raw, "col:")) && raw.starts_with("col:")
                {
                    continue;
                }
                out.push(Update { key: raw.to_string(), kind: e.kind, name: local_name(doc, e), change: UpdateKind::Removed })
            }
            Some(a) => {
                if local_fingerprint(doc, lib, raw, e, &refs) != Some(a.fingerprint) {
                    out.push(Update { key: raw.to_string(), kind: e.kind, name: a.name.clone(), change: UpdateKind::Changed });
                }
            }
        }
    }
    // Variables added to a collection that is already imported.
    for a in snap.assets.iter().filter(|a| a.kind == LibraryAssetKind::Variable) {
        if doc.library.external.contains_key(&local_key(lib, &a.key)) {
            continue;
        }
        let coll = snap.variables.get(&a.key["var:".len()..]).map(|v| v.collection.clone());
        if coll.is_some_and(|c| doc.library.external.contains_key(&local_key(lib, &format!("col:{c}")))) {
            out.push(Update { key: a.key.clone(), kind: a.kind, name: a.name.clone(), change: UpdateKind::Added });
        }
    }
    out.sort_by(|a, b| (a.name.to_lowercase(), &a.key).cmp(&(b.name.to_lowercase(), &b.key)));
    out
}

/// Keys of imported assets the library no longer has.
pub fn missing_from_library(doc: &Document, snap: &LibrarySnapshot) -> Vec<String> {
    pending_updates(doc, snap).into_iter().filter(|u| u.change == UpdateKind::Removed).map(|u| u.key).collect()
}

// ---------------------------------------------------------------------------
// Consumer side: copying
// ---------------------------------------------------------------------------

/// Pairs the nodes of a local copy with those of the snapshot subtree by
/// structure (name and kind, then position), so ids survive updates. Maps
/// snapshot id -> local id.
fn match_trees(doc: &Document, local: NodeId, snap: &LibrarySnapshot, sroot: NodeId, out: &mut BTreeMap<NodeId, NodeId>) {
    out.insert(sroot, local);
    let (Some(l), Some(s)) = (doc.get(local), snap.nodes.get(&sroot)) else { return };
    let mut used = vec![false; l.children.len()];
    let mut pairs: Vec<Option<usize>> = vec![None; s.children.len()];
    let kind_of = |n: &Node| std::mem::discriminant(&n.kind);
    for (i, sc) in s.children.iter().enumerate() {
        let Some(sn) = snap.nodes.get(sc) else { continue };
        let hit = l.children.iter().enumerate().position(|(j, lc)| !used[j] && doc.get(*lc).is_some_and(|ln| ln.name == sn.name && kind_of(ln) == kind_of(sn)));
        if let Some(j) = hit {
            used[j] = true;
            pairs[i] = Some(j);
        }
    }
    for (i, sc) in s.children.iter().enumerate() {
        if pairs[i].is_some() {
            continue;
        }
        let Some(sn) = snap.nodes.get(sc) else { continue };
        let hit = l.children.iter().enumerate().position(|(j, lc)| !used[j] && doc.get(*lc).is_some_and(|ln| kind_of(ln) == kind_of(sn)));
        if let Some(j) = hit {
            used[j] = true;
            pairs[i] = Some(j);
        }
    }
    for (i, p) in pairs.into_iter().enumerate() {
        if let Some(j) = p {
            match_trees(doc, l.children[j], snap, s.children[i], out);
        }
    }
}

/// The hidden library page, created on first use.
pub fn ensure_library_page(doc: &mut Document) -> NodeId {
    if let Some(p) = doc.library_page.filter(|p| doc.nodes.contains_key(p)) {
        return p;
    }
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Page { background: Color::rgb(0x5A, 0x5A, 0x5A) });
    n.name = "Library".into();
    doc.nodes.insert(id, n);
    doc.library_page = Some(id);
    doc.mark_node_dirty(id);
    id
}

/// Applies commands to the document as it goes (so later steps see earlier
/// ones) and reverts them on `finish`, returning the batch.
struct Trial<'a> {
    doc: &'a mut Document,
    snap: &'a LibrarySnapshot,
    cmds: Vec<Command>,
    inv: Vec<Command>,
    visiting: BTreeSet<String>,
}

impl<'a> Trial<'a> {
    fn new(doc: &'a mut Document, snap: &'a LibrarySnapshot) -> Self {
        Trial { doc, snap, cmds: Vec::new(), inv: Vec::new(), visiting: BTreeSet::new() }
    }

    fn push(&mut self, c: Command) {
        if let Ok(inv) = c.clone().apply(self.doc) {
            self.cmds.push(c);
            self.inv.push(inv);
        }
    }

    fn finish(self) -> Command {
        for inv in self.inv.into_iter().rev() {
            let _ = inv.apply(self.doc);
        }
        Command::Batch(self.cmds)
    }

    fn lib(&self) -> &'a str {
        self.snap.library_id.as_str()
    }

    fn ns(&self, k: &str) -> String {
        local_key(self.lib(), k)
    }

    fn set_state(&mut self, f: impl FnOnce(&mut LibraryState)) {
        let mut st = self.doc.library.clone();
        f(&mut st);
        let page = self.doc.library_page;
        self.push(Command::SetLibraryState { state: st, library_page: page });
    }

    fn register(&mut self, raw_key: &str, e: ExternalAsset) {
        let ekey = self.ns(raw_key);
        let (id, name, version) = (self.snap.library_id.clone(), self.snap.name.clone(), self.snap.version);
        self.set_state(|st| {
            st.external.insert(ekey, e);
            if !st.enabled.iter().any(|l| l.id == id) {
                st.enabled.push(LibraryRef { id, name, version });
            }
        });
    }

    fn edit_vars(&mut self, f: impl FnOnce(&mut BTreeMap<String, Variable>, &mut BTreeMap<String, VariableCollection>)) {
        let mut v = self.doc.variables.clone();
        let mut c = self.doc.variable_collections.clone();
        f(&mut v, &mut c);
        self.push(Command::SetVariables { variables: v, collections: c });
    }

    /// Writes style `local` (and its metadata) through `Command::SetStyles` so it is undoable.
    fn set_style(&mut self, local: &str, style: Style, meta: Option<StyleMeta>) {
        let mut styles = self.doc.styles.clone();
        let mut style_meta = self.doc.style_meta.clone();
        styles.insert(local.to_string(), style);
        if let Some(m) = meta {
            style_meta.insert(local.to_string(), StyleMeta { key: None, ..m });
        }
        self.push(Command::SetStyles { styles, style_meta });
    }

    fn imported(&self, raw_key: &str) -> Option<&ExternalAsset> {
        self.doc.library.external.get(&self.ns(raw_key))
    }

    /// Imports the asset `key`, with everything it depends on. Idempotent.
    fn import(&mut self, key: &str) {
        if self.imported(key).is_some() {
            return;
        }
        let Some(asset) = self.snap.asset(key).cloned() else { return };
        match asset.kind {
            LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => self.import_component(&asset),
            LibraryAssetKind::Style => self.import_style(&key["style:".len()..]),
            LibraryAssetKind::Variable => self.import_variable(&key["var:".len()..]),
            LibraryAssetKind::VariableCollection => {
                let raw = &key["col:".len()..];
                self.import_collection(raw);
                let members: Vec<String> = self.snap.variable_collections.get(raw).map(|c| c.variables.clone()).unwrap_or_default();
                for v in members.iter().filter(|v| self.snap.variables.contains_key(*v)) {
                    self.import_variable(v);
                }
            }
        }
    }

    fn import_style(&mut self, raw: &str) {
        let key = format!("style:{raw}");
        if self.imported(&key).is_some() {
            return;
        }
        let Some(style) = self.snap.styles.get(raw).cloned() else { return };
        let local = self.ns(raw);
        self.set_style(&local, style, self.snap.style_meta.get(raw).cloned());
        let version = self.snap.version;
        let library_id = self.snap.library_id.clone();
        self.register(&key, ExternalAsset { library_id, version, kind: LibraryAssetKind::Style, node: None, local_key: Some(local) });
    }

    fn import_collection(&mut self, raw: &str) {
        let key = format!("col:{raw}");
        if self.imported(&key).is_some() {
            return;
        }
        let Some(c) = self.snap.variable_collections.get(raw).cloned() else { return };
        let local = self.ns(raw);
        // the copy is a subscribed one: read-only here, edited in its library file
        let copy = VariableCollection { id: local.clone(), variables: Vec::new(), key: None, source_library_key: Some(self.snap.library_id.clone()), ..c };
        self.edit_vars(|_, colls| {
            colls.insert(local.clone(), copy);
        });
        let version = self.snap.version;
        let library_id = self.snap.library_id.clone();
        self.register(&key, ExternalAsset { library_id, version, kind: LibraryAssetKind::VariableCollection, node: None, local_key: Some(local) });
    }

    fn import_variable(&mut self, raw: &str) {
        let key = format!("var:{raw}");
        if self.imported(&key).is_some() || !self.visiting.insert(raw.to_string()) {
            return;
        }
        let Some(v) = self.snap.variables.get(raw).cloned() else { return };
        self.import_collection(&v.collection);
        for val in v.values.values() {
            if let VariableValue::Alias(a) = val {
                self.import_variable(a);
            }
        }
        let local = self.ns(raw);
        let copy = self.rewrite_variable(&v);
        let coll = copy.collection.clone();
        self.edit_vars(|vars, colls| {
            vars.insert(local.clone(), copy);
            if let Some(c) = colls.get_mut(&coll) {
                if !c.variables.contains(&local) {
                    c.variables.push(local.clone());
                }
            }
        });
        let version = self.snap.version;
        let library_id = self.snap.library_id.clone();
        self.register(&key, ExternalAsset { library_id, version, kind: LibraryAssetKind::Variable, node: None, local_key: Some(local) });
    }

    fn rewrite_variable(&self, v: &Variable) -> Variable {
        let mut c = v.clone();
        c.id = self.ns(&v.id);
        c.collection = self.ns(&v.collection);
        c.key = None;
        for val in c.values.values_mut() {
            if let VariableValue::Alias(a) = val {
                *a = self.ns(a);
            }
        }
        c
    }

    /// Imports the styles and variables the snapshot subtree `ids` refers to.
    fn import_refs(&mut self, ids: &[NodeId]) {
        let mut styles = BTreeSet::new();
        let mut vars = BTreeSet::new();
        let mut colls = BTreeSet::new();
        for id in ids {
            let n = &self.snap.nodes[id];
            styles.extend(n.style_refs.values().cloned());
            vars.extend(n.variables.bindings.values().cloned());
            colls.extend(n.variables.modes.keys().cloned());
        }
        for s in styles {
            self.import_style(&s);
        }
        for c in colls {
            self.import_collection(&c);
        }
        for v in vars {
            self.import_variable(&v);
        }
    }

    /// Snapshot id -> local id for every library component already copied
    /// (matched structurally against its snapshot subtree).
    fn existing_map(&self) -> BTreeMap<NodeId, NodeId> {
        let prefix = format!("{}/", self.lib());
        let mut map = BTreeMap::new();
        for (k, e) in &self.doc.library.external {
            if e.library_id != self.lib() || !matches!(e.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet) {
                continue;
            }
            let (Some(local), Some(sroot)) = (e.node, self.snap.asset(strip(k, &prefix)).and_then(|a| a.node)) else { continue };
            if self.doc.get(local).is_some() {
                match_trees(self.doc, local, self.snap, sroot, &mut map);
            }
        }
        map
    }

    fn convert(&self, id: NodeId, parent: Option<NodeId>, idmap: &BTreeMap<NodeId, NodeId>) -> Node {
        let mut n = self.snap.nodes[&id].clone();
        n.id = idmap[&id];
        n.parent = parent;
        n.children = n.children.iter().filter_map(|c| idmap.get(c).copied()).collect();
        n.source_id = None;
        n.component.source = n.component.source.and_then(|s| idmap.get(&s).copied());
        if let NodeKind::Instance { component, .. } = &mut n.kind {
            *component = component.and_then(|c| idmap.get(&c).copied());
        }
        for v in n.style_refs.values_mut() {
            *v = self.ns(v);
        }
        for v in n.variables.bindings.values_mut() {
            *v = self.ns(v);
        }
        n.variables.modes = std::mem::take(&mut n.variables.modes).into_iter().map(|(k, v)| (self.ns(&k), v)).collect();
        n
    }

    fn copy_images(&mut self, ids: &[NodeId]) {
        let mut refs = BTreeSet::new();
        for id in ids {
            image_refs_of(&self.snap.nodes[id], &mut refs);
        }
        for r in refs {
            if let Some(b) = self.snap.images.get(&r) {
                self.doc.images.entry(r).or_insert_with(|| b.clone());
            }
        }
    }

    fn import_component(&mut self, asset: &SnapshotAsset) {
        let Some(sroot) = asset.node.filter(|n| self.snap.nodes.contains_key(n)) else { return };
        for d in &asset.deps {
            self.import(d);
        }
        let page = ensure_library_page(self.doc);
        let ids = subtree(&self.snap.nodes, sroot);
        self.import_refs(&ids);
        self.copy_images(&ids);
        let mut idmap = self.existing_map();
        for id in &ids {
            idmap.insert(*id, self.doc.alloc_id());
        }
        let nodes: Vec<Node> = ids
            .iter()
            .map(|id| self.convert(*id, if *id == sroot { Some(page) } else { self.snap.nodes[id].parent.and_then(|p| idmap.get(&p).copied()) }, &idmap))
            .collect();
        let index = self.doc.get(page).map(|p| p.children.len()).unwrap_or(0);
        let root = idmap[&sroot];
        self.push(Command::Insert { parent: page, index, nodes });
        let (version, library_id) = (self.snap.version, self.snap.library_id.clone());
        self.register(&asset.key, ExternalAsset { library_id, version, kind: asset.kind, node: Some(root), local_key: None });
    }

    fn update_component(&mut self, asset: &SnapshotAsset) {
        let Some(sroot) = asset.node.filter(|n| self.snap.nodes.contains_key(n)) else { return };
        let Some(local_root) = self.imported(&asset.key).and_then(|e| e.node).filter(|n| self.doc.get(*n).is_some()) else {
            self.import(&asset.key);
            return;
        };
        for d in &asset.deps {
            self.import(d);
        }
        let ids = subtree(&self.snap.nodes, sroot);
        self.import_refs(&ids);
        self.copy_images(&ids);
        let mut idmap = self.existing_map();
        for id in &ids {
            if !idmap.contains_key(id) {
                idmap.insert(*id, self.doc.alloc_id());
            }
        }
        let Some(parent) = self.doc.get(local_root).and_then(|n| n.parent) else { return };
        let index = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == local_root)).unwrap_or(0);
        let nodes: Vec<Node> = ids
            .iter()
            .map(|id| self.convert(*id, if *id == sroot { Some(parent) } else { self.snap.nodes[id].parent.and_then(|p| idmap.get(&p).copied()) }, &idmap))
            .collect();
        self.push(Command::Remove { id: local_root });
        self.push(Command::Insert { parent, index, nodes });
    }

    fn update_style(&mut self, raw: &str) {
        let Some(style) = self.snap.styles.get(raw).cloned() else { return };
        let local = self.ns(raw);
        self.set_style(&local, style.clone(), self.snap.style_meta.get(raw).cloned());
        let targets: Vec<NodeId> =
            self.doc.nodes.values().filter(|n| n.style_refs.values().any(|v| *v == local) && !is_library_node(self.doc, n.id)).map(|n| n.id).collect();
        for id in targets {
            let mut n = self.doc.nodes[&id].clone();
            if apply_style_values(&mut n, &style, &local) {
                self.push(Command::Update { node: Box::new(n) });
            }
        }
    }

    fn update_collection(&mut self, raw: &str) {
        let Some(c) = self.snap.variable_collections.get(raw).cloned() else { return };
        let local = self.ns(raw);
        self.edit_vars(|_, colls| {
            if let Some(cur) = colls.get_mut(&local) {
                cur.name = c.name.clone();
                cur.modes = c.modes.clone();
                cur.default_mode = c.default_mode.clone();
            }
        });
    }

    fn update_variable(&mut self, raw: &str) {
        let Some(v) = self.snap.variables.get(raw).cloned() else { return };
        for val in v.values.values() {
            if let VariableValue::Alias(a) = val {
                self.import_variable(a);
            }
        }
        let local = self.ns(raw);
        let copy = self.rewrite_variable(&v);
        self.edit_vars(|vars, _| {
            vars.insert(local, copy);
        });
    }
}

/// The property a style applies to (metadata, else derived from its kind).
pub fn style_property(doc: &Document, key: &str) -> StyleProperty {
    doc.style_meta.get(key).map(|m| m.property).unwrap_or(match doc.styles.get(key) {
        Some(Style::Text(_)) => StyleProperty::Text,
        Some(Style::Effect(_)) => StyleProperty::Effect,
        Some(Style::Grid(_)) => StyleProperty::Grid,
        _ => StyleProperty::Fill,
    })
}

/// Copies a shared style's values onto `n` for every property it is linked to
/// (the same rules as applying a style in the editor, minus text re-fitting).
/// A colour style may be linked as a fill, as a stroke, or as both; each side
/// whose link names `key` takes the paints. Returns whether the node changed.
pub fn apply_style_values(n: &mut Node, style: &Style, key: &str) -> bool {
    let before = n.clone();
    match style {
        Style::Fill(paints) => {
            if n.style_refs.get("stroke").is_some_and(|k| k == key) {
                n.stroke.paints = paints.clone();
            }
            if n.style_refs.get("fill").is_some_and(|k| k == key) {
                match &mut n.kind {
                    NodeKind::Text(t) => {
                        // Runs carry their own colour; a linked colour style recolours them all.
                        t.style.fills = paints.clone();
                        for r in &mut t.runs {
                            r.style.fills = paints.clone();
                        }
                    }
                    _ => n.fills = paints.clone(),
                }
            }
        }
        Style::Text(ts) => {
            if let NodeKind::Text(t) = &mut n.kind {
                if n.style_refs.get("text").is_some_and(|k| k == key) {
                    t.style = TextStyle { fills: t.style.fills.clone(), ..ts.clone() };
                    for r in &mut t.runs {
                        r.style = TextStyle { fills: r.style.fills.clone(), ..ts.clone() };
                    }
                }
            }
        }
        Style::Effect(fx) => {
            if n.style_refs.get("effect").is_some_and(|k| k == key) {
                n.effects = fx.clone();
            }
        }
        Style::Grid(g) => {
            if n.style_refs.get("grid").is_some_and(|k| k == key) {
                if let Some(f) = n.kind.frame_props_mut() {
                    f.grids = g.clone();
                }
            }
        }
    }
    *n != before
}

/// Where to put an instance of an imported component.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub parent: NodeId,
    pub at: Vec2,
}

/// Copies the asset `key` (and what it depends on) from `snap` into `doc`:
/// components and sets land on the hidden library page with fresh ids, styles
/// and variables under namespaced keys; the library is recorded in
/// [`LibraryState`] (enabled, pinned to the snapshot version). With
/// `place`, an instance of the (first variant of the) component is inserted.
/// Importing an asset that is already imported only places the instance.
pub fn import_asset(doc: &mut Document, snap: &LibrarySnapshot, key: &str, place: Option<Placement>) -> Command {
    let Some(asset) = snap.asset(key).cloned() else { return nothing() };
    let mut t = Trial::new(doc, snap);
    t.import(key);
    if let Some(p) = place {
        if let Some(root) = t.imported(key).and_then(|e| e.node) {
            let comp = match t.doc.get(root).map(|n| &n.kind) {
                Some(NodeKind::ComponentSet(_)) => components_of(t.doc, root).first().copied(),
                Some(NodeKind::Component(_)) => Some(root),
                _ => None,
            };
            if let Some(comp) = comp {
                let c = create_instance(t.doc, comp, p.parent, p.at);
                t.push(c);
            }
        }
    }
    let _ = asset;
    t.finish()
}

/// Imports a style by its snapshot key (`style:<key>` asset), returning the
/// command and the local style key.
pub fn import_style_key(doc: &mut Document, snap: &LibrarySnapshot, asset_key: &str) -> (Command, Option<String>) {
    let cmd = import_asset(doc, snap, asset_key, None);
    let local = doc.library.external.get(&local_key(&snap.library_id, asset_key)).and_then(|e| e.local_key.clone());
    // The command has been reverted: the style itself is additive, so the key is valid.
    (cmd, local.or_else(|| asset_key.strip_prefix("style:").map(|s| local_key(&snap.library_id, s))))
}

fn topo(snap: &LibrarySnapshot, keys: &BTreeSet<String>) -> Vec<String> {
    fn visit(snap: &LibrarySnapshot, k: &str, keys: &BTreeSet<String>, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
        if !seen.insert(k.to_string()) {
            return;
        }
        if let Some(a) = snap.asset(k) {
            for d in &a.deps {
                if keys.contains(d) {
                    visit(snap, d, keys, seen, out);
                }
            }
        }
        out.push(k.to_string());
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for k in keys {
        visit(snap, k, keys, &mut seen, &mut out);
    }
    out
}

/// Replaces the local copies of the assets named by `keys` with the snapshot's
/// version, keeping ids stable where the structure matches, then re-syncs the
/// instances of every affected component (their overrides are preserved).
/// Assets the library removed are left alone. One undo step.
pub fn apply_updates(doc: &mut Document, snap: &LibrarySnapshot, keys: &[String]) -> Command {
    let pending = pending_updates(doc, snap);
    let chosen: Vec<&Update> = pending.iter().filter(|u| keys.contains(&u.key) && u.change != UpdateKind::Removed).collect();
    if chosen.is_empty() {
        return nothing();
    }
    let mut t = Trial::new(doc, snap);
    // Added first so changed assets can reference them.
    for u in chosen.iter().filter(|u| u.change == UpdateKind::Added) {
        t.import(&u.key);
    }
    let changed: Vec<&&Update> = chosen.iter().filter(|u| u.change == UpdateKind::Changed).collect();
    for u in changed.iter().filter(|u| u.kind == LibraryAssetKind::VariableCollection) {
        t.update_collection(&u.key["col:".len()..]);
    }
    for u in changed.iter().filter(|u| u.kind == LibraryAssetKind::Variable) {
        t.update_variable(&u.key["var:".len()..]);
    }
    for u in changed.iter().filter(|u| u.kind == LibraryAssetKind::Style) {
        t.update_style(&u.key["style:".len()..]);
    }
    let comp_keys: BTreeSet<String> =
        changed.iter().filter(|u| matches!(u.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet)).map(|u| u.key.clone()).collect();
    let order = topo(snap, &comp_keys);
    for k in &order {
        if let Some(a) = snap.asset(k).cloned() {
            t.update_component(&a);
        }
    }
    // Components that nest an updated one need syncing too (nearest last).
    let mut affected: BTreeSet<String> = comp_keys.clone();
    loop {
        let more: Vec<String> = snap
            .assets
            .iter()
            .filter(|a| !affected.contains(&a.key) && a.deps.iter().any(|d| affected.contains(d)) && t.imported(&a.key).is_some())
            .map(|a| a.key.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        affected.extend(more);
    }
    for k in topo(snap, &affected) {
        if let Some(root) = t.imported(&k).and_then(|e| e.node).filter(|n| t.doc.get(*n).is_some()) {
            let c = sync_instances(t.doc, root);
            t.push(c);
        }
    }
    // Bump the pinned versions.
    let applied: BTreeSet<String> = chosen.iter().map(|u| t.ns(&u.key)).collect();
    let all_done = pending.iter().filter(|u| u.change == UpdateKind::Changed).all(|u| keys.contains(&u.key));
    let (lib, version) = (snap.library_id.clone(), snap.version);
    t.set_state(|st| {
        for (k, e) in st.external.iter_mut() {
            if applied.contains(k) {
                e.version = version;
            }
        }
        if all_done {
            if let Some(r) = st.enabled.iter_mut().find(|r| r.id == lib) {
                r.version = version;
            }
        }
    });
    t.finish()
}

/// Enables `snap`'s library in this file, pinned to its version.
pub fn enable_library(doc: &Document, snap: &LibrarySnapshot) -> Command {
    let mut st = doc.library.clone();
    st.enabled.retain(|l| l.id != snap.library_id);
    st.enabled.push(LibraryRef { id: snap.library_id.clone(), name: snap.name.clone(), version: snap.version });
    Command::SetLibraryState { state: st, library_page: doc.library_page }
}

/// Disables a library. Local copies and their instances stay.
pub fn disable_library(doc: &Document, library_id: &str) -> Command {
    let mut st = doc.library.clone();
    st.enabled.retain(|l| l.id != library_id);
    Command::SetLibraryState { state: st, library_page: doc.library_page }
}

/// Records the file as published (after the snapshot was stored).
pub fn set_published(doc: &Document, info: PublishInfo) -> Command {
    let mut st = doc.library.clone();
    st.published = Some(info);
    Command::SetLibraryState { state: st, library_page: doc.library_page }
}

/// Importer hook: makes instances of a component that lives in another (here
/// unavailable) file work. Builds a stand-in main component on the hidden
/// library page from the first instance, points every instance at it (keeping
/// what differs between instances as overrides) and records it in
/// [`LibraryState::external`] under `<library_id>/<key>`. `library_id` should
/// start with [`REMOTE_LIBRARY_PREFIX`]; the library is not "enabled". The
/// document is edited directly: importers have no history. Returns the
/// stand-in component.
pub fn link_remote_component(doc: &mut Document, library_id: &str, key: &str, name: &str, instances: &[NodeId]) -> Option<NodeId> {
    let first = *instances.first()?;
    let tmpl = doc.get(first)?.clone();
    let NodeKind::Instance { frame, .. } = &tmpl.kind else { return None };
    let page = ensure_library_page(doc);
    let cid = doc.alloc_id();
    let mut comp = tmpl.clone();
    comp.id = cid;
    comp.parent = None;
    comp.children.clear();
    comp.source_id = None;
    comp.name = name.to_string();
    comp.transform = Affine::IDENTITY;
    comp.visible = true;
    comp.component = ComponentLink::default();
    comp.prototype = PrototypeProps::default();
    comp.kind = NodeKind::Component(frame.clone());
    let cid = doc.insert(page, comp).ok()?;

    fn copy(doc: &mut Document, src: NodeId, parent: NodeId) {
        let Some(orig) = doc.get(src).cloned() else { return };
        let id = doc.alloc_id();
        let mut n = orig.clone();
        n.id = id;
        n.children.clear();
        n.parent = None;
        n.source_id = None;
        if doc.insert(parent, n).is_ok() {
            for k in orig.children {
                copy(doc, k, id);
            }
        }
    }
    for k in tmpl.children.clone() {
        copy(doc, k, cid);
    }

    /// Pairs the children of `inst` with those of `comp` and records what differs.
    fn link(doc: &mut Document, inst: NodeId, comp: NodeId) {
        let (Some(i), Some(c)) = (doc.get(inst).cloned(), doc.get(comp).cloned()) else { return };
        let mut used = vec![false; c.children.len()];
        let mut pairs = Vec::new();
        for pass in 0..2 {
            for ik in &i.children {
                if pairs.iter().any(|(a, _)| a == ik) {
                    continue;
                }
                let Some(inode) = doc.get(*ik) else { continue };
                let hit = c.children.iter().enumerate().position(|(j, ck)| {
                    !used[j]
                        && doc
                            .get(*ck)
                            .is_some_and(|cn| std::mem::discriminant(&cn.kind) == std::mem::discriminant(&inode.kind) && (pass == 1 || cn.name == inode.name))
                });
                if let Some(j) = hit {
                    used[j] = true;
                    pairs.push((*ik, c.children[j]));
                }
            }
        }
        for (ik, ck) in pairs {
            let (Some(mut inode), Some(cnode)) = (doc.get(ik).cloned(), doc.get(ck).cloned()) else { continue };
            if inode.component.source.is_none() {
                inode.component.source = Some(ck);
                inode.component.overrides = record_override(&cnode, &inode);
                inode.component.grid_align_overrides = record_grid_align_overrides(&cnode, &inode);
                if let Some(slot) = doc.get_mut(ik) {
                    *slot = inode;
                }
                link(doc, ik, ck);
            }
        }
    }
    for &inst in instances {
        let Some(mut n) = doc.get(inst).cloned() else { continue };
        let base = doc.get(cid).cloned()?;
        let mut ov = record_override(&base, &n);
        let align_ov = record_grid_align_overrides(&base, &n);
        for f in [OverrideField::Transform, OverrideField::Visible, OverrideField::Locked] {
            ov.remove(&f);
        }
        if let NodeKind::Instance { component, .. } = &mut n.kind {
            *component = Some(cid);
        }
        n.component.source = Some(cid);
        n.component.overrides = ov;
        n.component.grid_align_overrides = align_ov;
        if let Some(slot) = doc.get_mut(inst) {
            *slot = n;
        }
        link(doc, inst, cid);
    }
    doc.library.external.insert(
        local_key(library_id, key),
        ExternalAsset { library_id: library_id.to_string(), version: 0, kind: LibraryAssetKind::Component, node: Some(cid), local_key: None },
    );
    Some(cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib_doc() -> Document {
        let mut d = Document::new("Lib");
        let page = d.add_page("P");
        d.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(255, 0, 0))]));
        let coll = VariableCollection {
            id: "c1".into(),
            name: "Core".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
            default_mode: "m".into(),
            variables: vec!["v1".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        };
        let coll2 = VariableCollection { id: "c2".into(), name: "Alias".into(), variables: vec!["v2".into()], ..coll.clone() };
        d.variable_collections.insert("c1".into(), coll);
        d.variable_collections.insert("c2".into(), coll2);
        let mk = |id: &str, name: &str, coll: &str, val: VariableValue| Variable {
            id: id.into(),
            name: name.into(),
            collection: coll.into(),
            kind: VariableKind::Number,
            values: [("m".to_string(), val)].into(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,

            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        };
        d.variables.insert("v1".into(), mk("v1", "Base", "c1", VariableValue::Number(8.0)));
        d.variables.insert("v2".into(), mk("v2", "Gap", "c2", VariableValue::Alias("v1".into())));

        let cid = d.alloc_id();
        let mut comp = Node::new(cid, NodeKind::Component(FrameProps::default()));
        comp.name = "Button".into();
        comp.size = Vec2::new(100.0, 40.0);
        comp.variables.bindings.insert("itemSpacing".into(), "v2".into());
        let comp = d.insert(page, comp).unwrap();
        let lid = d.alloc_id();
        let mut label = Node::text(lid, Rect::new(0.0, 0.0, 50.0, 20.0), "Label", TextStyle::default());
        label.name = "Label".into();
        label.style_refs.insert("fill".into(), "Brand".into());
        d.insert(comp, label).unwrap();
        d
    }

    fn consumer() -> Document {
        let mut d = Document::new("App");
        d.add_page("Main");
        d
    }

    fn run(d: &mut Document, c: Command) {
        c.apply(d).unwrap();
    }

    fn button_key(d: &Document) -> String {
        publishable_assets(d).into_iter().find(|a| a.2 == "Button").unwrap().0
    }

    #[test]
    fn publishable_keys_are_stable() {
        let d = lib_doc();
        let a = publishable_assets(&d);
        assert!(a.iter().any(|(k, kind, n)| k.starts_with('c') && *kind == LibraryAssetKind::Component && n == "Button"));
        assert!(a.iter().any(|(k, kind, _)| k == "style:Brand" && *kind == LibraryAssetKind::Style));
        assert!(a.iter().any(|(k, _, _)| k == "var:v2"));
        assert!(a.iter().any(|(k, _, _)| k == "col:c1"));
        assert_eq!(a, publishable_assets(&d));
    }

    #[test]
    fn import_copies_component_styles_and_variables_with_alias_closure() {
        let lib = lib_doc();
        let snap = snapshot(&lib, "L1", "Lib", 1);
        let mut d = consumer();
        let key = button_key(&lib);
        let page = d.pages[0];
        let cmd = import_asset(&mut d, &snap, &key, Some(Placement { parent: page, at: Vec2::new(5.0, 6.0) }));
        let mut h = History::default();
        h.exec(&mut d, cmd).unwrap();
        // Hidden page, not listed in pages.
        let lp = d.library_page.unwrap();
        assert!(!d.pages.contains(&lp));
        assert_eq!(d.get(lp).unwrap().children.len(), 1);
        // Instance placed and linked.
        let inst = *d.get(page).unwrap().children.last().unwrap();
        let comp = instance_component(d.get(inst).unwrap()).unwrap();
        assert!(is_library_node(&d, comp));
        assert!(!is_library_node(&d, inst));
        // Style and alias chain copied under namespaced keys.
        assert!(d.styles.contains_key("L1/Brand"));
        assert!(d.variables.contains_key("L1/v2") && d.variables.contains_key("L1/v1"));
        assert!(matches!(&d.variables["L1/v2"].values["m"], VariableValue::Alias(a) if a == "L1/v1"));
        assert_eq!(d.variables["L1/v2"].collection, "L1/c2");
        assert!(d.variable_collections.contains_key("L1/c1") && d.variable_collections.contains_key("L1/c2"));
        // a collection copied from a library is marked as one, so it is read-only like a library style
        assert!(d.variable_collections.values().all(|c| c.source_library_key.as_deref() == Some("L1")));
        assert_eq!(d.get(comp).unwrap().variables.bindings["itemSpacing"], "L1/v2");
        let info = library_instance_info(&d, inst).unwrap();
        assert_eq!((info.library_name.as_str(), info.asset_name.as_str(), info.remote), ("Lib", "Button", false));
        assert_eq!(d.library.enabled[0].version, 1);
        assert_eq!(pending_updates(&d, &snap), vec![]);
        // One undo step removes instance and component copy.
        h.undo(&mut d).unwrap();
        assert!(d.library.external.is_empty());
        assert!(d.get(d.library_page.unwrap()).unwrap().children.is_empty());
    }

    #[test]
    fn updates_keep_overrides_and_ids() {
        let mut lib = lib_doc();
        let snap1 = snapshot(&lib, "L1", "Lib", 1);
        let key = button_key(&lib);
        let mut d = consumer();
        let page = d.pages[0];
        let cmd = import_asset(&mut d, &snap1, &key, Some(Placement { parent: page, at: Vec2::new(0.0, 0.0) }));
        run(&mut d, cmd);
        let inst = *d.get(page).unwrap().children.last().unwrap();
        // Override the label text and size of the instance.
        let label_inst = d.get(inst).unwrap().children[0];
        let mut n = d.get(label_inst).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "Mine".into();
        }
        n.component.overrides.insert(OverrideField::Characters);
        run(&mut d, Command::Update { node: Box::new(n) });

        // Library: change the component's size and the label's fill style.
        let comp = lib.pages.iter().flat_map(|p| lib.get(*p).unwrap().children.clone()).next().unwrap();
        let mut cn = lib.get(comp).unwrap().clone();
        cn.size = Vec2::new(200.0, 40.0);
        run(&mut lib, Command::Update { node: Box::new(cn) });
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let ups = pending_updates(&d, &snap2);
        assert_eq!(ups.len(), 1, "{ups:?}");
        assert_eq!((ups[0].change, ups[0].kind), (UpdateKind::Changed, LibraryAssetKind::Component));

        let comp_local = instance_component(d.get(inst).unwrap()).unwrap();
        let label_local = d.get(comp_local).unwrap().children[0];
        let mut h = History::default();
        let cmd = apply_updates(&mut d, &snap2, &[ups[0].key.clone()]);
        h.exec(&mut d, cmd).unwrap();
        assert_eq!(d.get(comp_local).unwrap().size.x, 200.0);
        assert_eq!(d.get(label_local).unwrap().id, label_local, "child ids are stable");
        assert_eq!(d.get(inst).unwrap().size.x, 200.0, "instance follows the main component");
        let NodeKind::Text(t) = &d.get(d.get(inst).unwrap().children[0]).unwrap().kind else { panic!() };
        assert_eq!(t.content, "Mine", "override survives");
        assert_eq!(d.library.enabled[0].version, 2);
        assert!(pending_updates(&d, &snap2).is_empty());
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(comp_local).unwrap().size.x, 100.0);
        assert_eq!(d.library.enabled[0].version, 1);
    }

    #[test]
    fn removed_assets_are_flagged_and_kept() {
        let mut lib = lib_doc();
        let snap1 = snapshot(&lib, "L1", "Lib", 1);
        let key = button_key(&lib);
        let mut d = consumer();
        let c = import_asset(&mut d, &snap1, &key, None);
        run(&mut d, c);
        let comp = lib.pages.iter().flat_map(|p| lib.get(*p).unwrap().children.clone()).next().unwrap();
        run(&mut lib, Command::Remove { id: comp });
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let ups = pending_updates(&d, &snap2);
        assert!(ups.iter().any(|u| u.change == UpdateKind::Removed && u.name == "Button"));
        assert_eq!(missing_from_library(&d, &snap2), vec![key.clone()]);
        // Applying a removal changes nothing; the copy stays.
        let c = apply_updates(&mut d, &snap2, std::slice::from_ref(&key));
        assert!(matches!(&c, Command::Batch(v) if v.is_empty()));
        assert!(d.library.external.contains_key(&local_key("L1", &key)));
    }

    #[test]
    fn nested_library_instances_import_dependencies() {
        let mut lib = lib_doc();
        let page = lib.pages[0];
        let inner = lib.get(page).unwrap().children[0];
        // Second component containing an instance of the first.
        let oid = lib.alloc_id();
        let mut outer = Node::new(oid, NodeKind::Component(FrameProps::default()));
        outer.name = "Card".into();
        outer.size = Vec2::new(300.0, 200.0);
        let outer = lib.insert(page, outer).unwrap();
        let c = create_instance(&mut lib, inner, outer, Vec2::new(10.0, 10.0));
        run(&mut lib, c);
        let snap = snapshot(&lib, "L1", "Lib", 1);
        let card = snap.assets.iter().find(|a| a.name == "Card").unwrap();
        assert_eq!(card.deps.len(), 1);

        let mut d = consumer();
        let p = d.pages[0];
        let c = import_asset(&mut d, &snap, &card.key, Some(Placement { parent: p, at: Vec2::new(0.0, 0.0) }));
        run(&mut d, c);
        // Both components were copied; the nested instance points at the copy.
        assert_eq!(d.library.external.values().filter(|e| e.kind == LibraryAssetKind::Component).count(), 2);
        let card_local = d.library.external[&local_key("L1", &card.key)].node.unwrap();
        let nested = d.get(card_local).unwrap().children[0];
        let target = instance_component(d.get(nested).unwrap()).unwrap();
        assert!(is_library_node(&d, target));
        assert_eq!(d.get(target).unwrap().name, "Button");
        assert!(pending_updates(&d, &snap).is_empty());

        // Updating the inner component propagates through the outer instance.
        let inner_node = lib.get(inner).unwrap().clone();
        let mut n = inner_node;
        n.size = Vec2::new(180.0, 40.0);
        run(&mut lib, Command::Update { node: Box::new(n) });
        let c = sync_instances(&mut lib, inner);
        run(&mut lib, c);
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let keys: Vec<String> = pending_updates(&d, &snap2).into_iter().map(|u| u.key).collect();
        assert!(!keys.is_empty());
        let c = apply_updates(&mut d, &snap2, &keys);
        run(&mut d, c);
        assert_eq!(d.get(target).unwrap().size.x, 180.0);
        assert_eq!(d.get(nested).unwrap().size.x, 180.0);
        let top = *d.get(p).unwrap().children.last().unwrap();
        let top_nested = d.get(top).unwrap().children[0];
        assert_eq!(d.get(top_nested).unwrap().size.x, 180.0);
        assert!(pending_updates(&d, &snap2).is_empty());
    }

    #[test]
    fn variable_changes_and_additions() {
        let mut lib = lib_doc();
        let snap1 = snapshot(&lib, "L1", "Lib", 1);
        let mut d = consumer();
        let c = import_asset(&mut d, &snap1, "var:v2", None);
        run(&mut d, c);
        assert!(d.variables.contains_key("L1/v1"), "alias target pulled in across collections");
        // Library changes v1 and adds v3 to Core.
        lib.variables.get_mut("v1").unwrap().values.insert("m".into(), VariableValue::Number(16.0));
        let mut v3 = lib.variables["v1"].clone();
        v3.id = "v3".into();
        v3.name = "Extra".into();
        lib.variables.insert("v3".into(), v3);
        lib.variable_collections.get_mut("c1").unwrap().variables.push("v3".into());
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let ups = pending_updates(&d, &snap2);
        assert!(ups.iter().any(|u| u.key == "var:v1" && u.change == UpdateKind::Changed));
        assert!(ups.iter().any(|u| u.key == "var:v3" && u.change == UpdateKind::Added));
        let keys: Vec<String> = ups.into_iter().map(|u| u.key).collect();
        let c = apply_updates(&mut d, &snap2, &keys);
        run(&mut d, c);
        assert_eq!(d.resolve_variable(d.pages[0], "L1/v2"), Some(VariableValue::Number(16.0)));
        assert!(d.variables.contains_key("L1/v3"));
        assert!(pending_updates(&d, &snap2).is_empty());
    }

    #[test]
    fn snapshot_roundtrips_json_and_diffs() {
        let mut lib = lib_doc();
        let a = snapshot(&lib, "L1", "Lib", 1);
        let back: LibrarySnapshot = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&back).unwrap());
        assert!(diff_snapshots(Some(&a), &back).is_empty());
        assert!(diff_snapshots(None, &a).iter().all(|u| u.change == UpdateKind::Added));
        lib.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(0, 0, 255))]));
        let b = snapshot(&lib, "L1", "Lib", 2);
        let d = diff_snapshots(Some(&a), &b);
        assert!(d.iter().any(|u| u.key == "style:Brand" && u.change == UpdateKind::Changed));
        // The component only references the style by key: unchanged.
        assert!(!d.iter().any(|u| u.kind == LibraryAssetKind::Component));
    }

    #[test]
    fn style_update_reapplies_values() {
        let mut lib = lib_doc();
        let snap1 = snapshot(&lib, "L1", "Lib", 1);
        let mut d = consumer();
        let p = d.pages[0];
        let c = import_asset(&mut d, &snap1, "style:Brand", None);
        run(&mut d, c);
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 5.0, 5.0));
        r.style_refs.insert("fill".into(), "L1/Brand".into());
        d.insert(p, r).unwrap();
        lib.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(0, 0, 255))]));
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let c = apply_updates(&mut d, &snap2, &["style:Brand".into()]);
        run(&mut d, c);
        assert!(matches!(d.get(rid).unwrap().fills[0].kind, PaintKind::Solid(c) if c.b == 1.0));
    }

    #[test]
    fn style_update_reaches_layers_that_use_the_style_as_a_stroke() {
        let mut lib = lib_doc();
        let snap1 = snapshot(&lib, "L1", "Lib", 1);
        let mut d = consumer();
        let p = d.pages[0];
        let c = import_asset(&mut d, &snap1, "style:Brand", None);
        run(&mut d, c);
        let mut ids = Vec::new();
        for refs in [&["stroke"][..], &["fill", "stroke"][..]] {
            let id = d.alloc_id();
            let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0));
            for k in refs {
                r.style_refs.insert((*k).into(), "L1/Brand".into());
            }
            d.insert(p, r).unwrap();
            ids.push(id);
        }
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        lib.styles.insert("Brand".into(), Style::Fill(vec![blue.clone()]));
        let snap2 = snapshot(&lib, "L1", "Lib", 2);
        let c = apply_updates(&mut d, &snap2, &["style:Brand".into()]);
        run(&mut d, c);
        assert_eq!(d.get(ids[0]).unwrap().stroke.paints, vec![blue.clone()], "stroke-only link");
        assert_eq!(d.get(ids[1]).unwrap().stroke.paints, vec![blue.clone()], "stroke side of a fill + stroke link");
        assert_eq!(d.get(ids[1]).unwrap().fills, vec![blue]);
    }
}
