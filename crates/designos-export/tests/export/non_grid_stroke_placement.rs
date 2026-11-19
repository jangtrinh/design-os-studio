//! A Center or Outside stroke on a layer that is not a grid is an outline in CSS and in Tailwind,
//! so the stroke sits on the box the way the canvas draws it and does not shrink the content.
//! An Inside stroke stays a border, including per-side widths and a dash.

use designos_doc::*;
use designos_export::{css_for, tailwind_for};

fn rectangle(align: StrokeAlign, edit: impl FnOnce(&mut Node)) -> (Document, NodeId) {
    let mut doc = Document::new("stroke");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 80.0, 40.0));
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    n.stroke.weight = 4.0;
    n.stroke.align = align;
    edit(&mut n);
    let id = doc.insert(page, n).unwrap();
    (doc, id)
}

#[test]
fn center_stroke_on_a_rectangle_is_an_outline() {
    let (doc, id) = rectangle(StrokeAlign::Center, |_| {});
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\noutline: 4px solid #333;\noutline-offset: -2px;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 outline outline-4 outline-[#333333] outline-offset-[-2px]\"></div>");
}

#[test]
fn outside_stroke_on_a_rectangle_is_an_outline() {
    let (doc, id) = rectangle(StrokeAlign::Outside, |_| {});
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\noutline: 4px solid #333;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 outline outline-4 outline-[#333333]\"></div>");
}

#[test]
fn inside_stroke_on_a_rectangle_stays_a_border() {
    let (doc, id) = rectangle(StrokeAlign::Inside, |_| {});
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\nborder: 4px solid #333;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 border-4 border-[#333333]\"></div>");
}

#[test]
fn dashed_center_stroke_on_a_rectangle_is_a_dashed_outline() {
    let (doc, id) = rectangle(StrokeAlign::Center, |n| n.stroke.dashes = vec![4.0, 2.0]);
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\noutline: 4px dashed #333;\noutline-offset: -2px;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 outline outline-4 outline-dashed outline-[#333333] outline-offset-[-2px]\"></div>");
}

#[test]
fn dashed_inside_stroke_on_a_rectangle_stays_a_dashed_border() {
    let (doc, id) = rectangle(StrokeAlign::Inside, |n| n.stroke.dashes = vec![4.0, 2.0]);
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\nborder: 4px dashed #333;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 border-4 border-dashed border-[#333333]\"></div>");
}

#[test]
fn per_side_inside_stroke_on_a_rectangle_stays_side_borders() {
    let (doc, id) = rectangle(StrokeAlign::Inside, |n| n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]));
    assert_eq!(css_for(&doc, id), "width: 80px;\nheight: 40px;\nborder-width: 4px 1px 2px 3px;\nborder-style: solid;\nborder-color: #333;");
    assert_eq!(tailwind_for(&doc, id), "<div class=\"w-20 h-10 border-t-4 border-r border-b-2 border-l-[3px] border-[#333333]\"></div>");
}
