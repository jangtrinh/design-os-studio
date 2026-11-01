//! Structural fixtures for real-file `.fig` features, written as kiwi-shaped
//! JSON with the field names seen in real files: an internal-only canvas of
//! library components, nested library instances, overrides addressed by
//! `overrideKey`, component properties, library styles and variable sets.
//! Hand-written; nothing here comes from a real file.

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use serde_json::{json, Value};

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 10.0, "y": 10.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn solid(r: f64, gr: f64, b: f64) -> Value {
    json!({"type": "SOLID", "color": {"r": r, "g": gr, "b": b, "a": 1.0}, "opacity": 1.0, "visible": true, "blendMode": "NORMAL"})
}

fn at(x: f64, y: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": x, "m10": 0.0, "m11": 1.0, "m12": y})
}

fn fixture() -> Value {
    let lib_key = "lk-library-a";
    json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({"backgroundColor": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        // ---- library: an icon, and a button that nests the icon
        node(
            (5, 1),
            (0, 2),
            "a",
            "SYMBOL",
            "Icon",
            json!({
            "componentKey": "K_ICON", "sourceLibraryKey": lib_key, "size": {"x": 20.0, "y": 20.0}, "overrideKey": g(9, 1)})
        ),
        node(
            (5, 2),
            (5, 1),
            "a",
            "VECTOR",
            "Glyph",
            json!({
            "overrideKey": g(9, 2), "transform": at(4.0, 4.0), "size": {"x": 10.0, "y": 10.0}, "fillPaints": [solid(1.0, 0.0, 0.0)]})
        ),
        node(
            (5, 3),
            (0, 2),
            "b",
            "SYMBOL",
            "Button",
            json!({
            "componentKey": "K_BTN", "sourceLibraryKey": lib_key, "size": {"x": 100.0, "y": 40.0},
            "componentPropDefs": [{"id": g(7, 1), "name": "Show icon", "type": "BOOL", "initialValue": {"boolValue": true}}]})
        ),
        node(
            (5, 4),
            (5, 3),
            "a",
            "TEXT",
            "Label",
            json!({
            "overrideKey": g(9, 4), "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular"},
            "textData": {"characters": "Go"}, "fillPaints": [solid(0.0, 0.0, 0.0)]})
        ),
        node(
            (5, 5),
            (5, 3),
            "b",
            "INSTANCE",
            "icon",
            json!({
            "overrideKey": g(9, 5), "transform": at(70.0, 10.0), "size": {"x": 20.0, "y": 20.0},
            "symbolData": {"symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0},
            "componentPropRefs": [{"componentPropNodeField": "VISIBLE", "defID": g(7, 1)}]})
        ),
        // a library style, referenced by key from overrides
        node(
            (5, 6),
            (0, 2),
            "c",
            "ROUNDED_RECTANGLE",
            "brand/green",
            json!({
            "styleType": "FILL", "key": "STYLE_GREEN", "fillPaints": [solid(0.0, 1.0, 0.0)]})
        ),
        // ---- page: instances of library components
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "Buy",
            json!({
            "size": {"x": 100.0, "y": 40.0}, "transform": at(10.0, 10.0),
            "symbolData": {"symbolID": g(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {"guidPath": {"guids": [g(9, 4)]}, "textData": {"characters": "Buy"}},
                {"guidPath": {"guids": [g(9, 5), g(9, 2)]}, "fillPaints": [solid(0.0, 0.0, 1.0)]}]},
            "derivedSymbolData": [{"guidPath": {"guids": [g(9, 4)]}, "size": {"x": 50.0, "y": 20.0}}],
            "componentPropAssignments": [{"defID": g(7, 1), "value": {"boolValue": false}}]})
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "Buy 2",
            json!({
            "size": {"x": 100.0, "y": 40.0}, "transform": at(10.0, 60.0),
            "symbolData": {"symbolID": g(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {"guidPath": {"guids": [g(9, 5), g(9, 2)]}, "styleIdForFill": {"assetRef": {"key": "STYLE_GREEN", "version": "1:1"}}}]}})
        ),
        node(
            (1, 3),
            (0, 1),
            "c",
            "INSTANCE",
            "Small icon",
            json!({
            "size": {"x": 10.0, "y": 10.0}, "transform": at(10.0, 110.0),
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 0.5, "symbolOverrides": []}})
        ),
        // a frame whose width is bound to a variable through the library alias
        node(
            (1, 4),
            (0, 1),
            "d",
            "FRAME",
            "Bound",
            json!({
            "variableConsumptionMap": {"entries": [{"variableField": "WIDTH",
                "variableData": {"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"assetRef": {"key": "VAR_SIZE", "version": "1:1"}}}}}]},
            "variableModeBySetMap": {"entries": [{"variableModeID": g(6, 11), "variableSetID": {"assetRef": {"key": "SET_KEY", "version": "1:1"}}}]}})
        ),
        // ---- variables
        node(
            (6, 1),
            (0, 2),
            "d",
            "VARIABLE_SET",
            "Spacing",
            json!({"key": "SET_KEY", "variableSetModes": [
            {"id": g(6, 11), "name": "Dark", "sortPosition": "\""}, {"id": g(6, 10), "name": "Light", "sortPosition": "!"}]})
        ),
        node(
            (6, 2),
            (0, 2),
            "e",
            "VARIABLE",
            "size/base",
            json!({
            "key": "VAR_SIZE", "variableResolvedType": "FLOAT", "variableSetID": {"assetRef": {"key": "SET_KEY", "version": "1:1"}},
            "variableDataValues": {"entries": [
                {"modeID": g(6, 10), "variableData": {"dataType": "FLOAT", "resolvedDataType": "FLOAT", "value": {"floatValue": 8.0}}},
                {"modeID": g(6, 11), "variableData": {"dataType": "FLOAT", "resolvedDataType": "FLOAT", "value": {"floatValue": 12.0}}}]}})
        ),
        node(
            (6, 3),
            (0, 2),
            "f",
            "VARIABLE",
            "size/alias",
            json!({
            "key": "VAR_ALIAS", "variableResolvedType": "FLOAT", "variableSetID": {"assetRef": {"key": "SET_KEY", "version": "1:1"}},
            "variableDataValues": {"entries": [
                {"modeID": g(6, 10), "variableData": {"dataType": "ALIAS", "resolvedDataType": "FLOAT",
                    "value": {"alias": {"assetRef": {"key": "VAR_SIZE", "version": "1:1"}}}}}]}})
        )
    ])
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node {name}"))
}

fn child<'a>(doc: &'a Document, n: &Node, name: &str) -> &'a Node {
    n.children.iter().map(|c| doc.get(*c).unwrap()).find(|c| c.name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn solid_of(n: &Node) -> Color {
    match n.fills[0].kind {
        PaintKind::Solid(c) => c,
        _ => panic!("not solid"),
    }
}

#[test]
fn internal_canvas_becomes_the_library_page_with_external_assets() {
    let doc = import_fig_node_changes(&fixture(), "t").unwrap();
    assert_eq!(doc.pages.len(), 1, "the internal canvas is not a page");
    let lp = doc.library_page.expect("library page");
    assert!(!doc.pages.contains(&lp));
    for (key, name) in [("K_BTN", "Button"), ("K_ICON", "Icon")] {
        let asset = doc.library.external.get(&local_key("remote:lk-library-a", key)).unwrap_or_else(|| panic!("asset {key}"));
        assert_eq!(asset.kind, LibraryAssetKind::Component);
        let comp = asset.node.unwrap();
        assert_eq!(doc.get(comp).unwrap().name, name);
        assert!(is_library_node(&doc, comp));
    }
    let inst = find(&doc, "Buy");
    let comp = instance_component(inst).expect("resolved");
    assert_eq!(doc.get(comp).unwrap().name, "Button");
    assert!(matches!(&inst.kind, NodeKind::Instance { component_key: Some(k), .. } if k == "K_BTN"));
    assert!(library_instance_info(&doc, inst.id).unwrap().remote);
}

#[test]
fn nested_library_instances_expand_and_overrides_follow_override_keys() {
    let doc = import_fig_node_changes(&fixture(), "t").unwrap();
    let buy = find(&doc, "Buy");
    let label = child(&doc, buy, "Label");
    let NodeKind::Text(t) = &label.kind else { panic!() };
    assert_eq!(t.content, "Buy", "single-key path");
    assert_eq!(label.size, Vec2::new(50.0, 20.0), "derived size");
    assert!(label.component.source.is_some() && buy.component.source.is_some());

    // The nested icon was expanded (innermost first) and the two-key path reached its glyph.
    let icon = child(&doc, buy, "icon");
    let glyph = child(&doc, icon, "Glyph");
    assert_eq!(solid_of(glyph), Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 });
    assert!(glyph.component.source.is_some());
    // The component property hid the nested icon.
    assert!(!icon.visible);

    // The library component itself is untouched by the instance's overrides.
    let comp = doc.get(instance_component(buy).unwrap()).unwrap();
    let lib_icon = child(&doc, comp, "icon");
    assert!(lib_icon.visible);
    assert_eq!(solid_of(child(&doc, child(&doc, comp, "icon"), "Glyph")), Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
}

#[test]
fn overrides_that_name_a_library_style_take_its_paints() {
    let doc = import_fig_node_changes(&fixture(), "t").unwrap();
    let buy2 = find(&doc, "Buy 2");
    let glyph = child(&doc, child(&doc, buy2, "icon"), "Glyph");
    assert_eq!(solid_of(glyph), Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 });
    assert!(child(&doc, buy2, "icon").visible, "no assignment: the default stays");
}

#[test]
fn uniform_scale_scales_the_copied_contents() {
    let doc = import_fig_node_changes(&fixture(), "t").unwrap();
    let small = find(&doc, "Small icon");
    let glyph = child(&doc, small, "Glyph");
    assert_eq!(glyph.size, Vec2::new(5.0, 5.0));
    assert_eq!(glyph.transform.translation(), Vec2::new(2.0, 2.0));
}

#[test]
fn variable_sets_map_by_asset_key_with_modes_aliases_and_bindings() {
    let doc = import_fig_node_changes(&fixture(), "t").unwrap();
    assert_eq!((doc.variable_collections.len(), doc.variables.len()), (1, 2));
    let coll = &doc.variable_collections["VariableCollectionId:6:1"];
    assert_eq!(coll.key.as_deref(), Some("SET_KEY"));
    assert_eq!(coll.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Light", "Dark"], "ordered by sortPosition");
    assert_eq!(coll.default_mode, "6:10");
    let base = &doc.variables["VariableID:6:2"];
    assert_eq!(base.values["6:11"], VariableValue::Number(12.0));
    assert_eq!(doc.variables["VariableID:6:3"].values["6:10"], VariableValue::Alias("VariableID:6:2".into()));
    let bound = find(&doc, "Bound");
    assert_eq!(bound.variables.bindings.get("width").map(String::as_str), Some("VariableID:6:2"));
    assert_eq!(bound.variables.modes.get("VariableCollectionId:6:1").map(String::as_str), Some("6:11"));
    assert!(!doc.nodes.values().any(|n| matches!(n.kind, NodeKind::Unknown { .. })), "variables are not canvas nodes");
}

#[test]
fn files_without_content_on_the_internal_canvas_have_no_library_page() {
    let only = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true}))
    ]);
    let doc = import_fig_node_changes(&only, "t").unwrap();
    assert!(doc.library_page.is_none());
}

