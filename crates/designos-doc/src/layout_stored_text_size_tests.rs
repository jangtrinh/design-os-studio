//! Auto layout keeps the size Figma stored for an imported text while the text is unedited and the
//! box the parent offers still fits it. Anything else is shaped, which also drops the stored glyphs.

use crate::text_measure::{TextMeasure, ThreadLayoutMeasurerGuard};
use crate::{AutoLayout, Document, FigTextGeometry, LayoutDirection, Node, NodeId, NodeKind, Path, Rect, Sizing, TextAutoResize, TextProps, TextStyle, Vec2};
use std::sync::Arc;

/// Shapes a text unwrapped at 10 px per character by 20 px, and wrapped at `w` as `(w, 40)`. Neither
/// matches the stored sizes these tests use, so a shaped text is easy to tell from a stored one.
struct Shaper;

impl TextMeasure for Shaper {
    fn measure(&self, tp: &TextProps, wrap_width: Option<f32>) -> Vec2 {
        match wrap_width {
            None => Vec2::new(tp.content.chars().count() as f32 * 10.0, 20.0),
            Some(w) => Vec2::new(w, 40.0),
        }
    }
}

const STORED: Vec2 = Vec2 { x: 37.5, y: 19.25 };

fn row_frame(doc: &mut Document, page: NodeId, direction: LayoutDirection, width: f32, primary: Sizing) -> NodeId {
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, width, 60.0));
    if let NodeKind::Frame(fp) = &mut frame.kind {
        fp.auto_layout = Some(AutoLayout { direction, primary_sizing: primary, counter_sizing: Sizing::Hug, ..AutoLayout::default() });
    }
    doc.insert(page, frame).unwrap();
    id
}

/// A text of `stored` size whose snapshot of Figma's glyphs describes it exactly.
fn imported_text(doc: &mut Document, parent: NodeId, content: &str, auto_resize: TextAutoResize, stored: Vec2) -> NodeId {
    let id = doc.alloc_id();
    let mut node = Node::text(id, Rect::new(0.0, 0.0, stored.x, stored.y), content, TextStyle::default());
    let NodeKind::Text(tp) = &mut node.kind else { unreachable!() };
    tp.auto_resize = auto_resize;
    let text = tp.clone();
    node.fig_text_geometry =
        Some(Arc::new(FigTextGeometry { text, size: stored, path: Path::default(), blobs: Vec::new(), glyph_blobs: Vec::new(), glyphs: Default::default() }));
    doc.insert(parent, node).unwrap();
    id
}

fn plain_text(doc: &mut Document, parent: NodeId, content: &str) -> NodeId {
    let id = doc.alloc_id();
    let mut node = Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), content, TextStyle::default());
    let NodeKind::Text(tp) = &mut node.kind else { unreachable!() };
    tp.auto_resize = TextAutoResize::WidthAndHeight;
    doc.insert(parent, node).unwrap();
    id
}

fn keeps_glyphs(doc: &Document, id: NodeId) -> bool {
    let n = doc.get(id).unwrap();
    n.fig_text_geometry.as_ref().is_some_and(|g| g.matches(n))
}

fn set_content(doc: &mut Document, id: NodeId, content: &str) {
    let Some(NodeKind::Text(tp)) = doc.get_mut(id).map(|n| &mut n.kind) else { panic!("not a text") };
    tp.content = content.into();
}

#[test]
fn hug_text_keeps_its_stored_size_when_a_sibling_changes() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug");
    let page = doc.add_page("P");
    let row = row_frame(&mut doc, page, LayoutDirection::Horizontal, 300.0, Sizing::Hug);
    let label = imported_text(&mut doc, row, "Hello", TextAutoResize::WidthAndHeight, STORED);
    doc.get_mut(label).unwrap().layout_child.max_w = Some(STORED.x);
    let sibling = plain_text(&mut doc, row, "Other");
    doc.get_mut(sibling).unwrap().layout_child.max_w = Some(400.0);

    doc.relayout_subtree(row);
    assert_eq!(doc.get(label).unwrap().size, STORED, "the stored size survives the first layout");
    assert!(keeps_glyphs(&doc, label));

    set_content(&mut doc, sibling, "A much longer sibling");
    doc.relayout_subtree(row);
    let n = doc.get(label).unwrap();
    assert_eq!((n.size.x.to_bits(), n.size.y.to_bits()), (STORED.x.to_bits(), STORED.y.to_bits()), "bit-identical to the snapshot");
    assert!(keeps_glyphs(&doc, label));
    assert_eq!(doc.get(sibling).unwrap().size.x, 21.0 * 10.0, "the sibling is still shaped");
}

