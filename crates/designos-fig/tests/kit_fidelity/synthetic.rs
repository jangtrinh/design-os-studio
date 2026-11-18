//! Each part, run on a tiny file with one known deviation, counts it as a mismatch, and a file without it matches.
//! With `SCOREBOARD_BROKEN_COMPARATOR=1` every comparator reports a match, and these tests fail.

use super::board::Tally;
use super::fixtures::*;
use super::keys::NodeKeys;
use super::raw::{self, Raw};
use super::{geometry, import_cov, layout, save_loss};
use designos_doc::*;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, fl, guid, msg, square_blob, triangle_blob};
use std::collections::HashSet;

fn load(bytes: &[u8]) -> (Raw, Document, NodeKeys) {
    let r = raw::decode(bytes);
    let mut doc = designos_fig::import_fig(bytes).expect("import");
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    (r, doc, keys)
}

fn t(matched: u64, measured: u64) -> Tally {
    Tally { matched, measured }
}

#[test]
fn auto_layout_counts_a_child_off_its_solved_place() {
    let (_, doc, keys) = load(&auto_layout_fig(30.0));
    assert_eq!(layout::part(&doc, &keys).bucket("total")[0], t(2, 2), "children where the solve puts them");
    let (_, doc, keys) = load(&auto_layout_fig(33.0));
    assert_eq!(layout::part(&doc, &keys).bucket("total")[0], t(1, 2), "the second child stored 3 px off its solved place");
}

#[test]
fn import_coverage_counts_a_key_the_importer_drops() {
    let (r, doc, keys) = load(&version_fig("abc"));
    let p = import_cov::part(&import_cov::truth(&r), &doc, &keys);
    assert_eq!(p.bucket("key: version")[0], t(1, 1), "a value kept as metadata");
    let (r, doc, keys) = load(&version_fig(""));
    let p = import_cov::part(&import_cov::truth(&r), &doc, &keys);
    assert_eq!(p.bucket("key: version")[0], t(0, 1), "an empty value is neither mapped nor kept");
    assert!(import_cov::mapped_fields().contains("fillGeometry"), "the importer's field list is read from its source");
}

fn rect_record(opacity: Option<f32>, blob: u32) -> V {
    let mut f = vec![("guid", guid(1, 2)), ("type", en("RECTANGLE")), ("fillGeometry", geometry(blob))];
    if let Some(o) = opacity {
        f.push(("opacity", fl(o)));
    }
    msg(f)
}

#[test]
fn save_loss_counts_a_changed_and_a_lost_key() {
    let original = Raw { records: vec![rect_record(Some(0.5), 0)], blobs: vec![square_blob()] };
    let same = Raw { records: vec![rect_record(Some(0.5), 1)], blobs: vec![triangle_blob(), square_blob()] };
    let p = save_loss::part(&original, &same, &HashSet::new());
    assert_eq!(p.bucket("records")[0], t(1, 1), "the same record, its outline at another blob index");
    assert_eq!(p.details["key: fillGeometry"].get("normalised: blob renumbered"), Some(&1));
    let changed = Raw { records: vec![rect_record(Some(0.6), 0)], blobs: vec![square_blob()] };
    assert_eq!(save_loss::part(&original, &changed, &HashSet::new()).bucket("key: opacity")[0], t(0, 1), "a changed value");
    let lost = Raw { records: vec![rect_record(None, 0)], blobs: vec![square_blob()] };
    let p = save_loss::part(&original, &lost, &HashSet::new());
    assert_eq!(p.bucket("key: opacity")[0], t(0, 1), "a key the save left out");
    assert_eq!(p.details["key: opacity"].get("lost"), Some(&1));
    let other_bytes = Raw { records: vec![rect_record(Some(0.5), 0)], blobs: vec![triangle_blob()] };
    assert_eq!(save_loss::part(&original, &other_bytes, &HashSet::new()).bucket("key: fillGeometry")[0], t(0, 1), "an outline with other bytes");
}

#[test]
fn geometry_counts_a_live_outline_off_the_stored_one() {
    let (r, doc, keys) = load(&rectangle_fig(10.0));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("fill: total")[0], t(1, 1), "a 10 x 10 rectangle against its 10 x 10 outline");
    let (r, doc, keys) = load(&rectangle_fig(20.0));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("fill: total")[0], t(0, 1), "a 20 x 20 rectangle against a 10 x 10 outline");
}

#[test]
fn geometry_clips_a_stored_inside_band_by_the_fill_outline() {
    let (r, doc, keys) = load(&inside_stroke_fig(1.0));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("stroke: align INSIDE")[0], t(1, 1), "a weight-1 inside stroke against the clipped stored band");
    let (r, doc, keys) = load(&inside_stroke_fig(2.0));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("stroke: align INSIDE")[0], t(0, 1), "a weight-2 inside stroke against a weight-1 band");
}