#[test]
fn linked_text_styles_replace_stale_properties_before_character_overrides() {
    for style_ref in [json!({"guid": g(7, 1)}), json!({"assetRef": {"key": "BODY_TEXT"}})] {
        let input = json!([
            node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
            node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
            node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
            node(
                (7, 1),
                (0, 2),
                "a",
                "TEXT",
                "Body",
                json!({
                    "styleType": "TEXT", "key": "BODY_TEXT", "fontSize": 16.0,
                    "fontName": {"family": "Inter", "style": "Regular"},
                    "lineHeight": {"units": "PIXELS", "value": 28.0},
                    "letterSpacing": {"units": "PERCENT", "value": 0.0}
                })
            ),
            node(
                (1, 1),
                (0, 1),
                "a",
                "TEXT",
                "Caption",
                json!({
                    "fontSize": 48.0, "fontName": {"family": "Inter", "style": "Extra Bold"},
                    "lineHeight": {"units": "PIXELS", "value": 60.0},
                    "textDecoration": "UNDERLINE", "textAlignHorizontal": "RIGHT",
                    "styleIdForText": style_ref, "fillPaints": [solid(1.0, 0.0, 0.0)],
                    "textData": {"characters": "AB", "characterStyleIDs": [0, 1],
                        "styleOverrideTable": [{"styleID": 1, "fontSize": 20.0, "textDecoration": "UNDERLINE"}]}
                })
            ),
            node(
                (1, 2),
                (0, 1),
                "b",
                "TEXT",
                "Tracking",
                json!({
                    "fontSize": 48.0, "styleIdForText": style_ref,
                    "textData": {"characters": "C", "characterStyleIDs": [1],
                        "styleOverrideTable": [{"styleID": 1, "letterSpacing": {"units": "PERCENT", "value": 10.0}}]}
                })
            )
        ]);
        let doc = import_fig_node_changes(&input, "t").unwrap();
        let NodeKind::Text(t) = &find(&doc, "Caption").kind else { panic!("text") };
        assert_eq!(t.style.font_size, 16.0);
        assert_eq!(t.style.font_weight, 400);
        assert_eq!(t.style.line_height, LineHeight::Pixels(28.0));
        assert_eq!(t.style.decoration, TextDecoration::None);
        assert_eq!(t.style.align_h, TextAlignH::Right);
        assert_eq!(t.runs[0].style.font_size, 20.0);
        assert_eq!(t.runs[0].style.font_weight, 400);
        assert_eq!(t.runs[0].style.decoration, TextDecoration::Underline);
        assert_eq!(t.style.fills[0].kind, PaintKind::Solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }));
        let NodeKind::Text(t) = &find(&doc, "Tracking").kind else { panic!("text") };
        assert!((t.runs[0].style.letter_spacing - 1.6).abs() < 1e-6);
    }
}