#[test]
fn hug_text_narrower_than_its_stored_width_is_shaped_and_drops_the_glyphs() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-narrow");
    let page = doc.add_page("P");
    let row = row_frame(&mut doc, page, LayoutDirection::Horizontal, 300.0, Sizing::Hug);
    let label = imported_text(&mut doc, row, "Hello", TextAutoResize::WidthAndHeight, STORED);
    doc.get_mut(label).unwrap().layout_child.max_w = Some(STORED.x);

    doc.relayout_subtree(row);
    assert!(keeps_glyphs(&doc, label));

    doc.get_mut(label).unwrap().layout_child.max_w = Some(30.0);
    doc.relayout_subtree(row);
    let n = doc.get(label).unwrap();
    assert_eq!(n.size, Vec2::new(30.0, 40.0), "wrapped at the new cap by the shaper");
    assert!(!keeps_glyphs(&doc, label), "a wrapped text no longer matches Figma's lines");
}

/// A height the shaper never produces (it wraps to 40), so only the stored path can return it.
const STORED_WRAPPED_HEIGHT: f32 = 38.5;

/// 32 characters shape to 320 px unwrapped, so a cap above that never wraps.
const LONG: &str = "abcdefghijklmnopqrstuvwxyz012345";

fn wrapped_at_its_cap(doc: &mut Document, cap: f32) -> (NodeId, NodeId) {
    let row = row_frame(doc, doc.pages[0], LayoutDirection::Horizontal, 600.0, Sizing::Hug);
    let label = imported_text(doc, row, LONG, TextAutoResize::WidthAndHeight, Vec2::new(cap, STORED_WRAPPED_HEIGHT));
    doc.get_mut(label).unwrap().layout_child.max_w = Some(cap);
    (row, label)
}

#[test]
fn hug_text_wrapped_at_an_unchanged_cap_keeps_its_stored_size() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-same-cap");
    doc.add_page("P");
    let (row, label) = wrapped_at_its_cap(&mut doc, 200.0);

    doc.relayout_subtree(row);
    let n = doc.get(label).unwrap();
    assert_eq!((n.size.x.to_bits(), n.size.y.to_bits()), (200.0f32.to_bits(), STORED_WRAPPED_HEIGHT.to_bits()));
    assert!(keeps_glyphs(&doc, label));
}

#[test]
fn hug_text_wrapped_at_its_cap_is_shaped_when_the_cap_grows() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-cap-grows");
    doc.add_page("P");
    let (row, label) = wrapped_at_its_cap(&mut doc, 200.0);
    doc.relayout_subtree(row);
    assert!(keeps_glyphs(&doc, label));

    doc.get_mut(label).unwrap().layout_child.max_w = Some(400.0);
    doc.relayout_subtree(row);
    assert_eq!(doc.get(label).unwrap().size, Vec2::new(320.0, 20.0), "Figma's two lines no longer apply at the larger cap");
    assert!(!keeps_glyphs(&doc, label));
}

#[test]
fn capped_hug_text_stored_below_its_cap_is_shaped() {
    // The stored width alone cannot tell a text Figma wrapped at the cap from one that fitted under it, so
    // it is shaped until the stored line count says which.
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-below-cap");
    let page = doc.add_page("P");
    let row = row_frame(&mut doc, page, LayoutDirection::Horizontal, 300.0, Sizing::Hug);
    let label = imported_text(&mut doc, row, "Hello", TextAutoResize::WidthAndHeight, STORED);
    doc.get_mut(label).unwrap().layout_child.max_w = Some(200.0);

    doc.relayout_subtree(row);
    assert_eq!(doc.get(label).unwrap().size, Vec2::new(50.0, 20.0));
    assert!(!keeps_glyphs(&doc, label));
}

#[test]
fn edited_hug_text_is_shaped_as_before() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-edited");
    let page = doc.add_page("P");
    let row = row_frame(&mut doc, page, LayoutDirection::Horizontal, 300.0, Sizing::Hug);
    let label = imported_text(&mut doc, row, "Hello", TextAutoResize::WidthAndHeight, STORED);
    doc.get_mut(label).unwrap().layout_child.max_w = Some(200.0);
    set_content(&mut doc, label, "Hello!");

    doc.relayout_subtree(row);
    assert_eq!(doc.get(label).unwrap().size, Vec2::new(60.0, 20.0));
    assert!(!keeps_glyphs(&doc, label));
}

#[test]
fn hug_text_without_stored_data_is_shaped_as_before() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-hug-none");
    let page = doc.add_page("P");
    let row = row_frame(&mut doc, page, LayoutDirection::Horizontal, 300.0, Sizing::Hug);
    let label = imported_text(&mut doc, row, "Hello", TextAutoResize::WidthAndHeight, STORED);
    let n = doc.get_mut(label).unwrap();
    n.fig_text_geometry = None;
    n.layout_child.max_w = Some(200.0);

    doc.relayout_subtree(row);
    assert_eq!(doc.get(label).unwrap().size, Vec2::new(50.0, 20.0));
}

fn column_with_fill_text(doc: &mut Document, page: NodeId, width: f32, stored: Vec2) -> (NodeId, NodeId) {
    let col = row_frame(doc, page, LayoutDirection::Vertical, width, Sizing::Fixed);
    if let Some(NodeKind::Frame(fp)) = doc.get_mut(col).map(|n| &mut n.kind) {
        fp.auto_layout.as_mut().unwrap().counter_sizing = Sizing::Fixed;
    }
    let body = imported_text(doc, col, "Hello", TextAutoResize::Height, stored);
    doc.get_mut(body).unwrap().layout_child.horizontal = Sizing::Fill;
    (col, body)
}

