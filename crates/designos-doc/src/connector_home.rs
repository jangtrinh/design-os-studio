//! Connectable-layer eligibility, home-container resolution and default ink for connectors.

use crate::lint::rules::contrast::contrast_ratio;
use crate::{Color, Document, DocumentKind, Node, NodeId, NodeKind, PaintKind};

fn strict_ancestors(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut cur = doc.get(id).and_then(|n| n.parent);
    let limit = doc.nodes.len();
    while let Some(pid) = cur {
        if out.len() >= limit {
            break;
        }
        out.push(pid);
        cur = doc.get(pid).and_then(|n| n.parent);
    }
    out
}

fn is_inside_component_or_instance(doc: &Document, id: NodeId) -> bool {
    let mut cur = Some(id);
    let mut steps = 0usize;
    let limit = doc.nodes.len();
    while let Some(cid) = cur {
        if steps >= limit {
            return true;
        }
        steps += 1;
        let Some(node) = doc.get(cid) else {
            break;
        };
        if matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) {
            return true;
        }
        cur = node.parent;
    }
    false
}

/// Whether `id` is a valid attached endpoint for a connector.
///
/// A layer is connectable when it exists in `doc`, is visible and unlocked, has a kind other than
/// `Page`, `Connector`, or `Unknown`, and has no `Instance`, `Component`, or `ComponentSet`
/// strict ancestor (a top-level `Instance`, `Component`, or `ComponentSet` is itself connectable).
pub fn connectable_node(doc: &Document, id: NodeId) -> bool {
    let Some(node) = doc.get(id) else {
        return false;
    };
    if !node.visible || node.locked {
        return false;
    }
    if matches!(node.kind, NodeKind::Page { .. } | NodeKind::Connector(_) | NodeKind::Unknown { .. }) {
        return false;
    }
    for anc in strict_ancestors(doc, id) {
        let Some(parent) = doc.get(anc) else {
            continue;
        };
        if matches!(parent.kind, NodeKind::Instance { .. } | NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
            return false;
        }
    }
    true
}

/// Resolves the container that should own a connector joining `a` and `b` on `page`.
///
/// Starts at the nearest common ancestor strictly above both attached ends (or the single
/// attached end's parent, or `page` when neither end is attached), climbs past `Group` and
/// `BooleanOp` containers, climbs out of any `Component`, `ComponentSet`, or `Instance` subtree,
/// and stops at the first `Frame`, `Section`, or `Page` (falling back to `page` when the ends
/// have no common container on the same page).
pub fn connector_home(doc: &Document, a: Option<NodeId>, b: Option<NodeId>, page: NodeId) -> NodeId {
    let a = a.filter(|&id| doc.get(id).is_some());
    let b = b.filter(|&id| doc.get(id).is_some());
    let start = match (a, b) {
        (Some(a_id), Some(b_id)) => {
            let anc_a = strict_ancestors(doc, a_id);
            let anc_b = strict_ancestors(doc, b_id);
            anc_a.into_iter().find(|id| anc_b.contains(id))
        }
        (Some(id), None) | (None, Some(id)) => doc.get(id).and_then(|n| n.parent),
        (None, None) => None,
    };
    let mut cur = start;
    let mut steps = 0usize;
    let limit = doc.nodes.len();
    while let Some(cid) = cur {
        if steps >= limit {
            break;
        }
        steps += 1;
        let Some(node) = doc.get(cid) else {
            break;
        };
        if matches!(node.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Page { .. }) && !is_inside_component_or_instance(doc, cid) {
            return cid;
        }
        cur = node.parent;
    }
    page
}

/// Contrast a connector needs against the surface it is drawn on.
const CONNECTOR_INK_MIN_CONTRAST: f64 = 3.0;

/// A fill must keep at least this much of its colour (colour alpha x paint opacity) to count as
/// the surface behind a connector. The layer opacity of the layer that holds the fill is not
/// counted: the connector sits inside that layer, so it fades the line and the fill together.
const SURFACE_MIN_ALPHA: f32 = 0.5;

