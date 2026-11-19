//! The stored-data use, save-loss class, measurer row and glyph advance parts, each run on a tiny file with one known
//! deviation. With `SCOREBOARD_BROKEN_COMPARATOR=1` every comparator reports a match, and the tests that name a
//! deviation fail.

use super::board::Part;
use super::fixtures::*;
use super::keys::NodeKeys;
use super::missing_font_fig as mf;
use super::raw::Raw;
use super::synthetic::{load, t};
use super::text::TextTruth;
use super::text_glyphs::GlyphTruth;
use super::{geometry, instance_hydration, instances, layout, run, save_class, save_loss, stored_use, text, text_glyphs};
use designos_doc::*;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, fl, guid, msg};
use std::collections::HashSet;

fn use_part(r: &Raw, doc: &Document, keys: &NodeKeys) -> Part {
    let (txt, _) = text::truth(r);
    let guids: Vec<String> = txt.iter().map(|x| x.guid.clone()).collect();
    stored_use::part(&instances::truth(r), &geometry::truth(r), &guids, stored_use::empty_derived(r), doc, keys)
}

#[test]
fn stored_use_counts_a_text_that_draws_from_its_stored_glyphs() {
    let (r, doc, keys) = load(&mf::icon_fixture(mf::FAMILY));
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("texts: total")[0], t(1, 1), "a font that is not installed draws Figma's stored glyphs");
    let (r, doc, keys) = load(&mf::icon_fixture("Inter"));
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("texts: total")[0], t(0, 1), "Inter is bundled, so shaping draws it");
    assert_eq!(p.bucket("texts: not drawn, font available (bundled)")[0], t(0, 1), "and the drop names its reason");
}

#[test]
fn stored_use_names_why_a_text_with_stored_glyphs_is_not_drawn_from_them() {
    let (r, mut doc, keys) = load(&mf::icon_fixture(mf::FAMILY));
    let id = keys.by_source["1:2"];
    let NodeKind::Text(props) = &mut doc.get_mut(id).unwrap().kind else { panic!("a text layer") };
    props.content = "edited".into();
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("texts: total")[0], t(0, 1), "an edited text no longer matches its saved glyphs");
    assert_eq!(p.bucket("texts: not drawn, saved glyphs no longer match the text")[0], t(0, 1));
    let (r, mut doc, keys) = load(&mf::icon_fixture(mf::FAMILY));
    let id = keys.by_source["1:2"];
    doc.get_mut(id).unwrap().fig_text_geometry = None;
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("texts: total")[0], t(0, 1), "a text the importer kept no glyphs for");
    assert_eq!(p.bucket("texts: not drawn, importer kept no glyphs")[0], t(0, 1));
}

#[test]
fn stored_use_counts_an_instance_that_lost_its_stored_layout() {
    let (r, mut doc, keys) = load(&instance_fig());
    assert_eq!(use_part(&r, &doc, &keys).bucket("instances: total")[0], t(1, 1), "hydration kept the stored layout of the top-level instance");
    let inst = keys.by_source["1:20"];
    doc.get_mut(inst).unwrap().component.imported_layout = None;
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("instances: total")[0], t(0, 1), "an instance that holds no imported layout draws Studio's own");
    assert_eq!(instance_hydration::refused(&instances::truth(&r), &doc, &keys).len(), 1, "the same instance the layout leg calls refused");
}

#[test]
fn stored_use_counts_a_shape_whose_stored_outline_is_gone_or_stale() {
    let (r, mut doc, keys) = load(&rectangle_fig(10.0));
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("fills: total")[0], t(1, 1), "the stored fill outline is drawn");
    assert_eq!(p.bucket("fills: box layers (frame, instance, component, rectangle)")[0], t(1, 1), "a rectangle is a box layer");
    doc.get_mut(keys.by_source["1:2"]).unwrap().fill_geometry = None;
    assert_eq!(use_part(&r, &doc, &keys).bucket("fills: total")[0], t(0, 1), "a layer with no fill outline left draws its live path");
    let (r, mut doc, keys) = load(&stroke_fig("CENTER", 1.0, band_blob(0.5, 0.5)));
    assert_eq!(use_part(&r, &doc, &keys).bucket("strokes: total")[0], t(1, 1), "the stored stroke band is drawn");
    doc.get_mut(keys.by_source["1:2"]).unwrap().stroke.weight = 3.0;
    let p = use_part(&r, &doc, &keys);
    assert_eq!(p.bucket("strokes: total")[0], t(0, 1), "a stroke weight edit makes the stored band stale");
    assert_eq!(p.bucket("strokes: not drawn, stale outline")[0], t(0, 1), "and the drop names its reason");
}

