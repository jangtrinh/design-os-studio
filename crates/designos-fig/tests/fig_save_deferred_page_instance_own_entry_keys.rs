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
    /// The key the save writes the edited field under.
    wrote: &'static str,
    /// What the key does to the record: it states the field (flagged at open, replaced by an edit), is a baked outline
    /// (never flagged, removed by an edit), or is kept exactly as recorded.
    class: Class,
}

#[derive(Clone, Copy, PartialEq)]
enum Class {
    States,
    Outline,
    Kept,
}

fn row(key: &'static str, field: OverrideField, value: Value, edit: fn(&mut Node)) -> Row {
    let wrote = match key {
        "styleIdForFill" => "fillPaints",
        "styleIdForStrokeFill" | "strokeGeometry" => "strokePaints",
        "styleIdForEffect" | "inheritEffectStyleID" => "effects",
        "borderStrokeWeightsIndependent" | "borderTopWeight" | "borderRightWeight" | "borderBottomWeight" | "borderLeftWeight" => "strokeWeight",
        "rectangleCornerRadiiIndependent" | "fillGeometry" | "cornerSmoothing" => "cornerRadius",
        k if k.starts_with("rectangle") => "cornerRadius",
        k => k,
    };
    let class = match key {
        "strokeGeometry" | "fillGeometry" => Class::Outline,
        "cornerSmoothing" => Class::Kept,
        _ => Class::States,
    };
    Row { key, field, value, extra: Vec::new(), blobs: Vec::new(), edit, wrote, class }
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
        // A style link the file's library holds and the file does not (a link with no paints beside it).
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

/// The key a baked outline or smoothing is recorded beside: the field is then stated, and an edit restates it.
fn companion(r: &Row) -> Value {
    match r.field {
        OverrideField::Stroke => json!({"strokeWeight": 2.0}),
        _ => json!({"cornerRadius": 4.0}),
    }
}

/// Every row gets its own instance of the button, side by side in the variant, each holding that row's own entry. The
/// 25,000 layers that make the loader defer the page (more than 25,000 nodes and more than two pages, `rest_import.rs`)
/// are then built, encoded, opened and saved once for all rows instead of once per row; the rows touch different
/// instances, so they stay independent.
fn row_instance(i: usize) -> (u64, u64) {
    (2, i as u64 + 1)
}

fn row_name(i: usize) -> String {
    format!("row {i}")
}

/// A file with one instance per `(index, entry)`. The shared style nodes and blobs the rows need are added once.
fn rows_fixture(rows: &[Row], entries: Vec<(usize, Value)>) -> Vec<u8> {
    let mut extra: Vec<Value> = Vec::new();
    let mut blobs: Vec<Vec<u8>> = Vec::new();
    for r in rows {
        for e in &r.extra {
            if !extra.contains(e) {
                extra.push(e.clone());
            }
        }
        for b in &r.blobs {
            if !blobs.contains(b) {
                blobs.push(b.clone());
            }
        }
    }
    assert!(blobs.len() <= 1, "the rows share one blob, `commandsBlob: 0`");
    for (i, entry) in entries {
        let symbol = json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [own_entry_json(entry)]}});
        extra.push(fig_node(row_instance(i), VARIANT, "INSTANCE", &row_name(i), symbol));
    }
    fixture_with(json!({}), control_overrides(), extra, blobs)
}

/// The file's own instance (`INSTANCE`) is the control: it states fills and a stroke weight and no row edits it.
fn control_overrides() -> Value {
    json!([own_entry_json(json!({"fillPaints": [solid(&rgba(0.0, 1.0, 0.0), None)], "strokeWeight": 2.0}))])
}

fn control_entry(saved: &[u8]) -> Value {
    own_entry_of(&saved_entries(saved, INSTANCE))
}

fn assert_page_stays_unloaded(doc: &Document) {
    assert!(!page_hydrated(doc, page_named(doc, "404 pages")), "the page stays unloaded");
}

/// The own entry each row's instance holds in `saved`, by row index.
fn row_entries(saved: &[u8], rows: &[usize]) -> Vec<(usize, Value)> {
    let mut by_guid: std::collections::HashMap<String, Value> = node_changes(saved).into_iter().map(|nc| (guid_text(&nc["guid"]), nc)).collect();
    rows.iter()
        .map(|&i| {
            let id = row_instance(i);
            let nc = by_guid.remove(&guid_text(&guid(id.0, id.1))).unwrap_or_else(|| panic!("no instance for row {i} in the saved file"));
            (i, own_entry_of(nc["symbolData"]["symbolOverrides"].as_array().map(Vec::as_slice).unwrap_or_default()))
        })
        .collect()
}

