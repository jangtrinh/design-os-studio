//! Pixel tests of the board kinds (stickies, connectors, shapes with text, stamps) on the CPU renderer.

use crate::common;

use common::*;
use designos_doc::*;
use designos_render::{render_node, tiny_skia::Pixmap};

fn dark(p: [u8; 4]) -> bool {
    (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3 < 100
}

/// Number of dark pixels inside `r`.
fn ink(pm: &Pixmap, r: (u32, u32, u32, u32)) -> usize {
    let mut n = 0;
    for y in r.1..r.3 {
        for x in r.0..r.2 {
            if dark(px(pm, x, y)) {
                n += 1;
            }
        }
    }
    n
}

fn sticky(t: &mut T, r: Rect, text: &str, color: Color) -> NodeId {
    let id = t.add(None, |id| Node::sticky(id, r, text));
    if let NodeKind::Sticky(s) = &mut t.node(id).kind {
        s.color = color;
    }
    id
}

#[test]
fn sticky_is_a_rounded_colored_square_with_a_shadow() {
    let mut t = T::new();
    let c = Color::rgb(0xFF, 0xE5, 0x8F);
    sticky(&mut t, Rect::new(40.0, 40.0, 240.0, 240.0), "", c);
    let pm = t.render(360, 360);
    // fill
    assert!(near(px(&pm, 160, 160), [0xFF, 0xE5, 0x8F, 255], 2), "{:?}", px(&pm, 160, 160));
    // rounded corner: the very corner pixel is the page, a pixel a bit inside is the note
    assert!(px(&pm, 40, 40)[2] > 220, "the corner shows the page, not the note: {:?}", px(&pm, 40, 40));
    assert!(near(px(&pm, 52, 52), [0xFF, 0xE5, 0x8F, 255], 2));
    // soft shadow below the bottom edge, darker than the page and fading out
    let near_edge = px(&pm, 160, 284);
    let far = px(&pm, 160, 330);
    assert!(near_edge[0] < 250, "shadow right under the note: {near_edge:?}");
    assert!(far[0] >= near_edge[0], "shadow fades: {near_edge:?} -> {far:?}");
    // and a little above the top edge it is much lighter than below
    assert!(px(&pm, 160, 36)[0] > near_edge[0]);
}

#[test]
fn sticky_text_is_drawn_and_shrinks_as_it_grows() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(10.0, 10.0, 240.0, 240.0), "Hello", Color::WHITE);
    sticky(&mut t, Rect::new(270.0, 10.0, 240.0, 240.0), &"lorem ipsum dolor sit amet ".repeat(12), Color::WHITE);
    let pm = t.render(530, 270);
    let box_a = (10 + 16, 10 + 16, 10 + 240 - 16, 10 + 240 - 16);
    let box_b = (270 + 16, 10 + 16, 270 + 240 - 16, 10 + 240 - 16);
    assert!(ink(&pm, box_a) > 150, "short text draws big strokes: {}", ink(&pm, box_a));
    assert!(ink(&pm, box_b) > 400, "long text still draws");
    // the long text stays inside the note: nothing dark below the note body
    let below = (270, 10 + 240 + 12, 270 + 240, 268);
    assert_eq!(ink(&pm, below), 0, "text does not spill out of the note");
    // sticky nodes are visible in a node render too
    let pm2 = render_node(&t.doc, a, 1.0).unwrap();
    assert_eq!((pm2.width(), pm2.height()), (240, 240));
}

#[test]
fn author_line_sits_at_the_bottom() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(0.0, 0.0, 240.0, 240.0), "", Color::WHITE);
    if let NodeKind::Sticky(s) = &mut t.node(a).kind {
        s.author = Some("Kir Kir Kir".into());
        s.show_author = true;
    }
    let pm = t.render(240, 240);
    assert_eq!(ink(&pm, (0, 0, 240, 190)), 0);
    let mut faint = 0;
    for y in 200..232 {
        for x in 10..140 {
            if px(&pm, x, y)[0] < 200 {
                faint += 1;
            }
        }
    }
    assert!(faint > 20, "author name drawn in a softer ink: {faint}");
}

