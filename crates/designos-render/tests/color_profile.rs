//! A Display P3 document converts its colours to sRGB when drawn for the screen; an sRGB document is
//! drawn exactly as stored; a native render keeps the stored numbers (for a P3-tagged export).

mod common;

use common::*;
use designos_doc::*;
use designos_render::{render_node, render_node_native, render_page_native, tiny_skia::Pixmap, Viewport};

/// color(display-p3 0.5 0.4 0.8) in sRGB, from the conversion matrix derived from the CSS Color 4 primaries.
const P3_MID: Color = Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };
const P3_MID_AS_SRGB: [u8; 4] = [132, 101, 211, 255];
const P3_MID_STORED: [u8; 4] = [128, 102, 204, 255];

fn scene(profile: ColorProfile, fill: Color) -> T {
    let mut t = T::new();
    t.doc.color_profile = profile;
    t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), fill);
    t
}

#[test]
fn a_display_p3_fill_is_converted_to_srgb() {
    let pm = scene(ColorProfile::DisplayP3, P3_MID).render(40, 40);
    assert!(near(px(&pm, 20, 20), P3_MID_AS_SRGB, 1), "{:?}", px(&pm, 20, 20));
}

#[test]
fn a_native_render_keeps_the_stored_numbers() {
    let t = scene(ColorProfile::DisplayP3, P3_MID);
    let mut pm = Pixmap::new(40, 40).unwrap();
    render_page_native(&t.doc, t.page, &Viewport::default(), &mut pm);
    assert!(near(px(&pm, 20, 20), P3_MID_STORED, 1), "{:?}", px(&pm, 20, 20));
}

#[test]
fn a_native_node_render_keeps_the_stored_numbers_and_the_default_converts() {
    let t = scene(ColorProfile::DisplayP3, P3_MID);
    let rect = t.doc.get(t.page).unwrap().children[0];
    let native = render_node_native(&t.doc, rect, 1.0).unwrap();
    let screen = render_node(&t.doc, rect, 1.0).unwrap();
    assert!(near(px(&native, 20, 20), P3_MID_STORED, 1), "{:?}", px(&native, 20, 20));
    assert!(near(px(&screen, 20, 20), P3_MID_AS_SRGB, 1), "{:?}", px(&screen, 20, 20));
}

#[test]
fn an_srgb_document_is_drawn_exactly_as_stored() {
    let c = Color::rgb(200, 100, 50);
    let pm = scene(ColorProfile::Srgb, c).render(40, 40);
    assert_eq!(px(&pm, 20, 20), [200, 100, 50, 255]);
    let t = scene(ColorProfile::Srgb, c);
    let mut native = Pixmap::new(40, 40).unwrap();
    render_page_native(&t.doc, t.page, &Viewport::default(), &mut native);
    assert_eq!(native.data(), pm.data(), "native and screen renders of an sRGB document are the same bytes");
}

#[test]
fn only_the_profile_changes_the_pixels() {
    // The same stored numbers drawn as sRGB and as Display P3 differ; switching the profile back restores them.
    let srgb = scene(ColorProfile::Srgb, P3_MID).render(40, 40);
    let p3 = scene(ColorProfile::DisplayP3, P3_MID).render(40, 40);
    assert_ne!(srgb.data(), p3.data());
    assert_eq!(px(&srgb, 20, 20), P3_MID_STORED);
}

#[test]
fn page_background_gradient_and_shadow_colours_convert_too() {
    let mut t = T::new();
    t.doc.color_profile = ColorProfile::DisplayP3;
    t.node(t.page).kind = NodeKind::Page { background: P3_MID };
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 40.0, 40.0));
        n.fills = vec![paint(PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![stop(0.0, P3_MID), stop(1.0, P3_MID)],
        })];
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(70.0, 10.0, 20.0, 20.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.effects = vec![Effect::DropShadow {
            color: P3_MID,
            offset: Vec2::new(0.0, 40.0),
            radius: 0.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: BlendMode::Normal,
        }];
        n
    });
    let pm = t.render(120, 120);
    assert!(near(px(&pm, 5, 5), P3_MID_AS_SRGB, 1), "page background {:?}", px(&pm, 5, 5));
    assert!(near(px(&pm, 30, 30), P3_MID_AS_SRGB, 1), "gradient stops {:?}", px(&pm, 30, 30));
    assert!(near(px(&pm, 80, 60), P3_MID_AS_SRGB, 1), "drop shadow {:?}", px(&pm, 80, 60));
}
