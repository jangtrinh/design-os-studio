//! Which decoration a text layer draws when a text style link and a layer-level `textDecoration` disagree.
//!
//! Figma decides per character. A layer linked to a text style takes the style's decoration (none when the style has
//! none) and ignores its own `textDecoration` field; the field only counts while the layer has no link. A style
//! run takes its own `textDecoration`, else the decoration of the style it is linked to, else the layer's.
//!
//! The Untitled UI kit holds hundreds of instance copies whose override entry carries a stale layer-level underline
//! left behind by an earlier link, next to a link to a style without one. Drawing that underline had them refuse
//! Figma's stored glyphs, which hold no bar for them.
use designos_doc::*;
use designos_fig::import_fig_node_changes_with_report;
use serde_json::{json, Value};

fn g(s: u32, l: u32) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(guid: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut m = json!({
        "guid": g(guid.0, guid.1),
        "phase": "CREATED",
        "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty,
        "name": name,
        "visible": true,
        "opacity": 1.0,
        "size": {"x": 100.0, "y": 20.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        m[k] = v.clone();
    }
    m
}

const PLAIN_STYLE: (u32, u32) = (9, 1);
const UNDERLINED_STYLE: (u32, u32) = (9, 2);

fn font() -> Value {
    json!({"family": "Inter", "style": "Regular"})
}

fn link(style: (u32, u32)) -> Value {
    json!({"guid": g(style.0, style.1)})
}

fn detach() -> Value {
    json!({"guid": g(u32::MAX, u32::MAX)})
}

fn text_style(guid: (u32, u32), name: &str, extra: Value) -> Value {
    let mut fields = json!({"styleType": "TEXT", "key": format!("key-{}", guid.1), "fontSize": 16.0, "fontName": font()});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    node(guid, (0, 2), &format!("s{}", guid.1), "TEXT", name, fields)
}

fn page_and_styles() -> Vec<Value> {
    vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        text_style(PLAIN_STYLE, "Text sm/Regular", json!({})),
        text_style(UNDERLINED_STYLE, "Link", json!({"textDecoration": "UNDERLINE"})),
    ]
}

/// A component `Label k` whose text `Text k` has the fields in `main`, and one instance `Inst k` whose override
/// entry for that text is `entry`.
fn copy_case(k: u32, main: Value, entry: Value) -> Vec<Value> {
    let symbol = (1, 100 + k);
    let text = (1, 200 + k);
    let mut main_fields = json!({"fontSize": 16.0, "fontName": font(), "textData": {"characters": "Hello world"}});
    for (key, v) in main.as_object().unwrap() {
        main_fields[key] = v.clone();
    }
    let mut e = json!({"guidPath": {"guids": [g(text.0, text.1)]}});
    for (key, v) in entry.as_object().unwrap() {
        e[key] = v.clone();
    }
    vec![
        node(symbol, (0, 1), &format!("c{k}"), "SYMBOL", &format!("Label {k}"), json!({})),
        node(text, symbol, "a", "TEXT", &format!("Text {k}"), main_fields),
        node(
            (3, k),
            (0, 1),
            &format!("d{k}"),
            "INSTANCE",
            &format!("Inst {k}"),
            json!({"symbolData": {"symbolID": g(symbol.0, symbol.1), "symbolOverrides": [e]}}),
        ),
    ]
}

struct Imported {
    doc: Document,
}

fn import(cases: Vec<Vec<Value>>) -> Imported {
    let mut all = page_and_styles();
    for c in cases {
        all.extend(c);
    }
    let (doc, _) = import_fig_node_changes_with_report(&Value::Array(all), "decoration").expect("import");
    Imported { doc }
}

fn copy_text(doc: &Document, k: u32) -> &TextProps {
    let inst = doc.nodes.values().find(|n| n.name == format!("Inst {k}")).expect("instance");
    let child = *inst.children.first().expect("instance has its copy");
    doc.get(child).and_then(|n| n.kind.any_text()).expect("the copy is a text")
}

fn main_text<'a>(doc: &'a Document, name: &str) -> &'a TextProps {
    doc.nodes.values().find(|n| n.name == name && n.component.source.is_none()).and_then(|n| n.kind.any_text()).unwrap_or_else(|| panic!("text {name}"))
}

/// The decoration each character is drawn with (the content is ASCII).
fn drawn(t: &TextProps) -> Vec<TextDecoration> {
    (0..t.content.len()).map(|i| t.runs.iter().find(|r| r.start <= i && i < r.end).map(|r| r.style.decoration).unwrap_or(t.style.decoration)).collect()
}

use TextDecoration::{None as No, Underline as Ul};

#[test]
fn an_override_linking_a_plain_style_ignores_its_stale_layer_underline() {
    let im = import(vec![copy_case(
        1,
        json!({"styleIdForText": link(PLAIN_STYLE), "textData": {"characters": "Hello world, this is long"}}),
        json!({
            "styleIdForText": link(PLAIN_STYLE), "textDecoration": "UNDERLINE", "hyperlink": {},
            "textData": {"characters": "@phoenix.baker"}
        }),
    )]);
    let t = copy_text(&im.doc, 1);
    assert_eq!(t.content, "@phoenix.baker");
    assert_eq!(drawn(t), vec![No; 14]);
}

