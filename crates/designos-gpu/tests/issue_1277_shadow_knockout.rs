//! CPU/GPU regression coverage for #1277's fill-less shadow knockout cases.
//!
//! Pixel checks require a hardware adapter. The shared test helper skips when
//! only a software rasterizer or no adapter is available.

mod common;

use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const SIZE: (u32, u32) = (280, 220);

fn shadow() -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 },
        offset: Vec2::new(0.0, 18.0),
        radius: 18.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn base() -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("issue-1277-gpu");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };
    let frame = doc.alloc_id();
    let mut node = Node::frame(frame, Rect::new(20.0, 20.0, 220.0, 150.0));
    node.fills.clear();
    node.effects = vec![shadow()];
    doc.insert(page, node).unwrap();
    (doc, page, frame)
}

fn pixel(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn parity(name: &str, build: impl FnOnce(&mut Document, NodeId), probe: (u32, u32)) -> Option<[u8; 4]> {
    let mut gpu = common::gpu()?;
    let (mut doc, page, frame) = base();
    build(&mut doc, frame);
    let vp = Viewport::default();
    let mut cpu = Pixmap::new(SIZE.0, SIZE.1).unwrap();
    render_page(&doc, page, &vp, &mut cpu);
    let rendered = gpu.render_page(&doc, page, &vp, SIZE);
    let a = pixel(&cpu, probe.0, probe.1);
    let b = pixel(&rendered, probe.0, probe.1);
    assert!(a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 8), "{name}: probe {probe:?} CPU {a:?} GPU {b:?}");
    Some(a)
}

#[test]
fn masked_sibling_knockout_matches_cpu() {
    let Some(p) = parity(
        "mask",
        |doc, frame| {
            let mask = doc.alloc_id();
            let mut node = Node::ellipse(mask, Rect::new(50.0, 40.0, 80.0, 80.0));
            node.is_mask = true;
            doc.insert(frame, node).unwrap();
            let child = doc.alloc_id();
            let mut node = Node::rectangle(child, Rect::new(50.0, 40.0, 140.0, 50.0));
            node.fills = vec![Paint::solid(Color::WHITE)];
            doc.insert(frame, node).unwrap();
        },
        (160, 100),
    ) else {
        return;
    };
    assert!(p[3] > 0, "masked shadow outside ellipse was fully knocked out: {p:?}");
}

#[test]
fn grouped_child_knockout_matches_cpu() {
    let Some(p) = parity(
        "group",
        |doc, frame| {
            let group = doc.alloc_id();
            let mut node = Node::new(group, NodeKind::Group);
            node.transform = Affine::translate(70.0, 55.0);
            node.size = Vec2::new(100.0, 50.0);
            doc.insert(frame, node).unwrap();
            let child = doc.alloc_id();
            let mut node = Node::rectangle(child, Rect::new(0.0, 0.0, 100.0, 50.0));
            node.fills = vec![Paint::solid(Color { a: 0.5, ..Color::WHITE })];
            doc.insert(group, node).unwrap();
        },
        (120, 120),
    ) else {
        return;
    };
    assert!(p[0] >= 125 && p[0] <= 131 && p[3] <= 130, "group knockout left shadow under the child: {p:?}");
}

#[test]
fn paint_free_child_knockout_matches_cpu() {
    let Some(p) = parity(
        "paint-free",
        |doc, frame| {
            let painted = doc.alloc_id();
            doc.insert(frame, Node::rectangle(painted, Rect::new(20.0, 50.0, 100.0, 70.0))).unwrap();
            let empty = doc.alloc_id();
            let mut node = Node::frame(empty, Rect::new(120.0, 50.0, 80.0, 70.0));
            node.fills.clear();
            doc.insert(frame, node).unwrap();
        },
        (150, 100),
    ) else {
        return;
    };
    assert!(p[3] > 0, "paint-free child erased its whole frame: {p:?}");
}
