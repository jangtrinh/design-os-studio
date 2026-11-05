//! A `.fig` save keeps an instance text override as the file records it: an override of the text style alone does
//! not turn into an override of the characters.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn note_fixture() -> Value {
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node((10, 1), (0, 1), "SYMBOL", "Note", json!({})),
        fig_node(
            (10, 2),
            (10, 1),
            "TEXT",
            "Body",
            json!({"fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular", "postscript": ""}, "textData": {"characters": "Hello world"}}),
        ),
        fig_node((20, 1), (0, 1), "INSTANCE", "NoteInst", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": []}})),
    ]);
    Value::Array(nodes)
}

fn body_text<'a>(doc: &'a Document, holder: &str) -> (&'a Node, &'a TextProps) {
    let body = kid(doc, by_name(doc, holder), "Body");
    match &body.kind {
        NodeKind::Text(tp) => (body, tp),
        other => panic!("Body is not a text layer: {other:?}"),
    }
}

#[test]
fn style_only_text_override_does_not_pin_characters() {
    let mut original = import_fig_node_changes(&note_fixture(), "StyleOnly").unwrap();
    let body_id = body_text(&original, "NoteInst").0.id;
    {
        let n = original.get_mut(body_id).unwrap();
        if let NodeKind::Text(tp) = &mut n.kind {
            let mut larger = tp.style.clone();
            larger.font_size = 20.0;
            tp.runs = vec![TextRun { start: 0, end: 5, style: larger }];
        }
        n.component.overrides.insert(OverrideField::TextStyle);
    }

    let mut reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
    let (body, tp) = body_text(&reopened, "NoteInst");
    assert!(body.component.overrides.contains(&OverrideField::TextStyle), "the style override is kept: {:?}", body.component.overrides);
    assert!(!body.component.overrides.contains(&OverrideField::Characters), "a style-only override must not pin the characters");
    assert_eq!(tp.runs.len(), 1, "the styled run is kept");

    // A later change of the text on the main component still reaches the instance.
    let main = by_name(&reopened, "Note").id;
    let main_body = kid(&reopened, by_name(&reopened, "Note"), "Body").id;
    if let NodeKind::Text(tp) = &mut reopened.get_mut(main_body).unwrap().kind {
        tp.content = "Changed in main".into();
    }
    sync_instances(&mut reopened, main).apply(&mut reopened).unwrap();
    assert_eq!(body_text(&reopened, "NoteInst").1.content, "Changed in main", "the instance follows the main's text");
}