#[test]
fn unavailable_text_style_keeps_local_properties() {
    let input = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node(
            (1, 1),
            (0, 1),
            "a",
            "TEXT",
            "Fallback",
            json!({
                "fontSize": 24.0, "fontName": {"family": "Inter", "style": "Bold"},
                "styleIdForText": {"assetRef": {"key": "UNAVAILABLE"}}, "textData": {"characters": "Hello"}
            })
        )
    ]);
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let NodeKind::Text(t) = &find(&doc, "Fallback").kind else { panic!("text") };
    assert_eq!(t.style.font_size, 24.0);
    assert_eq!(t.style.font_weight, 700);
}

/// #1327: `styleIdForText` naming a style relinks it; a null or unresolved
/// `styleIdForText` clears the link (Figma's detach); every other override,
/// including font metrics with no style id, keeps the inherited link.
#[test]
fn instance_text_style_override_replaces_or_clears_the_master_link() {
    let alias = |key: &str| json!({"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"assetRef": {"key": key, "version": "1"}}}});
    let bind = |field: &str, key: &str| json!({"variableField": field, "variableData": alias(key)});
    let var = |id: u64, pos: &str, name: &str, key: &str, value: f64| {
        node(
            (6, id),
            (0, 2),
            pos,
            "VARIABLE",
            name,
            json!({
                "key": key, "variableResolvedType": "FLOAT",
                "variableSetID": {"assetRef": {"key": "SET_TYPE", "version": "1"}},
                "variableDataValues": {"entries": [
                    {"modeID": g(6, 10), "variableData": {"dataType": "FLOAT", "resolvedDataType": "FLOAT", "value": {"floatValue": value}}}
                ]}
            }),
        )
    };
    let text_style = |id: u64, pos: &str, name: &str, size: f64, style: &str, lh: f64, size_var: &str, lh_var: Option<&str>| {
        let mut entries = vec![bind("FONT_SIZE", size_var)];
        if let Some(k) = lh_var {
            entries.push(bind("LINE_HEIGHT", k));
        }
        node(
            (8, id),
            (0, 2),
            pos,
            "TEXT",
            name,
            json!({
                "styleType": "TEXT", "fontSize": size,
                "fontName": {"family": "Inter", "style": style},
                "lineHeight": {"units": "PIXELS", "value": lh},
                "variableConsumptionMap": {"entries": entries}
            }),
        )
    };
    let input = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node(
            (6, 1),
            (0, 2),
            "v",
            "VARIABLE_SET",
            "Type",
            json!({"key": "SET_TYPE", "variableSetModes": [
                {"id": g(6, 10), "name": "Mode", "sortPosition": "!"}
            ]})
        ),
        var(2, "w", "Font size/text-md", "VAR_MD", 16.0),
        var(3, "x", "Line height/text-md", "VAR_MD_LH", 24.0),
        var(4, "y", "Font size/text-sm", "VAR_SM", 14.0),
        var(5, "z", "Line height/text-sm", "VAR_SM_LH", 20.0),
        var(6, "A", "Font size/text-xl", "VAR_XL", 20.0),
        text_style(1, "s", "Text md/Semibold", 16.0, "Semi Bold", 24.0, "VAR_MD", Some("VAR_MD_LH")),
        text_style(2, "t", "Text sm/Semibold", 14.0, "Semi Bold", 20.0, "VAR_SM", Some("VAR_SM_LH")),
        text_style(3, "u", "Text xl/Regular", 18.0, "Regular", 28.0, "VAR_XL", None),
        node((5, 1), (0, 2), "c", "SYMBOL", "Button", json!({"size": {"x": 80.0, "y": 32.0}})),
        node(
            (5, 2),
            (5, 1),
            "a",
            "TEXT",
            "Label",
            json!({
                "overrideKey": g(9, 2), "size": {"x": 65.0, "y": 20.0},
                "fontSize": 16.0, "fontName": {"family": "Inter", "style": "Semi Bold"},
                "lineHeight": {"units": "PIXELS", "value": 24.0},
                "styleIdForText": {"guid": g(8, 1)},
                "textData": {"characters": "Label"},
                "fillPaints": [solid(0.0, 0.0, 0.0)]
            })
        ),
        node((5, 3), (0, 2), "d", "SYMBOL", "Header block", json!({"size": {"x": 200.0, "y": 40.0}})),
        node(
            (5, 4),
            (5, 3),
            "a",
            "TEXT",
            "Title",
            json!({
                "overrideKey": g(9, 4), "size": {"x": 180.0, "y": 28.0},
                "fontSize": 48.0, "fontName": {"family": "Inter", "style": "Extra Bold"},
                "lineHeight": {"units": "PERCENT", "value": 150.0},
                "styleIdForText": {"guid": g(8, 3)},
                "textData": {"characters": "Title"}
            })
        ),
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "Overview",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 0.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForText": {"guid": g(8, 2)},
                    "fontSize": 14.0,
                    "fontName": {"family": "Inter", "style": "Semi Bold"},
                    "lineHeight": {"units": "PIXELS", "value": 20.0},
                    "textData": {"characters": "Overview"}
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "Detach",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 40.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForText": {"guid": g(4294967295, 4294967295)},
                    "fontSize": 12.0,
                    "fontName": {"family": "Inter", "style": "Semi Bold"},
                    "lineHeight": {"units": "PIXELS", "value": 16.0},
                    "textData": {"characters": "Off"}
                }]}
            })
        ),
        node(
            (1, 3),
            (0, 1),
            "c",
            "INSTANCE",
            "Header",
            json!({
                "size": {"x": 200.0, "y": 40.0}, "transform": at(0.0, 80.0),
                "symbolData": {"symbolID": g(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 4)]},
                    "textData": {"characters": "Design system"}
                }]}
            })
        ),
        node(
            (1, 4),
            (0, 1),
            "d",
            "INSTANCE",
            "Card title",
            json!({
                "size": {"x": 200.0, "y": 40.0}, "transform": at(0.0, 120.0),
                "symbolData": {"symbolID": g(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 4)]},
                    "fontSize": 18.0,
                    "lineHeight": {"units": "PERCENT", "value": 150.0},
                    "fontName": {"family": "Inter", "style": "Regular"},
                    "fontVersion": "1.0"
                }]}
            })
        ),
    ]);
    let doc = import_fig_node_changes(&input, "t").unwrap();

    let overview = child(&doc, find(&doc, "Overview"), "Label");
    assert_eq!(doc.style_name(overview.style_refs.get("text").expect("sm style link")), "Text sm/Semibold");
    let NodeKind::Text(raw) = &overview.kind else { panic!("text") };
    assert!((raw.style.font_size - 14.0).abs() < 1e-3, "raw keeps the override size");
    let resolved = resolve_node(&doc, overview);
    let NodeKind::Text(t) = &resolved.kind else { panic!("text") };
    assert!((t.style.font_size - 14.0).abs() < 1e-3, "resolved size follows Text sm/Semibold, got {}", t.style.font_size);
    assert_eq!(t.style.line_height, LineHeight::Pixels(20.0));
    assert_eq!(t.style.font_weight, 600);

    let detach = child(&doc, find(&doc, "Detach"), "Label");
    assert!(!detach.style_refs.contains_key("text"), "a null styleIdForText detaches the master's link");
    let resolved = resolve_node(&doc, detach);
    let NodeKind::Text(t) = &resolved.kind else { panic!("text") };
    assert!((t.style.font_size - 12.0).abs() < 1e-3, "resolved size stays the override's own, got {}", t.style.font_size);
    assert_eq!(t.style.line_height, LineHeight::Pixels(16.0));
    assert_eq!(t.style.font_weight, 600);
    assert!(detach.variables.bindings.keys().all(|k| k.starts_with("fills/")), "inherited text-style bindings are cleared");

    let header = child(&doc, find(&doc, "Header"), "Title");
    assert_eq!(doc.style_name(header.style_refs.get("text").expect("xl style link")), "Text xl/Regular");
    let NodeKind::Text(raw) = &header.kind else { panic!("text") };
    assert!((raw.style.font_size - 18.0).abs() < 1e-3, "raw stays the style's pre-binding size");
    assert_eq!(raw.content, "Design system");
    let resolved = resolve_node(&doc, header);
    let NodeKind::Text(t) = &resolved.kind else { panic!("text") };
    assert!((t.style.font_size - 20.0).abs() < 1e-3, "the inherited link still resolves, got {}", t.style.font_size);
    assert_eq!(t.style.font_weight, 400);

    // Kit shape: font metrics and no styleIdForText. The stale 18 is not what renders.
    let card = child(&doc, find(&doc, "Card title"), "Title");
    assert_eq!(doc.style_name(card.style_refs.get("text").expect("xl style link")), "Text xl/Regular");
    let NodeKind::Text(raw) = &card.kind else { panic!("text") };
    assert!((raw.style.font_size - 18.0).abs() < 1e-3, "raw keeps the stale override size");
    assert_eq!(raw.style.line_height, LineHeight::Percent(150.0));
    let resolved = resolve_node(&doc, card);
    let NodeKind::Text(t) = &resolved.kind else { panic!("text") };
    assert!((t.style.font_size - 20.0).abs() < 1e-3, "the inherited link still resolves, got {}", t.style.font_size);
    assert_eq!(t.style.font_weight, 400);

    let master = child(&doc, find(&doc, "Button"), "Label");
    assert_eq!(doc.style_name(master.style_refs.get("text").unwrap()), "Text md/Semibold");
}

