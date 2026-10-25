//! Where a pasted or dropped layer goes: the frame it joins and the spot it takes in that frame.
//!
//! A frame that clips its content (every frame imported from Figma does) hides whatever sticks out of it, so the
//! frame is picked at the point the layer lands, never at the layer's own corner, and a layer that would land half
//! outside such a frame, or outside a clipping frame that holds the one it joins, is moved inside it.

use crate::app::App;
use crate::canvas::{create, interact};
use crate::edit::Tx;
use crate::layout;
use crate::util::world_aabb;
use designos_doc::{Command, NodeId, NodeKind, TextAutoResize, Vec2};

/// Widest a pasted text layer grows before it wraps, in px.
const PASTED_TEXT_MAX_WIDTH: f32 = 400.0;
/// Longest layer name taken from pasted text.
const PASTED_NAME_CHARS: usize = 40;

/// The frame a layer joins and the page point it is centered on.
#[derive(Clone, Copy, Debug)]
pub struct Landing {
    pub parent: NodeId,
    pub center: Vec2,
}

/// Where a paste lands.
/// - A selected frame (or the frame of a selected layer) takes the paste. It lands at the viewport center when that
///   is inside the frame, and at the frame's own center when it is not.
/// - With no frame selected, the frame under the viewport center takes it, as for a dropped file; over no frame, the page.
pub fn paste_landing(app: &App) -> Landing {
    let view = app.canvas_rect;
    let center = app.view.to_page(view.min, view.center());
    let selected = app.container_for_insert();
    if selected == app.page {
        return drop_landing(app, center);
    }
    let r = world_aabb(&app.doc, selected);
    let inside = center.x >= r.x && center.x <= r.x + r.w && center.y >= r.y && center.y <= r.y + r.h;
    Landing { parent: selected, center: if inside { center } else { Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0) } }
}

/// Where a layer dropped at page point `point` lands: the frame under that point, centered on it.
pub fn drop_landing(app: &App, point: Vec2) -> Landing {
    Landing { parent: create::container_at_with_index(&app.doc, interact::active_index(app), app.page, point, &[]), center: point }
}

/// A box in the landing frame's own coordinates.
#[derive(Clone, Copy, Debug)]
struct Span {
    min: Vec2,
    max: Vec2,
}

impl Span {
    fn intersect(self, o: Span) -> Option<Span> {
        let (min, max) = (Vec2::new(self.min.x.max(o.min.x), self.min.y.max(o.min.y)), Vec2::new(self.max.x.min(o.max.x), self.max.y.min(o.max.y)));
        (min.x < max.x && min.y < max.y).then_some(Span { min, max })
    }
}

/// The landing frame as seen from inside it.
struct ParentFrame {
    /// The landing point in the frame's own coordinates.
    center: Vec2,
    /// The part of the frame's coordinate space that shows: the box the clipping frames above it (or it itself) leave.
    /// `None` when nothing clips, so the layer stays where it is centered.
    shown: Option<Span>,
}

impl ParentFrame {
    fn of(app: &App, landing: &Landing) -> Option<Self> {
        let inv = app.doc.world_transform(landing.parent).invert()?;
        app.doc.get(landing.parent)?;
        // A frame that does not clip, or a group, inside one that does (a group in a screen imported from Figma) still
        // hides what sticks out of the clipping one: every clipping ancestor counts, the nearest first.
        let (mut nearest, mut all) = (None, None::<Span>);
        let mut up = Some(landing.parent);
        while let Some(id) = up {
            let node = app.doc.get(id)?;
            if id != app.page && node.kind.frame_props().is_some_and(|f| f.clips_content) {
                let r = world_aabb(&app.doc, id);
                let corners = [(r.x, r.y), (r.x + r.w, r.y), (r.x, r.y + r.h), (r.x + r.w, r.y + r.h)].map(|(x, y)| inv.apply(Vec2::new(x, y)));
                let min = corners.iter().fold(Vec2::new(f32::MAX, f32::MAX), |m, c| Vec2::new(m.x.min(c.x), m.y.min(c.y)));
                let max = corners.iter().fold(Vec2::new(f32::MIN, f32::MIN), |m, c| Vec2::new(m.x.max(c.x), m.y.max(c.y)));
                let span = Span { min, max };
                nearest = nearest.or(Some(span));
                all = match all {
                    None => Some(span),
                    // clipping frames that share no area leave nothing to aim at: the nearest one decides
                    Some(a) => a.intersect(span).or(nearest),
                };
            }
            up = node.parent;
        }
        Some(Self { center: inv.apply(landing.center), shown: all })
    }

