//! Part 3, instances: after every page is hydrated and every main is synced with no change, each sublayer Figma laid
//! out for an instance (`derivedSymbolData`) must keep the size and transform Figma stored, within 0.5 px.
//!
//! Sublayers are found by walking the instance next to its main: override paths name a layer by its main record's
//! `overrideKey` (or guid), and only nested instances add a step to the path.

use super::board::{broken_comparator, Part};
use super::keys::NodeKeys;
use super::raw::{affine_of, guid_path, keys_of, override_key, record_guid, vec2_of, Raw};
use designos_doc::*;
use designos_fig::kiwi::KiwiValue;
use std::collections::{HashMap, HashSet};

pub const TOL: f32 = 0.5;

pub struct Entry {
    pub path: Vec<String>,
    pub size: Option<Vec2>,
    pub transform: Option<Affine>,
}

pub struct InstanceTruth {
    pub guid: String,
    pub entries: Vec<Entry>,
    /// Override entries of the instance record: path and the fields it sets.
    pub overrides: Vec<(Vec<String>, Vec<String>)>,
}

pub struct Truth {
    pub instances: Vec<InstanceTruth>,
    /// Record guid -> its `overrideKey` (or its guid when it has none).
    pub path_key: HashMap<String, String>,
}

fn array<'a>(r: &'a KiwiValue, key: &str) -> &'a [KiwiValue] {
    r.get(key).or_else(|| r.get("symbolData").and_then(|s| s.get(key))).and_then(KiwiValue::as_array).unwrap_or(&[])
}

pub fn truth(raw: &Raw) -> Truth {
    let path_key: HashMap<String, String> = raw.live().filter_map(|r| Some((record_guid(r)?, override_key(r)?))).collect();
    let mut instances = Vec::new();
    for r in raw.live() {
        let derived = array(r, "derivedSymbolData");
        if derived.is_empty() {
            continue;
        }
        let Some(guid) = record_guid(r) else { continue };
        let entries = derived.iter().map(|o| Entry { path: guid_path(o), size: vec2_of(o, "size"), transform: affine_of(o, "transform") }).collect();
        let overrides = array(r, "symbolOverrides")
            .iter()
            .map(|o| (guid_path(o), keys_of(o).into_iter().filter(|k| *k != "guidPath").map(str::to_string).collect()))
            .collect();
        instances.push(InstanceTruth { guid, entries, overrides });
    }
    Truth { instances, path_key }
}

/// Instances whose stored derived data hydration refused: they carry derived data in the file, and after every page
/// is loaded they hold no record of an imported layout.
pub fn refused(t: &Truth, doc: &Document, keys: &NodeKeys) -> HashSet<String> {
    t.instances
        .iter()
        .filter(|i| keys.by_source.get(&i.guid).and_then(|id| doc.get(*id)).is_some_and(|n| n.component.imported_layout.is_none()))
        .map(|i| i.guid.clone())
        .collect()
}

/// Instances with derived data that sit on a page the import deferred (called before the pages are hydrated).
pub fn deferred(t: &Truth, doc: &Document) -> HashSet<String> {
    let wanted: HashSet<&str> = t.instances.iter().map(|i| i.guid.as_str()).collect();
    doc.nodes
        .values()
        .filter(|n| doc.is_pending_instance(n.id))
        .filter_map(|n| n.source_id.as_deref())
        .filter(|s| wanted.contains(s))
        .map(str::to_string)
        .collect()
}

/// A no-change sync of every main component that has instances.
pub fn sync_all(doc: &mut Document) -> usize {
    let mut mains: Vec<NodeId> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();
    mains.sort_by_key(|i| i.0);
    let limit = std::env::var("SCOREBOARD_SYNC_LIMIT").ok().and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let (mut n, t0) = (0usize, std::time::Instant::now());
    for m in mains.into_iter().take(limit) {
        drop(sync_instances(doc, m));
        n += 1;
        if n.is_multiple_of(1000) {
            eprintln!("synced {n} mains in {:.0} s", t0.elapsed().as_secs_f64());
        }
    }
    n
}

/// Each layer of instance `x` (laid out from main `m`) under the step keys an override path can name it by: its main
/// record's `overrideKey` and its guid, scoped by the instance (or nested instance) the step is taken in.
type Steps = HashMap<(NodeId, String), NodeId>;

fn walk(doc: &Document, x: NodeId, m: NodeId, scope: NodeId, t: &Truth, out: &mut Steps, bad: &mut usize) {
    let (Some(xn), Some(mn)) = (doc.get(x), doc.get(m)) else { return };
    if xn.children.len() != mn.children.len() {
        *bad += 1;
        return;
    }
    for (&xc, &mc) in xn.children.iter().zip(&mn.children) {
        let Some(src) = doc.get(mc).and_then(|n| n.source_id.as_deref()) else {
            *bad += 1;
            continue;
        };
        for k in [t.path_key.get(src).map(String::as_str), Some(src)].into_iter().flatten() {
            out.entry((scope, k.to_string())).or_insert(xc);
        }
        match doc.get(xc).map(|n| &n.kind) {
            Some(NodeKind::Instance { component: Some(main), .. }) => walk(doc, xc, *main, xc, t, out, bad),
            _ => walk(doc, xc, mc, scope, t, out, bad),
        }
    }
}

