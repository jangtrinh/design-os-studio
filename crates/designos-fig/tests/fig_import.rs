use designos_doc::*;
use designos_fig::container::Compression;
use designos_fig::testkit::{sample_fig, IMAGE_HASH};
use designos_fig::{decode_fig_to_json, export_rest_json, image_refs, import_fig, import_rest_json};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn check(doc: &Document) {
    // Pages: the internal-only canvas is dropped.
    assert_eq!(doc.pages.len(), 1);
    let page = doc.get(doc.pages[0]).unwrap();
    assert_eq!(page.name, "Page 1");
    assert!(matches!(page.kind, NodeKind::Page { background } if (background.r - 0.9).abs() < 1e-6));
    // Page children sorted by fractional index: Card(a), Pie(b), Star(c), Sticky(d)
    let names: Vec<_> = page.children.iter().map(|c| doc.get(*c).unwrap().name.as_str()).collect();
    assert_eq!(names, ["Card", "Pie", "Star", "Note"]);

    let card = find(doc, "Card");
    assert_eq!(card.source_id.as_deref(), Some("1:1"));
    assert_eq!(card.transform.translation(), Vec2::new(40.0, 50.0));
    assert_eq!(card.size, Vec2::new(200.0, 300.0));
    assert_eq!(card.corner_radii, [12.0; 4]);
    assert!(matches!(card.effects[0], Effect::DropShadow { radius, .. } if radius == 8.0));
    let f = card.kind.frame_props().unwrap();
    assert!(f.clips_content, "frameMaskDisabled absent means clipping on");
    let al = f.auto_layout.as_ref().unwrap();
    assert_eq!(al.direction, LayoutDirection::Vertical);
    assert_eq!(al.padding, [12.0, 16.0, 12.0, 16.0], "right/bottom fall back to horizontal/vertical padding");
    assert_eq!(al.primary_sizing, Sizing::Hug);
    assert_eq!(al.counter_sizing, Sizing::Fixed);
    assert_eq!(al.counter_align, LayoutAlign::Center);

    // Children sorted by position: text(a) rect(b) vector(c) component(d) instance(e)
    let kids: Vec<_> = card.children.iter().map(|c| doc.get(*c).unwrap().name.as_str()).collect();
    assert_eq!(kids, ["Title", "Image rect", "Tri", "Button", "Button copy"], "removed node dropped, order by position");

    let rect = find(doc, "Image rect");
    assert!(matches!(rect.kind, NodeKind::Rectangle));
    assert_eq!(rect.blend, BlendMode::Multiply);
    assert_eq!(rect.stroke.align, StrokeAlign::Outside);
    assert_eq!(rect.stroke.weight, 2.0);
    assert_eq!(rect.layout_child.horizontal, Sizing::Fill, "STRETCH align in a vertical layout");
    let hash = designos_fig::kiwi::hex(&IMAGE_HASH);
    assert!(matches!(&rect.fills[0].kind, PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fill } if *image_ref == hash));
    assert_eq!(image_refs(doc), vec![hash]);

    // Text: weight/italic from style names, % letter spacing resolved, UTF-16 style ids.
    let t = find(doc, "Title");
    let NodeKind::Text(tp) = &t.kind else { panic!() };
    assert_eq!(tp.content, "Hi \u{1F600}!");
    assert_eq!(tp.style.font_family, "Inter");
    assert_eq!(tp.style.font_weight, 500);
    assert!((tp.style.letter_spacing - 1.6).abs() < 1e-4);
    assert_eq!(tp.style.line_height, LineHeight::Pixels(24.0));
    assert_eq!(tp.style.align_h, TextAlignH::Center);
    assert_eq!(tp.auto_resize, TextAutoResize::WidthAndHeight);
    assert!(matches!(tp.style.fills[0].kind, PaintKind::Solid(c) if (c.b - 0.3).abs() < 1e-6));
    assert_eq!(tp.runs.len(), 1);
    assert_eq!(&tp.content[tp.runs[0].start..tp.runs[0].end], "\u{1F600}");
    assert_eq!(tp.runs[0].style.font_weight, 700);
    assert!(tp.runs[0].style.italic);
    assert_eq!(tp.runs[0].style.decoration, TextDecoration::Underline);
    assert_eq!(tp.runs[0].style.font_size, 16.0);

    // Vector geometry from the blob.
    let NodeKind::Vector { paths, .. } = &find(doc, "Tri").kind else { panic!() };
    assert_eq!(
        paths[0].cmds,
        vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(10.0, 0.0)), PathCmd::LineTo(Vec2::new(5.0, 8.0)), PathCmd::Close]
    );

    // Component + instance (childless instance gets a copy of the component's children).
    let comp = find(doc, "Button");
    assert!(matches!(comp.kind, NodeKind::Component(_)));
    let inst = find(doc, "Button copy");
    assert!(matches!(&inst.kind, NodeKind::Instance { component: Some(c), .. } if *c == comp.id));
    assert_eq!(inst.children.len(), 1);
    let copy = doc.get(inst.children[0]).unwrap();
    assert_eq!(copy.name, "Bg");
    assert_ne!(copy.id, comp.children[0]);
    assert_eq!(copy.source_id.as_deref(), Some("I1:7;1:6"));

    let pie = find(doc, "Pie");
    assert!(matches!(pie.kind, NodeKind::Ellipse(a) if a.end == 3.0 && a.inner_radius == 0.5));
    assert!(matches!(find(doc, "Star").kind, NodeKind::Star { count: 6, inner_ratio } if (inner_ratio - 0.5).abs() < 1e-6));
    assert!(matches!(&find(doc, "Note").kind, NodeKind::Sticky(_)), "FigJam stickies map to the board kind");
}

