//! An underline a user puts on a linked instance text keeps drawing after a `.fig` save and reopen, while the stale
//! layer-level underline of an imported copy that links a plain text style stays out.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn fixture(entry: Value) -> Value {
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node((0, 2), (0, 0), "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        fig_node(
            (9, 1),
            (0, 2),
            "TEXT",
            "Body style",
            json!({"styleType": "TEXT", "key": "key-body", "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular", "postscript": ""}}),
        ),
        fig_node((10, 1), (0, 1), "SYMBOL", "Note", json!({})),
        fig_node(
            (10, 2),
            (10, 1),
            "TEXT",
            "Body",
            json!({
                "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular", "postscript": ""},
                "styleIdForText": {"guid": guid(9, 1)}, "textData": {"characters": "Hello world"}
            }),
        ),
        fig_node((20, 1), (0, 1), "INSTANCE", "NoteInst", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": [entry]}})),
    ]);
    Value::Array(nodes)
}

/// The decoration each character of the copy is drawn with (the content is ASCII).
fn drawn(doc: &Document) -> Vec<TextDecoration> {
    let t = kid(doc, by_name(doc, "NoteInst"), "Body").kind.any_text().expect("text");
    (0..t.content.len()).map(|i| t.runs.iter().find(|r| r.start <= i && i < r.end).map(|r| r.style.decoration).unwrap_or(t.style.decoration)).collect()
}

fn copy_style(doc: &Document) -> TextStyle {
    let body = kid(doc, by_name(doc, "NoteInst"), "Body");
    body.kind.any_text().expect("text").style.clone()
}

#[test]
fn a_user_underline_on_a_linked_copy_survives_save_and_reopen() {
    let mut doc = import_fig_node_changes(&fixture(json!({"guidPath": {"guids": [guid(10, 2)]}})), "Linked").unwrap();
    let body = kid(&doc, by_name(&doc, "NoteInst"), "Body").id;
    assert_eq!(copy_style(&doc).decoration, TextDecoration::None, "the style has no decoration");
    {
        let n = doc.get_mut(body).unwrap();
        n.kind.any_text_mut().unwrap().style.decoration = TextDecoration::Underline;
        n.text_style_overrides.insert(OwnedField::Decoration);
        n.component.overrides.insert(OverrideField::TextStyle);
    }
    let once = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(drawn(&once), vec![TextDecoration::Underline; 11], "the underline is kept");
    assert_eq!(kid(&once, by_name(&once, "NoteInst"), "Body").kind.any_text().unwrap().content, "Hello world");
    // the save writes the copy's own record with a flagged run row, so the user's override is still an override
    assert!(text_style_overrides(&once, kid(&once, by_name(&once, "NoteInst"), "Body")).labels().contains(&"Decoration"));
    let twice = import_fig(&export_fig(&once).unwrap()).unwrap();
    assert_eq!(drawn(&twice), vec![TextDecoration::Underline; 11], "and kept again");
}

#[test]
fn a_stale_layer_underline_next_to_a_plain_link_stays_out_after_a_save() {
    let entry = json!({
        "guidPath": {"guids": [guid(10, 2)]}, "styleIdForText": {"guid": guid(9, 1)}, "textDecoration": "UNDERLINE", "hyperlink": {},
        "textData": {"characters": "Hello"}
    });
    let doc = import_fig_node_changes(&fixture(entry), "Stale").unwrap();
    assert_eq!(copy_style(&doc).decoration, TextDecoration::None);
    let reopened = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(copy_style(&reopened).decoration, TextDecoration::None, "a save does not turn the stale field into a drawn underline");
}

#[test]
fn an_unedited_figma_copy_with_a_stale_layer_underline_saves_and_reopens_without_one() {
    // The shape of the kit's copies whose link comes from the main: a layer-level UNDERLINE, no link of its own, no runs.
    let entry = json!({"guidPath": {"guids": [guid(10, 2)]}, "textDecoration": "UNDERLINE"});
    let doc = import_fig_node_changes(&fixture(entry), "Unedited").unwrap();
    assert_eq!(drawn(&doc), vec![TextDecoration::None; 11]);
    let once = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(drawn(&once), vec![TextDecoration::None; 11], "a no-edit save keeps it plain");
    let twice = import_fig(&export_fig(&once).unwrap()).unwrap();
    assert_eq!(drawn(&twice), vec![TextDecoration::None; 11]);
}

#[test]
fn a_user_underline_on_a_copy_that_also_overrides_its_characters_survives_too() {
    let mut doc = import_fig_node_changes(&fixture(json!({"guidPath": {"guids": [guid(10, 2)]}, "textData": {"characters": "Hello"}})), "Chars").unwrap();
    let body = kid(&doc, by_name(&doc, "NoteInst"), "Body").id;
    {
        let n = doc.get_mut(body).unwrap();
        n.kind.any_text_mut().unwrap().style.decoration = TextDecoration::Underline;
        n.text_style_overrides.insert(OwnedField::Decoration);
        n.component.overrides.insert(OverrideField::TextStyle);
    }
    let once = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(drawn(&once), vec![TextDecoration::Underline; 5]);
}