fn row_instance_node(doc: &Document, i: usize) -> NodeId {
    let variant = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop").expect("variant");
    let name = row_name(i);
    variant.children.iter().copied().find(|&c| doc.get(c).is_some_and(|n| n.name == name)).unwrap_or_else(|| panic!("no instance for row {i}"))
}

fn flags_of(doc: &Document, i: usize) -> Vec<OverrideField> {
    let node = doc.get(row_instance_node(doc, i)).expect("instance");
    OverrideField::ALL.into_iter().filter(|field| node.component.has_override(field)).collect()
}

/// The entry that states the field the row's key belongs to: the key itself, or, for a baked outline or smoothing, the
/// key beside the field it restates.
fn full_entry(r: &Row) -> Value {
    match r.class {
        Class::States => json!({ r.key: r.value.clone() }),
        Class::Outline | Class::Kept => {
            let mut both = companion(r);
            both[r.key] = r.value.clone();
            both
        }
    }
}

struct Seen {
    alone_flags: Option<Vec<OverrideField>>,
    flags: Vec<OverrideField>,
    recorded: Value,
    untouched: Value,
    edited: Value,
}

fn check_row(r: &Row, s: &Seen) -> Result<(), String> {
    if let Some(flags) = &s.alone_flags {
        // Alone, the key states nothing: the field is not flagged and keeps following the main.
        if flags.contains(&r.field) {
            return Err(format!("{:?} is flagged at open by the key alone (flags {flags:?})", r.field));
        }
    }
    if !s.flags.contains(&r.field) {
        return Err(format!("{:?} is not flagged at open (flags {:?})", r.field, s.flags));
    }
    let (recorded, untouched, edited) = (&s.recorded, &s.untouched, &s.edited);
    if untouched.get(r.key).is_none() {
        return Err("a save without an edit drops the key".into());
    }
    if r.blobs.is_empty() && untouched != recorded {
        return Err(format!("a save without an edit changed the entry: {recorded} -> {untouched}"));
    }
    match r.class {
        Class::States if r.wrote == r.key && edited.get(r.key) == untouched.get(r.key) => {
            return Err(format!("the recorded key survives an edit of {:?}: {}", r.field, edited.get(r.key).unwrap_or(&Value::Null)));
        }
        Class::States if r.wrote != r.key && edited.get(r.key).is_some() => return Err(format!("the recorded key survives an edit of {:?}", r.field)),
        Class::Outline if edited.get(r.key).is_some() => return Err("the baked outline survives an edit of its field".into()),
        Class::Kept if edited.get(r.key) != untouched.get(r.key) => return Err("the key was not kept as recorded".into()),
        _ => {}
    }
    if edited.get(r.wrote).is_none() {
        return Err(format!("the edited value is not written back under {}: {edited}", r.wrote));
    }
    Ok(())
}