#[test]
fn imports_every_container_variant() {
    for (how, zipped) in [(Compression::Deflate, false), (Compression::Zstd, false), (Compression::Deflate, true), (Compression::Zstd, true)] {
        let bytes = sample_fig(how, zipped);
        let doc = import_fig(&bytes).unwrap_or_else(|e| panic!("{how:?} zipped={zipped}: {e}"));
        check(&doc);
        if zipped {
            assert_eq!(doc.name, "Sample file");
            assert_eq!(doc.images[&designos_fig::kiwi::hex(&IMAGE_HASH)], vec![0x89, b'P', b'N', b'G']);
        } else {
            assert_eq!(doc.name, "Untitled");
            assert!(doc.images.is_empty());
        }
    }
}

#[test]
fn decode_to_json_is_readable() {
    let v = decode_fig_to_json(&sample_fig(Compression::Zstd, true)).unwrap();
    let nodes = v["message"]["nodeChanges"].as_array().unwrap();
    assert_eq!(nodes.len(), 14);
    assert_eq!(nodes[0]["type"], "INSTANCE");
    assert_eq!(nodes[0]["guid"]["localID"], 7);
    assert_eq!(v["message"]["blobs"][0]["bytes"].as_str().unwrap().len(), 2 * designos_fig::testkit::triangle_blob().len());
    assert_eq!(v["meta"]["file_name"], "Sample file");
}

#[test]
fn fig_document_exports_and_reimports() {
    let doc = import_fig(&sample_fig(Compression::Deflate, false)).unwrap();
    let json = export_rest_json(&doc);
    let again = import_rest_json(&json.to_string()).unwrap();
    // NodeIds differ (expanded instance children are allocated last on the first pass), so compare exported shapes.
    assert!(json == export_rest_json(&again), "export differs after re-import");
    let visible = |d: &Document| d.nodes.keys().filter(|id| !is_library_node(d, **id)).count();
    assert_eq!(visible(&doc), visible(&again));
}

#[test]
fn bad_input_never_panics() {
    assert!(import_fig(b"").is_err());
    assert!(import_fig(b"not a figma file").is_err());
    assert!(import_fig(b"fig-kiwi").is_err());
    assert!(import_fig(b"PK\x03\x04").is_err());
    for (how, zipped) in [(Compression::Deflate, false), (Compression::Zstd, false), (Compression::Deflate, true)] {
        let bytes = sample_fig(how, zipped);
        // every truncation
        for n in (0..bytes.len()).step_by(if zipped { 7 } else { 1 }) {
            let _ = import_fig(&bytes[..n]);
        }
        // byte corruption (deterministic xorshift)
        let mut x = 0x2545F491u32;
        for _ in 0..400 {
            let mut b = bytes.clone();
            for _ in 0..3 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let i = x as usize % b.len();
                b[i] ^= (x >> 8) as u8 | 1;
            }
            let _ = import_fig(&b);
            let _ = decode_fig_to_json(&b);
        }
    }
}