fn paint_record(opacity: f32, extra_field: bool) -> V {
    let mut paint = vec![("type", en("SOLID")), ("opacity", fl(opacity))];
    if extra_field {
        paint.push(("blendMode", en("NORMAL")));
    }
    msg(vec![("guid", guid(1, 2)), ("type", en("RECTANGLE")), ("fillPaints", V::Array(vec![msg(paint)]))])
}

fn raw_of(records: Vec<V>) -> Raw {
    Raw { records, blobs: Vec::new() }
}

#[test]
fn save_class_counts_what_figma_held_apart_from_what_only_the_save_holds() {
    let original = raw_of(vec![paint_record(0.5, false)]);
    let none = HashSet::new();
    let p = save_class::part(&original, &raw_of(vec![paint_record(0.5, false)]), &none);
    assert_eq!(p.bucket("class 1: Figma-held fields")[0], t(3, 3), "a save that keeps every field Figma held");

    let added = raw_of(vec![paint_record(0.5, true)]);
    let p = save_class::part(&original, &added, &none);
    assert_eq!(p.bucket("class 1: Figma-held fields")[0], t(2, 3), "a subfield the save adds inside a paint changes the field Figma held");
    assert_eq!(p.details["class 1 outcome"].get("changed, save added inside"), Some(&1));
    assert_eq!(
        p.unscored.get("class 2: key or subfield only the save adds to a Figma record").map(|u| u.0),
        Some(1),
        "and the addition is still listed in class 2"
    );
    assert_eq!(save_loss::part(&original, &added, &none).bucket("key: fillPaints")[0], t(0, 1), "as the raw comparison counts it");
    let many = raw_of(vec![msg(vec![
        ("guid", guid(1, 2)),
        ("type", en("RECTANGLE")),
        ("fillPaints", V::Array(vec![msg(vec![("type", en("SOLID")), ("opacity", fl(0.5)), ("blendMode", en("NORMAL"))]), msg(vec![("type", en("SOLID"))])])),
    ])]);
    let p = save_class::part(&original, &many, &none);
    assert_eq!(p.bucket("class 1: Figma-held fields")[0], t(2, 3), "two added paths in one key are one changed occurrence, not two");

    let changed = raw_of(vec![paint_record(0.9, false)]);
    let p = save_class::part(&original, &changed, &none);
    assert_eq!(p.bucket("class 1: Figma-held fields")[0], t(2, 3), "a changed opacity inside the paint");
    assert_eq!(p.details["class 1 failing key"].get("fillPaints"), Some(&1));
    assert_eq!(p.details["class 1 outcome"].get("changed"), Some(&1));

    let lost = raw_of(vec![msg(vec![("guid", guid(1, 2)), ("type", en("RECTANGLE"))])]);
    let p = save_class::part(&original, &lost, &none);
    assert_eq!(p.details["class 1 outcome"].get("lost"), Some(&1), "a key the save left out");

    let missing = save_class::part(&original, &raw_of(vec![]), &none);
    assert_eq!(missing.bucket("class 1: Figma-held fields")[0], t(0, 3), "a record the save did not write");
    assert_eq!(missing.details["class 1 outcome"].get("record not written"), Some(&3));
}

#[test]
fn save_class_counts_records_and_keys_only_the_save_writes() {
    let original = raw_of(vec![paint_record(0.5, false)]);
    let mut extra_key = paint_record(0.5, false);
    if let V::Message(f) = &mut extra_key {
        f.push(("pluginData".into(), V::Array(vec![])));
    }
    let studio_only = msg(vec![("guid", guid(1, 9)), ("type", en("FRAME"))]);
    let p = save_class::part(&original, &raw_of(vec![extra_key, studio_only]), &HashSet::new());
    assert_eq!(p.bucket("class 1: Figma-held fields")[0], t(3, 3), "nothing Figma held is lost");
    assert_eq!(p.unscored.get("class 2: record only the save writes").map(|u| u.0), Some(1));
    assert_eq!(p.unscored.get("class 2: key or subfield only the save adds to a Figma record").map(|u| u.0), Some(1), "the key added to the Figma record");
}

