//! Export bounds follow the rules measured in Figma: a shadow grows around the shape a frame paints
//! together with the shapes its children paint, a layer that paints nothing adds no box, and a frame
//! always adds its own.

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
