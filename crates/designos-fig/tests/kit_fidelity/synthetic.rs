//! Each part, run on a tiny file with one known deviation, counts it as a mismatch, and a file without it matches.
//! With `SCOREBOARD_BROKEN_COMPARATOR=1` every comparator reports a match, and these tests fail.

use super::board::Tally;
use super::fixtures::*;
use super::keys::NodeKeys;
use super::raw::{self, Raw};
use super::scratch::Pass;
use super::text::{Baseline, TextTruth};
use super::{geometry, import_cov, instance_hydration, instances, layout, save_loss, text};
use designos_doc::*;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, fl, guid, msg, square_blob, triangle_blob};
use std::collections::HashSet;

pub(super) fn load(bytes: &[u8]) -> (Raw, Document, NodeKeys) {
    let r = raw::decode(bytes);
    let mut doc = designos_fig::import_fig(bytes).expect("import");
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    (r, doc, keys)
}

pub(super) fn t(matched: u64, measured: u64) -> Tally {
    Tally { matched, measured }
}

#[test]
fn auto_layout_counts_a_child_off_its_solved_place() {
    let (_, doc, keys) = load(&auto_layout_fig(30.0));
    let none = HashSet::new();
    assert_eq!(layout::part(&doc, &keys, &none).bucket("child placement: total")[0], t(2, 2), "children where the solve puts them");
    let (_, doc, keys) = load(&auto_layout_fig(33.0));
    assert_eq!(layout::part(&doc, &keys, &none).bucket("child placement: total")[0], t(1, 2), "the second child stored 3 px off its solved place");
}

#[test]
fn auto_layout_counts_a_hug_frame_off_its_solved_size() {
    let none = HashSet::new();
    let (_, doc, keys) = load(&hug_row_fig(50.0));
    let p = layout::part(&doc, &keys, &none);
    assert_eq!(p.bucket("frame size: hug width")[0], t(1, 1), "a hug row as wide as its content");
    assert_eq!(p.bucket("child placement: total")[0], t(2, 2));
    let (_, doc, keys) = load(&hug_row_fig(53.0));
    let p = layout::part(&doc, &keys, &none);
    assert_eq!(p.bucket("frame size: hug width")[0], t(0, 1), "a hug row stored 3 px wider than its content");
    assert_eq!(p.bucket("child placement: total")[0], t(2, 2), "its children still sit where the solve puts them");
}

#[test]
fn auto_layout_leaves_out_frames_inside_a_refused_instance() {
    let (_, doc, keys) = load(&auto_layout_fig(33.0));
    let frame = keys.by_source["1:1"];
    let p = layout::part(&doc, &keys, &HashSet::from([frame]));
    assert_eq!(p.bucket("child placement: total")[0], t(0, 0), "Studio's own layout is not Figma's truth");
    assert_eq!(p.unscored.get("child inside an instance whose stored layout hydration refused").map(|u| u.0), Some(2));
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

#[test]
fn geometry_counts_a_1_px_stroke_drawn_off_its_stored_band() {
    let (r, doc, keys) = load(&stroke_fig("CENTER", 1.0, band_blob(0.5, 0.5)));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("stroke: total")[0], t(1, 1), "a centred 1 px stroke against its centred band");
    let (r, doc, keys) = load(&stroke_fig("CENTER", 1.0, band_blob(0.0, 1.0)));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("stroke: total")[0], t(0, 1), "a centred 1 px stroke against an inside band: 0.5 px apart, a third overlapping");
    let (r, doc, keys) = load(&stroke_fig("CENTER", 1.5, band_blob(0.5, 0.5)));
    let p = geometry::part(&geometry::truth(&r), &r.blobs, &doc, &keys);
    assert_eq!(p.bucket("stroke: total")[0], t(0, 1), "a 1.5 px stroke against a 1 px band");
}

