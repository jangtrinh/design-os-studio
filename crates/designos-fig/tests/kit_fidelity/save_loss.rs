//! Part 6, save loss against Figma's original: a no-edit `.fig` save is decoded and compared, record by record and
//! field by field, with the original file's records. Each key of each original record ends up equal, equal after a
//! documented normalisation, changed, or lost.

use super::board::{broken_comparator, Part};
use super::feature::feature_of;
use super::raw::{is_removed, record_guid, Raw};
use designos_fig::kiwi::KiwiValue;
use std::collections::{HashMap, HashSet};

/// Fields that hold an index into the file's blob table. Two files number their blobs independently, so these are
/// compared by the bytes they point at.
const BLOB_FIELDS: &[&str] = &["commandsBlob", "vectorNetworkBlob", "dataBlob"];

/// Normalisations the comparison accepts, each with its reason. Anything else that differs is a change or a loss.
pub const NORMALISATIONS: &[(&str, &str)] = &[
    ("blob renumbered", "a blob index points at the same bytes in the saved file's own blob table; the index itself has no meaning outside its file"),
    ("message field order", "kiwi message fields are named, so their order on the wire carries no data; fields are compared by name"),
    ("sibling order kept", "a parentIndex position string only orders a layer among its siblings; a rewritten string that leaves the order of the siblings both files hold unchanged carries the same data"),
];

/// Parents whose children (those both files place under that parent) come in the same order in both files.
fn order_kept(original: &Raw, saved: &Raw) -> HashSet<String> {
    let order = |raw: &Raw| -> HashMap<String, Vec<(String, String)>> {
        let mut m: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for r in raw.live() {
            let (Some(g), Some(pi)) = (record_guid(r), r.get("parentIndex")) else { continue };
            let (Some(parent), Some(pos)) = (pi.get("guid").and_then(super::raw::guid_of), super::raw::field_str(pi, "position")) else { continue };
            m.entry(parent).or_default().push((pos.to_string(), g));
        }
        for v in m.values_mut() {
            v.sort();
        }
        m
    };
    let (a, b) = (order(original), order(saved));
    a.iter()
        .filter(|(parent, kids)| {
            let Some(other) = b.get(*parent) else { return false };
            let (ka, kb): (HashSet<&str>, HashSet<&str>) = (kids.iter().map(|(_, g)| g.as_str()).collect(), other.iter().map(|(_, g)| g.as_str()).collect());
            let x: Vec<&str> = kids.iter().map(|(_, g)| g.as_str()).filter(|g| kb.contains(g)).collect();
            let y: Vec<&str> = other.iter().map(|(_, g)| g.as_str()).filter(|g| ka.contains(g)).collect();
            x == y
        })
        .map(|(p, _)| p.clone())
        .collect()
}

struct Ctx<'a> {
    a_blobs: &'a [Vec<u8>],
    b_blobs: &'a [Vec<u8>],
    renumbered: bool,
}

fn shape(path: &str) -> String {
    path.split('/').map(|s| if s.parse::<usize>().is_ok() { "#" } else { s }).collect::<Vec<_>>().join("/")
}

/// The first place `a` and `b` differ, as a path with array indices folded to `#`; `None` when they are equal.
fn diff(a: &KiwiValue, b: &KiwiValue, field: &str, path: &str, cx: &mut Ctx) -> Option<String> {
    use KiwiValue as V;
    if BLOB_FIELDS.contains(&field) {
        let (Some(x), Some(y)) = (super::raw::u64_of(a), super::raw::u64_of(b)) else { return Some(shape(path)) };
        let (bx, by) = (cx.a_blobs.get(x as usize), cx.b_blobs.get(y as usize));
        if bx.is_none() || bx != by {
            return Some(shape(path));
        }
        cx.renumbered |= x != y;
        return None;
    }
    match (a, b) {
        (V::Message(x) | V::Struct(x), V::Message(y) | V::Struct(y)) => {
            let ym: HashMap<&str, &KiwiValue> = y.iter().map(|(k, v)| (&**k, v)).collect();
            if x.len() != y.len() {
                let xk: HashSet<&str> = x.iter().map(|(k, _)| &**k).collect();
                let odd = y.iter().map(|(k, _)| &**k).find(|k| !xk.contains(k)).or_else(|| x.iter().map(|(k, _)| &**k).find(|k| !ym.contains_key(k)));
                return Some(shape(&format!("{path}/{}", odd.unwrap_or("?"))));
            }
            for (k, xv) in x {
                let p = format!("{path}/{k}");
                let Some(yv) = ym.get(&**k) else { return Some(shape(&p)) };
                if let Some(d) = diff(xv, yv, k, &p, cx) {
                    return Some(d);
                }
            }
            None
        }
        (V::Array(x), V::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{}[len]", shape(path)));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                if let Some(d) = diff(p, q, field, &format!("{path}/{i}"), cx) {
                    return Some(d);
                }
            }
            None
        }
        (V::Float(x), V::Float(y)) => (x.to_bits() != y.to_bits() && !(x.is_nan() && y.is_nan())).then(|| shape(path)),
        (V::Enum(x), V::Enum(y)) => (x != y).then(|| shape(path)),
        _ => (a != b).then(|| shape(path)),
    }
}

