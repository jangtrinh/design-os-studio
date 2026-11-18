//! Connectable-layer eligibility, home-container resolution and default ink for connectors.

use crate::lint::rules::contrast::contrast_ratio;
use crate::{resolve_node, BlendMode, Color, ColorProfile, Document, DocumentKind, GradientStop, NodeId, NodeKind, Paint, PaintKind};

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

/// Default ink of a connector on a light surface: the ink [`Node::connector`] is created with.
fn connector_ink_on_light() -> Color {
    Color::rgb(0x1E, 0x1E, 0x1E)
}

/// The mean colour of an image, asked of the caller by image reference: the colour of the decoded
/// pixels averaged and weighted by alpha, with `a` the mean alpha. `None` when the image cannot be
/// read, which makes it an unknown surface. designos-doc decodes no images itself.
pub type ImageMean<'a> = &'a dyn Fn(&str) -> Option<Color>;

/// A paint read as one flat colour laid over what is below it.
struct SurfaceLayer {
    color: Color,
    /// Colour alpha (the mean alpha of a gradient or image) times the paint opacity, in `0..=1`.
    alpha: f32,
    blend: BlendMode,
}

impl SurfaceLayer {
    /// Replaces what is below instead of mixing with it, so nothing under it can matter.
    fn covers_what_is_below(&self) -> bool {
        self.alpha >= 1.0 - 1e-4 && matches!(self.blend, BlendMode::Normal | BlendMode::PassThrough)
    }

    /// This layer painted over the opaque `below`. Only blend modes that act on each channel alone are
    /// modelled; the others never reach here (see [`paint_layer`]).
    fn over(&self, below: Color) -> Color {
        let mix = |below: f32, top: f32| {
            let blended = match self.blend {
                BlendMode::Multiply => below * top,
                BlendMode::Screen => below + top - below * top,
                BlendMode::Darken => below.min(top),
                BlendMode::Lighten => below.max(top),
                _ => top,
            };
            below * (1.0 - self.alpha) + blended * self.alpha
        };
        Color { r: mix(below.r, self.color.r), g: mix(below.g, self.color.g), b: mix(below.b, self.color.b), a: 1.0 }
    }
}

fn is_finite_color(c: &Color) -> bool {
    c.r.is_finite() && c.g.is_finite() && c.b.is_finite() && c.a.is_finite()
}

/// The mean of a gradient along its length: the stops sorted by position (clamped to `0..=1`), the
/// first stop's colour held back to the start and the last one's out to the end, colour and alpha
/// interpolated premultiplied between stops. Returns the straight mean colour and the mean alpha, or
/// `None` for no stops, a non-finite number, or a gradient that is transparent throughout.
fn gradient_mean(stops: &[GradientStop], profile: &ColorProfile) -> Option<(Color, f32)> {
    if stops.is_empty() || stops.iter().any(|s| !s.pos.is_finite() || !is_finite_color(&s.color)) {
        return None;
    }
    let mut points: Vec<(f64, [f64; 4])> = stops
        .iter()
        .map(|s| {
            let c = profile.to_srgb(&s.color);
            let a = f64::from(c.a.clamp(0.0, 1.0));
            (f64::from(s.pos.clamp(0.0, 1.0)), [f64::from(c.r) * a, f64::from(c.g) * a, f64::from(c.b) * a, a])
        })
        .collect();
    points.sort_by(|x, y| x.0.total_cmp(&y.0));
    let (first, last) = (points[0].1, points[points.len() - 1].1);
    points.insert(0, (0.0, first));
    points.push((1.0, last));
    let mut mean = [0.0f64; 4];
    for pair in points.windows(2) {
        let width = pair[1].0 - pair[0].0;
        for (m, (a, b)) in mean.iter_mut().zip(pair[0].1.iter().zip(&pair[1].1)) {
            *m += width * (a + b) / 2.0;
        }
    }
    let alpha = mean[3];
    if alpha.is_nan() || alpha <= 0.0 {
        return None;
    }
    let channel = |v: f64| (v / alpha).clamp(0.0, 1.0) as f32;
    Some((Color { r: channel(mean[0]), g: channel(mean[1]), b: channel(mean[2]), a: 1.0 }, alpha as f32))
}

