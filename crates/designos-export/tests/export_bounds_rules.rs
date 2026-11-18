//! Export bounds follow the rules measured in Figma: each stroke side reaches by its own weight, and a
//! frame's shadow grows around the shape it paints together with the shapes its children paint, cut
//! to the frame when it clips, but not around the children's own shadows.

use designos_doc::{Color, Document, Effect, Node, NodeId, NodeKind, Paint, Rect, StrokeAlign, Vec2};
use designos_export::{export_png, export_svg, render_bounds, SvgOptions};

fn drop_shadow(x: f32, y: f32, blur: f32, spread: f32) -> Effect {
    Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(x, y), radius: blur, spread, visible: true, show_behind: false, blend: Default::default() }
}

fn png_size(d: &Document, id: NodeId) -> (u32, u32) {
    let img = image::load_from_memory(&export_png(d, id, 1.0)).expect("decode exported png");
    (img.width(), img.height())
}

fn size(b: Rect) -> (f32, f32) {
    (b.w, b.h)
}

/// A 200x100 frame at the origin with an 80x80 child at (60, 60) that overflows its bottom edge by 40.
/// The frame's shadow is offset 0,8 with blur 16, as measured in Figma.
struct OverflowFrame {
    clip: bool,
    fill: bool,
    frame_shadow: bool,
    child_shadow: Option<Effect>,
}

impl OverflowFrame {
    fn new() -> Self {
        Self { clip: false, fill: true, frame_shadow: true, child_shadow: None }
    }

    fn build(&self) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.clips_content = self.clip;
        }
        if !self.fill {
            f.fills.clear();
        }
        if self.frame_shadow {
            f.effects.push(drop_shadow(0.0, 8.0, 16.0, 0.0));
        }
        let f = d.insert(page, f).unwrap();
        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(60.0, 60.0, 80.0, 80.0));
        c.effects.extend(self.child_shadow.clone());
        d.insert(f, c).unwrap();
        (d, f)
    }
}

#[test]
fn frame_shadow_wraps_the_children_it_does_not_clip() {
    // Figma: 232x172, the render box runs from -16 to 216 and from -8 to 164.
    let (d, f) = OverflowFrame::new().build();
    let b = render_bounds(&d, f);
    assert_eq!((b.x, b.y, b.w, b.h), (-16.0, -8.0, 232.0, 172.0));
    assert_eq!(png_size(&d, f), (232, 172));
    let svg = export_svg(&d, f, &SvgOptions::default());
    assert!(svg.contains("viewBox=\"-16 -8 232 172\""), "{svg}");
}

#[test]
fn clipping_frame_cuts_its_children_before_the_shadow_wraps_them() {
    // Figma: 232x132.
    let (d, f) = OverflowFrame { clip: true, ..OverflowFrame::new() }.build();
    assert_eq!(size(render_bounds(&d, f)), (232.0, 132.0));
    assert_eq!(png_size(&d, f), (232, 132));
}

#[test]
fn frame_without_fill_still_casts_its_shadow_around_its_children() {
    // Figma: 200x164. The frame paints nothing itself, so only the child gets the shadow.
    let (d, f) = OverflowFrame { fill: false, ..OverflowFrame::new() }.build();
    assert_eq!(size(render_bounds(&d, f)), (200.0, 164.0));
    assert_eq!(png_size(&d, f), (200, 164));
}

#[test]
fn clipping_frame_without_fill_casts_its_shadow_around_the_cut_children() {
    // Figma: 200x124. The frame paints nothing, so its shadow comes from the child cut to the frame's
    // box (60..140 by 60..100), grown by the shadow: down to 124, inside the box everywhere else.
    let (d, f) = OverflowFrame { clip: true, fill: false, ..OverflowFrame::new() }.build();
    let b = render_bounds(&d, f);
    assert_eq!((b.x, b.y, b.w, b.h), (0.0, 0.0, 200.0, 124.0));
    assert_eq!(png_size(&d, f), (200, 124));
}

