//! Every key the record's own entry of an unopened instance carries states a field, or is left as recorded. A key that
//! states a field is flagged when the file is opened, and a save writes the field from the node, instead of the recorded
//! key, once the user edited it. The assertions read the raw own entry of the saved file, not Studio's view of it.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

fn style_ref(key: &str) -> Value {
    json!({"assetRef": {"key": key, "version": "1:1"}})
}

fn shadow(y: f64) -> Value {
    json!({"type": "DROP_SHADOW", "color": rgba(0.0, 0.0, 0.0), "offset": {"x": 0.0, "y": y}, "radius": 8.0, "visible": true})
}

fn effect_style(key: &str) -> Value {
    fig_node((1, 60), (0, 1), "RECTANGLE", "Shadow Style", json!({"styleType": "EFFECT", "key": key, "effects": [shadow(4.0)]}))
}

fn blob_geometry() -> Value {
    json!([{"windingRule": "NONZERO", "commandsBlob": 0}])
}

fn own_entry_of(entries: &[Value]) -> Value {
    let me = guid_text(&guid(BUTTON.0, BUTTON.1));
    entries
        .iter()
        .find(|e| e["guidPath"]["guids"].as_array().is_some_and(|g| g.len() == 1 && guid_text(&g[0]) == me))
        .cloned()
        .unwrap_or_else(|| panic!("no own entry in {entries:?}"))
}

fn own_entries_of(entries: &[Value]) -> Vec<Value> {
    let me = guid_text(&guid(BUTTON.0, BUTTON.1));
    entries.iter().filter(|e| e["guidPath"]["guids"].as_array().is_some_and(|g| g.len() == 1 && guid_text(&g[0]) == me)).cloned().collect()
}

fn own_entry_json(rest: Value) -> Value {
    let mut own = json!({"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}});
    for (k, v) in rest.as_object().expect("entry object") {
        own[k] = v.clone();
    }
    own
}

fn fixture_for(entries: Vec<Value>, extra: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    fixture_with(json!({}), Value::Array(entries), extra, blobs)
}

fn edit_and_save(fig: &[u8], edit: impl FnOnce(&mut Node)) -> Vec<u8> {
    let mut live = import_fig(fig).expect("import");
    let button = variant_button(&live);
    edit(live.get_mut(button).expect("instance"));
    export_fig(&live).expect("export")
}

fn edit_fills(n: &mut Node) {
    n.style_refs.remove("fill");
    n.fills = vec![Paint::solid(Color::rgba(0, 0, 255, 255))];
}

fn edit_stroke(n: &mut Node) {
    n.style_refs.remove("stroke");
    n.stroke.paints = vec![Paint::solid(Color::rgba(0, 0, 255, 255))];
    n.stroke.weight = 7.0;
    n.stroke.align = StrokeAlign::Outside;
    n.stroke.join = StrokeJoin::Bevel;
    n.stroke.cap = StrokeCap::None;
    n.stroke.dashes = Vec::new();
    n.stroke.miter_limit = 9.0;
    n.stroke.side_weights = None;
    n.stroke_geometry = None;
}

fn edit_effects(n: &mut Node) {
    n.style_refs.remove("effect");
    n.effects = vec![Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 255),
        offset: Vec2::new(0.0, 11.0),
        radius: 3.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    }];
}

fn edit_corners(n: &mut Node) {
    n.corner_radii = [9.0; 4];
    n.corner_smoothing = 0.0;
    n.fill_geometry = None;
}

struct Row {
    key: &'static str,
    field: OverrideField,
    value: Value,
    extra: Vec<Value>,
    blobs: Vec<Vec<u8>>,
    edit: fn(&mut Node),
}

fn row(key: &'static str, field: OverrideField, value: Value, edit: fn(&mut Node)) -> Row {
    Row { key, field, value, extra: Vec::new(), blobs: Vec::new(), edit }
}

fn matrix(tx: f64, ty: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": tx, "m10": 0.0, "m11": 1.0, "m12": ty})
}