/// #1327: relinking or clearing the text style is an override. Editing the main
/// component's text style afterwards leaves the instance link as the override set it.
#[test]
fn instance_text_style_link_override_survives_main_edit() {
    let text_style = |id: u64, pos: &str, name: &str, size: f64| {
        node(
            (8, id),
            (0, 2),
            pos,
            "TEXT",
            name,
            json!({
                "styleType": "TEXT", "fontSize": size,
                "fontName": {"family": "Inter", "style": "Regular"}
            }),
        )
    };
    let input = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        text_style(1, "s", "Text md/Semibold", 16.0),
        text_style(2, "t", "Text sm/Semibold", 14.0),
        text_style(3, "u", "Text xl/Regular", 20.0),
        node((5, 1), (0, 2), "c", "SYMBOL", "Button", json!({"size": {"x": 80.0, "y": 32.0}})),
        node(
            (5, 2),
            (5, 1),
            "a",
            "TEXT",
            "Label",
            json!({
                "overrideKey": g(9, 2), "size": {"x": 65.0, "y": 20.0},
                "fontSize": 16.0, "fontName": {"family": "Inter", "style": "Semi Bold"},
                "styleIdForText": {"guid": g(8, 1)},
                "textData": {"characters": "Label"}
            })
        ),
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "Relink",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 0.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForText": {"guid": g(8, 2)}
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "External",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 40.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForText": {"assetRef": {"key": "EXT_SM", "version": "1"}}
                }]}
            })
        ),
        node(
            (1, 3),
            (0, 1),
            "c",
            "INSTANCE",
            "Detach",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 80.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForText": {"guid": g(4294967295, 4294967295)}
                }]}
            })
        ),
        node(
            (1, 4),
            (0, 1),
            "d",
            "INSTANCE",
            "Plain",
            json!({
                "size": {"x": 80.0, "y": 32.0}, "transform": at(0.0, 120.0),
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0}
            })
        ),
    ]);
    let mut doc = import_fig_node_changes(&input, "t").unwrap();

    let relink_id = child(&doc, find(&doc, "Relink"), "Label").id;
    let external_id = child(&doc, find(&doc, "External"), "Label").id;
    let detach_id = child(&doc, find(&doc, "Detach"), "Label").id;
    let plain_id = child(&doc, find(&doc, "Plain"), "Label").id;
    let button_id = find(&doc, "Button").id;
    let master_id = child(&doc, doc.get(button_id).unwrap(), "Label").id;
    assert_eq!(doc.style_name(doc.get(relink_id).unwrap().style_refs.get("text").expect("sm link")), "Text sm/Semibold");
    assert_eq!(doc.get(external_id).unwrap().style_refs.get("text").map(String::as_str), Some("key:EXT_SM"));
    assert!(!doc.get(detach_id).unwrap().style_refs.contains_key("text"), "null styleIdForText clears the link");

    let xl = doc.style_meta.iter().find(|(_, m)| m.name == "Text xl/Regular").expect("xl style").0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("text".into(), xl);
    sync_instances(&mut doc, button_id).apply(&mut doc).unwrap();

    assert_eq!(doc.style_name(doc.get(plain_id).unwrap().style_refs.get("text").expect("plain follows the main")), "Text xl/Regular");
    assert_eq!(doc.style_name(doc.get(relink_id).unwrap().style_refs.get("text").expect("relink kept")), "Text sm/Semibold");
    assert_eq!(
        doc.get(external_id).unwrap().style_refs.get("text").map(String::as_str),
        Some("key:EXT_SM"),
        "a style link override is not replaced by the main"
    );
    assert!(!doc.get(detach_id).unwrap().style_refs.contains_key("text"), "a cleared text style stays cleared after the main changes");
}