#[test]
fn measurer_row_sizes_text_with_the_app_measurer() {
    let (_, doc, keys) = load(&hug_text_fig());
    let none = HashSet::new();
    let without = layout::part(&doc, &keys, &none);
    assert_eq!(without.bucket("frame size: hug height")[0], t(1, 1), "with no measurer the stored text height stands, so the frame matches");
    let with = layout::measurer_part(&doc, &keys, &none);
    assert_eq!(with.id, "auto_layout_measurer", "a part of its own, so its items do not mix with the first row's");
    assert_eq!(with.bucket("frame size: hug height")[0], t(0, 1), "with the measurer the frame hugs the one line the text measures");
    assert!(designos_doc::text_measure::layout_measurer().is_none(), "the measurer is removed again when the row is done");
}

#[test]
fn glyph_truth_is_read_from_the_stored_glyph_records() {
    let (r, _, _) = load(&text_fig(12.0));
    let (truth, _) = text::truth(&r);
    let g = &truth[0].glyphs;
    assert_eq!(g.len(), 2);
    assert_eq!((g[0].first, g[0].x, g[0].advance, g[0].advance_px()), (0, 0.0, Some(0.75), Some(12.0)));
    assert_eq!((g[1].first, g[1].x, g[1].advance, g[1].advance_px()), (1, 12.0, Some(0.5625), Some(9.0)));
}

/// The stored truth of the fixture's text with each glyph taken from Studio's own shaping, moved by `dx` in x and
/// `dadv` in advance.
fn shaped_truth(doc: &Document, keys: &NodeKeys, dx: f32, dadv: f32) -> TextTruth {
    let n = doc.get(keys.by_source["1:30"]).unwrap();
    let NodeKind::Text(props) = &n.kind else { panic!("a text layer") };
    let g = designos_render::text_geometry(props, n.size);
    let glyphs = g
        .lines
        .iter()
        .flat_map(|l| {
            l.glyphs.iter().map(move |c| GlyphTruth {
                first: props.content[..c.start].encode_utf16().count() as u64,
                x: c.x + dx,
                y: l.baseline,
                advance: Some((c.w + dadv) / 16.0),
                font_size: 16.0,
            })
        })
        .collect();
    TextTruth {
        guid: "1:30".into(),
        layout: Some(Vec2::new(g.width, g.height)),
        baselines: g.lines.iter().map(|l| text::Baseline { first: l.start as u64, y: l.baseline }).collect(),
        line_height_unit: "absent".into(),
        letter_spacing_unit: "absent".into(),
        glyphs,
    }
}

#[test]
fn glyph_checks_reach_the_text_leg_and_a_shift_moves_x_but_not_the_steps() {
    let (_, doc, keys) = load(&text_fig(12.0));
    let score = |dx: f32, dadv: f32| text::part(&[shaped_truth(&doc, &keys, dx, dadv)], 0, &doc, &keys);
    let p = score(0.0, 0.0);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(5, 5));
    assert_eq!(p.bucket("check: glyph steps (advance for a line's last glyph)")[0], t(5, 5));
    assert_eq!(p.bucket("check: glyph x and step, whole text")[0], t(1, 1));
    assert_eq!(p.bucket("check: glyph x positions, first glyph of a line")[0], t(1, 1));
    let p = score(0.2, 0.0);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(0, 5), "every glyph stored 0.2 px to the right");
    assert_eq!(p.bucket("check: glyph steps (advance for a line's last glyph)")[0], t(5, 5), "a shift keeps every step");
    assert_eq!(p.bucket("check: glyph x positions, first glyph of a line")[0], t(0, 1));
    assert_eq!(p.details["glyph x miss, Studio minus Figma (upper edge of 0.05 px bins)"].get("-0.20"), Some(&5));
    let p = score(0.0, 0.1);
    assert_eq!(p.bucket("check: glyph steps (advance for a line's last glyph)")[0], t(4, 5), "the last glyph is scored by its advance, 0.1 px wider");
    assert_eq!(p.bucket("check: glyph advance of a line's last glyph")[0], t(0, 1));
    assert_eq!(p.bucket("total")[0], t(1, 1), "the glyph buckets stay out of the per-text pass, which keeps its meaning");
}

/// One Studio cluster by hand: start byte, end byte, x, width.
type Span = (usize, usize, f32, f32);