    /// Top-left corner, in the frame's coordinates, of a layer of `size` centered on the landing point. Where a
    /// clipping frame hides what sticks out, a layer that fits is moved inside the part that shows, and one that is
    /// larger is centered in it.
    fn origin_for(&self, size: Vec2) -> Vec2 {
        let axis = |center: f32, extent: f32, lo: f32, hi: f32| {
            let at = center - extent / 2.0;
            let at = match self.shown {
                None => at,
                Some(_) if extent <= hi - lo => at.clamp(lo, hi - extent),
                Some(_) => (lo + hi - extent) / 2.0,
            };
            at.round()
        };
        let Span { min, max } = self.shown.unwrap_or(Span { min: Vec2::ZERO, max: Vec2::ZERO });
        Vec2::new(axis(self.center.x, size.x, min.x, max.x), axis(self.center.y, size.y, min.y, max.y))
    }

    /// The widest a text layer may be before it wraps: the part of the frame that shows, at most 400 px.
    fn text_limit(&self) -> f32 {
        match self.shown {
            Some(s) if s.max.x > s.min.x => PASTED_TEXT_MAX_WIDTH.min(s.max.x - s.min.x),
            _ => PASTED_TEXT_MAX_WIDTH,
        }
    }
}

const COLLAPSED: &str = "Cannot place into a collapsed layer";

/// Adds the picture `bytes` to `tx` as an image layer on `landing`, at most 400 px on its longest side. The caller
/// commits `tx` and selects the layer. Shows a toast and returns `None` when nothing could be placed.
pub fn insert_image_at(app: &mut App, tx: &mut Tx, landing: &Landing, name: &str, bytes: Vec<u8>) -> Option<NodeId> {
    let Some(frame) = ParentFrame::of(app, landing) else {
        app.show_toast(COLLAPSED, true);
        return None;
    };
    interact::insert_image(app, tx, landing.parent, name, bytes, move |size| frame.origin_for(size))
}

/// Pastes the clipboard image as one selected image layer on `landing`, as one undo step.
pub fn paste_image(app: &mut App, landing: &Landing, png: Vec<u8>) {
    let mut tx = Tx::new();
    if let Some(id) = insert_image_at(app, &mut tx, landing, "Pasted image", png) {
        if app.run_tx(tx) {
            app.selection = vec![id];
            app.tool = crate::canvas::tools::Tool::Move;
        }
    }
}

/// Layer name of pasted text: its first line that has any text, cut to a short length.
fn pasted_name(content: &str) -> String {
    let line = content.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Text");
    line.chars().take(PASTED_NAME_CHARS).collect()
}

