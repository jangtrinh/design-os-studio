//! An instance that links an effect style keeps that style when its own root `symbolOverrides` entry
//! repeats an older effect list, and keeps it across a `.fig` save and reopen.
//!
//! Figma writes the instance's effect list into the root entry (`guidPath = [symbolID]`) even when the
//! style link itself was not overridden. In the Untitled UI PRO v8.0 kit that copy is older than the
//! style on hundreds of instances, and the master of the same component carries the link with an empty
//! effect list while the file renders the style's shadows. The linked style therefore wins; only an
//! entry that names its own style, or an instance without a link, supplies its own effects.
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

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
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    if let (Some(dst), Some(src)) = (m.as_object_mut(), extra.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    m
}

fn shadow(radius: f64) -> Value {
    json!({"type": "DROP_SHADOW", "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.25}, "offset": {"x": 0.0, "y": 2.0}, "radius": radius, "spread": 0.0, "visible": true})
}

fn effect_style(local: u32, name: &str, radius: f64) -> Value {
    node(
        (9, local),
        (0, 2),
        &format!("s{local}"),
        "RECTANGLE",
        name,
        json!({"styleType": "EFFECT", "key": format!("key-{local}"), "size": {"x": 1.0, "y": 1.0}, "effects": [shadow(radius)]}),
    )
}

/// A placed `Badge` (main 44x22). `own` holds the instance's own fields, `entry` its root override entry.
fn badge(local: u32, name: &str, own: Value, entry: Value) -> Value {
    let mut o = json!({"guidPath": {"guids": [g(1, 10)]}});
    for (k, v) in entry.as_object().expect("entry is an object") {
        o[k] = v.clone();
    }
    let mut fields = json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [o]}});
    for (k, v) in own.as_object().expect("own is an object") {
        fields[k] = v.clone();
    }
    node((3, local), (0, 1), &format!("b{local}"), "INSTANCE", name, fields)
}

fn linked(style_local: u32) -> Value {
    json!({"styleIdForEffect": {"guid": g(9, style_local)}})
}

fn changes() -> Value {
    Value::Array(vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        effect_style(1, "Shadow/Card", 8.0),
        effect_style(2, "Shadow/Raised", 12.0),
        node((1, 10), (0, 1), "a", "SYMBOL", "Badge", json!({"size": {"x": 44.0, "y": 22.0}})),
        node((1, 11), (1, 10), "a", "RECTANGLE", "Dot", json!({"size": {"x": 8.0, "y": 8.0}})),
        // The entry repeats an older effect list than the style holds.
        badge(1, "Stale badge", json!({"styleIdForEffect": linked(1)["styleIdForEffect"], "effects": [shadow(2.0)]}), json!({"effects": [shadow(2.0)]})),
        // The entry empties the effects while the instance still links the style.
        badge(2, "Emptied badge", linked(1), json!({"effects": []})),
        // No style link anywhere: the entry's effects are the instance's own.
        badge(3, "Raw badge", json!({"effects": [shadow(4.0)]}), json!({"effects": [shadow(4.0)]})),
        // No style link, and the entry empties the effects.
        badge(4, "Bare badge", json!({}), json!({"effects": []})),
        // The entry itself names another style next to an older list.
        badge(
            5,
            "Restyled badge",
            json!({"styleIdForEffect": linked(2)["styleIdForEffect"], "effects": [shadow(2.0)]}),
            json!({"styleIdForEffect": linked(2)["styleIdForEffect"], "effects": [shadow(2.0)]}),
        ),
    ])
}

fn radii(n: &Node) -> Vec<f32> {
    n.effects
        .iter()
        .map(|e| match e {
            Effect::DropShadow { radius, .. } => *radius,
            other => panic!("drop shadow expected, got {other:?}"),
        })
        .collect()
}

fn style_effects(doc: &Document, n: &Node) -> Option<Vec<Effect>> {
    let key = n.style_refs.get("effect")?;
    let Style::Effect(fx) = doc.styles.get(key)? else { panic!("{key} is not an effect style") };
    Some(fx.clone())
}

fn style_name(doc: &Document, n: &Node) -> Option<String> {
    n.style_refs.get("effect").map(|key| doc.style_name(key).to_string())
}

fn check(stage: &str, doc: &Document) {
    for name in ["Stale badge", "Emptied badge"] {
        let n = find(doc, name);
        assert_eq!(radii(n), vec![8.0], "{stage}: {name} shows the linked style's shadow, not the older copy");
        assert_eq!(style_name(doc, n).as_deref(), Some("Shadow/Card"), "{stage}: {name} keeps the style link");
        assert_eq!(style_effects(doc, n).expect("style definition"), n.effects, "{stage}: {name} effects equal the style's");
    }
    let raw = find(doc, "Raw badge");
    assert_eq!(radii(raw), vec![4.0], "{stage}: an unlinked instance keeps its own effects");
    assert!(!raw.style_refs.contains_key("effect"), "{stage}: and gains no style link");
    let bare = find(doc, "Bare badge");
    assert!(bare.effects.is_empty(), "{stage}: an unlinked instance whose entry empties the effects shows none");
    assert!(!bare.style_refs.contains_key("effect"), "{stage}: and gains no style link");
    let restyled = find(doc, "Restyled badge");
    assert_eq!(radii(restyled), vec![12.0], "{stage}: an entry that names a style gets that style's shadow");
    assert_eq!(style_name(doc, restyled).as_deref(), Some("Shadow/Raised"), "{stage}: and links it");
}

#[test]
fn a_stale_root_entry_never_mixes_into_the_linked_effect_style() {
    check("import", &import_fig_node_changes(&changes(), "Doc").expect("import"));
}

#[test]
fn fig_save_and_reopen_keeps_linked_effect_styles_and_effect_overrides() {
    let mut doc = import_fig_node_changes(&changes(), "Doc").expect("import");
    for round in 1..=2 {
        doc = import_fig(&export_fig(&doc).expect("export")).expect("reopen");
        check(&format!("round {round}"), &doc);
    }
}