fn rows() -> Vec<Row> {
    use OverrideField::*;
    let green = || json!([solid(&rgba(0.0, 1.0, 0.0), None)]);
    let mut v = vec![
        row("name", Name, json!("Renamed"), |n| n.name = "Other".into()),
        row("visible", Visible, json!(true), |n| n.visible = false),
        row("locked", Locked, json!(false), |n| n.locked = true),
        row("proportionsConstrained", LockAspect, json!(false), |n| n.lock_aspect = true),
        row("opacity", Opacity, json!(0.5), |n| n.opacity = 0.25),
        row("blendMode", Blend, json!("MULTIPLY"), |n| n.blend = BlendMode::Darken),
        row("size", Size, json!({"x": 77.0, "y": 33.0}), |n| n.size = Vec2::new(90.0, 40.0)),
        row("transform", Transform, matrix(5.0, 6.0), |n| n.transform = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 40.0, ty: 50.0 }),
        row("fillPaints", Fills, green(), edit_fills),
        // A style link the file's library holds and the file does not (finding: the link is the only thing the entry states).
        row("styleIdForFill", Fills, style_ref("LIB_MISSING"), edit_fills),
        row("strokePaints", Stroke, green(), edit_stroke),
        row("strokeWeight", Stroke, json!(2.0), edit_stroke),
        row("strokeAlign", Stroke, json!("CENTER"), edit_stroke),
        row("strokeJoin", Stroke, json!("ROUND"), edit_stroke),
        row("strokeCap", Stroke, json!("ROUND"), edit_stroke),
        row("dashPattern", Stroke, json!([4.0, 2.0]), edit_stroke),
        row("miterLimit", Stroke, json!(2.0), edit_stroke),
        row("borderStrokeWeightsIndependent", Stroke, json!(true), edit_stroke),
        row("borderTopWeight", Stroke, json!(3.0), edit_stroke),
        row("borderRightWeight", Stroke, json!(3.0), edit_stroke),
        row("borderBottomWeight", Stroke, json!(3.0), edit_stroke),
        row("borderLeftWeight", Stroke, json!(3.0), edit_stroke),
        row("styleIdForStrokeFill", Stroke, style_ref("LIB_MISSING"), edit_stroke),
        row("effects", Effects, json!([shadow(4.0)]), edit_effects),
        row("styleIdForEffect", Effects, style_ref("LIB_MISSING"), edit_effects),
        row("cornerRadius", CornerRadii, json!(4.0), edit_corners),
        row("cornerSmoothing", CornerRadii, json!(0.4), edit_corners),
        row("rectangleCornerRadiiIndependent", CornerRadii, json!(true), edit_corners),
        row("rectangleTopLeftCornerRadius", CornerRadii, json!(2.0), edit_corners),
        row("rectangleTopRightCornerRadius", CornerRadii, json!(2.0), edit_corners),
        row("rectangleBottomLeftCornerRadius", CornerRadii, json!(2.0), edit_corners),
        row("rectangleBottomRightCornerRadius", CornerRadii, json!(2.0), edit_corners),
    ];
    // A style the file does hold: the link resolves.
    let mut local_effect = row("styleIdForEffect", Effects, style_ref("FX_ONE"), edit_effects);
    local_effect.extra = vec![effect_style("FX_ONE")];
    v.push(local_effect);
    let mut inherit_effect = row("inheritEffectStyleID", Effects, guid(1, 60), edit_effects);
    inherit_effect.extra = vec![effect_style("FX_ONE")];
    v.push(inherit_effect);
    let mut stroke_geometry = row("strokeGeometry", Stroke, blob_geometry(), edit_stroke);
    stroke_geometry.blobs = vec![line_blob(5.0)];
    v.push(stroke_geometry);
    let mut fill_geometry = row("fillGeometry", CornerRadii, blob_geometry(), edit_corners);
    fill_geometry.blobs = vec![line_blob(5.0)];
    v.push(fill_geometry);
    v
}

fn check_row(r: &Row) -> Result<(), String> {
    let fig = fixture_for(vec![own_entry_json(json!({ r.key: r.value.clone() }))], r.extra.clone(), r.blobs.clone());
    let recorded = own_entry_of(&saved_entries(&fig, INSTANCE));
    let live = import_fig(&fig).map_err(|e| format!("import: {e}"))?;
    let flags = &live.get(variant_button(&live)).ok_or("no instance")?.component.overrides;
    if !flags.contains(&r.field) {
        return Err(format!("{:?} is not flagged at open (flags {flags:?})", r.field));
    }
    let untouched = own_entry_of(&saved_entries(&export_fig(&live).map_err(|e| format!("export: {e}"))?, INSTANCE));
    if untouched.get(r.key).is_none() {
        return Err("a save without an edit drops the key".into());
    }
    if r.blobs.is_empty() && untouched != recorded {
        return Err(format!("a save without an edit changed the entry: {recorded} -> {untouched}"));
    }
    let edited = own_entry_of(&saved_entries(&edit_and_save(&fig, r.edit), INSTANCE));
    if edited.get(r.key) == untouched.get(r.key) {
        return Err(format!("the recorded key survives an edit of {:?}: {}", r.field, edited.get(r.key).unwrap_or(&Value::Null)));
    }
    Ok(())
}

