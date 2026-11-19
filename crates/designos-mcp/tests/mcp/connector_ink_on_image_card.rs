//! The ink an agent-created connector gets on a card that is filled with an image.

use designos_doc::{Color, NodeId, NodeKind, Paint, PaintKind};
use designos_mcp::engine::Engine;
use serde_json::json;

fn png(rgba: [u8; 4]) -> Vec<u8> {
    let mut pixmap = designos_render::tiny_skia::Pixmap::new(2, 2).expect("pixmap");
    let px = designos_render::tiny_skia::ColorU8::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3]).premultiply();
    pixmap.pixels_mut().fill(px);
    pixmap.encode_png().expect("encode test image")
}

fn image_fill(image_ref: &str) -> Paint {
    Paint {
        kind: PaintKind::Image { image_ref: image_ref.into(), scale_mode: designos_doc::ImageScaleMode::Fill, params: Default::default() },
        opacity: 1.0,
        visible: true,
        blend: designos_doc::BlendMode::Normal,
    }
}

/// A light screen holding an image card with two boxes in it, joined by a connector; returns the connector's ink.
fn ink_on_image_card(image: Option<Vec<u8>>) -> Color {
    let mut e = Engine::default();
    let page = e.resolve_default_page().expect("default page");
    let built = e
        .call(
            "designos_apply_batch",
            json!({"ops":[
                {"op":"insert","as":"screen","parent":page.0,"kind":"frame","name":"Light screen","x":0.0,"y":0.0,"width":640.0,"height":360.0},
                {"op":"insert","as":"card","parent":"@screen","kind":"frame","name":"Image card","x":0.0,"y":0.0,"width":640.0,"height":360.0},
                {"op":"insert","as":"a","parent":"@card","kind":"rectangle","name":"A","x":40.0,"y":100.0,"width":80.0,"height":40.0},
                {"op":"insert","as":"b","parent":"@card","kind":"rectangle","name":"B","x":420.0,"y":100.0,"width":80.0,"height":40.0}
            ]}),
        )
        .expect("build the card");
    let id = |name: &str| NodeId(built["name_to_id"][name].as_u64().expect("created id"));
    e.doc.get_mut(id("screen")).expect("screen").fills = vec![Paint::solid(Color::WHITE)];
    e.doc.get_mut(id("card")).expect("card").fills = vec![image_fill("card-photo")];
    if let Some(bytes) = image {
        e.doc.images.insert("card-photo".into(), bytes);
    }
    let linked = e.call("designos_apply_batch", json!({"ops":[{"op":"connect","as":"link","from":id("a").0,"to":id("b").0}]})).expect("connect");
    let link = NodeId(linked["name_to_id"]["link"].as_u64().expect("connector id"));
    let node = e.doc.get(link).expect("connector");
    assert!(matches!(node.kind, NodeKind::Connector(_)));
    match node.stroke.paints.first().map(|p| &p.kind) {
        Some(PaintKind::Solid(c)) => *c,
        other => panic!("the connector has a solid ink, got {other:?}"),
    }
}

#[test]
fn a_connector_on_a_dark_image_card_gets_the_light_ink() {
    assert_eq!(ink_on_image_card(Some(png([0x12, 0x12, 0x18, 0xFF]))), Color::WHITE);
}

#[test]
fn a_connector_on_a_light_image_card_keeps_the_dark_ink() {
    assert_eq!(ink_on_image_card(Some(png([0xF2, 0xF2, 0xEE, 0xFF]))), Color::rgb(0x1E, 0x1E, 0x1E));
}

#[test]
fn a_connector_on_an_image_that_cannot_be_read_gets_the_ink_of_the_surface_below() {
    assert_eq!(ink_on_image_card(None), Color::rgb(0x1E, 0x1E, 0x1E), "the white screen under the card is the surface");
}