#[test]
fn connector_draws_line_caps_and_label() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(20.0, 20.0, 100.0, 100.0), "", Color::rgb(0xB6, 0xD4, 0xFF));
    let b = sticky(&mut t, Rect::new(300.0, 20.0, 100.0, 100.0), "", Color::rgb(0xC4, 0xEB, 0xA6));
    let c = t.add(None, |id| {
        let mut p = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        p.end_cap = ConnectorCap::Triangle;
        p.start_cap = ConnectorCap::Circle;
        Node::connector(id, p)
    });
    sync_connector(&mut t.doc, c);
    let pm = t.render(420, 140);
    // the horizontal line at y = 70 between the notes
    assert!(dark(px(&pm, 200, 70)), "line {:?}", px(&pm, 200, 70));
    assert_eq!(px(&pm, 200, 60), WHITE);
    // the triangle cap just before the target is wider than the line
    assert!(dark(px(&pm, 289, 65)) && dark(px(&pm, 289, 75)), "triangle cap");
    // the circle cap at the source
    assert!(dark(px(&pm, 125, 66)) && dark(px(&pm, 125, 74)), "circle cap");

    // a label sits over the line on a white background
    if let NodeKind::Connector(p) = &mut t.node(c).kind {
        let mut l = board_text(16.0, TextAlignH::Center, TextAlignV::Center);
        l.content = "next".into();
        p.label = Some(l);
    }
    sync_connector(&mut t.doc, c);
    let pm = t.render(420, 140);
    assert_eq!(px(&pm, 190, 70), WHITE, "the line is hidden behind the label background");
    assert!(dark(px(&pm, 170, 70)), "and visible before it");
    assert!(ink(&pm, (190, 55, 235, 85)) > 15, "label text");
    assert!(dark(px(&pm, 260, 70)), "line continues past the label");
}

#[test]
fn elbow_connector_runs_orthogonally_between_offset_notes() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(20.0, 20.0, 80.0, 80.0), "", Color::rgb(0xFF, 0xE5, 0x8F));
    let b = sticky(&mut t, Rect::new(260.0, 200.0, 80.0, 80.0), "", Color::rgb(0xFF, 0xE5, 0x8F));
    let c = t.add(None, |id| {
        let p = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        Node::connector(id, p)
    });
    sync_connector(&mut t.doc, c);
    let pm = t.render(360, 300);
    let route = connector_route(&t.doc, c).unwrap();
    assert!(route.pts.len() >= 3);
    // every vertex of the route is on a dark pixel, and the segment midpoints too
    for w in route.pts.windows(2) {
        let m = ((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
        assert!(dark(px(&pm, m.0 as u32, m.1 as u32)), "segment {:?} -> {:?}", w[0], w[1]);
    }
    // it does not cut through the source note
    assert!(!route.crosses_rect(Rect::new(20.0, 20.0, 80.0, 80.0)));
}

#[test]
fn connector_moves_with_its_node() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(20.0, 20.0, 80.0, 80.0), "", Color::WHITE);
    let b = sticky(&mut t, Rect::new(260.0, 20.0, 80.0, 80.0), "", Color::WHITE);
    let c = t.add(None, |id| {
        Node::connector(
            id,
            ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() },
        )
    });
    sync_connector(&mut t.doc, c);
    let before = t.render(360, 260);
    assert!(dark(px(&before, 180, 60)));
    t.node(b).transform = Affine::translate(260.0, 160.0);
    let touched = reroute_attached(&mut t.doc, &[b]);
    assert_eq!(touched, vec![c]);
    let after = t.render(360, 260);
    // the nodes are now offset: the line runs from a's right side to b's left side
    let route = connector_route(&t.doc, c).unwrap();
    assert_eq!(route.start(), Vec2::new(100.0, 60.0));
    assert_eq!(route.end(), Vec2::new(260.0, 200.0));
    assert!(dark(px(&after, 254, 200)), "the cap reaches the moved note");
    assert!(route_hits(&t.doc, c, 180.0, 60.0) || route_hits(&t.doc, c, 180.0, 200.0) || route_hits(&t.doc, c, 180.0, 130.0));
}