/// A paint as a [`SurfaceLayer`], or `None` when it is not a usable surface: a blend mode with no
/// per-channel model, a shader, an unreadable image, a paint with nothing visible in it, or a number
/// that is not finite. This is a small estimator, not a renderer: connector ink is chosen once when a
/// connector is created, a gradient is read as the mean of its stops, and an image as the mean of its
/// pixels, because the connector's geometry is not known yet.
fn paint_layer(doc: &Document, paint: &Paint, image_mean: ImageMean) -> Option<SurfaceLayer> {
    if !matches!(paint.blend, BlendMode::Normal | BlendMode::PassThrough | BlendMode::Multiply | BlendMode::Screen | BlendMode::Darken | BlendMode::Lighten) {
        return None;
    }
    if !paint.opacity.is_finite() {
        return None;
    }
    let profile = &doc.color_profile;
    let (color, alpha) = match &paint.kind {
        PaintKind::Solid(c) => {
            if !is_finite_color(c) {
                return None;
            }
            (profile.to_srgb(c), c.a)
        }
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => gradient_mean(stops, profile)?,
        PaintKind::Image { image_ref, .. } => {
            let mean = image_mean(image_ref)?;
            if !is_finite_color(&mean) {
                return None;
            }
            (mean, mean.a)
        }
        PaintKind::Shader(_) => return None,
    };
    let alpha = (alpha * paint.opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    (alpha > 0.0).then_some(SurfaceLayer { color: Color { a: 1.0, ..color }, alpha, blend: paint.blend })
}

/// The surface a connector homed in `home` is drawn on: the page background with the visible fills of
/// `home` and its ancestors painted over it, from the page toward the connector, each node read with
/// its variable and style bindings applied. A translucent paint therefore affects the contrast check
/// without hiding the surface underneath it. The walk runs from `home` upward and stops at the first
/// fill that covers what is below it, so a layer hidden behind an opaque card is never read (an image
/// there is never decoded). Shaders, blend modes with no per-channel model and images `image_mean`
/// cannot read are skipped explicitly and fall back to the next known surface.
fn surface_behind(doc: &Document, home: NodeId, image_mean: ImageMean) -> Option<Color> {
    let mut ancestors = strict_ancestors(doc, home);
    ancestors.reverse();
    ancestors.push(home);
    let page = ancestors.iter().find_map(|id| match &doc.get(*id)?.kind {
        NodeKind::Page { background } => Some(*background),
        _ => None,
    })?;
    let mut base = page;
    let mut layers = Vec::new();
    'walk: for id in ancestors.iter().rev() {
        let Some(node) = doc.get(*id) else { continue };
        if matches!(node.kind, NodeKind::Page { .. }) {
            continue;
        }
        let node = resolve_node(doc, node);
        if !node.visible {
            continue;
        }
        // the last fill is painted on top
        for paint in node.fills.iter().rev().filter(|p| p.visible) {
            let Some(layer) = paint_layer(doc, paint, image_mean) else { continue };
            if layer.covers_what_is_below() {
                base = layer.color;
                break 'walk;
            }
            layers.push(layer);
        }
    }
    let surface = layers.iter().rev().fold(base, |below, layer| layer.over(below));
    is_finite_color(&surface).then_some(surface)
}

/// The ink of a new connector homed in `home` when nobody picked one: the dark board ink, or
/// white when that ink would fall below 3:1 contrast against the surface behind the connector (a
/// dark page, a dark screen or a dark gradient). Images are not read here: use
/// [`default_connector_ink_with_images`] where they can be decoded. See [`connector_ink_on`].
pub fn default_connector_ink(doc: &Document, home: NodeId) -> Color {
    default_connector_ink_with_images(doc, home, &|_| None)
}