/// Both parent indexes name the same parent, and that parent's children keep their order.
fn same_parent_kept(a: &KiwiValue, b: &KiwiValue, kept: &HashSet<String>) -> bool {
    let pa = a.get("guid").and_then(super::raw::guid_of);
    pa.is_some() && pa == b.get("guid").and_then(super::raw::guid_of) && pa.is_some_and(|p| kept.contains(&p))
}

fn by_guid(raw: &Raw) -> HashMap<String, &KiwiValue> {
    raw.live().filter_map(|r| Some((record_guid(r)?, r))).collect()
}

fn fields(r: &KiwiValue) -> &[(std::sync::Arc<str>, KiwiValue)] {
    match r {
        KiwiValue::Message(f) | KiwiValue::Struct(f) => f,
        _ => &[],
    }
}

pub fn part(original: &Raw, saved: &Raw, hidden: &HashSet<String>) -> Part {
    let mut p = Part::new(
        "save_loss",
        "Save loss vs Figma's original",
        "each key of each original record equal in a no-edit save, or equal after a documented normalisation",
    );
    for (n, why) in NORMALISATIONS {
        p.note(format!("normalisation \"{n}\": {why}"));
    }
    p.note(format!("original tombstone records (phase REMOVED) not compared: {}", original.records.iter().filter(|r| is_removed(r)).count()));
    let saved_by = by_guid(saved);
    let kept_order = order_kept(original, saved);
    let broken = broken_comparator();
    let mut orig: Vec<(String, &KiwiValue)> = original.live().filter_map(|r| Some((record_guid(r)?, r))).collect();
    orig.sort_by(|a, b| a.0.cmp(&b.0));
    let orig_guids: HashSet<&str> = orig.iter().map(|(g, _)| g.as_str()).collect();
    p.note(format!("saved records with no original record: {}", saved_by.keys().filter(|g| !orig_guids.contains(g.as_str())).count()));
    for (guid, r) in orig {
        let hid = hidden.contains(&guid);
        let s = saved_by.get(&guid).copied();
        let mut all_ok = true;
        for (k, v) in fields(r) {
            let bucket = format!("key: {k}");
            let what = match s.and_then(|s| s.get(k)) {
                None if s.is_none() => "record not written".to_string(),
                None => "lost".to_string(),
                Some(sv) => {
                    let mut cx = Ctx { a_blobs: &original.blobs, b_blobs: &saved.blobs, renumbered: false };
                    match diff(v, sv, k, "", &mut cx) {
                        None if cx.renumbered => "normalised: blob renumbered".to_string(),
                        None => "equal".to_string(),
                        Some(at) if &**k == "parentIndex" && at == "/position" && same_parent_kept(v, sv, &kept_order) => {
                            "normalised: sibling order kept".to_string()
                        }
                        Some(at) => format!("changed at {k}{at}"),
                    }
                }
            };
            let ok = broken || what == "equal" || what.starts_with("normalised");
            all_ok &= ok;
            p.tally(&bucket, hid, ok);
            p.tally(&format!("feature: {}", feature_of(k)), hid, ok);
            p.tally("total occurrences", hid, ok);
            p.detail(&bucket, &what);
        }
        if let Some(s) = s {
            let ok_keys: HashSet<&str> = fields(r).iter().map(|(k, _)| &**k).collect();
            for (k, _) in fields(s).iter().filter(|(k, _)| !ok_keys.contains(&**k)) {
                p.detail(&format!("key: {k}"), "added by the save");
            }
        }
        p.record(guid, hid, all_ok, &["records".to_string()]);
    }
    p
}