#[test]
fn every_key_that_states_a_field_follows_an_edit_of_that_field() {
    let rows = rows();
    // Each row opens a file of 25,000 layers a few times; the rows are independent.
    let failures: Vec<String> = std::thread::scope(|scope| {
        let runs: Vec<_> = rows.iter().map(|r| scope.spawn(move || check_row(r).err().map(|e| format!("{}: {e}", r.key)))).collect();
        runs.into_iter().filter_map(|h| h.join().expect("a row panicked")).collect()
    });
    assert!(failures.is_empty(), "{} of {} keys fail:\n{}", failures.len(), rows.len(), failures.join("\n"));
}

#[test]
fn a_stroke_edit_keeps_the_layout_keys_of_the_entry() {
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL", "bordersTakeSpace": true,
        "strokePaints": [solid(&rgba(1.0, 0.0, 0.0), None)], "strokeWeight": 2.0
    }));
    let fig = fixture_for(vec![own], Vec::new(), Vec::new());
    let edited = own_entry_of(&saved_entries(&edit_and_save(&fig, edit_stroke), INSTANCE));
    assert_eq!(edited.get("bordersTakeSpace"), Some(&json!(true)), "the layout key is not a stroke key: {edited}");
    assert_eq!(edited.get("stackMode"), Some(&json!("HORIZONTAL")));
    assert_eq!(edited["strokeWeight"].as_f64(), Some(7.0), "the stroke itself is restated");
}

#[test]
fn a_fill_edit_leaves_the_stroke_and_layout_keys_as_recorded() {
    let own = own_entry_json(json!({
        "stackMode": "VERTICAL", "bordersTakeSpace": true, "styleIdForGrid": style_ref("GRID_ONE"),
        "fillPaints": [solid(&rgba(1.0, 0.0, 0.0), None)], "strokeWeight": 2.0
    }));
    let fig = fixture_for(vec![own], Vec::new(), Vec::new());
    let recorded = own_entry_of(&saved_entries(&fig, INSTANCE));
    let edited = own_entry_of(&saved_entries(&edit_and_save(&fig, edit_fills), INSTANCE));
    for key in ["stackMode", "bordersTakeSpace", "styleIdForGrid", "strokeWeight"] {
        assert_eq!(edited.get(key), recorded.get(key), "{key} is not a fill key");
    }
}

#[test]
fn a_field_edit_removes_the_old_value_from_every_own_entry_and_writes_the_new_one_in_the_last() {
    let red = solid(&rgba(1.0, 0.0, 0.0), None);
    let green = solid(&rgba(0.0, 1.0, 0.0), None);
    let first = own_entry_json(json!({"fillPaints": [red], "opacity": 0.5}));
    let second = own_entry_json(json!({"fillPaints": [green], "styleIdForFill": style_ref("LIB_MISSING")}));
    let fig = fixture_for(vec![first, second], Vec::new(), Vec::new());
    let saved = saved_entries(&edit_and_save(&fig, edit_fills), INSTANCE);
    let own = own_entries_of(&saved);
    assert_eq!(own.len(), 2, "both entries stay: {own:?}");
    for e in &own {
        assert!(e.get("styleIdForFill").is_none(), "the old link stays in {e}");
    }
    assert!(own[0].get("fillPaints").is_none(), "the old fills stay in the first entry: {}", own[0]);
    assert_eq!(own[0].get("opacity").and_then(Value::as_f64), Some(0.5), "an untouched key is kept");
    let fresh = own[1]["fillPaints"][0]["color"]["b"].as_f64().expect("the last entry holds the new fills");
    assert!(fresh > 0.99, "the new fill is blue, got {fresh}");
}

#[test]
fn a_save_without_an_edit_keeps_every_own_entry_as_recorded() {
    let first = own_entry_json(json!({"fillPaints": [solid(&rgba(1.0, 0.0, 0.0), None)], "bordersTakeSpace": true}));
    let second = own_entry_json(json!({"styleIdForFill": style_ref("LIB_MISSING"), "strokeWeight": 2.0, "styleIdForEffect": style_ref("LIB_MISSING")}));
    let fig = fixture_for(vec![first, second], Vec::new(), Vec::new());
    let live = import_fig(&fig).expect("import");
    let saved = export_fig(&live).expect("export");
    assert_eq!(own_entries_of(&saved_entries(&saved, INSTANCE)), own_entries_of(&saved_entries(&fig, INSTANCE)));
}