#[test]
fn geometry_counts_a_live_outline_beyond_the_stored_one() {
    let square = |x: f32| Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(x, 0.0)),
            PathCmd::LineTo(Vec2::new(x + 10.0, 0.0)),
            PathCmd::LineTo(Vec2::new(x + 10.0, 10.0)),
            PathCmd::LineTo(Vec2::new(x, 10.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let node = |paths: Vec<Path>| {
        let mut n = Node::new(NodeId(7), NodeKind::Vector { paths, network: None });
        n.size = Vec2::new(10.0, 10.0);
        n
    };
    let figma = vec![(super::raster::to_sk(&square(0.0)).unwrap(), false)];
    let v = geometry::compare(&[node(vec![square(0.0)])], Pass::Fill, &figma, geometry::Clip::None);
    assert!(matches!(v, geometry::Verdict::Match), "the same square");
    let v = geometry::compare(&[node(vec![square(0.0), square(30.0)])], Pass::Fill, &figma, geometry::Clip::None);
    assert!(matches!(v, geometry::Verdict::Mismatch(_)), "a second square 20 px past the stored outline and the layer box");
}

#[test]
fn instances_count_a_sublayer_off_its_derived_place() {
    let (r, mut doc, keys) = load(&instance_fig());
    let truth = instances::truth(&r);
    assert_eq!(truth.instances.len(), 1);
    let refused = instance_hydration::refused(&truth, &doc, &keys);
    instance_hydration::sync_all(&mut doc);
    let keys = NodeKeys::build(&doc);
    assert_eq!(instances::part(&truth, &doc, &keys, &refused).bucket("total")[0], t(1, 1), "the sublayer where Figma laid it out");
    let inst = keys.by_source["1:20"];
    let sub = doc.get(inst).unwrap().children[0];
    doc.get_mut(sub).unwrap().transform.tx += 3.0;
    assert_eq!(instances::part(&truth, &doc, &keys, &refused).bucket("total")[0], t(0, 1), "the sublayer moved 3 px");
}

#[test]
fn instances_never_score_a_field_figma_did_not_store() {
    let (r, mut doc, keys) = load(&instance_fig());
    let mut truth = instances::truth(&r);
    let refused = instance_hydration::refused(&truth, &doc, &keys);
    instance_hydration::sync_all(&mut doc);
    let keys = NodeKeys::build(&doc);
    let sub = doc.get(keys.by_source["1:20"]).unwrap().children[0];
    doc.get_mut(sub).unwrap().transform.tx += 3.0;
    truth.instances[0].entries[0].transform = None;
    let p = instances::part(&truth, &doc, &keys, &refused);
    assert_eq!(p.bucket("total")[0], t(1, 1), "with no stored transform only the size is scored");
    assert_eq!(p.unscored.get("derived entry without a transform (its size alone is scored)").map(|u| u.0), Some(1));
    truth.instances[0].entries[0].size = None;
    let p = instances::part(&truth, &doc, &keys, &refused);
    assert_eq!(p.bucket("total")[0], t(0, 0), "an entry storing neither is not a match");
    assert_eq!(p.unscored.get("derived entry storing neither size nor transform").map(|u| u.0), Some(1));
}

#[test]
fn text_counts_a_baseline_off_the_stored_one() {
    let (r, doc, keys) = load(&text_fig(12.0));
    let (truth, without) = text::truth(&r);
    assert_eq!((truth.len(), without), (1, 0));
    assert_eq!((truth[0].baselines[0].first, truth[0].baselines[0].y), (0, 12.0), "derivedTextData is read from the raw record");
    let n = doc.get(keys.by_source["1:30"]).unwrap();
    let NodeKind::Text(props) = &n.kind else { panic!("a text layer") };
    let g = designos_render::text_geometry(props, n.size);
    let studio = |shift: f32| TextTruth {
        guid: "1:30".into(),
        layout: Some(Vec2::new(g.width, g.height)),
        baselines: g.lines.iter().map(|l| Baseline { first: l.start as u64, y: l.baseline + shift }).collect(),
        line_height_unit: "absent".into(),
        letter_spacing_unit: "absent".into(),
        glyphs: Vec::new(),
    };
    assert_eq!(text::part(&[studio(0.0)], 0, &doc, &keys).bucket("total")[0], t(1, 1), "Figma's lines are Studio's");
    assert_eq!(text::part(&[studio(3.0)], 0, &doc, &keys).bucket("total")[0], t(0, 1), "a baseline 3 px lower");
    assert_eq!(text::part(&[studio(f32::NAN)], 0, &doc, &keys).bucket("total")[0], t(0, 1), "a baseline stored without a position");
}

#[test]
fn text_font_check_reads_weight_and_style_not_only_the_family() {
    let face = |family: &str, weight: u16, italic: bool| {
        super::text_fonts::font_state(&TextStyle { font_family: family.into(), font_weight: weight, italic, ..TextStyle::default() })
    };
    assert!(face("Inter", 400, false) == super::text_fonts::FontState::Present, "embedded Inter regular");
    assert!(face("Inter", 700, true) == super::text_fonts::FontState::Present, "embedded Inter bold italic");
    assert!(face("Roboto Mono", 400, true) == super::text_fonts::FontState::Missing, "Roboto Mono has no embedded italic");
    assert!(face("Roboto Mono", 900, false) == super::text_fonts::FontState::Missing, "embedded Roboto Mono stops at 700");
    assert!(face("No Such Family Anywhere", 400, false) == super::text_fonts::FontState::Missing, "an unknown family");
}

#[test]
fn regressions_list_a_visible_item_that_matched_before_and_fails_now() {
    use super::board::{part_json, regressions, Part};
    let mut before = Part::new("auto_layout", "Auto layout", "rule");
    before.record("1:2".into(), false, true, &["total".into()]);
    before.record("1:3".into(), false, true, &["total".into()]);
    before.record("1:4".into(), true, true, &["total".into()]);
    let prev = serde_json::json!({ "parts": [part_json(&before)] });
    let mut now = Part::new("auto_layout", "Auto layout", "rule");
    now.record("1:2".into(), false, false, &["total".into()]);
    now.record("1:3".into(), false, true, &["total".into()]);
    now.record("1:4".into(), true, false, &["total".into()]);
    now.record("1:5".into(), false, false, &["total".into()]);
    let r = regressions(&[now], &prev);
    assert_eq!(r.get("auto_layout"), Some(&vec!["1:2".to_string()]), "only the visible item that matched before");
    let mut later = Part::new("auto_layout", "Auto layout", "rule");
    later.record("1:2".into(), false, true, &["total".into()]);
    later.unscored("derived path not found", "gone");
    let r = regressions(&[later], &prev);
    assert_eq!(r.get("auto_layout: left measurement"), Some(&vec!["1:3".to_string()]), "an item that matched before and is no longer measured");
}
