//! An edit that sets no override flag on an unopened instance (the agent tools write the node's fields directly) is
//! still an edit: a save follows the size the instance has now, and a fill, stroke or effect it states now wins over
//! the style the file linked to it.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

const STYLE: (u64, u64) = (1, 60);

/// The pending instance's own entry states only its fill (and the given style links); its derived data states the size
/// the producing app computed for the instance itself and for the label.
fn fixture_with_own_entry(style_keys: &[&str]) -> Vec<u8> {
    let style_ref = json!({"assetRef": {"key": "STYLE_GREEN", "version": "1:1"}});
    let mut own = json!({"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}, "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)]});
    let mut fields = json!({
        "size": {"x": 130.0, "y": 48.0},
        "derivedSymbolData": [
            {"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}, "size": {"x": 130.0, "y": 48.0}},
            {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}}
        ]
    });
    for key in style_keys {
        own[*key] = style_ref.clone();
        fields[*key] = style_ref.clone();
    }
    let mut extra = Vec::new();
    if !style_keys.is_empty() {
        extra.push(fig_node(
            STYLE,
            (0, 1),
            "RECTANGLE",
            "Green",
            json!({"styleType": "FILL", "key": "STYLE_GREEN", "fillPaints": [solid(&rgba(0.0, 1.0, 0.0), None)]}),
        ));
    }
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(own);
    fixture_with(fields, overrides, extra, Vec::new())
}

fn reopened(saved: &[u8]) -> Document {
    let mut doc = import_fig(saved).expect("reopen");
    doc.hydrate_all_pages();
    doc
}

fn first_fill(doc: &Document, id: NodeId) -> Option<Color> {
    match doc.get(id)?.fills.first()?.kind {
        PaintKind::Solid(c) => Some(c),
        _ => None,
    }
}

fn own_entry(saved: &[u8]) -> Value {
    entry_at(&saved_entries(saved, INSTANCE), BUTTON).clone()
}

fn has_key(entry: &Value, key: &str) -> bool {
    entry.get(key).is_some()
}

#[test]
fn a_resize_that_sets_no_flag_survives_a_save_when_the_file_states_no_own_size() {
    let fig = fixture_with_own_entry(&[]);
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    assert!(!live.get(button).expect("instance").component.overrides.contains(&OverrideField::Size), "the record states no size for the instance");
    live.get_mut(button).expect("instance").size = Vec2::new(200.0, 60.0);

    let saved = export_fig(&live).expect("export");
    let back = reopened(&saved);
    let size = back.get(variant_button(&back)).expect("instance").size;
    assert_eq!((size.x, size.y), (200.0, 60.0), "the derived size of the old instance does not put its old size back");
    assert!(saved_derived(&saved, INSTANCE).is_empty(), "derived data computed for the old size is not written");
}

#[test]
fn derived_data_stays_while_the_unflagged_instance_keeps_its_size() {
    let fig = fixture_with_own_entry(&[]);
    let live = import_fig(&fig).expect("import");
    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_derived(&saved, INSTANCE), saved_derived(&fig, INSTANCE));

    // A move or a fill does not change the sizes inside the instance.
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    live.get_mut(button).expect("instance").fills = vec![Paint::solid(Color::rgba(255, 0, 0, 255))];
    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_derived(&saved, INSTANCE), saved_derived(&fig, INSTANCE));
}

#[test]
fn a_fill_change_wins_over_the_fill_style_the_file_linked() {
    let fig = fixture_with_own_entry(&["styleIdForFill"]);
    let at_open = import_fig(&fig).expect("import");
    assert!(at_open.get(variant_button(&at_open)).expect("instance").component.overrides.contains(&OverrideField::Fills));
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    {
        let n = live.get_mut(button).expect("instance");
        n.fills = vec![Paint::solid(Color::rgba(255, 0, 0, 255))];
        n.style_refs.remove("fill");
    }

    let saved = export_fig(&live).expect("export");
    assert!(!has_key(&own_entry(&saved), "styleIdForFill"), "the link to the old style is not written back");
    let back = reopened(&saved);
    assert_eq!(first_fill(&back, variant_button(&back)), Some(Color::rgba(255, 0, 0, 255)));
}

#[test]
fn a_fill_nobody_touched_keeps_its_style_link() {
    let fig = fixture_with_own_entry(&["styleIdForFill"]);
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    live.get_mut(button).expect("instance").size = Vec2::new(200.0, 60.0);
    let saved = export_fig(&live).expect("export");
    assert_eq!(own_entry(&saved)["styleIdForFill"], own_entry(&fig)["styleIdForFill"]);
}

#[test]
fn a_stroke_change_wins_over_the_stroke_style_the_file_linked() {
    let fig = fixture_with_own_entry(&["styleIdForStrokeFill"]);
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    {
        let n = live.get_mut(button).expect("instance");
        n.stroke = Stroke { paints: vec![Paint::solid(Color::rgba(255, 0, 0, 255))], weight: 2.0, ..Stroke::default() };
        n.style_refs.remove("stroke");
        n.component.overrides.insert(OverrideField::Stroke);
    }
    let saved = export_fig(&live).expect("export");
    let own = own_entry(&saved);
    assert!(has_key(&own, "strokePaints"), "the stroke is written from the node");
    assert!(!has_key(&own, "styleIdForStrokeFill"), "the link to the old stroke style is not written back");
}

#[test]
fn an_effect_change_wins_over_the_effect_style_the_file_linked() {
    let fig = fixture_with_own_entry(&["styleIdForEffect"]);
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    {
        let n = live.get_mut(button).expect("instance");
        n.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 64),
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: BlendMode::Normal,
        }];
        n.style_refs.remove("effect");
        n.component.overrides.insert(OverrideField::Effects);
    }
    let saved = export_fig(&live).expect("export");
    let own = own_entry(&saved);
    assert!(has_key(&own, "effects"), "the effects are written from the node");
    assert!(!has_key(&own, "styleIdForEffect"), "the link to the old effect style is not written back");
}
