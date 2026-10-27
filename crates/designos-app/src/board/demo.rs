//! A small sample board (`--fixture board`): a section with stickies, a decision shape and
#![cfg(any(test, feature = "fixtures"))]
//! connectors, plus loose notes and stamps. Used for screenshots and the UI tests.

use super::{color_of, create};
use designos_doc::*;

fn sticky(d: &mut Document, parent: NodeId, name: &str, at: (f32, f32), color: u32, text: &str) -> NodeId {
    let id = d.alloc_id();
    let mut n = Node::sticky(id, Rect::new(at.0, at.1, STICKY_SIZE, STICKY_SIZE), text);
    n.name = name.into();
    if let NodeKind::Sticky(s) = &mut n.kind {
        s.color = color_of(color);
    }
    d.insert(parent, n).unwrap()
}

fn connect(d: &mut Document, page: NodeId, name: &str, (a, am): (NodeId, Magnet), (b, bm): (NodeId, Magnet), label: &str) -> NodeId {
    let id = d.alloc_id();
    let mut p = ConnectorProps { start: ConnectorEnd::attached(a, am), end: ConnectorEnd::attached(b, bm), ..Default::default() };
    p.end_cap = ConnectorCap::Triangle;
    if !label.is_empty() {
        let mut l = board_text(14.0, TextAlignH::Center, TextAlignV::Center);
        l.content = label.into();
        p.label = Some(l);
    }
    let mut n = Node::connector(id, p);
    n.name = name.into();
    let id = d.insert(page, n).unwrap();
    sync_connector(d, id);
    id
}

/// The sample board.
pub fn document() -> Document {
    let mut d = create::new_document("Sprint retro");
    let page = d.pages[0];

    let sid = d.alloc_id();
    let mut section = Node::new(sid, NodeKind::Section);
    section.name = "Went well".into();
    section.transform = Affine::translate(80.0, 80.0);
    section.size = Vec2::new(1060.0, 700.0);
    section.fills = vec![Paint::solid(Color::WHITE)];
    section.stroke.paints = vec![Paint::solid(Color::rgb(0xCF, 0xCF, 0xCF))];
    section.stroke.weight = 1.0;
    let section = d.insert(page, section).unwrap();

    let a = sticky(&mut d, section, "Sticky 1", (40.0, 60.0), 0xFFE58F, "Shipped the board mode");
    let b = sticky(&mut d, section, "Sticky 2", (340.0, 60.0), 0xB6D4FF, "Connectors follow their notes when they move");
    sticky(&mut d, section, "Sticky 3", (640.0, 60.0), 0xC4EBA6, "Text shrinks to fit the note");

    let id = d.alloc_id();
    let mut decision = Node::shape_with_text(id, Rect::new(120.0, 420.0, 220.0, 150.0), ShapeKind::Diamond, "Ship it?");
    decision.name = "Decision".into();
    decision.fills = vec![Paint::solid(color_of(0xFFC48A))];
    let decision = d.insert(section, decision).unwrap();

    let id = d.alloc_id();
    let mut release = Node::shape_with_text(id, Rect::new(520.0, 445.0, 220.0, 100.0), ShapeKind::RoundedRect, "Release");
    release.name = "Release".into();
    release.fills = vec![Paint::solid(color_of(0xD5C3FF))];
    let release = d.insert(section, release).unwrap();

    // loose notes and stamps on the canvas
    let c = sticky(&mut d, page, "Sticky 4", (1240.0, 120.0), 0xFFB3B3, "Needs a follow-up");
    let _ = c;
    for (i, k) in ["heart", "thumbs_up", "star"].iter().enumerate() {
        let id = d.alloc_id();
        d.insert(page, Node::stamp(id, Vec2::new(1260.0 + i as f32 * 80.0, 440.0), k, 64.0)).unwrap();
    }

    connect(&mut d, page, "Connector 1", (a, Magnet::Bottom), (decision, Magnet::Top), "");
    connect(&mut d, page, "Connector 2", (decision, Magnet::Right), (release, Magnet::Left), "yes");
    connect(&mut d, page, "Connector 3", (b, Magnet::Bottom), (release, Magnet::Top), "");
    d
}
