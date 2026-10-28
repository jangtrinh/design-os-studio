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