/// Studio's side by hand: lines of `(baseline, clusters)`.
fn geometry_of(lines: &[(f32, &[Span])]) -> designos_render::TextGeometry {
    designos_render::TextGeometry {
        lines: lines
            .iter()
            .map(|(baseline, spans)| designos_render::LineGeom {
                para: 0,
                top: *baseline - 14.0,
                height: 20.0,
                baseline: *baseline,
                width: 0.0,
                start: spans.first().map_or(0, |s| s.0),
                end: spans.last().map_or(0, |s| s.1),
                empty_x: 0.0,
                rtl: false,
                glyphs: spans.iter().map(|&(start, end, x, w)| designos_render::GlyphSpan { start, end, x, w, rtl: false }).collect(),
            })
            .collect(),
        width: 0.0,
        height: 0.0,
    }
}

/// Figma's side by hand, 16 px glyphs: `(first character in UTF-16 units, x, baseline, advance in px)`.
fn stored(glyphs: &[(u64, f32, f32, f32)]) -> Vec<GlyphTruth> {
    glyphs.iter().map(|&(first, x, y, adv)| GlyphTruth { first, x, y, advance: Some(adv / 16.0), font_size: 16.0 }).collect()
}

fn scored(content: &str, g: &designos_render::TextGeometry, truth: &[GlyphTruth], line_starts: &[u64], props: Option<TextProps>) -> Part {
    let props = props.unwrap_or_else(|| TextProps { content: content.into(), ..TextProps::default() });
    let mut p = Part::new("glyphs", "glyphs", "");
    text_glyphs::tally(&mut p, "", false, &text_glyphs::score(g, &props, truth, line_starts));
    p
}

const STEPS: &str = "check: glyph steps (advance for a line's last glyph)";

#[test]
fn glyph_steps_follow_a_kerned_pair_that_its_advance_does_not() {
    // "AV": the stored advance of A is the unkerned 10.0, the stored x of V is kerned to 9.4. Studio's cluster tiles
    // the line, so its A is 9.4 wide and V starts at 9.4: the same step as Figma's, not the same advance.
    let g = geometry_of(&[(14.0, &[(0, 1, 0.0, 9.4), (1, 2, 9.4, 10.5)])]);
    let p = scored("AV", &g, &stored(&[(0, 0.0, 14.0, 10.0), (1, 9.4, 14.0, 10.5)]), &[0], None);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(2, 2));
    assert_eq!(p.bucket(STEPS)[0], t(2, 2), "the kerned step matches");
    assert_eq!(p.bucket("check: glyph advance alone, as the first version compared it")[0], t(1, 2), "advance alone calls the kern a miss");
    assert_eq!(p.details["advance-alone misses by Figma's own step"].get("Figma's step differs from its advance"), Some(&1));
    let g = geometry_of(&[(14.0, &[(0, 1, 0.0, 9.4), (1, 2, 9.9, 10.5)])]);
    let p = scored("AV", &g, &stored(&[(0, 0.0, 14.0, 10.0), (1, 9.4, 14.0, 10.5)]), &[0], None);
    assert_eq!(p.bucket(STEPS)[0], t(1, 2), "a V Studio puts 0.5 px late is a missed step");
}

#[test]
fn glyph_matching_counts_characters_in_utf16_units() {
    // "a", an emoji (a surrogate pair, 4 bytes), "e" with a combining acute (3 bytes, 2 units), "b".
    let content = "a\u{1F600}e\u{301}b";
    let g = geometry_of(&[(14.0, &[(0, 1, 0.0, 8.0), (1, 5, 8.0, 10.0), (5, 8, 18.0, 10.0), (8, 9, 28.0, 9.0)])]);
    let truth = stored(&[(0, 0.0, 14.0, 8.0), (1, 8.0, 14.0, 10.0), (3, 18.0, 14.0, 10.0), (5, 28.0, 14.0, 9.0)]);
    let p = scored(content, &g, &truth, &[0], None);
    assert!(p.unscored.is_empty(), "every stored glyph found its cluster: {:?}", p.unscored);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(4, 4));
    assert_eq!(p.bucket(STEPS)[0], t(4, 4));
}

