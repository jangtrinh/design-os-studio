//! Screenshot fixtures for the vector tools: `--fixture shape-builder` (three
#![cfg(any(test, feature = "fixtures"))]
//! overlapping shapes with a hovered region) and `--fixture vector-edit` (a
//! vector in point editing with a rounded vertex selected).

use crate::app::App;
use crate::canvas::{tools::Tool, vector};
use designos_doc::{Color, Document, Node, NodeKind, Paint, Rect, StrokeAlign, StrokeJoin, Vec2, VectorNetwork};

pub const FIXTURES: [&str; 2] = ["shape-builder", "vector-edit"];

/// The empty document the fixtures draw into.
pub fn document() -> Document {
    let mut d = Document::new("Untitled");
    d.add_page("Page 1");
    d
}

fn fill(c: Color) -> Vec<Paint> {
    vec![Paint::solid(c)]
}

pub fn apply(app: &mut App, name: &str) {
    match name {
        "shape-builder" => shape_builder(app),
        "vector-edit" => vector_edit(app),
        _ => {}
    }
}

fn shape_builder(app: &mut App) {
    let page = app.page;
    let mut ids = vec![];
    let mut add = |app: &mut App, mut n: Node, name: &str, c: Color| {
        n.name = name.into();
        n.fills = fill(c);
        let id = app.doc.insert(page, n).expect("page exists");
        ids.push(id);
    };
    let a = app.doc.alloc_id();
    add(app, Node::ellipse(a, Rect::new(200.0, 120.0, 240.0, 240.0)), "Circle A", Color::rgb(0xFF, 0xB3, 0x8A));
    let b = app.doc.alloc_id();
    add(app, Node::ellipse(b, Rect::new(340.0, 120.0, 240.0, 240.0)), "Circle B", Color::rgb(0xFF, 0xDF, 0x7E));
    let c = app.doc.alloc_id();
    let mut r = Node::rectangle(c, Rect::new(260.0, 270.0, 260.0, 130.0));
    r.corner_radii = [18.0; 4];
    add(app, r, "Bar", Color::rgb(0xA6, 0xDB, 0xA8));
    app.selection = ids.clone();
    app.set_tool(Tool::ShapeBuilder);
    if let Some(b) = app.canvas.builder.as_mut() {
        // hover the lens where the circles overlap, outside the bar
        b.hover = b.cells.iter().position(|c| c.members == vec![0, 1]);
        b.pin_hover = true;
        // and a region already merged with the bar's part of circle B
        if let Some(i) = b.cells.iter().position(|c| c.members == vec![1, 2]) {
            b.group[i] = Some(1);
        }
    }
}

fn vector_edit(app: &mut App) {
    // an irregular pentagon with two smooth sides
    let pts = [(150.0, 30.0), (280.0, 110.0), (240.0, 260.0), (60.0, 260.0), (20.0, 110.0)];
    let mut net = VectorNetwork::default();
    for (x, y) in pts {
        net.add_vertex(Vec2::new(x, y));
    }
    for i in 0..pts.len() {
        net.connect(i, (i + 1) % pts.len());
    }
    // two curved sides
    net.segments[0].ta = Vec2::new(40.0, -10.0);
    net.segments[0].tb = Vec2::new(-30.0, -40.0);
    net.segments[2].ta = Vec2::new(-20.0, 30.0);
    net.segments[2].tb = Vec2::new(40.0, 30.0);
    let faces = net.faces();
    if let Some(f) = faces.first() {
        net.toggle_face(f);
    }
    for i in [1usize, 3, 4] {
        net.vertices[i].corner_radius = 22.0;
    }
    net.vertices[2].cap = Some(designos_doc::StrokeCap::Round);
    let bb = net.bounds();
    net.translate(Vec2::new(-bb.x, -bb.y));
    let id = app.doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
    n.name = "Gem".into();
    n.transform = designos_doc::Affine::translate(300.0, 120.0);
    n.size = Vec2::new(bb.w, bb.h);
    n.fills = fill(Color::rgb(0xB9, 0xA5, 0xFF));
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x2B, 0x1B, 0x6E))];
    n.stroke.weight = 4.0;
    n.stroke.align = StrokeAlign::Center;
    n.stroke.join = StrokeJoin::Round;
    let page = app.page;
    let id = app.doc.insert(page, n).expect("page exists");
    app.selection = vec![id];
    vector::enter(app, id);
    if let Some(v) = app.canvas.vedit.as_mut() {
        v.sel = [1usize].into_iter().collect();
    }
}
