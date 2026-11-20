//! Save loss by class. A no-edit save is compared with Figma's original in two classes:
//!
//! - Class 1: a field Figma's own file holds, on a record that came from Figma (its guid is in the original). A field
//!   the save drops or changes is lost or changed, and so is a field the save adds an entry or subfield inside (the
//!   field no longer equals Figma's). Each key of each record counts once, however many paths were added, so the
//!   denominator is the original's key count. It must be 0.
//! - Class 2: what only the save holds: a record with no original record, a key the save adds to a Figma record, a
//!   subfield or array entry it adds inside a Figma field. Reported apart and never scored.
//!
//! The leaf comparison, the blob renumbering and the sibling-order normalisation are the save loss part's own
//! (`save_loss.rs`), so the two parts differ only in what they count.

use super::board::{broken_comparator, Part};
use super::feature::feature_of;
use super::raw::{record_guid, Raw};
use super::save_loss::{by_guid, fields, order_kept, same_parent_kept, shape, Ctx, BLOB_FIELDS};
use designos_fig::kiwi::KiwiValue;
use std::collections::{HashMap, HashSet};

pub const CLASS_1: &str = "class 1: Figma-held fields";
pub const OUTCOMES: &str = "class 1 outcome";
pub const FAILING_KEYS: &str = "class 1 failing key";
pub const NEW_RECORD: &str = "class 2: record only the save writes";
pub const NEW_FIELD: &str = "class 2: key or subfield only the save adds to a Figma record";

/// Where the saved value of one Figma field differs from the original's, and what the save holds beyond it.
#[derive(Default)]
struct Walk {
    /// Shapes of the paths the original holds that the save left out.
    lost: Vec<String>,
    /// Shapes of the paths the original holds whose value the save changed.
    changed: Vec<String>,
    /// Shapes of the paths only the save holds.
    added: Vec<String>,
}

fn walk(a: &KiwiValue, b: &KiwiValue, field: &str, path: &str, cx: &mut Ctx, out: &mut Walk) {
    use KiwiValue as V;
    if BLOB_FIELDS.contains(&field) {
        let (Some(x), Some(y)) = (super::raw::u64_of(a), super::raw::u64_of(b)) else { return out.changed.push(shape(path)) };
        let (bx, by) = (cx.a_blobs.get(x as usize), cx.b_blobs.get(y as usize));
        if bx.is_none() || bx != by {
            out.changed.push(shape(path));
        } else {
            cx.renumbered |= x != y;
        }
        return;
    }
    match (a, b) {
        (V::Message(x) | V::Struct(x), V::Message(y) | V::Struct(y)) => {
            let ym: HashMap<&str, &KiwiValue> = y.iter().map(|(k, v)| (&**k, v)).collect();
            for (k, xv) in x {
                let p = format!("{path}/{k}");
                match ym.get(&**k) {
                    Some(yv) => walk(xv, yv, k, &p, cx, out),
                    None => out.lost.push(shape(&p)),
                }
            }
            let xk: HashSet<&str> = x.iter().map(|(k, _)| &**k).collect();
            out.added.extend(y.iter().filter(|(k, _)| !xk.contains(&**k)).map(|(k, _)| shape(&format!("{path}/{k}"))));
        }
        (V::Array(x), V::Array(y)) => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                walk(p, q, field, &format!("{path}/{i}"), cx, out);
            }
            if x.len() > y.len() {
                out.lost.push(format!("{}[len]", shape(path)));
            } else if y.len() > x.len() {
                out.added.push(format!("{}[len]", shape(path)));
            }
        }
        (V::Float(x), V::Float(y)) => {
            if x.to_bits() != y.to_bits() && !(x.is_nan() && y.is_nan()) {
                out.changed.push(shape(path));
            }
        }
        (V::Enum(x), V::Enum(y)) => {
            if x != y {
                out.changed.push(shape(path));
            }
        }
        _ => {
            if a != b {
                out.changed.push(shape(path));
            }
        }
    }
}