/// Default ink of a connector on a light surface: the ink [`Node::connector`] is created with.
fn connector_ink_on_light() -> Color {
    Color::rgb(0x1E, 0x1E, 0x1E)
}

/// The colour `n` paints as a surface: its topmost visible solid fill that keeps at least
/// [`SURFACE_MIN_ALPHA`] of its colour. Images, gradients and faint fills are not a surface.
fn opaque_solid_fill(n: &Node) -> Option<Color> {
    n.fills.iter().rev().filter(|p| p.visible).find_map(|p| match p.kind {
        PaintKind::Solid(c) if c.a * p.opacity >= SURFACE_MIN_ALPHA => Some(c),
        _ => None,
    })
}

/// The surface a connector homed in `home` is drawn on: the nearest ancestor of the connector,
/// from `home` upward, with a mostly opaque solid fill, else the page background.
fn surface_behind(doc: &Document, home: NodeId) -> Option<Color> {
    let mut cur = Some(home);
    for _ in 0..=doc.nodes.len() {
        let n = doc.get(cur?)?;
        if let NodeKind::Page { background } = &n.kind {
            return Some(*background);
        }
        if let Some(c) = opaque_solid_fill(n) {
            return Some(c);
        }
        cur = n.parent;
    }
    None
}

/// The ink of a new connector homed in `home` when nobody picked one: the dark board ink, or
/// white when that ink would fall below 3:1 contrast against the surface behind the connector (a
/// dark page, or a dark screen). See [`connector_ink_on`].
pub fn default_connector_ink(doc: &Document, home: NodeId) -> Color {
    connector_ink_on(doc, home, connector_ink_on_light(), Color::WHITE)
}

