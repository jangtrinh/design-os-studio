//! With `DESIGNOS_BOARD_SHOTS=<dir>` set, writes a gallery of the board kinds for eyeballing.

use crate::common;

use common::*;
use designos_doc::*;

fn sticky(t: &mut T, r: Rect, text: &str, c: u32) -> NodeId {
    let id = t.add(None, |id| Node::sticky(id, r, text));
    if let NodeKind::Sticky(s) = &mut t.node(id).kind {
        s.color = Color::rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    }
    id
}

#[test]
fn gallery() {
    let Ok(dir) = std::env::var("DESIGNOS_BOARD_SHOTS") else { return };
    let mut t = T::new();
    t.doc.get_mut(t.page).unwrap().kind = NodeKind::Page { background: Color::rgb(0xF4, 0xF4, 0xF4) };
    let colors = [0xFFE58F, 0xFFC48A, 0xFFB3B3, 0xD5C3FF];
    let long = "A much longer note about everything we want to try during the next sprint, with plenty of words to force the text to shrink.";
    let texts = ["Ship the board mode", "Short", long, "Idea"];
    let mut stickies = Vec::new();
    for i in 0..4 {
        stickies.push(sticky(&mut t, Rect::new(40.0 + i as f32 * 300.0, 40.0, 240.0, 240.0), texts[i], colors[i]));
    }
    let shape = t.add(None, |id| Node::shape_with_text(id, Rect::new(340.0, 380.0, 200.0, 120.0), ShapeKind::Diamond, "Decide"));
    let shape2 = t.add(None, |id| Node::shape_with_text(id, Rect::new(640.0, 380.0, 200.0, 120.0), ShapeKind::RoundedRect, "Do it"));
    let mk = |t: &mut T, a: NodeId, b: NodeId, line: ConnectorLine, ma: Magnet, mb: Magnet, label: &str| {
        let c = t.add(None, |id| {
            let mut p = ConnectorProps { start: ConnectorEnd::attached(a, ma), end: ConnectorEnd::attached(b, mb), line, ..Default::default() };
            p.end_cap = ConnectorCap::Triangle;
            if !label.is_empty() {
                let mut l = board_text(14.0, TextAlignH::Center, TextAlignV::Center);
                l.content = label.into();
                p.label = Some(l);
            }
            Node::connector(id, p)
        });
        sync_connector(&mut t.doc, c);
    };
    mk(&mut t, stickies[0], shape, ConnectorLine::Elbowed, Magnet::Bottom, Magnet::Top, "");
    mk(&mut t, shape, shape2, ConnectorLine::Straight, Magnet::Right, Magnet::Left, "yes");
    mk(&mut t, stickies[3], shape2, ConnectorLine::Curved, Magnet::Bottom, Magnet::Right, "");
    for (i, k) in STAMP_KINDS.iter().enumerate() {
        t.add(None, |id| Node::stamp(id, Vec2::new(40.0 + i as f32 * 80.0, 540.0), k, 64.0));
    }
    let pm = t.render(1240, 640);
    pm.save_png(std::path::Path::new(&dir).join("board_gallery.png")).unwrap();
}