#[test]
fn every_key_that_states_a_field_follows_an_edit_of_that_field() {
    let rows = rows();
    let all: Vec<usize> = (0..rows.len()).collect();
    // The baked outlines and smoothing, each alone in its entry.
    let lone: Vec<usize> = all.iter().copied().filter(|&i| rows[i].class != Class::States).collect();
    let alone = rows_fixture(&rows, lone.iter().map(|&i| (i, lone_entry(&rows[i]))).collect());
    let alone_live = import_fig(&alone).expect("import the keys alone");
    assert_page_stays_unloaded(&alone_live);
    let alone_flags: Vec<(usize, Vec<OverrideField>)> = lone.iter().map(|&i| (i, flags_of(&alone_live, i))).collect();

    let fig = rows_fixture(&rows, all.iter().map(|&i| (i, full_entry(&rows[i]))).collect());
    let recorded = row_entries(&fig, &all);
    let live = import_fig(&fig).expect("import");
    assert_page_stays_unloaded(&live);
    let flags: Vec<Vec<OverrideField>> = all.iter().map(|&i| flags_of(&live, i)).collect();
    let untouched_bytes = export_fig(&live).expect("export");
    let untouched = row_entries(&untouched_bytes, &all);

    let mut edited_live = import_fig(&fig).expect("import");
    assert_page_stays_unloaded(&edited_live);
    for &i in &all {
        let instance = row_instance_node(&edited_live, i);
        (rows[i].edit)(edited_live.get_mut(instance).expect("instance"));
    }
    let edited_bytes = export_fig(&edited_live).expect("export");
    assert_page_stays_unloaded(&import_fig(&edited_bytes).expect("reopen the edited save"));
    let edited = row_entries(&edited_bytes, &all);
    // No row edits the control: the edits of the others must not reach it.
    let (control_recorded, control_untouched, control_edited) = (control_entry(&fig), control_entry(&untouched_bytes), control_entry(&edited_bytes));
    assert_eq!(control_untouched, control_recorded, "a save without an edit changed the control entry");
    assert_eq!(control_edited, control_untouched, "an edit of another instance changed the control entry");

    let failures: Vec<String> = all
        .iter()
        .filter_map(|&i| {
            let seen = Seen {
                alone_flags: alone_flags.iter().find(|(j, _)| *j == i).map(|(_, f)| f.clone()),
                flags: flags[i].clone(),
                recorded: recorded[i].1.clone(),
                untouched: untouched[i].1.clone(),
                edited: edited[i].1.clone(),
            };
            check_row(&rows[i], &seen).err().map(|e| format!("row {i} {}: {e}", rows[i].key))
        })
        .collect();
    println!("own-entry key rows checked: {}", rows.len());
    assert!(failures.is_empty(), "{} of {} keys fail:\n{}", failures.len(), rows.len(), failures.join("\n"));
}

/// The key alone in its entry.
fn lone_entry(r: &Row) -> Value {
    json!({ r.key: r.value.clone() })
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
fn an_unopened_instance_layout_edit_replaces_recorded_spacing_without_rewriting_child_metadata() {
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0,
        "stackPositioning": "ABSOLUTE",
        "minSize": {"value": {"x": 12.0, "y": 8.0}},
        "maxSize": {"value": {"x": 240.0, "y": 180.0}},
        "targetAspectRatio": {"value": {"x": 16.0, "y": 9.0}}
    }));
    let fig = deferred_layout_fixture(json!([own]));
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout").item_spacing = 20.0;
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    assert_eq!(edited["stackSpacing"].as_f64(), Some(20.0), "edited spacing must replace the record: {edited}");
    for key in ["stackPositioning", "minSize", "maxSize", "targetAspectRatio"] {
        assert_eq!(edited.get(key), own.get(key), "{key} must stay record-only");
    }
    let reopened = import_fig(&saved).expect("reopen saved fig");
    let instance = variant_button(&reopened);
    assert_eq!(reopened.get(instance).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).map(|al| al.item_spacing), Some(20.0),);
}

#[test]
fn an_unopened_instance_spacing_edit_preserves_other_recorded_own_layout_fields_and_grid_anchors() {
    let col_a = guid(9, 10);
    let col_b = guid(9, 11);
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0,
        "stackPrimaryAlignItems": "CENTER",
        "stackCounterAlignContent": "SPACE_BETWEEN",
        "stackLayoutVersion": 1,
        "stackPadding": 16.0,
        "frameMaskDisabled": false,
        "layoutGrids": [{"pattern": "GRID", "sectionSize": 8.0, "visible": true, "color": rgba(1.0, 0.0, 0.0)}],
        "gridColumns": {"entries": [
            {"id": col_a, "position": "!"},
            {"id": col_b, "position": "\""}
        ]},
        "gridColumnsSizing": {"entries": [
            {"id": col_a, "trackSize": {"minSizing": {"type": "FLEX", "value": 1.0}, "maxSizing": {"type": "FLEX", "value": 1.0}}},
            {"id": col_b, "trackSize": {"minSizing": {"type": "FIXED", "value": 120.0}, "maxSizing": {"type": "FIXED", "value": 120.0}}}
        ]}
    }));
    let child_anchor = json!({
        "guidPath": {"guids": [guid(LABEL.0, LABEL.1)]},
        "gridColumnAnchor": col_b
    });
    let fig = deferred_layout_fixture(json!([own, child_anchor]));

    // Edit only the spacing via the existing frame_props.auto_layout path used by
    // properties::update_al, variables_apply::set_bound_node_property, and spec::apply_spec_to_node.
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout").item_spacing = 20.0;
    });

    let mut reopened = import_fig(&saved).expect("reopen saved fig");
    assert_page_stays_unloaded(&reopened);
    reopened.hydrate_all_pages();

    let instance = variant_button(&reopened);
    let inst_node = reopened.get(instance).expect("hydrated instance");
    let fp = inst_node.kind.frame_props().expect("hydrated instance frame");
    let al = fp.auto_layout.as_ref().expect("hydrated instance auto-layout");
    let label_id =
        inst_node.children.iter().copied().find(|&c| reopened.get(c).is_some_and(|n| matches!(n.kind, NodeKind::Text(_)))).expect("hydrated label child");
    let label_col = reopened.get(label_id).expect("label").layout_child.grid_column;
    assert_eq!(
        (
            al.item_spacing,
            al.primary_align,
            al.counter_align_content,
            al.layout_version,
            al.padding,
            fp.clips_content,
            fp.grids.len(),
            al.grid_columns,
            &al.grid_col_tracks[..2],
            label_col,
        ),
        (
            20.0,
            LayoutAlign::Center,
            CounterAlignContent::SpaceBetween,
            1,
            [16.0, 16.0, 16.0, 16.0],
            true,
            1,
            2,
            &[GridTrack::Flex(1.0), GridTrack::Fixed(120.0)][..],
            Some(1),
        ),
    );
}