/// Picks `on_light`, or `on_dark` when `on_light` would fall below 3:1 contrast against the
/// surface a connector homed in `home` is drawn on: the nearest ancestor of the connector, from
/// `home` upward, with a mostly opaque solid fill, else the page background. A board file is drawn
/// on its own light canvas, so it always gets `on_light`.
pub fn connector_ink_on(doc: &Document, home: NodeId, on_light: Color, on_dark: Color) -> Color {
    if doc.kind == DocumentKind::Board {
        return on_light;
    }
    match surface_behind(doc, home) {
        Some(surface) if contrast_ratio(on_light, surface) < CONNECTOR_INK_MIN_CONTRAST => on_dark,
        _ => on_light,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BooleanOp, ConnectorProps, FrameProps, Node, NodeKind, Paint, Rect};

    fn add_child(doc: &mut Document, parent: NodeId, node: Node) -> NodeId {
        let id = node.id;
        doc.insert(parent, node).expect("insert child");
        id
    }

    fn make_frame(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 200.0));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_rect(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 60.0, 40.0));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_group(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Group);
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_boolean_op(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_section(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Section);
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_component(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Component(FrameProps::default()));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_component_set(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::ComponentSet(FrameProps::default()));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_instance(doc: &mut Document, parent: NodeId, component: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Instance { frame: FrameProps::default(), component: Some(component), component_key: None });
        n.name = name.into();
        add_child(doc, parent, n)
    }

    #[test]
    fn connectable_node_rejects_missing_id_and_accepts_standard_shapes() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let rect = make_rect(&mut doc, page, "Box");
        let frame = make_frame(&mut doc, page, "Card");

        assert!(!connectable_node(&doc, NodeId(999_999)), "missing node must not be connectable");
        assert!(connectable_node(&doc, rect), "visible unlocked rectangle on page is connectable");
        assert!(connectable_node(&doc, frame), "visible unlocked frame on page is connectable");
    }

    #[test]
    fn connectable_node_rejects_page_connector_and_unknown_kinds() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let rect = make_rect(&mut doc, page, "Box");

        let conn_id = doc.alloc_id();
        let conn = add_child(&mut doc, page, Node::connector(conn_id, ConnectorProps::default()));

        let unk_id = doc.alloc_id();
        let unk = add_child(&mut doc, page, Node::new(unk_id, NodeKind::Unknown { type_name: "CUSTOM".into() }));

        assert!(connectable_node(&doc, rect), "rectangle baseline must be connectable");
        assert!(!connectable_node(&doc, page), "Page is not connectable");
        assert!(!connectable_node(&doc, conn), "Connector is not connectable");
        assert!(!connectable_node(&doc, unk), "Unknown kind is not connectable");
    }

    #[test]
    fn connectable_node_allows_top_level_component_set_component_and_instance_but_rejects_their_descendants() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");

        let comp = make_component(&mut doc, page, "Button");
        let inside_comp = make_rect(&mut doc, comp, "Bg");

        let comp_set = make_component_set(&mut doc, page, "ButtonSet");
        let variant_comp = make_component(&mut doc, comp_set, "Primary");
        let inside_variant = make_rect(&mut doc, variant_comp, "Label");

        let inst = make_instance(&mut doc, page, comp, "ButtonInst");
        let inside_inst = make_rect(&mut doc, inst, "InstChild");

        assert!(connectable_node(&doc, comp), "top-level Component is connectable");
        assert!(connectable_node(&doc, comp_set), "top-level ComponentSet is connectable");
        assert!(connectable_node(&doc, inst), "top-level Instance is connectable");

        assert!(!connectable_node(&doc, inside_comp), "descendant of Component is not connectable");
        assert!(!connectable_node(&doc, variant_comp), "Component inside ComponentSet has ComponentSet ancestor and is not connectable");
        assert!(!connectable_node(&doc, inside_variant), "descendant inside ComponentSet is not connectable");
        assert!(!connectable_node(&doc, inside_inst), "descendant of Instance is not connectable");
    }

    #[test]
    fn connectable_node_rejects_hidden_or_locked_nodes() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let visible_unlocked = make_rect(&mut doc, page, "Open");
        let hidden = make_rect(&mut doc, page, "Hidden");
        doc.get_mut(hidden).unwrap().visible = false;

        let locked = make_rect(&mut doc, page, "Locked");
        doc.get_mut(locked).unwrap().locked = true;

        assert!(connectable_node(&doc, visible_unlocked), "visible unlocked node must be connectable");
        assert!(!connectable_node(&doc, hidden), "hidden node is not connectable");
        assert!(!connectable_node(&doc, locked), "locked node is not connectable");
    }

    #[test]
    fn connector_home_picks_nearest_common_frame_or_section_or_page() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let section = make_section(&mut doc, page, "FlowSection");
        let outer_frame = make_frame(&mut doc, section, "Board");
        let a = make_rect(&mut doc, outer_frame, "A");
        let b = make_rect(&mut doc, outer_frame, "B");
        let in_section = make_rect(&mut doc, section, "C");
        let on_page = make_rect(&mut doc, page, "D");

        assert_eq!(connector_home(&doc, Some(a), Some(b), page), outer_frame);
        assert_eq!(connector_home(&doc, Some(a), Some(in_section), page), section);
        assert_eq!(connector_home(&doc, Some(a), Some(on_page), page), page);
    }

    #[test]
    fn connector_home_is_strictly_above_both_ends_so_an_end_is_never_its_own_home() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let outer = make_frame(&mut doc, page, "Outer");
        let inner = make_frame(&mut doc, outer, "Inner");
        let child = make_rect(&mut doc, inner, "Child");

        // `inner` is an ancestor of `child`; the connector cannot live inside `inner`.
        assert_eq!(connector_home(&doc, Some(inner), Some(child), page), outer);
        assert_eq!(connector_home(&doc, Some(child), Some(inner), page), outer);
        // `outer` lives directly on `page`, so connecting `outer` to `child` homes on `page`.
        assert_eq!(connector_home(&doc, Some(outer), Some(child), page), page);
    }

    #[test]
    fn connector_home_climbs_past_group_and_boolean_op() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page, "HostFrame");
        let group = make_group(&mut doc, frame, "G");
        let bool_op = make_boolean_op(&mut doc, group, "Union");
        let a = make_rect(&mut doc, group, "A");
        let b = make_rect(&mut doc, group, "B");
        let c = make_rect(&mut doc, bool_op, "C");
        let d = make_rect(&mut doc, bool_op, "D");

        assert_eq!(connector_home(&doc, Some(a), Some(b), page), frame, "climbs past Group to enclosing Frame");
        assert_eq!(connector_home(&doc, Some(c), Some(d), page), frame, "climbs past BooleanOp and Group to enclosing Frame");
    }

    #[test]
    fn connector_home_climbs_out_of_component_component_set_and_instance() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page, "CanvasFrame");

        let comp = make_component(&mut doc, frame, "Comp");
        let comp_inner_frame = make_frame(&mut doc, comp, "CompInner");
        let ca = make_rect(&mut doc, comp_inner_frame, "CA");
        let cb = make_rect(&mut doc, comp_inner_frame, "CB");

        let comp_set = make_component_set(&mut doc, frame, "Set");
        let v1 = make_component(&mut doc, comp_set, "V1");
        let v2 = make_component(&mut doc, comp_set, "V2");

        let inst = make_instance(&mut doc, frame, comp, "Inst");
        let inst_inner = make_frame(&mut doc, inst, "InstInner");
        let ia = make_rect(&mut doc, inst_inner, "IA");
        let ib = make_rect(&mut doc, inst_inner, "IB");

        assert_eq!(connector_home(&doc, Some(ca), Some(cb), page), frame, "climbs out of Component");
        assert_eq!(connector_home(&doc, Some(v1), Some(v2), page), frame, "climbs out of ComponentSet");
        assert_eq!(connector_home(&doc, Some(ia), Some(ib), page), frame, "climbs out of Instance");
    }

    #[test]
    fn connector_home_handles_single_attached_end_both_free_and_cross_page_fallback() {
        let mut doc = Document::new("Test");
        let page1 = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page1, "Frame1");
        let group = make_group(&mut doc, frame, "Group1");
        let a = make_rect(&mut doc, group, "A");

        let page2 = doc.add_page("Page 2");
        let b_other_page = make_rect(&mut doc, page2, "B");

        // One attached end uses its parent (and climbs past Group to Frame1).
        assert_eq!(connector_home(&doc, Some(a), None, page1), frame);
        assert_eq!(connector_home(&doc, None, Some(a), page1), frame);
        // Both free ends use the given `page`.
        assert_eq!(connector_home(&doc, None, None, page1), page1);
        assert_eq!(connector_home(&doc, None, None, page2), page2);
        // Ends on different pages have no common ancestor and fall back to `page`.
        assert_eq!(connector_home(&doc, Some(a), Some(b_other_page), page1), page1);
    }
    fn set_page_background(doc: &mut Document, page: NodeId, background: Color) {
        doc.get_mut(page).expect("page").kind = NodeKind::Page { background };
    }

    fn make_screen_with_wrapper(doc: &mut Document, page: NodeId) -> (NodeId, NodeId) {
        let screen = make_frame(doc, page, "Screen");
        let wrapper = make_frame(doc, screen, "Wrapper");
        doc.get_mut(wrapper).expect("wrapper").fills.clear();
        (screen, wrapper)
    }

    #[test]
    fn connector_ink_is_white_on_the_dark_default_page_and_keeps_3_to_1() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let ink = default_connector_ink(&doc, page);
        assert_eq!(ink, Color::WHITE);
        assert!(contrast_ratio(ink, Color::rgb(0x5A, 0x5A, 0x5A)) >= 3.0);
    }

    #[test]
    fn connector_ink_is_dark_on_a_light_page() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        set_page_background(&mut doc, page, Color::rgb(0xF5, 0xF5, 0xF5));
        let ink = default_connector_ink(&doc, page);
        assert_eq!(ink, Color::rgb(0x1E, 0x1E, 0x1E));
        assert!(contrast_ratio(ink, Color::rgb(0xF5, 0xF5, 0xF5)) >= 3.0);
    }

    #[test]
    fn connector_ink_reads_the_nearest_opaque_ancestor_fill() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        let ink = Color::rgb(0x1E, 0x1E, 0x1E);
        assert_eq!(default_connector_ink(&doc, wrapper), ink, "a white screen above a fill-less wrapper on a dark page");
        assert_eq!(default_connector_ink(&doc, screen), ink);
        doc.get_mut(wrapper).expect("wrapper").fills = vec![Paint::solid(Color::BLACK)];
        assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "the nearest fill wins over the white screen above it");

        // a dark screen above a fill-less wrapper on a light page: the ancestor, not the page, is the surface
        set_page_background(&mut doc, page, Color::WHITE);
        let (night, night_wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(night).expect("screen").fills = vec![Paint::solid(Color::BLACK)];
        assert_eq!(default_connector_ink(&doc, night_wrapper), Color::WHITE);
    }

    #[test]
    fn connector_ink_ignores_faint_and_non_solid_fills() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, wrapper) = make_screen_with_wrapper(&mut doc, page);
        let dark = Color::rgb(0x1E, 0x1E, 0x1E);

        let mut faint_paint = Paint::solid(Color::BLACK);
        faint_paint.opacity = 0.2;
        doc.get_mut(wrapper).expect("wrapper").fills = vec![faint_paint];
        assert_eq!(default_connector_ink(&doc, wrapper), dark, "a 20% paint is not a surface");

        let mut hidden = Paint::solid(Color::BLACK);
        hidden.visible = false;
        doc.get_mut(wrapper).expect("wrapper").fills = vec![hidden];
        assert_eq!(default_connector_ink(&doc, wrapper), dark, "a hidden paint is not a surface");

        let gradient = Paint {
            kind: PaintKind::LinearGradient {
                handles: [crate::Vec2::new(0.0, 0.5), crate::Vec2::new(1.0, 0.5), crate::Vec2::new(0.0, 1.0)],
                stops: Vec::new(),
            },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        };
        doc.get_mut(wrapper).expect("wrapper").fills = vec![gradient];
        assert_eq!(default_connector_ink(&doc, wrapper), dark, "a gradient is not a solid surface");
    }

    #[test]
    fn connector_ink_gives_a_dark_wrapper_faded_by_its_layer_opacity_the_light_ink() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(wrapper).expect("wrapper").fills = vec![Paint::solid(Color::BLACK)];
        doc.get_mut(wrapper).expect("wrapper").opacity = 0.3;
        assert_eq!(
            default_connector_ink(&doc, wrapper),
            Color::WHITE,
            "the connector sits inside the faded wrapper, so the wrapper's fill is the surface it is drawn on"
        );
    }

    #[test]
    fn connector_ink_reads_the_topmost_solid_fill() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let screen = make_frame(&mut doc, page, "Screen");
        doc.get_mut(screen).expect("screen").fills = vec![Paint::solid(Color::WHITE), Paint::solid(Color::BLACK)];
        assert_eq!(default_connector_ink(&doc, screen), Color::WHITE, "black is painted over white");
    }

    #[test]
    fn connector_ink_in_a_board_file_stays_dark_on_any_page_colour() {
        let mut doc = Document::new("Board");
        doc.kind = DocumentKind::Board;
        let page = doc.add_page("Page 1");
        assert_eq!(default_connector_ink(&doc, page), Color::rgb(0x1E, 0x1E, 0x1E), "a board is drawn on its own light canvas");
    }

    #[test]
    fn connector_ink_on_picks_between_the_inks_the_caller_passes() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (light_ink, dark_ink) = (Color::rgb(0x10, 0x20, 0x30), Color::rgb(0xF0, 0xE0, 0xD0));
        assert_eq!(connector_ink_on(&doc, page, light_ink, dark_ink), dark_ink, "the near-black ink is below 3:1 on the dark default page");
        set_page_background(&mut doc, page, Color::WHITE);
        assert_eq!(connector_ink_on(&doc, page, light_ink, dark_ink), light_ink);
    }
}