fn paint_style_fixture(overrides: Value) -> Value {
    let color_var = |id: u64, pos: &str, name: &str, key: &str, r: f64, gr: f64, b: f64| {
        node(
            (6, id),
            (0, 2),
            pos,
            "VARIABLE",
            name,
            json!({
                "key": key, "variableResolvedType": "COLOR", "variableSetID": {"guid": g(6, 1)},
                "variableDataValues": {"entries": [{
                    "modeID": g(6, 10),
                    "variableData": {"dataType": "COLOR", "resolvedDataType": "COLOR", "value": {"colorValue": {"r": r, "g": gr, "b": b, "a": 1.0}}}
                }]}
            }),
        )
    };
    let fill_style = |id: u64, pos: &str, name: &str, r: f64, gr: f64, b: f64, var_id: u64| {
        let mut p = solid(r, gr, b);
        p["colorVar"] = json!({"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": g(6, var_id)}}});
        node((8, id), (0, 2), pos, "RECTANGLE", name, json!({"styleType": "FILL", "fillPaints": [p]}))
    };
    let mut arr = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node(
            (6, 1),
            (0, 2),
            "a",
            "VARIABLE_SET",
            "Palette",
            json!({"key": "PAL", "variableSetModes": [{"id": g(6, 10), "name": "Light", "sortPosition": "!"}]}),
        ),
        color_var(2, "b", "Colors/Brand/600", "VAR_BRAND", 0.33, 0.22, 0.62),
        color_var(3, "c", "Colors/Neutral/600", "VAR_NEUTRAL", 0.32, 0.32, 0.32),
        color_var(4, "d", "Colors/Accent/600", "VAR_ACCENT", 0.05, 0.58, 0.53),
        node(
            (6, 5),
            (0, 2),
            "e",
            "VARIABLE",
            "Op/25",
            json!({
                "key": "VAR_OP", "variableResolvedType": "FLOAT", "variableSetID": {"guid": g(6, 1)},
                "variableDataValues": {"entries": [{
                    "modeID": g(6, 10),
                    "variableData": {"dataType": "FLOAT", "resolvedDataType": "FLOAT", "value": {"floatValue": 0.25}}
                }]}
            }),
        ),
        fill_style(1, "s", "Brand/600", 0.50, 0.34, 0.85, 2),
        fill_style(2, "t", "Neutral/600", 0.32, 0.32, 0.32, 3),
        fill_style(3, "u", "Accent/600", 0.05, 0.58, 0.53, 4),
        node(
            (8, 9),
            (0, 2),
            "v",
            "TEXT",
            "Heading",
            json!({"styleType": "TEXT", "fontSize": 30.0, "fontName": {"family": "Inter", "style": "Regular"}, "textData": {"characters": "Ag"}}),
        ),
        node((5, 1), (0, 2), "c", "SYMBOL", "Swatch", json!({"size": {"x": 80.0, "y": 40.0}})),
        node(
            (5, 2),
            (5, 1),
            "a",
            "VECTOR",
            "Color",
            json!({
                "overrideKey": g(9, 2), "size": {"x": 80.0, "y": 40.0},
                "styleIdForFill": {"guid": g(8, 1)},
                "fillPaints": [solid(0.50, 0.34, 0.85)],
                "styleIdForStrokeFill": {"guid": g(8, 1)},
                "strokePaints": [solid(0.50, 0.34, 0.85)],
                "strokeWeight": 1.0
            }),
        ),
        node(
            (5, 3),
            (5, 1),
            "b",
            "TEXT",
            "Label",
            json!({
                "overrideKey": g(9, 3), "size": {"x": 80.0, "y": 20.0},
                "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular"}, "textData": {"characters": "Go"},
                "styleIdForFill": {"guid": g(8, 1)},
                "fillPaints": [solid(0.50, 0.34, 0.85)]
            }),
        ),
    ];
    for inst in overrides.as_array().unwrap() {
        arr.push(inst.clone());
    }
    Value::Array(arr)
}

fn stroke_solid_of(n: &Node) -> Color {
    match n.stroke.paints[0].kind {
        PaintKind::Solid(c) => c,
        _ => panic!("not solid stroke"),
    }
}

/// A fill override with a named `styleIdForFill` relinks `style_refs["fill"]`
/// instead of keeping the main component's fill style link.
#[test]
fn instance_fill_override_with_named_style_relinks() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "RelinkFill",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"guid": g(8, 2)},
                    "fillPaints": [solid(0.28, 0.33, 0.40)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "ExternalFill",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"assetRef": {"key": "EXT_FILL", "version": "1"}}
                }]}
            })
        )
    ]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let relink_id = child(&doc, find(&doc, "RelinkFill"), "Color").id;
    let ext_id = child(&doc, find(&doc, "ExternalFill"), "Color").id;
    let relink = doc.get(relink_id).unwrap();
    assert_eq!(doc.style_name(relink.style_refs.get("fill").expect("fill style link")), "Neutral/600");
    assert!(relink.component.overrides.contains(&OverrideField::Fills));
    assert_eq!(solid_of(relink), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 }, "local style supplies raw paints over stale override cache");
    assert_eq!(solid_of(&resolve_node(&doc, relink)), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
    assert_eq!(doc.get(ext_id).unwrap().style_refs.get("fill").map(String::as_str), Some("key:EXT_FILL"));

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("fill".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert_eq!(doc.style_name(doc.get(relink_id).unwrap().style_refs.get("fill").unwrap()), "Neutral/600");
    assert_eq!(doc.get(ext_id).unwrap().style_refs.get("fill").map(String::as_str), Some("key:EXT_FILL"));
}

/// A fill override with `fillPaints` and no `styleIdForFill` clears the
/// inherited fill style link so `resolve_node` keeps the override's paints.
#[test]
fn instance_fill_override_with_no_style_clears_inherited_link() {
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "NoStyleFill",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                "guidPath": {"guids": [g(9, 2)]},
                "fillPaints": [solid(0.10, 0.80, 0.20)]
            }]}
        })
    )]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let cid = child(&doc, find(&doc, "NoStyleFill"), "Color").id;
    let c = doc.get(cid).unwrap();
    assert!(!c.style_refs.contains_key("fill"), "fillPaints without styleIdForFill detaches inherited fill style");
    assert!(c.component.overrides.contains(&OverrideField::Fills));
    assert_eq!(solid_of(&resolve_node(&doc, c)), Color { r: 0.10, g: 0.80, b: 0.20, a: 1.0 });

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("fill".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert!(!doc.get(cid).unwrap().style_refs.contains_key("fill"));
}

/// The "no style" sentinel GUID (`4294967295:4294967295`) in `styleIdForFill`
/// clears the inherited fill style link.
#[test]
fn instance_fill_override_with_sentinel_clears_inherited_link() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "SentinelFill",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"guid": g(4294967295, 4294967295)},
                    "fillPaints": [solid(0.90, 0.40, 0.10)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "SentinelFillOnly",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"guid": g(4294967295, 4294967295)}
                }]}
            })
        )
    ]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let cid = child(&doc, find(&doc, "SentinelFill"), "Color").id;
    let only_id = child(&doc, find(&doc, "SentinelFillOnly"), "Color").id;
    let c = doc.get(cid).unwrap();
    assert!(!c.style_refs.contains_key("fill"), "sentinel styleIdForFill clears inherited fill style");
    assert!(c.component.overrides.contains(&OverrideField::Fills));
    assert_eq!(solid_of(&resolve_node(&doc, c)), Color { r: 0.90, g: 0.40, b: 0.10, a: 1.0 });
    assert!(!doc.get(only_id).unwrap().style_refs.contains_key("fill"));

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("fill".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert!(!doc.get(cid).unwrap().style_refs.contains_key("fill"));
    assert!(!doc.get(only_id).unwrap().style_refs.contains_key("fill"));
}

