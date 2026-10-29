mod common;
use common::*;
use designos_doc::*;

const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

/// White page with a 10px black stripe at x 10..20 and a clear (alpha 0) pane
/// over x 0..40 that blurs its backdrop. `wrap` decides where the pane lives.
fn scene(wrap: impl FnOnce(&mut T) -> NodeId) -> Pixmap {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let parent = wrap(&mut t);
    let pane = t.add(Some(parent), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::TRANSPARENT)];
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let _ = pane;
    t.render(60, 40)
}

use designos_render::tiny_skia::Pixmap;

#[test]
fn background_blur_at_top_level_blurs_page_content() {
    let pm = scene(|t| t.page);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 40 && c[0] < 200, "blurred stripe centre: {c:?}");
    assert_eq!(px(&pm, 55, 20), WHITE);
}

#[test]
fn background_blur_inside_opacity_group_sees_parent_canvas() {
    let pm = scene(|t| {
        let g = t.add(None, |id| Node::new(id, NodeKind::Group));
        t.node(g).opacity = 0.9;
        g
    });
    let c = px(&pm, 15, 20);
    // without a backdrop the group layer is empty and the stripe stays solid black
    assert!(c[0] > 40, "stripe must be blurred inside the group: {c:?}");
    assert!(c[0] < 215, "but still visible: {c:?}");
    assert_eq!(px(&pm, 55, 20), WHITE);
}

#[test]
fn background_blur_in_nested_groups_and_sees_layer_siblings() {
    let mut t = T::new();
    let outer = t.add(None, |id| Node::new(id, NodeKind::Group));
    t.node(outer).opacity = 0.8;
    // the stripe lives in the same group, below the blurred pane
    t.rect(Some(outer), Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let inner = t.add(Some(outer), |id| Node::new(id, NodeKind::Group));
    t.node(inner).opacity = 0.9;
    t.add(Some(inner), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::TRANSPARENT)];
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 60, "stripe blurred through two layers: {c:?}");
    assert!(c[0] < 215, "{c:?}");
}

#[test]
fn background_blur_on_translucent_node_itself() {
    // the blurred node has its own opacity (so it is drawn in a layer)
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::rgba(255, 255, 255, 40))];
        n.opacity = 0.8;
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 60, "{c:?}");
}

#[test]
fn background_blur_without_a_visible_fill_leaves_backdrop_sharp() {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills.clear();
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    assert_eq!(px(&pm, 15, 20), [0, 0, 0, 255], "a fill-less backdrop blur must not alter the backdrop");
    assert_eq!(px(&pm, 25, 20), WHITE, "the area outside the stripe remains unchanged");
}