#[test]
fn a_link_inherited_from_the_main_also_beats_the_overrides_layer_underline() {
    let im = import(vec![
        // No runs: the entry only restates the layer field.
        copy_case(2, json!({"styleIdForText": link(PLAIN_STYLE)}), json!({"textDecoration": "UNDERLINE"})),
        // Runs: only the run that states its own underline draws one, the rest of the text stays plain.
        copy_case(
            3,
            json!({"styleIdForText": link(PLAIN_STYLE)}),
            json!({
                "textDecoration": "UNDERLINE",
                "textData": {
                    "characters": "Hello world",
                    "characterStyleIDs": [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    "styleOverrideTable": [{"styleID": 5, "textDecoration": "UNDERLINE", "styleIdForText": detach()}]
                }
            }),
        ),
    ]);
    assert_eq!(drawn(copy_text(&im.doc, 2)), vec![No; 11]);
    assert_eq!(copy_text(&im.doc, 2).style.decoration, No);
    let runs = drawn(copy_text(&im.doc, 3));
    assert_eq!(&runs[..2], &[Ul, Ul]);
    assert_eq!(&runs[2..], &[No; 9]);
    assert_eq!(copy_text(&im.doc, 3).style.decoration, No, "no id-0 character is underlined");
}

#[test]
fn a_layer_with_no_link_anywhere_draws_the_layer_underline() {
    let im = import(vec![copy_case(4, json!({}), json!({"textDecoration": "UNDERLINE"}))]);
    assert_eq!(drawn(copy_text(&im.doc, 4)), vec![Ul; 11]);
}

#[test]
fn a_detached_override_falls_back_to_the_layer_underline() {
    let im = import(vec![copy_case(5, json!({"styleIdForText": link(PLAIN_STYLE)}), json!({"styleIdForText": detach(), "textDecoration": "UNDERLINE"}))]);
    assert_eq!(drawn(copy_text(&im.doc, 5)), vec![Ul; 11]);
}

#[test]
fn a_linked_underlined_style_draws_its_underline_whatever_the_layer_field_says() {
    let im =
        import(vec![copy_case(6, json!({"styleIdForText": link(PLAIN_STYLE)}), json!({"styleIdForText": link(UNDERLINED_STYLE), "textDecoration": "NONE"}))]);
    assert_eq!(drawn(copy_text(&im.doc, 6)), vec![Ul; 11]);
}

#[test]
fn a_run_with_its_own_underline_draws_it_over_a_plain_linked_base() {
    // Every character in one run that states its own underline, the layer links a plain style.
    let im = import(vec![copy_case(
        7,
        json!({"styleIdForText": link(PLAIN_STYLE), "textDecoration": "UNDERLINE"}),
        json!({
            "textData": {
                "characters": "Hello world",
                "characterStyleIDs": [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
                "styleOverrideTable": [{"styleID": 4, "textDecoration": "UNDERLINE", "styleIdForText": link(PLAIN_STYLE)}]
            }
        }),
    )]);
    let t = copy_text(&im.doc, 7);
    assert_eq!(drawn(t), vec![Ul; 11]);
    assert_eq!(t.style.decoration, No);
}

#[test]
fn an_own_link_that_names_a_style_missing_from_the_file_keeps_the_layer_field() {
    let im =
        import(vec![copy_case(8, json!({}), json!({"styleIdForText": {"assetRef": {"key": "library-style-not-in-the-file"}}, "textDecoration": "UNDERLINE"}))]);
    assert_eq!(drawn(copy_text(&im.doc, 8)), vec![Ul; 11]);
}

#[test]
fn a_run_linked_to_a_plain_style_is_not_underlined_by_an_unlinked_underlined_base() {
    let main = |name: &str, run: Value| {
        node(
            (1, 300 + name.len() as u32),
            (0, 1),
            name,
            "TEXT",
            name,
            json!({
                "fontSize": 16.0, "fontName": font(), "textDecoration": "UNDERLINE",
                "textData": {"characters": "Hello", "characterStyleIDs": [1, 1, 0, 0, 0], "styleOverrideTable": [run]}
            }),
        )
    };
    let mut all = page_and_styles();
    all.push(main("linked-run", json!({"styleID": 1, "styleIdForText": link(PLAIN_STYLE)})));
    all.push(main("own-run", json!({"styleID": 1, "styleIdForText": link(PLAIN_STYLE), "textDecoration": "UNDERLINE"})));
    all.push(main("plain-run", json!({"styleID": 1, "fontSize": 20.0})));
    let (doc, _) = import_fig_node_changes_with_report(&Value::Array(all), "runs").expect("import");
    assert_eq!(drawn(main_text(&doc, "linked-run")), vec![No, No, Ul, Ul, Ul], "the run takes its style's decoration, which is none");
    assert_eq!(drawn(main_text(&doc, "own-run")), vec![Ul; 5], "the run's own decoration wins");
    assert_eq!(drawn(main_text(&doc, "plain-run")), vec![Ul; 5], "a run with no link and no decoration follows the base");
}
