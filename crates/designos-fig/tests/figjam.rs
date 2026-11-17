use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::testkit::sample_fig;
use designos_fig::{export_rest_json, import_fig, import_rest_json};

const FIXTURE: &str = include_str!("fixtures/figjam_file.json");

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

#[test]
fn editor_type_makes_a_board() {
    let doc = import_rest_json(FIXTURE).unwrap();
    assert_eq!(doc.kind, DocumentKind::Board);
    // a design file stays a design file
    let design = import_rest_json(&FIXTURE.replace("\"editorType\": \"figjam\",", "")).unwrap();
    assert_eq!(design.kind, DocumentKind::Design);
}

#[test]
fn jam_container_magic_makes_a_board() {
    let fig = sample_fig(Compression::Zstd, false);
    assert_eq!(import_fig(&fig).expect("import .fig container").kind, DocumentKind::Design);
    let mut jam = fig;
    jam[..container::MAGIC_JAM.len()].copy_from_slice(container::MAGIC_JAM);
    let doc = import_fig(&jam).expect("import .jam container");
    assert_eq!(doc.kind, DocumentKind::Board);
}

#[test]
fn sticky_import() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let n = find(&doc, "Sticky");
    let NodeKind::Sticky(s) = &n.kind else { panic!("{:?}", n.kind) };
    assert_eq!(s.text.content, "Shipped board mode");
    assert_eq!(s.text.style.font_size, 24.0);
    assert_eq!(s.author.as_deref(), Some("Kir"));
    assert!(s.show_author);
    assert_eq!(s.color.to_hex(), "FFE58F");
    assert!(n.fills.is_empty(), "the colour lives in the props");
    assert_eq!(n.size, Vec2::new(240.0, 240.0));
    // it sits inside its section
    let parent = doc.get(n.parent.unwrap()).unwrap();
    assert_eq!(parent.kind, NodeKind::Section);
    assert_eq!(parent.name, "Went well");
}

#[test]
fn shape_with_text_import() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let n = find(&doc, "Decision");
    let NodeKind::ShapeWithText(s) = &n.kind else { panic!() };
    assert_eq!(s.shape, ShapeKind::Diamond);
    assert_eq!(s.text.content, "Ship it?");
    assert_eq!(s.text.style.font_size, 18.0);
    assert_eq!(s.text.style.align_h, TextAlignH::Center);
    assert_eq!(n.stroke.weight, 2.0);
    assert_eq!(n.fills.len(), 1);
}

#[test]
fn connector_import_links_nodes_and_routes() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let sticky = find(&doc, "Sticky").id;
    let shape = find(&doc, "Decision").id;
    let c = find(&doc, "Connector");
    let NodeKind::Connector(p) = &c.kind else { panic!() };
    assert_eq!(p.start.node, Some(sticky));
    assert_eq!(p.end.node, Some(shape));
    assert_eq!(p.start.magnet, Magnet::Right);
    assert_eq!(p.end.magnet, Magnet::Auto);
    assert_eq!(p.line, ConnectorLine::Elbowed);
    assert_eq!(p.end_cap, ConnectorCap::Triangle);
    assert_eq!(p.start_cap, ConnectorCap::None);
    assert_eq!(p.label.as_ref().map(|l| l.content.as_str()), Some("yes"));
    // placed on its route: it covers the line from the sticky's right side to the diamond's left corner
    let r = connector_route(&doc, c.id).unwrap();
    assert_eq!(r.start(), Vec2::new(280.0, 180.0), "right side of the sticky, in section space");
    assert_eq!(r.end(), Vec2::new(380.0, 150.0), "left corner of the diamond");
    assert!(c.transform.tx <= 280.0 && c.transform.tx + c.size.x >= 380.0);

    // free end: absolute page position becomes section space
    let l = find(&doc, "Loose end");
    let NodeKind::Connector(p) = &l.kind else { panic!() };
    assert_eq!(p.start.node, Some(shape));
    assert_eq!(p.end.node, None);
    assert_eq!(p.end.point, Vec2::new(480.0, 380.0));
    assert_eq!(p.line, ConnectorLine::Straight);
    assert_eq!(p.end_cap, ConnectorCap::Arrow);
    assert_eq!(l.stroke.weight, 3.0);
}