#[test]
fn an_unopened_instance_layout_grid_edit_replaces_recorded_guides() {
    let own = own_entry_json(json!({
        "layoutGrids": [{"pattern": "GRID", "sectionSize": 8.0, "visible": true, "color": rgba(1.0, 0.0, 0.0)}]
    }));
    let fig = deferred_layout_fixture(json!([own]));
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").grids = vec![LayoutGrid::grid(24.0, Color::rgba(0, 255, 0, 128))];
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    let guide = edited["layoutGrids"].as_array().and_then(|g| g.first()).expect("restated layout grid");
    assert_eq!(guide["sectionSize"].as_f64(), Some(24.0), "edited entry: {edited}");
    assert_eq!(guide["color"]["g"].as_f64(), Some(1.0));
}

#[test]
fn an_unopened_instance_layout_clear_replaces_recorded_mode_with_none() {
    let own = own_entry_json(json!({"stackMode": "HORIZONTAL", "stackSpacing": 4.0}));
    let fig = deferred_layout_fixture(json!([own]));
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout = None;
        n.component.layout_cleared = true;
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    assert_eq!(edited.get("stackMode"), Some(&json!("NONE")), "cleared layout must not be re-inherited: {edited}");
    assert!(edited.get("stackSpacing").is_none(), "stale auto-layout fields must be removed: {edited}");
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

#[test]
fn a_corner_radius_edit_keeps_the_recorded_corner_smoothing() {
    let own = own_entry_json(json!({"cornerRadius": 4.0, "cornerSmoothing": 0.6}));
    let fig = fixture_for(vec![own], Vec::new(), Vec::new());
    let edited = own_entry_of(&saved_entries(&edit_and_save(&fig, |n| n.corner_radii = [10.0; 4]), INSTANCE));
    assert_eq!(edited["cornerRadius"].as_f64(), Some(10.0), "the radius is restated: {edited}");
    assert_eq!(edited["cornerSmoothing"].as_f64(), Some(0.6), "the recorded smoothing is kept: {edited}");
}

fn main_stroke(n: &mut Node) {
    n.stroke.paints = vec![Paint::solid(Color::rgba(0, 0, 255, 255))];
    n.stroke.weight = 5.0;
}

fn main_radius(n: &mut Node) {
    n.corner_radii = [12.0; 4];
}

/// An own entry that carries only baked outlines or smoothing states no field: the instance keeps following its main.
fn follows_main_edit(entry: Value, blobs: Vec<Vec<u8>>, edit_main: fn(&mut Node), seen: fn(&Node) -> bool) {
    let fig = fixture_for(vec![own_entry_json(entry)], Vec::new(), blobs);
    for hydrate_first in [false, true] {
        let mut live = import_fig(&fig).expect("import");
        if hydrate_first {
            live.hydrate_all_pages();
        }
        let button = variant_button(&live);
        let flags = &live.get(button).expect("instance").component.overrides;
        assert!(!flags.contains(&OverrideField::Stroke) && !flags.contains(&OverrideField::CornerRadii), "flags at open: {flags:?}");
        let main = live.get(button).and_then(|n| n.component.source).expect("main of the instance");
        edit_main(live.get_mut(main).expect("main"));
        sync_instances(&mut live, main).apply(&mut live).expect("sync");
        assert!(seen(live.get(button).expect("instance")), "the instance stopped following its main (hydrated first: {hydrate_first})");
    }
}

#[test]
fn an_entry_with_only_a_stroke_outline_follows_a_stroke_edit_of_its_main() {
    follows_main_edit(json!({"size": {"x": 130.0, "y": 48.0}, "strokeGeometry": blob_geometry()}), vec![line_blob(5.0)], main_stroke, |n| {
        n.stroke.weight == 5.0
    });
}

#[test]
fn an_entry_with_only_a_fill_outline_follows_a_radius_edit_of_its_main() {
    follows_main_edit(json!({"size": {"x": 130.0, "y": 48.0}, "fillGeometry": blob_geometry()}), vec![line_blob(5.0)], main_radius, |n| {
        n.corner_radii == [12.0; 4]
    });
}

#[test]
fn an_entry_with_only_corner_smoothing_follows_a_radius_edit_of_its_main() {
    follows_main_edit(json!({"cornerSmoothing": 0.6}), Vec::new(), main_radius, |n| n.corner_radii == [12.0; 4]);
}

fn deferred_layout_fixture_with_main(main_fields: Value, instance_overrides: Value) -> Vec<u8> {
    let instance = json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": instance_overrides}});
    let mut changes = vec![
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Cover", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Page 2", json!({})),
        fig_node((0, 3), (0, 0), "CANVAS", "404 pages", json!({})),
        fig_node(BUTTON, (0, 3), "SYMBOL", "Buttons/Button", main_fields),
        fig_node(LABEL, BUTTON, "TEXT", "Text", json!({"fontSize": 14.0, "textData": {"characters": "Button CTA"}})),
        fig_node(ICON, BUTTON, "ROUNDED_RECTANGLE", "Icon", json!({"strokePaints": [solid(&rgba(0.0, 0.0, 0.0), None)], "strokeWeight": 1.5})),
        fig_node(VARIANT, (0, 3), "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", json!({"size": {"x": 1440.0, "y": 960.0}})),
        fig_node(INSTANCE, VARIANT, "INSTANCE", "Buttons/Button", instance),
    ];
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    fig_bytes(changes, Vec::new())
}

