use designos_doc::{Node, NodeKind, PathCmd, Vec2};
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{figma_like_schema, fl, msg, node, solid, st, vec2};
use designos_fig::{export_fig, import_fig};

fn icon_blob() -> Vec<u8> {
    let mut blob = Vec::new();
    for (op, x, y) in [(1, 0.0f32, 0.0f32), (2, 1.0, 0.0), (2, 0.5, 0.8)] {
        blob.push(op);
        blob.extend(x.to_le_bytes());
        blob.extend(y.to_le_bytes());
    }
    blob.push(0);
    blob
}

fn fixture() -> Vec<u8> {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![]),
        node((1, 1), (0, 1), "a", "FRAME", "Clipping frame", vec![("size", vec2(24.0, 24.0))]),
        node(
            (1, 2),
            (1, 1),
            "a",
            "TEXT",
            "Icon text",
            vec![
                ("size", vec2(20.0, 20.0)),
                ("fontSize", fl(20.0)),
                (
                    "fontName",
                    st(vec![
                        ("family", V::String("MissingIconFaceForTest".into())),
                        ("style", V::String("Regular".into())),
                        ("postscript", V::String(String::new())),
                    ]),
                ),
                ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
                ("textData", msg(vec![("characters", V::String("\u{e900}".into()))])),
                (
                    "derivedTextData",
                    msg(vec![
                        ("layoutSize", vec2(20.0, 20.0)),
                        (
                            "fontMetaData",
                            V::Array(vec![msg(vec![(
                                "key",
                                st(vec![
                                    ("family", V::String("MissingIconFaceForTest".into())),
                                    ("style", V::String("Regular".into())),
                                    ("postscript", V::String(String::new())),
                                ]),
                            )])]),
                        ),
                        ("glyphs", V::Array(vec![msg(vec![("commandsBlob", V::Uint(0)), ("position", vec2(0.0, 20.0)), ("fontSize", fl(20.0))])])),
                    ]),
                ),
            ],
        ),
    ];
    let schema = figma_like_schema();
    let body = schema
        .encode_message(
            schema.find("Message").unwrap(),
            &msg(vec![("nodeChanges", V::Array(nodes)), ("blobs", V::Array(vec![st(vec![("bytes", V::Bytes(icon_blob()))])]))]),
        )
        .unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 161)
}

fn icon_node(doc: &designos_doc::Document) -> &Node {
    doc.nodes.values().find(|node| node.name == "Icon text").unwrap()
}

#[test]
fn missing_icon_font_uses_embedded_glyph_without_losing_editability() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    let saved = icon.fig_text_geometry.as_ref().expect("embedded glyphs retained");
    assert_eq!(
        saved.path.cmds,
        vec![PathCmd::MoveTo(Vec2::new(0.0, 20.0)), PathCmd::LineTo(Vec2::new(20.0, 20.0)), PathCmd::LineTo(Vec2::new(10.0, 4.0)), PathCmd::Close,]
    );
    assert!(saved.matches(icon));
    let outlines = designos_render::text_outlines_for_node(icon);
    assert_eq!(outlines.len(), 1);
    let bounds = outlines[0].path.bounds();
    assert!((bounds.left() - 0.0).abs() < 1e-4 && (bounds.right() - 20.0).abs() < 1e-4);

    let mut edited = icon.clone();
    if let NodeKind::Text(text) = &mut edited.kind {
        text.content = "edited".into();
    }
    assert!(!saved.matches(&edited), "the cached icon cannot override an edited text layer");
    edited.size.x += 1.0;
    assert!(!saved.matches(&edited), "the cached icon cannot override a resized text layer");

    // The geometry also survives Studio's document serialization.
    let restored: Node = serde_json::from_str(&serde_json::to_string(icon).unwrap()).unwrap();
    assert_eq!(restored.fig_text_geometry, icon.fig_text_geometry);

    let reopened = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(icon_node(&reopened).fig_text_geometry, icon.fig_text_geometry);

    let mut edited_doc = doc.clone();
    if let NodeKind::Text(text) = &mut edited_doc.get_mut(icon.id).unwrap().kind {
        text.content = "edited".into();
    }
    let reopened_edit = import_fig(&export_fig(&edited_doc).unwrap()).unwrap();
    assert!(icon_node(&reopened_edit).fig_text_geometry.is_none(), "edited text must not export stale icon geometry");
}
