//! `render_node` must refuse absurd output sizes instead of aborting on the allocation.

use designos_doc::{Document, Node, NodeId, Rect};
use designos_render::{checked_pixmap_size, render_node, MAX_PIXMAP_PIXELS, MAX_PIXMAP_SIDE};

fn doc_with_frame(w: f32, h: f32) -> (Document, NodeId) {
    let mut d = Document::new("Big");
    let page = d.add_page("P");
    let id = d.alloc_id();
    d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, w, h))).unwrap();
    (d, id)
}

#[test]
fn huge_node_is_rejected_not_allocated() {
    let (d, id) = doc_with_frame(1e6, 1e6);
    assert!(render_node(&d, id, 4.0).is_none());
}

#[test]
fn huge_scale_on_a_small_node_is_rejected() {
    let (d, id) = doc_with_frame(100.0, 100.0);
    assert!(render_node(&d, id, 1e9).is_none());
    assert!(render_node(&d, id, f32::INFINITY).is_none());
}

#[test]
fn one_pixel_over_the_side_limit_is_rejected_and_the_limit_itself_renders() {
    let (d, id) = doc_with_frame(16_385.0, 1.0);
    assert!(render_node(&d, id, 1.0).is_none());
    let (d, id) = doc_with_frame(16_384.0, 1.0);
    let pm = render_node(&d, id, 1.0).expect("the side limit itself is allowed");
    assert_eq!((pm.width(), pm.height()), (16_384, 1));
}

#[test]
fn pixel_budget_equals_the_side_limit_squared() {
    // The budget used to be smaller than side x side, so a square at the side limit was refused. It now
    // matches, and checking the size function avoids allocating 1 GiB in a test.
    assert_eq!(MAX_PIXMAP_PIXELS, u64::from(MAX_PIXMAP_SIDE) * u64::from(MAX_PIXMAP_SIDE));
    assert_eq!(checked_pixmap_size(16_384.0, 16_384.0), Some((16_384, 16_384)));
    assert_eq!(checked_pixmap_size(16_385.0, 16_384.0), None);
}

#[test]
fn large_legitimate_exports_are_allowed_and_absurd_ones_rejected() {
    assert_eq!(checked_pixmap_size(16_000.0, 12_000.0), Some((16_000, 12_000)));
    // 4000x3000 at 4x = 192 Mpx
    assert_eq!(checked_pixmap_size(4000.0 * 4.0, 3000.0 * 4.0), Some((16_000, 12_000)));
    assert_eq!(checked_pixmap_size(1e6, 1e6), None);
    assert_eq!(checked_pixmap_size(f32::NAN, 10.0), None);
    assert_eq!(checked_pixmap_size(10.0, f32::INFINITY), None);
}

#[test]
fn ordinary_sizes_are_unchanged() {
    let (d, id) = doc_with_frame(30.0, 20.0);
    let pm = render_node(&d, id, 2.0).unwrap();
    assert_eq!((pm.width(), pm.height()), (60, 40));
}
