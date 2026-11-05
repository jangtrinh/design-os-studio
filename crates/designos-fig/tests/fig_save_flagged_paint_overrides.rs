//! An instance paint override the file records is still recorded after a `.fig` save, even when the paint equals
//! the one its main component paints, and a swapped nested instance writes no override it only inherits.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn flagged_equal_fixture(bound: bool) -> Value {
    let v = if bound { Some((1, 7)) } else { None };
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set((1, 5), "Colors", (1, 6), json!({})),
        color_var((1, 7), (1, 5), (1, 6), "Bg/Primary", &white(), json!({})),
        fig_node((10, 1), (0, 1), "SYMBOL", "Card", json!({})),
        fig_node((10, 2), (10, 1), "RECTANGLE", "Box", json!({"fillPaints": [solid(&white(), v)]})),
        fig_node(
            (20, 1),
            (0, 1),
            "INSTANCE",
            "CardInst",
            json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": [{"guidPath": {"guids": [guid(10, 2)]}, "fillPaints": [solid(&white(), v)]}]}}),
        ),
        fig_node((20, 2), (0, 1), "INSTANCE", "CardPlain", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": []}})),
    ]);
    Value::Array(nodes)
}

fn stroke_fixture() -> Value {
    let stroked = || json!({"strokePaints": [solid(&white(), None)], "strokeWeight": 1.0, "strokeAlign": "INSIDE"});
    let mut main_box = stroked();
    main_box["fillPaints"] = json!([solid(&blue(), None)]);
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node((10, 1), (0, 1), "SYMBOL", "Card", json!({})),
        fig_node((10, 2), (10, 1), "RECTANGLE", "Box", main_box),
        fig_node(
            (20, 1),
            (0, 1),
            "INSTANCE",
            "CardInst",
            json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": [{"guidPath": {"guids": [guid(10, 2)]},
                "strokePaints": [solid(&white(), None)], "strokeWeight": 1.0}]}}),
        ),
        fig_node((20, 2), (0, 1), "INSTANCE", "CardPlain", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": []}})),
    ]);
    Value::Array(nodes)
}

/// Repaints the main's box, syncs its instances, and returns the resolved `(instance, plain instance)` fills.
fn fills_after_main_fill_edit(mut doc: Document) -> (String, String) {
    let main = by_name(&doc, "Card").id;
    let main_box = kid(&doc, by_name(&doc, "Card"), "Box").id;
    {
        let b = doc.get_mut(main_box).unwrap();
        b.fills = vec![Paint::solid(Color::from_hex("#22C55E").unwrap())];
        b.variables.bindings.clear();
    }
    sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    let own = kid(&doc, by_name(&doc, "CardInst"), "Box").id;
    let plain = kid(&doc, by_name(&doc, "CardPlain"), "Box").id;
    (resolved_fill(&doc, own), resolved_fill(&doc, plain))
}

fn strokes_after_main_stroke_edit(mut doc: Document) -> (String, String) {
    let main = by_name(&doc, "Card").id;
    let main_box = kid(&doc, by_name(&doc, "Card"), "Box").id;
    doc.get_mut(main_box).unwrap().stroke.paints = vec![Paint::solid(Color::from_hex("#22C55E").unwrap())];
    sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    let own = kid(&doc, by_name(&doc, "CardInst"), "Box").id;
    let plain = kid(&doc, by_name(&doc, "CardPlain"), "Box").id;
    (resolved_stroke(&doc, own), resolved_stroke(&doc, plain))
}

#[test]
fn flagged_fill_override_equal_to_main_survives_fig_round_trip() {
    for bound in [false, true] {
        let form = if bound { "variable-bound" } else { "literal" };
        let original = import_fig_node_changes(&flagged_equal_fixture(bound), "FlaggedEqual").unwrap();
        let own = kid(&original, by_name(&original, "CardInst"), "Box");
        assert!(own.component.overrides.contains(&OverrideField::Fills), "{form}: the import flags the override");

        let reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
        let own = kid(&reopened, by_name(&reopened, "CardInst"), "Box");
        assert!(own.component.overrides.contains(&OverrideField::Fills), "{form}: the Fills flag survives the save");
        let plain = kid(&reopened, by_name(&reopened, "CardPlain"), "Box");
        assert!(!plain.component.overrides.contains(&OverrideField::Fills), "{form}: an instance without the record gains none");

        let (own_after, plain_after) = fills_after_main_fill_edit(reopened);
        assert_eq!(own_after, "#FFFFFF", "{form}: the instance keeps its own paint when the main is repainted");
        assert_eq!(plain_after, "#22C55E", "{form}: an instance without the record follows the main");
        assert_eq!(fills_after_main_fill_edit(original), (own_after, plain_after), "{form}: same behaviour before and after the save");
    }
}