#[test]
fn shadow_is_not_cast_around_the_childs_own_shadow() {
    // Figma: with a child shadow at 0,20 the export stays 232x172 and the bottom stays at 164: the
    // frame's shadow wraps the child's shape, not the child's shadow.
    let (d, f) = OverflowFrame { child_shadow: Some(drop_shadow(0.0, 20.0, 0.0, 0.0)), ..OverflowFrame::new() }.build();
    let b = render_bounds(&d, f);
    assert_eq!((b.y + b.h, size(b)), (164.0, (232.0, 172.0)));
}

#[test]
fn child_shadow_alone_still_extends_a_frame_without_shadow() {
    // Figma: 200x160. The child's shadow reaches 160 and the frame casts none.
    let (d, f) = OverflowFrame { frame_shadow: false, child_shadow: Some(drop_shadow(0.0, 20.0, 0.0, 0.0)), ..OverflowFrame::new() }.build();
    assert_eq!(size(render_bounds(&d, f)), (200.0, 160.0));
}

/// The kit's Context menu shape: a filled menu with an outside stroke and shadow 0,12 blur 16 spread -4,
/// holding a sub-menu placed 20 px below its bottom edge. `child_shadow` gives the sub-menu a shadow too.
fn menu_with_sub_menu(child_shadow: Option<Effect>) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let mid = d.alloc_id();
    let mut menu = Node::frame(mid, Rect::new(0.0, 0.0, 200.0, 100.0));
    menu.stroke.paints = vec![Paint::solid(Color::BLACK)];
    menu.stroke.align = StrokeAlign::Outside;
    menu.stroke.weight = 1.0;
    menu.effects.push(drop_shadow(0.0, 12.0, 16.0, -4.0));
    let menu = d.insert(page, menu).unwrap();
    let sid = d.alloc_id();
    let mut sub = Node::rectangle(sid, Rect::new(10.0, 60.0, 50.0, 60.0));
    sub.effects.extend(child_shadow);
    d.insert(menu, sub).unwrap();
    (d, menu)
}

#[test]
fn menu_shadow_wraps_an_absolutely_placed_sub_menu() {
    // the menu's own shape ends at 101 (stroke), its shadow at 101 + 24 = 125; the sub-menu ends at 120,
    // so its shape plus the menu's shadow reach 144.
    let (d, menu) = menu_with_sub_menu(None);
    let b = render_bounds(&d, menu);
    assert_eq!((b.x, b.y, b.w, b.h), (-13.0, -1.0, 226.0, 145.0));
    assert_eq!(png_size(&d, menu), (226, 145));
}

#[test]
fn menu_shadow_is_not_added_on_top_of_the_sub_menus_own_shadow() {
    // the sub-menu's own shadow (0,40 blur 40) reaches 120 + 80 = 200; the menu's shadow around the
    // sub-menu's shape only reaches 144, so the export ends at 200, not at 200 + 24.
    let (d, menu) = menu_with_sub_menu(Some(drop_shadow(0.0, 40.0, 40.0, 0.0)));
    let b = render_bounds(&d, menu);
    assert_eq!(b.y + b.h, 200.0);
}

#[test]
fn group_shadow_wraps_the_painted_shapes_of_its_children_not_their_effects() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 10.0, 10.0));
    g.kind = NodeKind::Group;
    g.fills.clear();
    g.effects.push(drop_shadow(0.0, 4.0, 8.0, 0.0));
    let g = d.insert(page, g).unwrap();
    let cid = d.alloc_id();
    let mut c = Node::rectangle(cid, Rect::new(20.0, 30.0, 40.0, 50.0));
    c.effects.push(drop_shadow(0.0, 100.0, 0.0, 0.0));
    d.insert(g, c).unwrap();
    // the child's own shadow reaches 80 + 100 = 180; the group's shadow around the child's shape reaches
    // 80 + 12 = 92, which does not add to it
    let b = render_bounds(&d, g);
    assert_eq!((b.x, b.y, b.w, b.h), (12.0, 26.0, 56.0, 154.0));
    // the SVG filter of the group follows the children, not the group's stored box
    let svg = export_svg(&d, g, &SvgOptions::default());
    assert!(svg.contains("<filter id=\"shadow0\" x=\"12\" y=\"26\" width=\"56\" height=\"66\""), "{svg}");
}

