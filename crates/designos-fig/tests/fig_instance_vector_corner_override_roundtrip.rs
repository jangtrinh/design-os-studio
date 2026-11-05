//! A nested vector whose instance overrides its corner radius keeps its vector network across a `.fig` save and reopen.
//!
//! The `cornerRadius` key of the override is all the file needs for a vector: the importer leaves a vector's outline
//! alone for it. A `fillGeometry` written next to it is read as a replacement outline and drops the network, so the
//! next save has no vertices or handles left to write.
use designos_doc::*;
use designos_fig::{export_fig, import_fig};

const OVERRIDDEN_RADIUS: f32 = 6.0;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn outline() -> Path {
    Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(100.0, 0.0)),
            PathCmd::LineTo(Vec2::new(100.0, 40.0)),
            PathCmd::LineTo(Vec2::new(0.0, 40.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }
}

/// The `Glyph` vector inside the container named `container`.
fn glyph_in<'a>(doc: &'a Document, container: &str) -> &'a Node {
    let parent = find(doc, container);
    parent.children.iter().filter_map(|&id| doc.get(id)).find(|n| n.name == "Glyph").unwrap_or_else(|| panic!("no Glyph inside {container}"))
}

fn network_vertices(n: &Node) -> Option<usize> {
    match &n.kind {
        NodeKind::Vector { network, .. } => network.as_ref().map(|net| net.vertices.len()),
        other => panic!("{} is not a vector: {other:?}", n.name),
    }
}

/// A component `Icon` holding a `Glyph` vector with both a vector network and baked fill geometry, and an instance
/// `Icon copy` of it whose `Glyph` overrides its corner radius. Returns the document and the vertex count of the network.
fn fixture() -> (Document, usize) {
    let mut doc = Document::new("VectorCornerOverride");
    let page = doc.add_page("Page 1");

    let comp_id = doc.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "Icon".into();
    comp.size = Vec2::new(100.0, 40.0);
    doc.insert(page, comp).expect("insert component");

    let path = outline();
    let network = VectorNetwork::from_paths(std::slice::from_ref(&path));
    let vertices = network.vertices.len();
    assert!(vertices > 0, "the fixture network has vertices");
    let glyph_id = doc.alloc_id();
    let mut glyph = Node::new(glyph_id, NodeKind::Vector { paths: vec![path.clone()], network: Some(network) });
    glyph.name = "Glyph".into();
    glyph.size = Vec2::new(100.0, 40.0);
    glyph.fills = vec![Paint::solid(Color::BLACK)];
    glyph.fill_geometry = Some(vec![path]);
    doc.insert(comp_id, glyph).expect("insert glyph");

    create_instance(&mut doc, comp_id, page, Vec2::new(200.0, 0.0)).apply(&mut doc).expect("create instance");
    let inst_id = instances_of(&doc, comp_id)[0];
    doc.get_mut(inst_id).expect("instance").name = "Icon copy".into();
    let copy_id = doc.get(inst_id).expect("instance").children.iter().copied().find(|&id| doc.get(id).is_some_and(|n| n.name == "Glyph")).expect("Glyph copy");
    let copy = doc.get_mut(copy_id).expect("Glyph copy");
    copy.corner_radii = [OVERRIDDEN_RADIUS; 4];
    copy.component.overrides.insert(OverrideField::CornerRadii);
    (doc, vertices)
}

#[test]
fn fig_save_and_reopen_keeps_the_network_of_a_vector_with_a_corner_override() {
    let (mut doc, vertices) = fixture();
    assert_eq!(network_vertices(glyph_in(&doc, "Icon copy")), Some(vertices), "fixture: the copy starts with its network");
    for round in 1..=2 {
        doc = import_fig(&export_fig(&doc).expect("export")).expect("reopen");
        let copy = glyph_in(&doc, "Icon copy");
        assert_eq!(network_vertices(copy), Some(vertices), "round {round}: the overridden vector keeps its network");
        assert_eq!(copy.corner_radii, [OVERRIDDEN_RADIUS; 4], "round {round}: the corner radius override survives");
        assert!(copy.component.overrides.contains(&OverrideField::CornerRadii), "round {round}: the CornerRadii override flag survives");
        let main = glyph_in(&doc, "Icon");
        assert_eq!(network_vertices(main), Some(vertices), "round {round}: the main keeps its network");
        assert_eq!(main.corner_radii, [0.0; 4], "round {round}: the main keeps its own corner radius");
    }
}