#[test]
fn stamp_import() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let n = find(&doc, "Heart");
    assert_eq!(n.kind, NodeKind::Stamp { kind: "heart".into() });
    assert_eq!(n.size, Vec2::new(64.0, 64.0));
}

#[test]
fn export_marks_figjam_and_roundtrips_every_board_kind() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let json = export_rest_json(&doc);
    assert_eq!(json["editorType"], "figjam");
    let text = serde_json::to_string(&json).unwrap();
    for ty in ["STICKY", "SHAPE_WITH_TEXT", "CONNECTOR", "STAMP", "SECTION"] {
        assert!(text.contains(&format!("\"type\":\"{ty}\"")), "{ty} missing");
    }
    let back = import_rest_json(&text).unwrap();
    assert_eq!(back.kind, DocumentKind::Board);
    for name in ["Sticky", "Decision", "Connector", "Loose end", "Heart", "Went well"] {
        let (a, b) = (find(&doc, name), find(&back, name));
        // node ids differ between imports; compare kinds with ends mapped by name
        match (&a.kind, &b.kind) {
            (NodeKind::Connector(x), NodeKind::Connector(y)) => {
                assert_eq!(x.line, y.line, "{name}");
                assert_eq!((x.start_cap, x.end_cap), (y.start_cap, y.end_cap), "{name}");
                assert_eq!((x.start.magnet, x.end.magnet), (y.start.magnet, y.end.magnet), "{name}");
                assert_eq!(x.label, y.label, "{name}");
                let nm = |d: &Document, id: Option<NodeId>| id.and_then(|i| d.get(i)).map(|n| n.name.clone());
                assert_eq!(nm(&doc, x.start.node), nm(&back, y.start.node), "{name}");
                assert_eq!(nm(&doc, x.end.node), nm(&back, y.end.node), "{name}");
            }
            (x, y) => assert_eq!(x, y, "{name}"),
        }
        assert_eq!(a.size, b.size, "{name}");
        assert_eq!(a.transform, b.transform, "{name}");
        assert_eq!(a.stroke.weight, b.stroke.weight, "{name}");
        assert_eq!(a.fills, b.fills, "{name}");
    }
}

#[test]
fn natively_built_boards_roundtrip_through_rest_json() {
    let mut d = Document::new("native");
    d.kind = DocumentKind::Board;
    let page = d.add_page("p");
    let a = d.alloc_id();
    let a = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 200.0, 200.0), "hello")).unwrap();
    let b = d.alloc_id();
    let b = d.insert(page, Node::shape_with_text(b, Rect::new(400.0, 0.0, 120.0, 90.0), ShapeKind::Hexagon, "x")).unwrap();
    let c = d.alloc_id();
    let mut props = ConnectorProps {
        start: ConnectorEnd::attached(a, Magnet::Auto),
        end: ConnectorEnd::attached(b, Magnet::Top),
        line: ConnectorLine::Curved,
        ..Default::default()
    };
    props.start_cap = ConnectorCap::Diamond;
    d.insert(page, Node::connector(c, props)).unwrap();
    sync_connector(&mut d, c);
    let back = import_rest_json(&serde_json::to_string(&export_rest_json(&d)).unwrap()).unwrap();
    assert_eq!(back.kind, DocumentKind::Board);
    let conn = find(&back, "Connector");
    let NodeKind::Connector(p) = &conn.kind else { panic!() };
    assert_eq!((p.line, p.start_cap, p.end_cap), (ConnectorLine::Curved, ConnectorCap::Diamond, ConnectorCap::Arrow));
    assert!(p.start.node.is_some() && p.end.node.is_some());
    assert_eq!(p.end.magnet, Magnet::Top);
    let hex = find(&back, "Shape");
    assert!(matches!(&hex.kind, NodeKind::ShapeWithText(s) if s.shape == ShapeKind::Hexagon && s.text.content == "x"));
    let st = find(&back, "Sticky note");
    assert!(matches!(&st.kind, NodeKind::Sticky(s) if s.text.content == "hello" && s.color == StickyProps::default().color));
}