#[test]
fn an_unopened_instance_spacing_edit_changes_only_stack_spacing_and_keeps_inheriting_main_layout() {
    let main_fields = json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0,
        "stackPadding": 24.0,
        "stackPrimaryAlignItems": "CENTER"
    });
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0
    }));
    let fig = deferred_layout_fixture_with_main(main_fields, json!([own]));
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout").item_spacing = 20.0;
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    let expected = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 20.0
    }));
    assert_eq!(edited, expected, "only stackSpacing may change in the saved own entry");

    let mut reopened = import_fig(&saved).expect("reopen saved fig");
    assert_page_stays_unloaded(&reopened);
    reopened.hydrate_all_pages();
    let instance = variant_button(&reopened);
    let al = reopened.get(instance).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).expect("reopened auto layout");
    assert_eq!(
        (al.item_spacing, al.padding, al.primary_align),
        (20.0, [24.0, 24.0, 24.0, 24.0], LayoutAlign::Center),
        "reopened instance must keep inheriting main padding and alignment"
    );
}

#[test]
fn an_unopened_instance_with_every_layout_own_key_keeps_all_on_no_edit_and_changes_one_key_on_single_edit() {
    let col_a = guid(9, 10);
    let row_a = guid(9, 20);
    let own = own_entry_json(json!({
        "frameMaskDisabled": true,
        "gridColumns": {"entries": [{"id": col_a, "position": "!"}]},
        "gridColumnsSizing": {"entries": [{"id": col_a, "trackSize": {"minSizing": {"type": "FLEX", "value": 1.0}, "maxSizing": {"type": "FLEX", "value": 1.0}}}]},
        "gridRows": {"entries": [{"id": row_a, "position": "!"}]},
        "gridRowsSizing": {"entries": [{"id": row_a, "trackSize": {"minSizing": {"type": "FIXED", "value": 40.0}, "maxSizing": {"type": "FIXED", "value": 40.0}}}]},
        "gridAutoTracks": "ROWS",
        "gridColumnGap": 6.0,
        "gridRowGap": 8.0,
        "layoutGrids": [{"pattern": "GRID", "sectionSize": 8.0, "visible": true, "color": rgba(1.0, 0.0, 0.0)}],
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0,
        "stackCounterSpacing": 10.0,
        "stackPadding": 16.0,
        "stackHorizontalPadding": 12.0,
        "stackVerticalPadding": 14.0,
        "stackPaddingRight": 18.0,
        "stackPaddingBottom": 22.0,
        "stackPrimaryAlignItems": "CENTER",
        "stackCounterAlignItems": "MAX",
        "stackCounterAlignContent": "SPACE_BETWEEN",
        "stackLayoutVersion": 1,
        "stackPrimarySizing": "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE",
        "stackCounterSizing": "RESIZE_TO_FIT",
        "stackWrap": "WRAP",
        "bordersTakeSpace": true,
        "stackReverseZIndex": true
    }));
    assert_eq!(own.as_object().expect("own object").len(), 27, "guidPath + all 26 Layout own keys");
    let fig = deferred_layout_fixture(json!([own]));
    let recorded = own_entry_of(&saved_entries(&fig, INSTANCE));

    let live = import_fig(&fig).expect("import");
    assert_page_stays_unloaded(&live);
    let untouched = own_entry_of(&saved_entries(&export_fig(&live).expect("no-edit export"), INSTANCE));
    assert_eq!(untouched, recorded, "no-edit save must keep every Layout own key as read");

    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout").item_spacing = 20.0;
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    let mut expected = recorded;
    expected["stackSpacing"] = json!(20.0);
    assert_eq!(edited, expected, "a one-field edit must change only stackSpacing across all 26 Layout own keys");
}