/// Stroke twin — `styleIdForStrokeFill` naming a style relinks `style_refs["stroke"]`.
#[test]
fn instance_stroke_override_with_named_style_relinks() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "RelinkStroke",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForStrokeFill": {"guid": g(8, 2)},
                    "strokePaints": [solid(0.28, 0.33, 0.40)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "ExternalStroke",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForStrokeFill": {"assetRef": {"key": "EXT_STROKE", "version": "1"}}
                }]}
            })
        )
    ]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let relink_id = child(&doc, find(&doc, "RelinkStroke"), "Color").id;
    let ext_id = child(&doc, find(&doc, "ExternalStroke"), "Color").id;
    let relink = doc.get(relink_id).unwrap();
    assert_eq!(doc.style_name(relink.style_refs.get("stroke").expect("stroke style link")), "Neutral/600");
    assert!(relink.component.overrides.contains(&OverrideField::Stroke));
    assert_eq!(stroke_solid_of(relink), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
    assert_eq!(stroke_solid_of(&resolve_node(&doc, relink)), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
    assert_eq!(doc.get(ext_id).unwrap().style_refs.get("stroke").map(String::as_str), Some("key:EXT_STROKE"));

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("stroke".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert_eq!(doc.style_name(doc.get(relink_id).unwrap().style_refs.get("stroke").unwrap()), "Neutral/600");
    assert_eq!(doc.get(ext_id).unwrap().style_refs.get("stroke").map(String::as_str), Some("key:EXT_STROKE"));
}

/// Stroke twin — `strokePaints` with no `styleIdForStrokeFill` clears `style_refs["stroke"]`.
#[test]
fn instance_stroke_override_with_no_style_clears_inherited_link() {
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "NoStyleStroke",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                "guidPath": {"guids": [g(9, 2)]},
                "strokePaints": [solid(0.10, 0.80, 0.20)]
            }]}
        })
    )]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let cid = child(&doc, find(&doc, "NoStyleStroke"), "Color").id;
    let c = doc.get(cid).unwrap();
    assert!(!c.style_refs.contains_key("stroke"), "strokePaints without styleIdForStrokeFill detaches inherited stroke style");
    assert!(c.component.overrides.contains(&OverrideField::Stroke));
    assert_eq!(stroke_solid_of(&resolve_node(&doc, c)), Color { r: 0.10, g: 0.80, b: 0.20, a: 1.0 });

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("stroke".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert!(!doc.get(cid).unwrap().style_refs.contains_key("stroke"));
}

/// Stroke twin — the sentinel GUID (`4294967295:4294967295`) in `styleIdForStrokeFill`
/// clears `style_refs["stroke"]`.
#[test]
fn instance_stroke_override_with_sentinel_clears_inherited_link() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "SentinelStroke",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForStrokeFill": {"guid": g(4294967295, 4294967295)},
                    "strokePaints": [solid(0.90, 0.40, 0.10)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "SentinelStrokeOnly",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForStrokeFill": {"guid": g(4294967295, 4294967295)}
                }]}
            })
        )
    ]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let cid = child(&doc, find(&doc, "SentinelStroke"), "Color").id;
    let only_id = child(&doc, find(&doc, "SentinelStrokeOnly"), "Color").id;
    let c = doc.get(cid).unwrap();
    assert!(!c.style_refs.contains_key("stroke"), "sentinel styleIdForStrokeFill clears inherited stroke style");
    assert!(c.component.overrides.contains(&OverrideField::Stroke));
    assert_eq!(stroke_solid_of(&resolve_node(&doc, c)), Color { r: 0.90, g: 0.40, b: 0.10, a: 1.0 });
    assert!(!doc.get(only_id).unwrap().style_refs.contains_key("stroke"));

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("stroke".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert!(!doc.get(cid).unwrap().style_refs.contains_key("stroke"));
    assert!(!doc.get(only_id).unwrap().style_refs.contains_key("stroke"));
}

/// A copy without a fill override keeps the main component's fill style link
/// (and stroke style link when only non-paint fields like `strokeWeight` or `opacity`
/// are overridden), while a sibling with a fill override takes the override's link.
#[test]
fn instance_copy_without_fill_override_keeps_main_style_link() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "Unchanged",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "opacity": 0.75,
                    "strokeWeight": 2.0
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "Overridden",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(9, 2)]},
                    "fillPaints": [solid(0.10, 0.80, 0.20)]
                }]}
            })
        )
    ]));
    let mut doc = import_fig_node_changes(&input, "t").unwrap();
    let keep_id = child(&doc, find(&doc, "Unchanged"), "Color").id;
    let ov_id = child(&doc, find(&doc, "Overridden"), "Color").id;
    let keep = doc.get(keep_id).unwrap();
    assert_eq!(doc.style_name(keep.style_refs.get("fill").expect("kept fill style")), "Brand/600");
    assert_eq!(doc.style_name(keep.style_refs.get("stroke").expect("kept stroke style")), "Brand/600");
    assert_eq!(solid_of(&resolve_node(&doc, keep)), Color { r: 0.33, g: 0.22, b: 0.62, a: 1.0 });
    assert!(!doc.get(ov_id).unwrap().style_refs.contains_key("fill"));

    let comp_id = find(&doc, "Swatch").id;
    let master_id = child(&doc, doc.get(comp_id).unwrap(), "Color").id;
    let accent = doc.style_meta.iter().find(|(_, m)| m.name == "Accent/600").unwrap().0.clone();
    doc.get_mut(master_id).unwrap().style_refs.insert("fill".into(), accent);
    sync_instances(&mut doc, comp_id).apply(&mut doc).unwrap();
    assert_eq!(doc.style_name(doc.get(keep_id).unwrap().style_refs.get("fill").unwrap()), "Accent/600");
    assert!(!doc.get(ov_id).unwrap().style_refs.contains_key("fill"));
}

/// A self-override entry (`guidPath` equal to the instance's `symbolID` or `guid`)
/// that carries `fillPaints` without `styleIdForFill` keeps the fill style link
/// declared on the instance node itself.
#[test]
fn instance_self_fill_override_with_paints_keeps_instance_style_link() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "SelfFillBySym",
            json!({
                "styleIdForFill": {"guid": g(8, 2)},
                "fillPaints": [solid(0.32, 0.32, 0.32)],
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(5, 1)]},
                    "fillPaints": [solid(0.90, 0.90, 0.10)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "SelfFillByGuid",
            json!({
                "styleIdForFill": {"guid": g(8, 2)},
                "fillPaints": [solid(0.32, 0.32, 0.32)],
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(1, 2)]},
                    "fillPaints": [solid(0.90, 0.90, 0.10)]
                }]}
            })
        )
    ]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    for name in ["SelfFillBySym", "SelfFillByGuid"] {
        let inst = find(&doc, name);
        assert_eq!(doc.style_name(inst.style_refs.get("fill").expect("self fill style link preserved")), "Neutral/600");
        assert!(inst.component.overrides.contains(&OverrideField::Fills));
        assert_eq!(solid_of(&resolve_node(&doc, inst)), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
    }
}