/// The layer an override path names, step by step from the instance.
fn resolve(steps: &Steps, inst: NodeId, inst_keys: &[&str], path: &[String]) -> Option<NodeId> {
    let mut cur = inst;
    for (i, k) in path.iter().enumerate() {
        cur = match steps.get(&(cur, k.clone())) {
            Some(&n) => n,
            None if i == 0 && inst_keys.contains(&k.as_str()) => inst,
            None => return None,
        };
    }
    Some(cur)
}

fn close(e: &Entry, n: &Node, tol: f32) -> bool {
    let fs = e.size.unwrap_or(n.size);
    if (fs.x - n.size.x).abs() > tol || (fs.y - n.size.y).abs() > tol {
        return false;
    }
    let ft = e.transform.unwrap_or(n.transform);
    [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].iter().all(|&(u, v)| {
        let a = ft.apply(Vec2::new(u * fs.x, v * fs.y));
        let b = n.transform.apply(Vec2::new(u * n.size.x, v * n.size.y));
        (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol
    })
}

fn override_kind(field: &str) -> &'static str {
    match field {
        "textData" | "characters" | "fontName" | "fontSize" => "text",
        "fillPaints" | "strokePaints" | "styleIdForFill" | "styleIdForStrokeFill" => "paint",
        "visible" => "visibility",
        "overriddenSymbolID" => "swap",
        "componentPropAssignments" => "component property",
        "size" | "transform" => "size",
        "effects" | "styleIdForEffect" => "effect",
        _ => "other",
    }
}

pub fn part(t: &Truth, doc: &Document, keys: &NodeKeys, refused: &HashSet<String>) -> Part {
    let mut p = Part::new("instances", "Instances", "each derivedSymbolData sublayer within 0.5 px of the stored size and transform after a no-change sync");
    p.note("geometry of sublayers is not compared here: shape outlines are part 1, on main records");
    let tol = if broken_comparator() { f32::INFINITY } else { TOL };
    for inst in &t.instances {
        let Some(&id) = keys.by_source.get(&inst.guid) else {
            p.unscored("instance record with no layer", "the record did not become a layer on a page");
            continue;
        };
        let Some(NodeKind::Instance { component: Some(main), .. }) = doc.get(id).map(|n| &n.kind) else {
            p.unscored("instance with no main", "the layer is not an instance of a main in the file");
            continue;
        };
        let (mut steps, mut bad) = (Steps::new(), 0usize);
        walk(doc, id, *main, id, t, &mut steps, &mut bad);
        // A first step may name the instance itself, by its own key or by its main's (as the importer resolves it).
        let main_src = doc.get(*main).and_then(|n| n.source_id.as_deref());
        let inst_keys: Vec<&str> =
            [Some(inst.guid.as_str()), main_src].into_iter().flatten().flat_map(|g| [Some(g), t.path_key.get(g).map(String::as_str)]).flatten().collect();
        let (inst_n, main_n) = (doc.get(id).unwrap(), doc.get(*main).unwrap());
        let resized = (inst_n.size.x - main_n.size.x).abs() > TOL || (inst_n.size.y - main_n.size.y).abs() > TOL;
        let variant = main_n.parent.and_then(|p| doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
        let is_refused = refused.contains(&inst.guid);
        for e in &inst.entries {
            let Some(nid) = resolve(&steps, id, &inst_keys, &e.path) else {
                p.unscored("derived path not found in the instance", "the hydrated instance has no layer at that override path");
                p.detail("derived paths not found", &format!("depth {}, instance walk unpaired: {}", e.path.len().min(4), bad > 0));
                continue;
            };
            let n = doc.get(nid).unwrap();
            let mut b = vec!["total".to_string(), format!("depth: {}", e.path.len().min(4))];
            let kinds: HashSet<&str> = inst.overrides.iter().filter(|(op, _)| *op == e.path).flat_map(|(_, f)| f.iter().map(|x| override_kind(x))).collect();
            if kinds.is_empty() {
                b.push("override: none".into());
            }
            b.extend(kinds.into_iter().map(|k| format!("override: {k}")));
            b.push(format!("variant: {}", if variant { "yes" } else { "no" }));
            b.push(format!("slot: {}", if n.component.is_slot || n.component.is_slot_content { "yes" } else { "no" }));
            b.push(format!("resized instance: {}", if resized { "yes" } else { "no" }));
            b.push(format!("refused hydration: {}", if is_refused { "yes" } else { "no" }));
            let ok = close(e, n, tol);
            if !ok && !keys.is_hidden(nid) {
                let bin = |x: f32| format!("{:+.1}", (x * 2.0).round() / 2.0);
                let (fs, ft) = (e.size.unwrap_or(n.size), e.transform.unwrap_or(n.transform));
                let what = format!("w {} h {} x {} y {}", bin(n.size.x - fs.x), bin(n.size.y - fs.y), bin(n.transform.tx - ft.tx), bin(n.transform.ty - ft.ty));
                p.detail(&format!("failing sublayer delta (Studio minus Figma), refused hydration: {}", if is_refused { "yes" } else { "no" }), &what);
            }
            p.record(format!("{}>{}", inst.guid, e.path.join(">")), keys.is_hidden(nid), ok, &b);
        }
        if bad > 0 {
            p.detail("instances with a sublayer the walk could not pair", "instances");
        }
    }
    p
}