fn route_hits(doc: &Document, c: NodeId, x: f32, y: f32) -> bool {
    connector_route(doc, c).unwrap().distance_to(Vec2::new(x, y)) < 6.0
}

#[test]
fn shapes_with_text_draw_fill_outline_and_centered_text() {
    let mut t = T::new();
    let mut ids = Vec::new();
    for (i, k) in ShapeKind::ALL.iter().enumerate() {
        let r = Rect::new(10.0 + (i % 5) as f32 * 130.0, 10.0 + (i / 5) as f32 * 110.0, 120.0, 100.0);
        let id = t.add(None, |id| {
            let mut n = Node::shape_with_text(id, r, *k, "Hi");
            n.fills = vec![Paint::solid(Color::rgb(0xC4, 0xEB, 0xA6))];
            n
        });
        ids.push((id, r, *k));
    }
    let pm = t.render(670, 230);
    for (_, r, k) in &ids {
        let (cx, cy) = ((r.x + r.w / 2.0) as u32, (r.y + r.h / 2.0) as u32);
        // text ink around the center
        assert!(ink(&pm, (cx - 14, cy - 14, cx + 14, cy + 14)) > 10, "{k:?} text");
        // the fill is visible next to the text
        let fill = px(&pm, cx, cy - 30);
        let inside = matches!(k, ShapeKind::Triangle | ShapeKind::TriangleDown | ShapeKind::Star | ShapeKind::Pentagon);
        if !inside {
            assert!(near(fill, [0xC4, 0xEB, 0xA6, 255], 3), "{k:?} fill {fill:?}");
        }
        // the page outside is untouched
        if !matches!(k, ShapeKind::Square | ShapeKind::TriangleDown) {
            assert_eq!(px(&pm, r.x as u32 + 1, r.y as u32 + 1), WHITE, "{k:?} corner");
        }
    }
    // a diamond's corners are empty, its middle is filled
    let (_, dr, _) = ids.iter().find(|(_, _, k)| *k == ShapeKind::Diamond).unwrap();
    assert_eq!(px(&pm, dr.x as u32 + 8, dr.y as u32 + 8), WHITE);
}

#[test]
fn stamps_draw_badges_with_white_glyphs() {
    let mut t = T::new();
    for (i, k) in STAMP_KINDS.iter().enumerate() {
        t.add(None, |id| Node::stamp(id, Vec2::new(10.0 + i as f32 * 90.0, 10.0), k, 80.0));
    }
    let pm = t.render(470, 100);
    for (i, k) in STAMP_KINDS.iter().enumerate() {
        let x0 = 10 + i as u32 * 90;
        let layers = stamp_layers(k);
        let bg = layers[0].fill.unwrap();
        let want = [(bg.r * 255.0) as u8, (bg.g * 255.0) as u8, (bg.b * 255.0) as u8, 255];
        // badge colour near the rim, white glyph pixels in the middle
        assert!(near(px(&pm, x0 + 40, 14), want, 3), "{k}: badge at the rim {:?} want {:?}", px(&pm, x0 + 40, 14), want);
        let mut white = 0;
        for y in 25..75 {
            for x in x0 + 15..x0 + 65 {
                if px(&pm, x, y)[0] > 240 && px(&pm, x, y)[1] > 240 {
                    white += 1;
                }
            }
        }
        assert!(white > 120, "{k}: glyph pixels {white}");
        // outside the round badge the page shows
        assert_eq!(px(&pm, x0 + 1, 11), WHITE, "{k} corner");
    }
}

#[test]
fn board_items_respect_opacity_and_visibility() {
    let mut t = T::new();
    let a = sticky(&mut t, Rect::new(20.0, 20.0, 100.0, 100.0), "x", Color::rgb(0, 0, 0));
    t.node(a).opacity = 0.5;
    let pm = t.render(160, 160);
    let p = px(&pm, 70, 90);
    assert!(near(p, [128, 128, 128, 255], 40), "half-transparent black on white: {p:?}");
    t.node(a).visible = false;
    let pm = t.render(160, 160);
    assert_eq!(px(&pm, 70, 90), WHITE);
}