/// Stroke twin — a self-override entry that carries `strokePaints` without
/// `styleIdForStrokeFill` keeps the stroke style link declared on the instance node itself.
#[test]
fn instance_self_stroke_override_with_paints_keeps_instance_style_link() {
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "SelfStrokeBySym",
            json!({
                "styleIdForStrokeFill": {"guid": g(8, 2)},
                "strokePaints": [solid(0.32, 0.32, 0.32)],
                "strokeWeight": 1.0,
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(5, 1)]},
                    "strokePaints": [solid(0.90, 0.90, 0.10)]
                }]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "SelfStrokeByGuid",
            json!({
                "styleIdForStrokeFill": {"guid": g(8, 2)},
                "strokePaints": [solid(0.32, 0.32, 0.32)],
                "strokeWeight": 1.0,
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [{
                    "guidPath": {"guids": [g(1, 2)]},
                    "strokePaints": [solid(0.90, 0.90, 0.10)]
                }]}
            })
        )
    ]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    for name in ["SelfStrokeBySym", "SelfStrokeByGuid"] {
        let inst = find(&doc, name);
        assert_eq!(doc.style_name(inst.style_refs.get("stroke").expect("self stroke style link preserved")), "Neutral/600");
        assert!(inst.component.overrides.contains(&OverrideField::Stroke));
        assert_eq!(stroke_solid_of(&resolve_node(&doc, inst)), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
    }
}

/// An override whose `fillPaints` binds `colorVar` and also names a local style
/// keeps its own bound paints and variable binding rather than replacing them with the style's paints.
#[test]
fn instance_override_with_color_var_and_named_style_keeps_bound_paints() {
    let mut bound_paint = solid(0.05, 0.58, 0.53);
    bound_paint["colorVar"] = json!({"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": g(6, 4)}}});
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "BoundOverStyle",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"guid": g(8, 2)},
                    "fillPaints": [bound_paint]
                }
            ]}
        })
    )]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let inst = find(&doc, "BoundOverStyle");
    let c = child(&doc, inst, "Color");
    assert_eq!(doc.style_name(c.style_refs.get("fill").expect("relinked fill style")), "Neutral/600");
    assert_eq!(c.variables.bindings.get("fills/0").map(String::as_str), Some("VariableID:6:4"));
    assert_eq!(solid_of(c), Color { r: 0.05, g: 0.58, b: 0.53, a: 1.0 }, "colorVar-bound override keeps its own raw paints");
}

/// An override with a named local style and stale `opacityVar` / `stopsVar` on its own
/// inline paints (without `colorVar`) skips those stale variable bindings and resolves
/// to the named style's values.
#[test]
fn instance_override_with_named_style_skips_stale_stop_and_opacity_bindings() {
    let mut stale_paint = solid(0.28, 0.33, 0.40);
    stale_paint["opacityVar"] = json!({"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"guid": g(6, 5)}}});
    stale_paint["stopsVar"] = json!([{"colorVar": {"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": g(6, 4)}}}}]);
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "StaleVarsOverride",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {
                    "guidPath": {"guids": [g(9, 2)]},
                    "styleIdForFill": {"guid": g(8, 2)},
                    "fillPaints": [stale_paint]
                }
            ]}
        })
    )]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let inst = find(&doc, "StaleVarsOverride");
    let c = child(&doc, inst, "Color");
    assert!(c.variables.bindings.is_empty(), "stale opacityVar and stopsVar must be skipped when local style wins");
    let resolved = resolve_node(&doc, c);
    assert_eq!(resolved.fills[0].opacity, 1.0);
    assert_eq!(solid_of(&resolved), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
}

/// A text override entry that sets both `styleIdForText` and `styleIdForFill`
/// merges into `styles` so the target text node keeps both `"text"` and `"fill"` style links.
#[test]
fn instance_text_override_with_text_and_fill_styles_keeps_both_links() {
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "TextBothStyles",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {
                    "guidPath": {"guids": [g(9, 3)]},
                    "styleIdForFill": {"guid": g(8, 2)},
                    "fillPaints": [solid(0.32, 0.32, 0.32)],
                    "styleIdForText": {"guid": g(8, 9)}
                }
            ]}
        })
    )]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let inst = find(&doc, "TextBothStyles");
    let label = child(&doc, inst, "Label");
    assert_eq!(doc.style_name(label.style_refs.get("fill").expect("text fill style link")), "Neutral/600");
    assert_eq!(doc.style_name(label.style_refs.get("text").expect("text typography style link")), "Heading");
}

/// A fill override on a `TEXT` node records `OverrideField::TextStyle` (not `OverrideField::Fills`).
#[test]
fn instance_text_fill_override_marks_text_style_override_field() {
    let input = paint_style_fixture(json!([node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "TextFillOnly",
        json!({
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [
                {
                    "guidPath": {"guids": [g(9, 3)]},
                    "fillPaints": [solid(0.10, 0.80, 0.20)]
                }
            ]}
        })
    )]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let inst = find(&doc, "TextFillOnly");
    let label = child(&doc, inst, "Label");
    assert!(!label.style_refs.contains_key("fill"));
    assert!(label.component.overrides.contains(&OverrideField::TextStyle));
    assert!(!label.component.overrides.contains(&OverrideField::Fills), "text fill override must not insert OverrideField::Fills");
}