#[test]
fn svg_filter_region_of_a_frame_covers_the_shadow_around_its_children() {
    let (d, f) = OverflowFrame::new().build();
    let svg = export_svg(&d, f, &SvgOptions::default());
    assert!(svg.contains("<filter id=\"shadow0\" x=\"-16\" y=\"-8\" width=\"232\" height=\"172\""), "{svg}");
}

// ---- which layers add a box ------------------------------------------------------------------------

/// A 200x100 frame with no fill holding a visible 50x50 rectangle at (10, 10) and `child` placed at
/// (300, 0), the way the Figma probes were laid out. The frame is the exported layer.
fn holder_with(mut child: Node, clip: bool) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
    f.fills.clear();
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.clips_content = clip;
    }
    let f = d.insert(page, f).unwrap();
    let aid = d.alloc_id();
    d.insert(f, Node::rectangle(aid, Rect::new(10.0, 10.0, 50.0, 50.0))).unwrap();
    child.id = d.alloc_id();
    child.transform.tx = 300.0;
    child.transform.ty = 0.0;
    d.insert(f, child).unwrap();
    (d, f)
}

fn rect_40(id: u64) -> Node {
    Node::rectangle(NodeId(id), Rect::new(0.0, 0.0, 40.0, 40.0))
}

#[test]
fn a_layer_that_paints_nothing_adds_no_box() {
    let hidden_paint = {
        let mut n = rect_40(0);
        let mut p = Paint::solid(Color::rgb(255, 0, 0));
        p.visible = false;
        n.fills = vec![p];
        n
    };
    let transparent_fill = {
        let mut n = rect_40(0);
        n.fills = vec![Paint::solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 0.0 })];
        n
    };
    let no_paint = {
        let mut n = rect_40(0);
        n.fills.clear();
        n
    };
    let zero_opacity = {
        let mut n = rect_40(0);
        n.opacity = 0.0;
        n
    };
    let not_visible = {
        let mut n = rect_40(0);
        n.visible = false;
        n
    };
    let vector_without_paint = {
        let mut n = rect_40(0);
        n.kind = NodeKind::Vector { paths: Vec::new(), network: None };
        n.fills.clear();
        n
    };
    let empty_text = Node::text(NodeId(0), Rect::new(0.0, 0.0, 40.0, 40.0), "", Default::default());
    for (name, child) in [
        ("rectangle with no fill and no stroke", no_paint),
        ("fill with alpha 0", transparent_fill),
        ("fill paint that is not visible", hidden_paint),
        ("layer opacity 0", zero_opacity),
        ("layer that is not visible", not_visible),
        ("vector with no paint", vector_without_paint),
        ("empty text", empty_text),
    ] {
        let (d, f) = holder_with(child, false);
        assert_eq!(size(render_bounds(&d, f)), (200.0, 100.0), "{name} must not widen the frame");
    }
}

#[test]
fn a_painted_layer_adds_its_box() {
    let (d, f) = holder_with(rect_40(0), false);
    assert_eq!(size(render_bounds(&d, f)), (340.0, 100.0));
}

#[test]
fn a_group_of_unpainted_layers_adds_no_box() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
    f.fills.clear();
    let f = d.insert(page, f).unwrap();
    let gid = d.alloc_id();
    let mut g = Node::rectangle(gid, Rect::new(300.0, 0.0, 40.0, 40.0));
    g.kind = NodeKind::Group;
    g.fills.clear();
    let g = d.insert(f, g).unwrap();
    let rid = d.alloc_id();
    let mut r = rect_40(0);
    r.id = rid;
    r.fills.clear();
    d.insert(g, r).unwrap();
    assert_eq!(size(render_bounds(&d, f)), (200.0, 100.0));
}