#[test]
fn an_unopened_instance_resize_to_fit_and_stack_padding_stay_as_recorded_when_unedited() {
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 4.0,
        "stackCounterSizing": "RESIZE_TO_FIT",
        "stackPadding": 16.0
    }));
    let fig = deferred_layout_fixture(json!([own]));
    let recorded = own_entry_of(&saved_entries(&fig, INSTANCE));
    let saved = edit_and_save(&fig, |n| {
        n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout").item_spacing = 20.0;
    });
    let edited = own_entry_of(&saved_entries(&saved, INSTANCE));
    let mut expected = recorded;
    expected["stackSpacing"] = json!(20.0);
    assert_eq!(edited, expected, "RESIZE_TO_FIT and stackPadding must stay as recorded when unedited");
}

#[test]
fn a_budget_skipped_instance_on_an_opened_page_keeps_its_recorded_layout_on_no_edit_save() {
    let own = own_entry_json(json!({
        "stackMode": "HORIZONTAL",
        "stackSpacing": 12.0,
        "stackPadding": 16.0,
        "stackCounterSizing": "RESIZE_TO_FIT"
    }));
    let instance = json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [own]}});
    let mut changes = vec![
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Cover", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Page 2", json!({})),
        fig_node((0, 3), (0, 0), "CANVAS", "404 pages", json!({})),
        fig_node(BUTTON, (0, 3), "SYMBOL", "Buttons/Button", json!({"stackMode": "HORIZONTAL", "stackSpacing": 4.0})),
        fig_node(LABEL, BUTTON, "TEXT", "Text", json!({"fontSize": 14.0, "textData": {"characters": "Button CTA"}})),
        fig_node(ICON, BUTTON, "ROUNDED_RECTANGLE", "Icon", json!({"strokePaints": [solid(&rgba(0.0, 0.0, 0.0), None)], "strokeWeight": 1.5})),
        fig_node((1, 13), (0, 3), "SYMBOL", "SwapTarget", json!({})),
        fig_node((1, 14), (1, 13), "INSTANCE", "Inner", json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": []}})),
        fig_node((1, 15), (0, 3), "SYMBOL", "OuterComp", json!({})),
        fig_node((1, 17), (0, 3), "SYMBOL", "EmptyComp", json!({})),
        fig_node((1, 16), (1, 15), "INSTANCE", "Slot", json!({"symbolData": {"symbolID": guid(1, 17), "symbolOverrides": []}})),
        fig_node(
            (1, 18),
            (0, 3),
            "INSTANCE",
            "OuterUse",
            json!({
                "symbolData": {
                    "symbolID": guid(1, 15),
                    "symbolOverrides": [{"guidPath": {"guids": [guid(1, 16)]}, "overriddenSymbolID": guid(1, 13)}]
                }
            }),
        ),
        fig_node(VARIANT, (0, 3), "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", json!({"size": {"x": 1440.0, "y": 960.0}})),
        fig_node(INSTANCE, VARIANT, "INSTANCE", "Buttons/Button", instance),
    ];
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    let fig = fig_bytes(changes, Vec::new());
    let recorded = own_entry_of(&saved_entries(&fig, INSTANCE));
    let (mut live, _) = designos_fig::import_fig_with_budget(&fig, 6).expect("import with budget 6");
    assert_page_stays_unloaded(&live);
    let page = page_named(&live, "404 pages");
    assert!(live.ensure_page_hydrated(page), "page hydration succeeds when instance is skipped by clone budget");
    assert_eq!(live.hydration_unexpanded_instances(), 1, "instance was skipped by clone budget");
    let button = variant_button(&live);
    assert!(live.get(button).expect("instance").children.is_empty(), "target instance must remain unexpanded after budget skip");

    let saved = export_fig(&live).expect("export after budget-skipped page hydration");
    let after = own_entry_of(&saved_entries(&saved, INSTANCE));
    assert_eq!(after, recorded, "budget-skipped instance must keep recorded layout keys on a no-edit save");
}