#[test]
fn flagged_stroke_override_equal_to_main_survives_fig_round_trip() {
    let original = import_fig_node_changes(&stroke_fixture(), "StrokeEqual").unwrap();
    let own = kid(&original, by_name(&original, "CardInst"), "Box");
    assert!(own.component.overrides.contains(&OverrideField::Stroke), "the import flags the override");

    let reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
    let own = kid(&reopened, by_name(&reopened, "CardInst"), "Box");
    assert!(own.component.overrides.contains(&OverrideField::Stroke), "the Stroke flag survives the save");
    let plain = kid(&reopened, by_name(&reopened, "CardPlain"), "Box");
    assert!(!plain.component.overrides.contains(&OverrideField::Stroke), "an instance without the record gains none");

    let (own_after, plain_after) = strokes_after_main_stroke_edit(reopened);
    assert_eq!(own_after, "#FFFFFF", "the instance keeps its own stroke when the main's stroke changes");
    assert_eq!(plain_after, "#22C55E", "an instance without the record follows the main");
    assert_eq!(strokes_after_main_stroke_edit(original), (own_after, plain_after), "same behaviour before and after the save");
}

fn swap_fixture(shell_override: Value, child_override: Option<Value>) -> Value {
    let gray = rgba(0.4, 0.45, 0.52);
    let red = rgba(0.9, 0.2, 0.2);
    let mut shell = json!({"guidPath": {"guids": [guid(10, 3)]}, "overriddenSymbolID": guid(11, 2)});
    for (k, x) in shell_override.as_object().unwrap() {
        shell[k] = x.clone();
    }
    let mut overrides = vec![shell];
    if let Some(c) = child_override {
        let mut entry = json!({"guidPath": {"guids": [guid(10, 3), guid(11, 4)]}});
        for (k, x) in c.as_object().unwrap() {
            entry[k] = x.clone();
        }
        overrides.push(entry);
    }
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set((1, 5), "Colors", (1, 6), json!({})),
        color_var((1, 7), (1, 5), (1, 6), "Bg/Primary", &white(), json!({})),
        color_var((1, 8), (1, 5), (1, 6), "Fg/Quaternary", &gray, json!({})),
        color_var((1, 9), (1, 5), (1, 6), "Bg/Danger", &red, json!({})),
        fig_node((11, 1), (0, 1), "SYMBOL", "Badge=Neutral", json!({"fillPaints": [solid(&white(), Some((1, 7)))]})),
        fig_node((11, 3), (11, 1), "RECTANGLE", "Dot", json!({"fillPaints": [solid(&white(), Some((1, 7)))]})),
        fig_node((11, 2), (0, 1), "SYMBOL", "Badge=Danger", json!({"fillPaints": [solid(&red, Some((1, 9)))]})),
        fig_node((11, 4), (11, 2), "RECTANGLE", "Dot", json!({"fillPaints": [solid(&red, Some((1, 9)))]})),
        fig_node((10, 1), (0, 1), "SYMBOL", "Chip", json!({})),
        fig_node(
            (10, 3),
            (10, 1),
            "INSTANCE",
            "NestedBadge",
            json!({"symbolData": {"symbolID": guid(11, 1), "symbolOverrides": []}, "fillPaints": [solid(&white(), Some((1, 7)))]}),
        ),
        fig_node((20, 3), (0, 1), "INSTANCE", "SwapChip", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": overrides}})),
    ]);
    Value::Array(nodes)
}

/// What the swapped nested badge and its dot look like, as a comparable line.
fn swapped_state(doc: &Document) -> String {
    let nested = kid(doc, by_name(doc, "SwapChip"), "NestedBadge");
    let comp = match &nested.kind {
        NodeKind::Instance { component: Some(c), .. } => doc.get(*c).map(|n| n.name.clone()).unwrap_or_default(),
        _ => "-".into(),
    };
    let dot = kid(doc, nested, "Dot");
    format!(
        "comp={comp} shell_fill={} shell_bind={:?} shell_flags={:?} dot_fill={} dot_bind={:?} dot_flags={:?}",
        resolved_fill(doc, nested.id),
        nested.variables.bindings.get("fills/0"),
        nested.component.overrides,
        resolved_fill(doc, dot.id),
        dot.variables.bindings.get("fills/0"),
        dot.component.overrides,
    )
}

fn swap_before_and_after_save(shell: Value, child: Option<Value>) -> (String, String) {
    let original = import_fig_node_changes(&swap_fixture(shell, child), "Swap").unwrap();
    let reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
    (swapped_state(&original), swapped_state(&reopened))
}

#[test]
fn swapped_nested_instance_paints_are_unchanged_by_a_fig_round_trip() {
    let (before, after) = swap_before_and_after_save(json!({"fillPaints": [solid(&blue(), None)]}), None);
    assert!(before.contains(&format!("shell_fill={BLUE_HEX}")), "literal shell override is imported: {before}");
    assert_eq!(before, after, "literal shell override");

    let (before, after) = swap_before_and_after_save(json!({"fillPaints": [solid(&rgba(0.4, 0.45, 0.52), Some((1, 8)))]}), None);
    assert_eq!(before, after, "re-bound shell override");

    let (before, after) = swap_before_and_after_save(json!({}), Some(json!({"fillPaints": [solid(&blue(), None)]})));
    assert!(before.contains(&format!("dot_fill={BLUE_HEX}")), "literal child override is imported: {before}");
    assert_eq!(before, after, "literal child override");

    let (before, after) = swap_before_and_after_save(json!({}), None);
    assert_eq!(before, after, "a plain swap writes no paint override");
}
