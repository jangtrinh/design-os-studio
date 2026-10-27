//! #161: the blend mode of drop and inner shadows survives `.fig` and REST import / export.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{color, en, figma_like_schema, fl, msg, node, vec2};
use designos_fig::{export_fig, export_rest_json, import_fig, import_rest_json};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn kiwi_shadow(ty: &str, blend: Option<&str>) -> V {
    let mut f = vec![
        ("type", en(ty)),
        ("color", color(0.0, 0.0, 0.0, 0.25)),
        ("offset", vec2(0.0, 4.0)),
        ("radius", fl(6.0)),
        ("spread", fl(0.0)),
        ("visible", V::Bool(true)),
    ];
    if let Some(b) = blend {
        f.push(("blendMode", en(b)));
    }
    msg(f)
}

fn fig_fixture(effects: Vec<V>) -> Vec<u8> {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((2, 10), (0, 1), "10", "FRAME", "Shadowed", vec![("size", vec2(100.0, 100.0)), ("effects", V::Array(effects))]),
    ];
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(nodes))])).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 161)
}

fn modes(n: &Node) -> Vec<BlendMode> {
    n.effects
        .iter()
        .map(|e| match e {
            Effect::DropShadow { blend, .. } | Effect::InnerShadow { blend, .. } => *blend,
            other => panic!("expected a shadow, got {other:?}"),
        })
        .collect()
}

#[test]
fn fig_import_export_reimport_keeps_the_shadow_blend_mode() {
    let bytes = fig_fixture(vec![
        kiwi_shadow("DROP_SHADOW", Some("MULTIPLY")),
        kiwi_shadow("INNER_SHADOW", Some("SCREEN")),
        kiwi_shadow("DROP_SHADOW", Some("LINEAR_BURN")),
        kiwi_shadow("INNER_SHADOW", Some("LUMINOSITY")),
        kiwi_shadow("DROP_SHADOW", None),
    ]);
    let want = vec![BlendMode::Multiply, BlendMode::Screen, BlendMode::LinearBurn, BlendMode::Luminosity, BlendMode::Normal];
    let d1 = import_fig(&bytes).unwrap();
    assert_eq!(modes(find(&d1, "Shadowed")), want, "first import");
    let d2 = import_fig(&export_fig(&d1).unwrap()).unwrap();
    assert_eq!(modes(find(&d2, "Shadowed")), want, "after export and re-import");
}

fn doc_with(effects: Vec<Effect>) -> Document {
    let mut doc = Document::new("fx");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.name = "Shadowed".into();
    n.effects = effects;
    doc.insert(page, n).unwrap();
    doc
}

fn shadows(blends: &[BlendMode]) -> Vec<Effect> {
    blends
        .iter()
        .enumerate()
        .map(|(i, &blend)| {
            if i % 2 == 0 {
                Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 2.0), radius: 4.0, spread: 0.0, visible: true, show_behind: false, blend }
            } else {
                Effect::InnerShadow { color: Color::BLACK, offset: Vec2::new(0.0, 2.0), radius: 4.0, spread: 0.0, visible: true, blend }
            }
        })
        .collect()
}

#[test]
fn rest_export_reimport_keeps_the_shadow_blend_mode() {
    let all = [BlendMode::Multiply, BlendMode::Screen, BlendMode::LinearDodge, BlendMode::ColorBurn, BlendMode::Normal, BlendMode::Hue];
    let doc = doc_with(shadows(&all));
    let json = export_rest_json(&doc);
    assert_eq!(json.to_string().matches("\"blendMode\":\"MULTIPLY\"").count(), 1);
    let back = import_rest_json(&json.to_string()).unwrap();
    assert_eq!(modes(find(&back, "Shadowed")), all);
}

#[test]
fn rest_and_fig_exports_agree_after_a_cross_round_trip() {
    // .fig -> REST -> .fig keeps the mode too
    let d1 = import_fig(&fig_fixture(vec![kiwi_shadow("DROP_SHADOW", Some("OVERLAY"))])).unwrap();
    let via_rest = import_rest_json(&export_rest_json(&d1).to_string()).unwrap();
    let d2 = import_fig(&export_fig(&via_rest).unwrap()).unwrap();
    assert_eq!(modes(find(&d2, "Shadowed")), vec![BlendMode::Overlay]);
}

#[test]
fn rest_shadows_without_a_blend_mode_load_as_normal() {
    let json = r#"{"name":"f","document":{"id":"0:0","type":"DOCUMENT","children":[{"id":"0:1","type":"CANVAS","name":"p","children":[
        {"id":"1:1","type":"RECTANGLE","name":"Shadowed","size":{"x":10,"y":10},
         "effects":[{"type":"DROP_SHADOW","radius":4,"offset":{"x":0,"y":2}},{"type":"INNER_SHADOW","radius":4,"offset":{"x":0,"y":2}}]}]}]}}"#;
    let doc = import_rest_json(json).unwrap();
    assert_eq!(modes(find(&doc, "Shadowed")), vec![BlendMode::Normal, BlendMode::Normal]);
}

#[test]
fn imported_pass_through_shadow_blend_round_trips_unchanged() {
    // the Effects panel doesn't offer pass-through, but an imported value is kept as is
    let d1 = import_fig(&fig_fixture(vec![kiwi_shadow("DROP_SHADOW", Some("PASS_THROUGH"))])).unwrap();
    assert_eq!(modes(find(&d1, "Shadowed")), vec![BlendMode::PassThrough], "fig import");
    let d2 = import_fig(&export_fig(&d1).unwrap()).unwrap();
    assert_eq!(modes(find(&d2, "Shadowed")), vec![BlendMode::PassThrough], "fig re-import");
    let back = import_rest_json(&export_rest_json(&d2).to_string()).unwrap();
    assert_eq!(modes(find(&back, "Shadowed")), vec![BlendMode::PassThrough], "REST round trip");
}