/// Pastes `text` as one selected text layer on `landing`, as one undo step. Text wider than 400 px wraps at that
/// width, or at the width of a clipping frame that is narrower. Returns false, with the document untouched, when
/// `text` is only whitespace.
pub fn paste_text(app: &mut App, landing: &Landing, text: &str) -> bool {
    let content = text.replace("\r\n", "\n").trim_end().to_string();
    if content.trim().is_empty() {
        return false;
    }
    let parent = landing.parent;
    let Some(frame) = ParentFrame::of(app, landing) else {
        app.show_toast(COLLAPSED, true);
        return true;
    };
    let Some((mut node, _)) = create::new_text(app, parent, landing.center) else { return false };
    node.name = pasted_name(&content);
    let limit = frame.text_limit();
    if let NodeKind::Text(t) = &mut node.kind {
        t.content = content;
        node.size = layout::estimate_text_size(t);
        if node.size.x > limit {
            t.auto_resize = TextAutoResize::Height;
            node.size = layout::fit_text(t, Vec2::new(limit, node.size.y));
        }
    }
    let origin = frame.origin_for(node.size);
    node.transform.tx = origin.x;
    node.transform.ty = origin.y;
    let id = node.id;
    if !app.slot_insert_allowed(parent, &[&node]) {
        return true;
    }
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if let Err(e) = tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }) {
        app.show_toast(e.to_string(), true);
        return true;
    }
    App::stretch_into_slot_if_configured(&mut app.doc, &mut tx.ed, parent, id);
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    layout::relayout_around(&mut app.doc, &mut tx.ed, parent);
    app.run_tx(tx);
    app.selection = vec![id];
    app.fonts.load_for(&app.doc);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(clips: bool) -> ParentFrame {
        ParentFrame { center: Vec2::new(0.0, 0.0), shown: clips.then_some(Span { min: Vec2::new(0.0, 0.0), max: Vec2::new(360.0, 640.0) }) }
    }

    #[test]
    fn a_layer_is_centered_on_the_landing_point_in_a_frame_that_does_not_clip() {
        let f = ParentFrame { center: Vec2::new(100.0, 50.0), ..frame(false) };
        let o = f.origin_for(Vec2::new(40.0, 20.0));
        assert_eq!((o.x, o.y), (80.0, 40.0), "sticking out of a frame that shows it is fine");
        let f = ParentFrame { center: Vec2::new(5.0, 5.0), ..frame(false) };
        assert_eq!((f.origin_for(Vec2::new(40.0, 20.0)).x, f.origin_for(Vec2::new(40.0, 20.0)).y), (-15.0, -5.0));
    }

    #[test]
    fn a_layer_that_fits_a_clipping_frame_is_moved_inside_it_and_a_larger_one_is_centered_in_it() {
        let f = ParentFrame { center: Vec2::new(5.0, 630.0), ..frame(true) };
        let o = f.origin_for(Vec2::new(200.0, 100.0));
        assert_eq!((o.x, o.y), (0.0, 540.0), "pulled in from the left and bottom edges");
        let o = f.origin_for(Vec2::new(400.0, 100.0));
        assert_eq!((o.x, o.y), (-20.0, 540.0), "400 px in a 360 px frame is centered on that axis only");
    }

    #[test]
    fn a_layer_is_kept_inside_a_clipping_frame_that_sits_around_the_frame_it_joins() {
        // the frame it joins does not clip; the screen around it does, and starts 100 px to its left and 50 above it
        let f = ParentFrame { center: Vec2::new(330.0, 30.0), shown: Some(Span { min: Vec2::new(-100.0, -50.0), max: Vec2::new(260.0, 590.0) }) };
        let o = f.origin_for(Vec2::new(120.0, 80.0));
        assert_eq!((o.x, o.y), (140.0, -10.0), "pulled in from the screen's right edge, not the joined frame's");
        let o = f.origin_for(Vec2::new(400.0, 80.0));
        assert_eq!(o.x, -120.0, "wider than the screen: centered in it");
        assert_eq!(f.text_limit(), 360.0, "text wraps at the width that shows");
        assert_eq!(frame(false).text_limit(), 400.0);
    }

    #[test]
    fn clipping_boxes_are_intersected_and_boxes_that_share_no_area_leave_nothing() {
        let a = Span { min: Vec2::new(0.0, 0.0), max: Vec2::new(100.0, 100.0) };
        let b = Span { min: Vec2::new(50.0, 20.0), max: Vec2::new(200.0, 80.0) };
        let i = a.intersect(b).expect("they overlap");
        assert_eq!((i.min.x, i.min.y, i.max.x, i.max.y), (50.0, 20.0, 100.0, 80.0));
        assert!(a.intersect(Span { min: Vec2::new(100.0, 0.0), max: Vec2::new(200.0, 100.0) }).is_none());
    }

    #[test]
    fn pasted_text_names_its_layer_after_its_first_line() {
        assert_eq!(pasted_name("\n  Hello world \nsecond"), "Hello world");
        assert_eq!(pasted_name(&"x".repeat(100)).chars().count(), PASTED_NAME_CHARS);
        assert_eq!(pasted_name("   \n  "), "Text");
    }
}