#[test]
fn a_frame_always_adds_its_own_box_even_without_fill_and_children() {
    // Figma: an empty frame with no fill at x=300 widens its parent to 340, clipping or not.
    for clip in [false, true] {
        let mut empty = Node::frame(NodeId(0), Rect::new(0.0, 0.0, 40.0, 40.0));
        empty.fills.clear();
        if let NodeKind::Frame(fp) = &mut empty.kind {
            fp.clips_content = clip;
        }
        let (d, f) = holder_with(empty, false);
        assert_eq!(size(render_bounds(&d, f)), (340.0, 100.0), "empty frame, clip={clip}");
    }
}

#[test]
fn a_clipping_frame_adds_its_own_box_and_cuts_its_children_to_it() {
    // Figma: a 40x40 clipping frame at x=300 holding a painted rectangle at x=200 (outside the clip) is 340.
    let mut clipper = Node::frame(NodeId(0), Rect::new(0.0, 0.0, 40.0, 40.0));
    clipper.fills.clear();
    if let NodeKind::Frame(fp) = &mut clipper.kind {
        fp.clips_content = true;
    }
    let (mut d, f) = holder_with(clipper, false);
    let clipper_id = *d.get(f).unwrap().children.last().unwrap();
    let rid = d.alloc_id();
    let mut far = rect_40(0);
    far.id = rid;
    far.transform.tx = 200.0;
    far.transform.ty = 0.0;
    d.insert(clipper_id, far).unwrap();
    assert_eq!(size(render_bounds(&d, f)), (340.0, 100.0));
}

#[test]
fn the_exported_layer_keeps_its_box_even_when_it_paints_nothing() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let rid = d.alloc_id();
    let mut r = Node::rectangle(rid, Rect::new(5.0, 5.0, 200.0, 100.0));
    r.fills.clear();
    d.insert(page, r).unwrap();
    assert_eq!(render_bounds(&d, rid), Rect::new(0.0, 0.0, 200.0, 100.0));
    assert_eq!(png_size(&d, rid), (200, 100));
}

/// A 200x100 fill-less holder with a visible 50x50 rectangle, as above, plus a 100x100 child frame at
/// (300, 0) holding a painted 20x20 rectangle at (`child_x`, 10). The holder is the exported layer.
fn holder_with_frame(fill: bool, clip: bool, child: Option<f32>) -> (Document, NodeId) {
    let mut frame = Node::frame(NodeId(0), Rect::new(0.0, 0.0, 100.0, 100.0));
    if !fill {
        frame.fills.clear();
    }
    if let NodeKind::Frame(fp) = &mut frame.kind {
        fp.clips_content = clip;
    }
    let (mut d, holder) = holder_with(frame, false);
    let frame_id = *d.get(holder).unwrap().children.last().unwrap();
    if let Some(x) = child {
        let rid = d.alloc_id();
        d.insert(frame_id, Node::rectangle(rid, Rect::new(x, 10.0, 20.0, 20.0))).unwrap();
    }
    (d, holder)
}

#[test]
fn a_fill_less_frame_adds_its_full_box_whatever_it_holds() {
    // Figma, all measured at 400 wide: the frame ends at 300 + 100 although its painted child ends at 330.
    for (what, d_f) in [
        ("no fill, no clip, painted child inside", holder_with_frame(false, false, Some(10.0))),
        ("no fill, clip, painted child inside", holder_with_frame(false, true, Some(10.0))),
        ("fill, no clip, painted child inside", holder_with_frame(true, false, Some(10.0))),
        ("no fill, no clip, empty", holder_with_frame(false, false, None)),
        ("no fill, clip, child cut away", holder_with_frame(false, true, Some(150.0))),
    ] {
        let (d, f) = d_f;
        assert_eq!(size(render_bounds(&d, f)), (400.0, 100.0), "{what}");
        assert_eq!(png_size(&d, f), (400, 100), "{what}");
    }
}

#[test]
fn a_child_past_an_unclipped_frames_box_still_reaches_further() {
    // Figma: 470 wide, the child at 300 + 150 ends at 470.
    let (d, f) = holder_with_frame(false, false, Some(150.0));
    assert_eq!(size(render_bounds(&d, f)), (470.0, 100.0));
}