/// Multiple `symbolOverrides` entries targeting the same `guidPath` apply in list order:
/// a later paints-only entry clears an earlier named style link, a later named style entry
/// relinks over an earlier paints-only entry, and a later non-paint entry preserves the link.
#[test]
fn instance_same_path_multi_entry_overrides_apply_in_order() {
    let named = json!({"guidPath": {"guids": [g(9, 2)]}, "styleIdForFill": {"guid": g(8, 2)}, "fillPaints": [solid(0.32, 0.32, 0.32)]});
    let paints_only = json!({"guidPath": {"guids": [g(9, 2)]}, "fillPaints": [solid(0.10, 0.80, 0.20)]});
    let opacity_only = json!({"guidPath": {"guids": [g(9, 2)]}, "opacity": 0.5});
    let input = paint_style_fixture(json!([
        node(
            (1, 1),
            (0, 1),
            "a",
            "INSTANCE",
            "NamedThenPaints",
            json!({
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [named.clone(), paints_only.clone()]}
            })
        ),
        node(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "PaintsThenNamed",
            json!({"symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [paints_only, named.clone()]}})
        ),
        node(
            (1, 3),
            (0, 1),
            "c",
            "INSTANCE",
            "NamedThenOpacity",
            json!({"symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": [named, opacity_only]}})
        )
    ]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let ntp_inst = find(&doc, "NamedThenPaints");
    let ntp = child(&doc, ntp_inst, "Color");
    assert!(!ntp.style_refs.contains_key("fill"), "later paints-only entry clears earlier named style link");
    assert_eq!(solid_of(&resolve_node(&doc, ntp)), Color { r: 0.10, g: 0.80, b: 0.20, a: 1.0 });

    let ptn = child(&doc, find(&doc, "PaintsThenNamed"), "Color");
    assert_eq!(doc.style_name(ptn.style_refs.get("fill").expect("relinked by later entry")), "Neutral/600");
    assert_eq!(solid_of(&resolve_node(&doc, ptn)), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });

    let nto = child(&doc, find(&doc, "NamedThenOpacity"), "Color");
    assert_eq!(doc.style_name(nto.style_refs.get("fill").expect("kept across opacity entry")), "Neutral/600");
    assert_eq!(nto.opacity, 0.5);
}

/// A plain (non-instance) node that links a local fill style skips stale inline
/// `opacityVar` and `stopsVar` bindings when `colorVar` is not bound.
#[test]
fn plain_node_with_local_fill_style_skips_stale_paint_variable_bindings() {
    let mut stale_paint = solid(0.28, 0.33, 0.40);
    stale_paint["opacityVar"] = json!({"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"guid": g(6, 5)}}});
    stale_paint["stopsVar"] = json!([{"colorVar": {"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": g(6, 4)}}}}]);
    let input = paint_style_fixture(json!([node(
        (2, 1),
        (0, 1),
        "p",
        "RECTANGLE",
        "PlainStyled",
        json!({
            "styleIdForFill": {"guid": g(8, 2)},
            "fillPaints": [stale_paint]
        })
    )]));
    let doc = import_fig_node_changes(&input, "t").unwrap();
    let plain = find(&doc, "PlainStyled");
    assert!(plain.variables.bindings.is_empty(), "plain node with local fill style skips stale inline paint bindings");
    let resolved = resolve_node(&doc, plain);
    assert_eq!(resolved.fills[0].opacity, 1.0);
    assert_eq!(solid_of(&resolved), Color { r: 0.32, g: 0.32, b: 0.32, a: 1.0 });
}

fn sname(doc: &Document, n: &Node, slot: &str) -> String {
    n.style_refs.get(slot).map(|s| doc.style_name(s).to_string()).unwrap_or_else(|| "<none>".into())
}

/// Main "Card" names Brand for fill and stroke; instances name the same styles on their own nodeChange (as the kit's buttons do).
fn self_entry_fixture(instances: Vec<Value>) -> Value {
    let style = |id: u64, pos: &str, name: &str, r: f64, gr: f64, b: f64| {
        node((8, id), (0, 2), pos, "RECTANGLE", name, json!({"styleType": "FILL", "fillPaints": [solid(r, gr, b)]}))
    };
    let mut arr = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        style(1, "s", "Brand", 0.5, 0.3, 0.8),
        style(2, "t", "Neutral", 0.3, 0.3, 0.3),
        node(
            (5, 1),
            (0, 2),
            "c",
            "SYMBOL",
            "Card",
            json!({
                "size": {"x": 80.0, "y": 40.0},
                "styleIdForFill": {"guid": g(8, 1)}, "fillPaints": [solid(0.5, 0.3, 0.8)],
                "styleIdForStrokeFill": {"guid": g(8, 1)}, "strokePaints": [solid(0.5, 0.3, 0.8)], "strokeWeight": 1.0
            }),
        ),
        node((5, 2), (5, 1), "a", "RECTANGLE", "Body", json!({"overrideKey": g(9, 2), "size": {"x": 80.0, "y": 40.0}})),
    ];
    arr.extend(instances);
    Value::Array(arr)
}

fn self_entry_inst(id: u64, pos: &str, name: &str, ovs: Value) -> Value {
    node(
        (1, id),
        (0, 1),
        pos,
        "INSTANCE",
        name,
        json!({
            "size": {"x": 80.0, "y": 40.0},
            "styleIdForFill": {"guid": g(8, 1)}, "fillPaints": [solid(0.5, 0.3, 0.8)],
            "styleIdForStrokeFill": {"guid": g(8, 1)}, "strokePaints": [solid(0.5, 0.3, 0.8)], "strokeWeight": 1.0,
            "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 1.0, "symbolOverrides": ovs}
        }),
    )
}

#[test]
fn self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main() {
    let mut doc = import_fig_node_changes(
        &self_entry_fixture(vec![self_entry_inst(1, "a", "SelfOpacityOnly", json!([{"guidPath": {"guids": [g(5, 1)]}, "opacity": 0.5}]))]),
        "t",
    )
    .unwrap();
    let n = find(&doc, "SelfOpacityOnly");
    assert!(!n.component.overrides.contains(&OverrideField::Fills), "an opacity-only self entry overrides no fill: {:?}", n.component.overrides);
    assert!(!n.component.overrides.contains(&OverrideField::Stroke), "an opacity-only self entry overrides no stroke: {:?}", n.component.overrides);
    let card = find(&doc, "Card").id;
    let neutral = doc.style_meta.iter().find(|(_, m)| m.name == "Neutral").unwrap().0.clone();
    {
        let m = doc.get_mut(card).unwrap();
        m.style_refs.insert("fill".into(), neutral.clone());
        m.style_refs.insert("stroke".into(), neutral.clone());
        m.fills = vec![Paint::solid(Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 })];
        m.stroke.paints = vec![Paint::solid(Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 })];
    }
    sync_instances(&mut doc, card).apply(&mut doc).unwrap();
    let n = find(&doc, "SelfOpacityOnly");
    assert_eq!(sname(&doc, n, "fill"), "Neutral", "the copy follows its main's fill style");
    assert_eq!(sname(&doc, n, "stroke"), "Neutral", "the copy follows its main's stroke style");
}

#[test]
fn self_entry_with_paints_only_for_fill_adds_no_stroke_override() {
    let doc = import_fig_node_changes(
        &self_entry_fixture(vec![self_entry_inst(
            4,
            "d",
            "SelfStalePaints",
            json!([{"guidPath": {"guids": [g(5, 1)]}, "fillPaints": [solid(0.9, 0.9, 0.1)]}]),
        )]),
        "t",
    )
    .unwrap();
    let n = find(&doc, "SelfStalePaints");
    assert_eq!(sname(&doc, n, "fill"), "Brand");
    assert!(n.component.overrides.contains(&OverrideField::Fills));
    assert!(!n.component.overrides.contains(&OverrideField::Stroke), "{:?}", n.component.overrides);
}

#[test]
fn self_entry_that_names_its_own_style_keeps_that_style() {
    let sentinel = g(u32::MAX as u64, u32::MAX as u64);
    let doc = import_fig_node_changes(
        &self_entry_fixture(vec![
            self_entry_inst(
                3,
                "c",
                "SelfOwnKey",
                json!([{"guidPath": {"guids": [g(5, 1)]}, "styleIdForFill": {"guid": g(8, 2)}, "fillPaints": [solid(0.3, 0.3, 0.3)]}]),
            ),
            self_entry_inst(
                5,
                "e",
                "SelfSentinel",
                json!([{"guidPath": {"guids": [g(5, 1)]}, "styleIdForFill": {"guid": sentinel}, "fillPaints": [solid(0.3, 0.3, 0.3)]}]),
            ),
        ]),
        "t",
    )
    .unwrap();
    let n = find(&doc, "SelfOwnKey");
    assert_eq!(sname(&doc, n, "fill"), "Neutral", "the self entry's own key wins over the instance's");
    let detached = find(&doc, "SelfSentinel");
    assert_eq!(sname(&doc, detached, "fill"), "<none>", "the self entry's sentinel detach wins over the instance's");
}
