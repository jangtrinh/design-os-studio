//! The editor solver (`layout::compute`) and the headless spec solver (`Document::relayout_subtree`)
//! must put every child at the same position for the same frame.

use super::*;
use designos_doc::{FrameProps, Node};

fn auto(direction: LayoutDirection, primary: LayoutAlign) -> AutoLayout {
    AutoLayout {
        direction,
        primary_align: primary,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        padding: [0.0; 4],
        item_spacing: 0.0,
        counter_spacing: 0.0,
        ..Default::default()
    }
}

type Origins = Vec<(f32, f32)>;

/// Child origins from the editor solver, then from the spec solver run on a real document.
fn positions_from_both(al: AutoLayout, frame: Vec2, kids: &[(f32, f32)]) -> (Origins, Origins) {
    let ins: Vec<ChildIn> = kids.iter().map(|&(w, h)| ChildIn { size: v2(w, h), main_fill: false, cross_fill: false, grow: 0.0 }).collect();
    let editor = compute(&al, frame, &ins).rects.iter().map(|r| (r.x, r.y)).collect();

    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, frame.x, frame.y));
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let f = d.insert(page, f).unwrap();
    let mut ids = vec![];
    for &(w, h) in kids {
        let id = d.alloc_id();
        ids.push(d.insert(f, Node::rectangle(id, Rect::new(0.0, 0.0, w, h))).unwrap());
    }
    d.relayout_subtree(f);
    let spec = ids.iter().map(|id| (d.get(*id).unwrap().transform.tx, d.get(*id).unwrap().transform.ty)).collect();
    (editor, spec)
}

fn frame_for(direction: LayoutDirection, main: f32, cross: f32) -> Vec2 {
    if direction == LayoutDirection::Horizontal {
        v2(main, cross)
    } else {
        v2(cross, main)
    }
}

/// `n` children of `main` x `cross` (main along the flow, cross across it).
fn kids_for(direction: LayoutDirection, n: usize, main: f32, cross: f32) -> Vec<(f32, f32)> {
    let size = if direction == LayoutDirection::Horizontal { (main, cross) } else { (cross, main) };
    vec![size; n]
}

const DIRECTIONS: [LayoutDirection; 2] = [LayoutDirection::Horizontal, LayoutDirection::Vertical];

#[test]
fn space_around_and_evenly_agree_with_free_space() {
    for direction in DIRECTIONS {
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut al = auto(direction, align);
            al.item_spacing = 6.0;
            let kids = kids_for(direction, 4, 10.0, 10.0);
            let (editor, spec) = positions_from_both(al, frame_for(direction, 120.0, 20.0), &kids);
            assert_eq!(editor, spec, "{direction:?} {align:?}");
        }
    }
}

#[test]
fn space_around_and_evenly_overflow_pack_from_the_start_ignoring_item_spacing() {
    // four 40-long items in a 120 frame overflow: no free space, so no gap, not the 10 px item spacing
    for direction in DIRECTIONS {
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut al = auto(direction, align);
            al.item_spacing = 10.0;
            let kids = kids_for(direction, 4, 40.0, 10.0);
            let frame = frame_for(direction, 120.0, 20.0);
            let (editor, spec) = positions_from_both(al, frame, &kids);
            let main = |p: &(f32, f32)| if direction == LayoutDirection::Horizontal { p.0 } else { p.1 };
            assert_eq!(editor.iter().map(main).collect::<Vec<_>>(), vec![0.0, 40.0, 80.0, 120.0], "editor {direction:?} {align:?}");
            assert_eq!(editor, spec, "{direction:?} {align:?}");
        }
    }
}

#[test]
fn wrapped_space_between_agrees_with_counter_align_content() {
    // 100-long main axis holds two 40-long items per line: three lines of 10, counter gap spread over a 100 frame
    for direction in DIRECTIONS {
        for content in [CounterAlignContent::Auto, CounterAlignContent::SpaceBetween] {
            let mut al = auto(direction, LayoutAlign::SpaceBetween);
            al.wrap = true;
            al.item_spacing = 4.0;
            al.counter_spacing = 2.0;
            al.counter_align_content = content;
            let kids = kids_for(direction, 6, 40.0, 10.0);
            let (editor, spec) = positions_from_both(al, frame_for(direction, 100.0, 100.0), &kids);
            assert_eq!(editor, spec, "{direction:?} {content:?}");
        }
    }
}

#[test]
fn wrapped_counter_space_between_spreads_lines_edge_to_edge() {
    // literal: lines of cross 10 in a 100 cross frame, gap = (100 - 30) / 2 = 35, so lines at 0, 45, 90
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
    al.wrap = true;
    al.counter_align_content = CounterAlignContent::SpaceBetween;
    let (editor, spec) = positions_from_both(al, v2(100.0, 100.0), &[(40.0, 10.0); 6]);
    let ys = |p: &[(f32, f32)]| p.iter().map(|q| q.1).collect::<Vec<_>>();
    assert_eq!(ys(&editor), vec![0.0, 0.0, 45.0, 45.0, 90.0, 90.0]);
    assert_eq!(ys(&spec), ys(&editor));
}