#[test]
fn a_shortened_grid_track_list_leaves_recorded_grid_tracks_and_anchors_untouched() {
    let col_a = guid(9, 10);
    let col_b = guid(9, 11);
    let own = own_entry_json(json!({
        "stackMode": "GRID",
        "gridColumnGap": 4.0,
        "gridColumns": {"entries": [
            {"id": col_a, "position": "!"},
            {"id": col_b, "position": "\""}
        ]},
        "gridColumnsSizing": {"entries": [
            {"id": col_a, "trackSize": {"minSizing": {"type": "FLEX", "value": 1.0}, "maxSizing": {"type": "FLEX", "value": 1.0}}},
            {"id": col_b, "trackSize": {"minSizing": {"type": "FIXED", "value": 120.0}, "maxSizing": {"type": "FIXED", "value": 120.0}}}
        ]}
    }));
    let child_first = json!({"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "gridColumnAnchor": col_a});
    let child_last = json!({"guidPath": {"guids": [guid(ICON.0, ICON.1)]}, "gridColumnAnchor": col_b});
    let main_fields = json!({"stackMode": "GRID", "gridColumnGap": 4.0, "gridRowGap": 0.0});
    let fig = deferred_layout_fixture_with_main(main_fields, json!([own, child_first, child_last]));
    let recorded_entries = saved_entries(&fig, INSTANCE);

    // Case 1: removed first track (leaving only Fixed(120.0)) + spacing edit.
    // Case 2: removed last track (leaving only Flex(1.0)) + spacing edit.
    for remaining_track in [GridTrack::Fixed(120.0), GridTrack::Flex(1.0)] {
        let saved = edit_and_save(&fig, |n| {
            let al = n.kind.frame_props_mut().expect("instance frame").auto_layout.as_mut().expect("recorded auto layout");
            al.item_spacing = 20.0;
            al.grid_columns = 1;
            al.grid_col_tracks = vec![remaining_track].into();
        });
        let entries = saved_entries(&saved, INSTANCE);
        let mut expected = recorded_entries.clone();
        expected[0]["gridColumnGap"] = json!(20.0);
        assert_eq!(entries, expected, "grid tracks and child anchors must stay as recorded when track count changed ({remaining_track:?})");
    }
}

#[test]
fn native_document_save_does_not_contain_unopened_outer_frame_metadata() {
    let own = own_entry_json(json!({"stackMode": "HORIZONTAL", "stackSpacing": 12.0}));
    let fig = deferred_layout_fixture(json!([own]));
    let mut live = import_fig(&fig).expect("import");
    let instance = variant_button(&live);
    assert!(
        !live.get(instance).expect("instance").fig_metadata.contains_key("_unopenedOuterFrame"),
        "Node::fig_metadata must not carry internal _unopenedOuterFrame key"
    );
    assert!(
        !serde_json::to_string(&live).expect("serialize live document").contains("_unopenedOuterFrame"),
        "serialized Document must not contain _unopenedOuterFrame while deferred"
    );
    live.load_pending_pages().expect("load pending pages for native save");
    let native_json = live.to_json().expect("serialize native document");
    assert!(!native_json.contains("_unopenedOuterFrame"), "native document JSON must not contain _unopenedOuterFrame");
}