#[test]
fn glyph_checks_cover_a_centred_two_line_text() {
    // "AV" on a first line centred at x = 5.3 and "c" on a second line centred at x = 12; "\n" is character 2.
    let g = geometry_of(&[(14.0, &[(0, 1, 5.3, 9.4), (1, 2, 14.7, 10.5)]), (34.0, &[(3, 4, 12.0, 8.0)])]);
    let truth = stored(&[(0, 5.3, 14.0, 10.0), (1, 14.7, 14.0, 10.5), (3, 12.0, 34.0, 8.0)]);
    let p = scored("AV\nc", &g, &truth, &[0, 3], None);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(3, 3), "centred x on both lines");
    assert_eq!(p.bucket("check: glyph x positions, first glyph of a line")[0], t(2, 2), "two line starts");
    assert_eq!(p.bucket(STEPS)[0], t(3, 3), "one step across a kern, two line ends by advance");
    assert_eq!(p.bucket("check: glyph advance of a line's last glyph")[0], t(2, 2));
    assert_eq!(p.bucket("check: glyph advance alone, as the first version compared it")[0], t(2, 3));
    let g = geometry_of(&[(14.0, &[(0, 1, 5.3, 9.4), (1, 2, 14.7, 10.5)]), (34.0, &[(3, 4, 10.0, 8.0)])]);
    let p = scored("AV\nc", &g, &truth, &[0, 3], None);
    assert_eq!(p.bucket("check: glyph x positions, first glyph of a line")[0], t(1, 2), "a second line that is not centred like Figma's");
}

#[test]
fn glyph_checks_take_the_letter_spacing_of_the_last_glyph_off_by_run() {
    // Figma's advance leaves the letter spacing out; Studio's cluster width holds it. Here only a run covering V sets it.
    let g = geometry_of(&[(14.0, &[(0, 1, 0.0, 10.0), (1, 2, 10.0, 13.5)])]);
    let mut props = TextProps { content: "AV".into(), ..TextProps::default() };
    let mut run_style = props.style.clone();
    run_style.letter_spacing = 3.0;
    props.runs.push(TextRun { start: 1, end: 2, style: run_style });
    let p = scored("AV", &g, &stored(&[(0, 0.0, 14.0, 10.0), (1, 10.0, 14.0, 10.5)]), &[0], Some(props));
    assert_eq!(p.bucket("check: glyph advance of a line's last glyph")[0], t(1, 1), "V's width less the run's 3 px spacing is its advance");
}

#[test]
fn glyph_checks_leave_out_a_text_whose_line_breaks_differ() {
    let (_, doc, keys) = load(&text_fig(12.0));
    let mut truth = shaped_truth(&doc, &keys, 0.0, 0.0);
    truth.baselines[0].first = 3;
    let p = text::part(&[truth], 0, &doc, &keys);
    assert_eq!(p.bucket("check: glyph x positions")[0], t(0, 0), "one wrap difference would fail every later glyph");
    assert_eq!(p.unscored.get("glyph positions of a text whose line breaks differ").map(|u| u.0), Some(1));
}

#[test]
fn glyph_checks_measure_the_unit_of_the_stored_advance() {
    let (r, _, _) = load(&text_fig(12.0));
    let (truth, _) = text::truth(&r);
    let note = text_glyphs::unit_note(&truth);
    assert!(note.contains("in px on 0 of 1") && note.contains("font size on 1 of 1"), "the stored advance is a fraction of the font size: {note}");
}

#[test]
fn parts_are_selected_by_id_and_by_the_layout_and_text_aliases() {
    let only: HashSet<String> = ["stored_use", "save_class", "layout", "text"].iter().map(|s| s.to_string()).collect();
    let on = |id: &str| run::selected(Some(&only), id);
    assert!(on("stored_use") && on("save_class") && on("auto_layout") && on("auto_layout_measurer") && on("text_layout"));
    assert!(!on("save_loss") && !on("geometry") && !on("instances") && !on("import_coverage"));
    assert!(run::selected(None, "geometry"), "no list selects every part");
    let one: HashSet<String> = ["auto_layout".to_string()].into();
    assert!(run::selected(Some(&one), "auto_layout_measurer"), "auto_layout carries its measurer row");
    assert!(!run::selected(Some(&one), "text_layout"));
}

#[test]
fn a_whole_run_writes_every_part_on_one_file() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("scoreboard-whole-run");
    drop(std::fs::remove_dir_all(&dir));
    run::run_bytes(&mf::icon_fixture(mf::FAMILY), &dir, None);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("scoreboard.json")).unwrap()).unwrap();
    let parts = json["parts"].as_array().unwrap();
    for id in run::PARTS {
        let part = parts.iter().find(|p| p["id"] == id).unwrap_or_else(|| panic!("part {id} is in the report"));
        assert!(part["not_measured"].is_null(), "part {id} ran");
    }
    let stored = parts.iter().find(|p| p["id"] == "stored_use").unwrap();
    assert_eq!(stored["buckets"]["texts: total"]["visible"]["matched"], 1, "the text with a missing font draws from its stored glyphs");
}