#[test]
fn auto_height_text_keeps_its_stored_size_when_the_parent_offers_the_stored_width() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-height");
    let page = doc.add_page("P");
    let stored = Vec2::new(100.0, 19.25);
    let (col, body) = column_with_fill_text(&mut doc, page, 100.0, stored);

    doc.relayout_subtree(col);
    let n = doc.get(body).unwrap();
    assert_eq!((n.size.x.to_bits(), n.size.y.to_bits()), (stored.x.to_bits(), stored.y.to_bits()));
    assert!(keeps_glyphs(&doc, body));
}

#[test]
fn auto_height_text_in_another_width_is_shaped_and_drops_the_glyphs() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-height-rewrap");
    let page = doc.add_page("P");
    let stored = Vec2::new(100.0, 19.25);
    let (col, body) = column_with_fill_text(&mut doc, page, 100.0, stored);
    doc.relayout_subtree(col);
    assert!(keeps_glyphs(&doc, body));

    doc.get_mut(col).unwrap().size.x = 90.0;
    doc.relayout_subtree(col);
    let n = doc.get(body).unwrap();
    assert_eq!(n.size, Vec2::new(90.0, 40.0));
    assert!(!keeps_glyphs(&doc, body));
}

#[test]
fn edited_auto_height_text_is_shaped_as_before() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    let mut doc = Document::new("stored-height-edited");
    let page = doc.add_page("P");
    let stored = Vec2::new(100.0, 19.25);
    let (col, body) = column_with_fill_text(&mut doc, page, 100.0, stored);
    set_content(&mut doc, body, "Hello!");

    doc.relayout_subtree(col);
    assert_eq!(doc.get(body).unwrap().size, Vec2::new(100.0, 40.0));
    assert!(!keeps_glyphs(&doc, body));
}

/// A 375-wide column padded 12.3 on each side, holding a Fill auto-height text of the stored size `stored`.
fn padded_column(doc: &mut Document, stored: Vec2, with_snapshot: bool) -> (NodeId, NodeId, NodeId) {
    let page = doc.add_page("P");
    let (col, body) = column_with_fill_text(doc, page, 375.0, stored);
    if let Some(NodeKind::Frame(fp)) = doc.get_mut(col).map(|n| &mut n.kind) {
        fp.auto_layout.as_mut().unwrap().padding = [12.3; 4];
    }
    if !with_snapshot {
        doc.get_mut(body).unwrap().fig_text_geometry = None;
    }
    let sibling = plain_text(doc, col, "Other");
    // A capped hug text is measured on every layout, so a content change moves its size.
    doc.get_mut(sibling).unwrap().layout_child.max_w = Some(400.0);
    (col, body, sibling)
}

/// Either the stored box, bit for bit, with the glyphs; or the text shaped in both dimensions without them.
fn assert_whole_box(doc: &Document, body: NodeId, stored: Vec2, label: &str) {
    let n = doc.get(body).unwrap();
    if keeps_glyphs(doc, body) {
        assert_eq!((n.size.x.to_bits(), n.size.y.to_bits()), (stored.x.to_bits(), stored.y.to_bits()), "{label}: glyphs kept around a changed box");
    } else {
        assert_eq!(n.size.y, 40.0, "{label}: a shaped width must come with a shaped height, got {:?}", n.size);
    }
}

#[test]
fn auto_height_text_in_a_padded_column_is_never_a_mixed_box() {
    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(Shaper));
    // The width the solver arrives at for this column, as an unrelated text sees it.
    let mut probe = Document::new("stored-height-probe");
    let (col, body, _) = padded_column(&mut probe, Vec2::new(10.0, 10.0), false);
    probe.relayout_subtree(col);
    let slot = probe.get(body).unwrap().size.x;

    for (label, x) in [
        ("exact slot width", slot),
        ("decimal value of the slot, the same f32", 350.4),
        ("one ulp above", f32::from_bits(slot.to_bits() + 1)),
        ("a pixel off", slot + 1.0),
    ] {
        let stored = Vec2::new(x, 19.25);
        let mut doc = Document::new("stored-height-padded");
        let (col, body, sibling) = padded_column(&mut doc, stored, true);
        doc.relayout_subtree(col);
        assert_whole_box(&doc, body, stored, label);

        let sibling_before = doc.get(sibling).unwrap().size;
        set_content(&mut doc, sibling, "A much longer sibling");
        doc.relayout_subtree(col);
        assert!(doc.get(sibling).unwrap().size != sibling_before, "{label}: the sibling change must reach the solver");
        assert_whole_box(&doc, body, stored, label);
        if x.to_bits() == slot.to_bits() {
            assert!(keeps_glyphs(&doc, body), "{label}: the slot width equals the stored width, so the box is Figma's");
        } else {
            assert!(!keeps_glyphs(&doc, body), "{label}: a different width is shaped");
        }
    }
}