pub fn part(original: &Raw, saved: &Raw, hidden: &HashSet<String>) -> Part {
    let mut p = Part::new(
        "save_class",
        "Save loss by class",
        "class 1: each field of each record that came from Figma equal in a no-edit save (after the save loss part's normalisations); a key or record only the save writes is class 2 and never fails it, and an entry or subfield it adds inside a Figma field changes that field",
    );
    p.note("class 1 counts each key of each record once: a field or subfield missing from the save is lost, a leaf that differs or an entry or subfield added inside the field is changed, a record with no saved record is not written");
    p.note("class 2 counts what only the save holds (a record with no original record, a key or subfield it adds) and scores none of it");
    p.print_detail(FAILING_KEYS);
    p.print_detail(OUTCOMES);
    p.print_detail("class 1 derivedTextData outcome");
    let saved_by = by_guid(saved);
    let kept_order = order_kept(original, saved);
    let broken = broken_comparator();
    let mut orig: Vec<(String, &KiwiValue)> = original.live().filter_map(|r| Some((record_guid(r)?, r))).collect();
    orig.sort_by(|a, b| a.0.cmp(&b.0));
    let orig_guids: HashSet<&str> = orig.iter().map(|(g, _)| g.as_str()).collect();
    let studio_only = saved_by.keys().filter(|g| !orig_guids.contains(g.as_str())).count() as u64;
    p.unscored_n(NEW_RECORD, "a record with no original record is not a Figma field", studio_only);
    for (guid, r) in orig {
        let hid = hidden.contains(&guid);
        let s = saved_by.get(&guid).copied();
        let mut all_ok = true;
        for (k, v) in fields(r) {
            let mut w = Walk::default();
            let mut cx = Ctx { a_blobs: &original.blobs, b_blobs: &saved.blobs, renumbered: false };
            let what = match s.map(|s| s.get(k)) {
                None => "record not written",
                Some(None) => "lost",
                Some(Some(sv)) => {
                    walk(v, sv, k, "", &mut cx, &mut w);
                    if !w.lost.is_empty() {
                        "lost"
                    } else if w.changed.is_empty() && w.added.is_empty() {
                        "equal"
                    } else if w.changed.is_empty() {
                        // Figma reads the whole field: an entry or subfield the save appends inside it changes it.
                        "changed, save added inside"
                    } else if &**k == "parentIndex" && w.changed == ["/position"] && w.added.is_empty() && same_parent_kept(v, sv, &kept_order) {
                        "normalised: sibling order kept"
                    } else {
                        "changed"
                    }
                }
            };
            let ok = broken || what == "equal" || what.starts_with("normalised");
            all_ok &= ok;
            p.tally(CLASS_1, hid, ok);
            p.tally(&format!("class 1 feature: {}", feature_of(k)), hid, ok);
            if &**k == "derivedTextData" {
                p.tally("class 1 derivedTextData", hid, ok);
                if !hid {
                    p.detail("class 1 derivedTextData outcome", if ok { "equal or normalised" } else { what });
                }
            }
            if !hid {
                p.detail(OUTCOMES, if ok { "equal or normalised" } else { what });
                if !ok {
                    p.detail(FAILING_KEYS, k);
                }
            }
            for _ in &w.added {
                p.unscored(NEW_FIELD, "a key or subfield only the save holds is not a Figma field");
                p.detail("class 2: field the save adds to, by key", k);
            }
        }
        if let Some(s) = s {
            let held: HashSet<&str> = fields(r).iter().map(|(k, _)| &**k).collect();
            for (k, _) in fields(s).iter().filter(|(k, _)| !held.contains(&**k)) {
                p.unscored(NEW_FIELD, "a key or subfield only the save holds is not a Figma field");
                p.detail("class 2: key the save adds to a Figma record, by key", k);
            }
        }
        p.record(guid, hid, all_ok, &["class 1: records".to_string()]);
    }
    p
}