/// [`default_connector_ink`] that also reads image fills through `image_mean` (see [`ImageMean`]).
pub fn default_connector_ink_with_images(doc: &Document, home: NodeId, image_mean: ImageMean) -> Color {
    connector_ink_on(doc, home, connector_ink_on_light(), Color::WHITE, image_mean)
}

/// Picks `on_light`, or `on_dark` when `on_light` would fall below 3:1 contrast against the
/// surface a connector homed in `home` is drawn on and `on_dark` reads better there. The surface is
/// the page background with the visible fills of the connector's ancestors painted over it, from the
/// page down to `home`; an image fill counts as the mean `image_mean` returns for it. A board file
/// is drawn on its own light canvas, so it always gets `on_light`.
pub fn connector_ink_on(doc: &Document, home: NodeId, on_light: Color, on_dark: Color, image_mean: ImageMean) -> Color {
    if doc.kind == DocumentKind::Board {
        return on_light;
    }
    match surface_behind(doc, home, image_mean) {
        Some(surface) => {
            let light_ratio = contrast_ratio(on_light, surface);
            let dark_ratio = contrast_ratio(on_dark, surface);
            if light_ratio >= CONNECTOR_INK_MIN_CONTRAST || light_ratio >= dark_ratio {
                on_light
            } else {
                on_dark
            }
        }
        None => on_light,
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
        assert_eq!(default_connector_ink(&doc, wrapper), dark, "an empty gradient is not a surface");
    }

    #[test]
    fn connector_ink_reads_gradient_mean_and_blends_translucent_fills() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        let gradient = Paint {
            kind: PaintKind::LinearGradient {
                handles: [crate::Vec2::new(0.0, 0.5), crate::Vec2::new(1.0, 0.5), crate::Vec2::new(0.0, 1.0)],
                stops: vec![crate::GradientStop { pos: 0.0, color: Color::BLACK }, crate::GradientStop { pos: 1.0, color: Color::BLACK }],
            },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        };
        doc.get_mut(wrapper).expect("wrapper").fills = vec![gradient];
        assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "a dark gradient is a dark surface");

        let mut translucent = Paint::solid(Color::WHITE);
        translucent.opacity = 0.45;
        doc.get_mut(screen).expect("screen").fills.clear();
        doc.get_mut(wrapper).expect("wrapper").fills = vec![translucent];
        let chosen = default_connector_ink(&doc, wrapper);
        assert_eq!(chosen, Color::rgb(0x1E, 0x1E, 0x1E), "the translucent white paint must be blended over the dark page");
    }

    #[test]
    fn connector_ink_reads_a_gradient_that_fades_out_by_the_colour_it_keeps() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        set_page_background(&mut doc, page, Color::BLACK);
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(screen).expect("screen").fills.clear();
        let fade = Paint {
            kind: PaintKind::LinearGradient {
                handles: [crate::Vec2::new(0.0, 0.5), crate::Vec2::new(1.0, 0.5), crate::Vec2::new(0.0, 1.0)],
                stops: vec![crate::GradientStop { pos: 0.0, color: Color::WHITE }, crate::GradientStop { pos: 1.0, color: Color::TRANSPARENT }],
            },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        };
        doc.get_mut(wrapper).expect("wrapper").fills = vec![fade];
        assert_eq!(
            default_connector_ink(&doc, wrapper),
            Color::rgb(0x1E, 0x1E, 0x1E),
            "white fading out over a black page is a mid-light surface, not a dark one"
        );
    }

    fn image_paint(image_ref: &str) -> Paint {
        Paint {
            kind: PaintKind::Image { image_ref: image_ref.into(), scale_mode: crate::ImageScaleMode::Fill, params: Default::default() },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        }
    }

    #[test]
    fn connector_ink_reads_an_image_as_the_mean_the_sampler_gives() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(wrapper).expect("wrapper").fills = vec![image_paint("photo")];
        let asked = std::cell::RefCell::new(Vec::new());
        let dark_photo = |image_ref: &str| {
            asked.borrow_mut().push(image_ref.to_string());
            Some(Color::rgb(0x10, 0x10, 0x10))
        };
        assert_eq!(default_connector_ink_with_images(&doc, wrapper, &dark_photo), Color::WHITE, "a dark image is a dark surface");
        assert_eq!(asked.into_inner(), vec!["photo".to_string()], "the sampler is asked for the image the paint references");

        let light_photo = |_: &str| Some(Color::rgb(0xF0, 0xF0, 0xF0));
        let mut dark_screen = Document::new("Test");
        let dark_page = dark_screen.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut dark_screen, dark_page);
        dark_screen.get_mut(screen).expect("screen").fills = vec![Paint::solid(Color::BLACK)];
        dark_screen.get_mut(wrapper).expect("wrapper").fills = vec![image_paint("photo")];
        assert_eq!(
            default_connector_ink_with_images(&dark_screen, wrapper, &light_photo),
            Color::rgb(0x1E, 0x1E, 0x1E),
            "a light image on a dark screen is a light surface"
        );
    }

    #[test]
    fn connector_ink_treats_an_unreadable_or_faint_image_as_the_surface_below() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(screen).expect("screen").fills = vec![Paint::solid(Color::WHITE)];
        doc.get_mut(wrapper).expect("wrapper").fills = vec![image_paint("photo")];
        let dark = Color::rgb(0x1E, 0x1E, 0x1E);
        assert_eq!(default_connector_ink(&doc, wrapper), dark, "no sampler answer gives the result an image-blind document gets");
        assert_eq!(default_connector_ink_with_images(&doc, wrapper, &|_| None), dark, "a sampler that cannot decode leaves the white screen as the surface");

        let faint = |_: &str| Some(Color { a: 0.2, ..Color::BLACK });
        assert_eq!(default_connector_ink_with_images(&doc, wrapper, &faint), dark, "an image that is mostly transparent is not a surface");

        let mut half = image_paint("photo");
        half.opacity = 0.2;
        doc.get_mut(wrapper).expect("wrapper").fills = vec![half];
        let opaque_black = |_: &str| Some(Color::BLACK);
        assert_eq!(default_connector_ink_with_images(&doc, wrapper, &opaque_black), dark, "paint opacity counts toward the surface alpha");
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

    fn blended(color: Color, mode: crate::BlendMode) -> Paint {
        let mut paint = Paint::solid(color);
        paint.blend = mode;
        paint
    }

    /// A doc whose screen holds `below` and whose wrapper holds `top`; the ink of a connector in the wrapper.
    fn ink_of_stack(below: Paint, top: Paint) -> Color {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(screen).expect("screen").fills = vec![below];
        doc.get_mut(wrapper).expect("wrapper").fills = vec![top];
        default_connector_ink(&doc, wrapper)
    }

    #[test]
    fn connector_ink_composites_multiply_over_the_surface_below() {
        let dark = Color::rgb(0x1E, 0x1E, 0x1E);
        // black multiplied over white is black: main picked white here and so must this
        assert_eq!(ink_of_stack(Paint::solid(Color::WHITE), blended(Color::BLACK, crate::BlendMode::Multiply)), Color::WHITE);
        // white multiplied over anything leaves the surface as it was
        assert_eq!(ink_of_stack(Paint::solid(Color::WHITE), blended(Color::WHITE, crate::BlendMode::Multiply)), dark);
    }

    #[test]
    fn connector_ink_composites_screen_darken_and_lighten_over_the_surface_below() {
        let dark = Color::rgb(0x1E, 0x1E, 0x1E);
        assert_eq!(ink_of_stack(Paint::solid(Color::BLACK), blended(Color::WHITE, crate::BlendMode::Screen)), dark, "white screened over black is white");
        assert_eq!(ink_of_stack(Paint::solid(Color::WHITE), blended(Color::BLACK, crate::BlendMode::Screen)), dark, "black screened over white leaves white");
        assert_eq!(
            ink_of_stack(Paint::solid(Color::WHITE), blended(Color::BLACK, crate::BlendMode::Darken)),
            Color::WHITE,
            "the darker of black and white is black"
        );
        assert_eq!(ink_of_stack(Paint::solid(Color::BLACK), blended(Color::WHITE, crate::BlendMode::Darken)), Color::WHITE, "darken keeps the black below");
        assert_eq!(ink_of_stack(Paint::solid(Color::BLACK), blended(Color::WHITE, crate::BlendMode::Lighten)), dark, "the lighter of white and black is white");
        assert_eq!(ink_of_stack(Paint::solid(Color::WHITE), blended(Color::BLACK, crate::BlendMode::Lighten)), dark, "lighten keeps the white below");
    }

    #[test]
    fn connector_ink_skips_blend_modes_it_cannot_composite() {
        assert_eq!(
            ink_of_stack(Paint::solid(Color::WHITE), blended(Color::BLACK, crate::BlendMode::ColorBurn)),
            Color::rgb(0x1E, 0x1E, 0x1E),
            "a blend mode without a destination model falls back to the known underlying surface"
        );
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
        assert_eq!(connector_ink_on(&doc, page, light_ink, dark_ink, &|_| None), dark_ink, "the near-black ink is below 3:1 on the dark default page");
        set_page_background(&mut doc, page, Color::WHITE);
        assert_eq!(connector_ink_on(&doc, page, light_ink, dark_ink, &|_| None), light_ink);
    }

    fn gradient_paint(stops: Vec<crate::GradientStop>) -> Paint {
        Paint {
            kind: PaintKind::LinearGradient { handles: [crate::Vec2::new(0.0, 0.5), crate::Vec2::new(1.0, 0.5), crate::Vec2::new(0.0, 1.0)], stops },
            opacity: 1.0,
            visible: true,
            blend: crate::BlendMode::Normal,
        }
    }

    fn stop(pos: f32, color: Color) -> crate::GradientStop {
        crate::GradientStop { pos, color }
    }

    #[test]
    fn connector_ink_integrates_gradient_stops_over_their_positions() {
        let grey_page = Color::rgb(0x5A, 0x5A, 0x5A);
        let ink_over_page = |stops: Vec<crate::GradientStop>| {
            let mut doc = Document::new("Test");
            let page = doc.add_page("Page 1");
            set_page_background(&mut doc, page, grey_page);
            let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
            doc.get_mut(screen).expect("screen").fills.clear();
            doc.get_mut(wrapper).expect("wrapper").fills = vec![gradient_paint(stops)];
            default_connector_ink(&doc, wrapper)
        };
        // white for the first tenth, then black: the mean is a dark grey, not the middle grey of the stop count
        assert_eq!(ink_over_page(vec![stop(0.0, Color::WHITE), stop(0.1, Color::BLACK)]), Color::WHITE);
        // the same stops given out of order and past the ends read the same
        assert_eq!(ink_over_page(vec![stop(0.1, Color::BLACK), stop(-3.0, Color::WHITE)]), Color::WHITE);
        // opaque black that fades out over the whole length leaves half its weight on the grey page
        assert_eq!(ink_over_page(vec![stop(0.0, Color::BLACK), stop(1.0, Color { a: 0.0, ..Color::WHITE })]), Color::WHITE);
        // black that only starts at the last tenth leaves the white before it as the surface
        assert_eq!(ink_over_page(vec![stop(0.0, Color::WHITE), stop(0.9, Color::WHITE), stop(1.0, Color::BLACK)]), Color::rgb(0x1E, 0x1E, 0x1E));
    }

    #[test]
    fn connector_ink_reads_the_bound_value_of_a_variable_fill() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let card = make_frame(&mut doc, page, "Card");
        doc.get_mut(card).expect("card").fills = vec![Paint::solid(Color::WHITE)];
        doc.variable_collections.insert(
            "c".into(),
            crate::VariableCollection {
                extensions: Default::default(),
                id: "c".into(),
                name: "Colours".into(),
                modes: vec![crate::VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["v".into()],
                key: None,
                source_library_key: None,
            },
        );
        doc.variables.insert(
            "v".into(),
            crate::Variable {
                extensions: Default::default(),
                mode_extensions: Default::default(),
                id: "v".into(),
                name: "Surface".into(),
                collection: "c".into(),
                kind: crate::VariableKind::Color,
                values: std::collections::BTreeMap::from([("m1".to_string(), crate::VariableValue::Color(Color::rgb(0x12, 0x12, 0x12)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        assert_eq!(default_connector_ink(&doc, card), Color::rgb(0x1E, 0x1E, 0x1E), "unbound, the raw white fill is the surface");
        doc.get_mut(card).expect("card").variables.bindings.insert("fills/0".into(), "v".into());
        assert_eq!(default_connector_ink(&doc, card), Color::WHITE, "bound, the variable's #121212 is the surface");
    }

    #[test]
    fn connector_ink_skips_hidden_layers_and_counts_stacked_faint_fills() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(screen).expect("screen").fills = vec![Paint::solid(Color::WHITE)];
        doc.get_mut(wrapper).expect("wrapper").fills = vec![Paint::solid(Color::BLACK)];
        doc.get_mut(wrapper).expect("wrapper").visible = false;
        assert_eq!(default_connector_ink(&doc, wrapper), Color::rgb(0x1E, 0x1E, 0x1E), "a hidden layer paints nothing");

        doc.get_mut(wrapper).expect("wrapper").visible = true;
        let faint = || {
            let mut p = Paint::solid(Color::BLACK);
            p.opacity = 0.30;
            p
        };
        doc.get_mut(wrapper).expect("wrapper").fills = vec![faint(), faint(), faint()];
        assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "three 30% black fills leave a surface about 66% black");
    }

    #[test]
    fn connector_ink_lets_a_non_finite_number_fall_back_to_the_surface_below() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        set_page_background(&mut doc, page, Color::BLACK);
        let (screen, wrapper) = make_screen_with_wrapper(&mut doc, page);
        doc.get_mut(screen).expect("screen").fills.clear();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            doc.get_mut(wrapper).expect("wrapper").fills = vec![gradient_paint(vec![stop(0.0, Color { r: bad, ..Color::WHITE }), stop(1.0, Color::WHITE)])];
            assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "a {bad} channel in a stop leaves the black page as the surface");
            doc.get_mut(wrapper).expect("wrapper").fills = vec![gradient_paint(vec![stop(bad, Color::WHITE), stop(1.0, Color::WHITE)])];
            assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "a {bad} stop position leaves the black page as the surface");
            let mut p = Paint::solid(Color::WHITE);
            p.opacity = bad;
            doc.get_mut(wrapper).expect("wrapper").fills = vec![p];
            assert_eq!(default_connector_ink(&doc, wrapper), Color::WHITE, "a {bad} paint opacity leaves the black page as the surface");
        }
        let nan_photo = |_: &str| Some(Color { r: f32::NAN, ..Color::BLACK });
        doc.get_mut(wrapper).expect("wrapper").fills = vec![image_paint("photo")];
        assert_eq!(default_connector_ink_with_images(&doc, wrapper, &nan_photo), Color::WHITE, "a sampler answer with a NaN channel is not a surface");
    }

    #[test]
    fn connector_ink_never_reads_an_image_hidden_behind_an_opaque_card() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let hero = make_frame(&mut doc, page, "Hero");
        doc.get_mut(hero).expect("hero").fills = vec![image_paint("photo")];
        let card = make_frame(&mut doc, hero, "Card");
        doc.get_mut(card).expect("card").fills = vec![Paint::solid(Color::WHITE)];
        let calls = std::cell::Cell::new(0u32);
        let sampler = |_: &str| {
            calls.set(calls.get() + 1);
            Some(Color::BLACK)
        };
        assert_eq!(default_connector_ink_with_images(&doc, card, &sampler), Color::rgb(0x1E, 0x1E, 0x1E), "the white card is the surface");
        assert_eq!(calls.get(), 0, "the image behind an opaque card is not decoded");

        doc.get_mut(card).expect("card").fills.clear();
        assert_eq!(default_connector_ink_with_images(&doc, card, &sampler), Color::WHITE, "without the card the image is the surface");
        assert_eq!(calls.get(), 1);
    }
}
