//! A hugging cross axis keeps the natural size of `align_self` stretch children when nothing else
//! sizes the line, through `Document::relayout_subtree`.

use designos_doc::{AlignSelf, AutoLayout, Document, FrameProps, LayoutAlign, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing};

/// A fixed-width horizontal frame hugging its height, holding one rectangle per `(w, h, align_self)`.
fn hugging_row(kids: &[(f32, f32, AlignSelf)]) -> (Document, NodeId, Vec<NodeId>) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 1.0));
    let al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_align: LayoutAlign::Start,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let frame = d.insert(page, frame).unwrap();
    let ids = kids
        .iter()
        .map(|&(w, h, align_self)| {
            let id = d.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
            n.layout_child.align_self = align_self;
            d.insert(frame, n).unwrap()
        })
        .collect();
    (d, frame, ids)
}

fn heights(d: &Document, frame: NodeId, kids: &[NodeId]) -> (f32, Vec<f32>) {
    (d.get(frame).unwrap().size.y, kids.iter().map(|id| d.get(*id).unwrap().size.y).collect())
}

#[test]
fn a_lone_stretch_child_keeps_its_height_in_a_hugging_frame() {
    let (mut d, frame, kids) = hugging_row(&[(30.0, 20.0, AlignSelf::Stretch)]);
    d.relayout_subtree(frame);
    assert_eq!(heights(&d, frame, &kids), (20.0, vec![20.0]));
}

#[test]
fn stretch_children_all_take_the_tallest_natural_height() {
    let (mut d, frame, kids) = hugging_row(&[(30.0, 20.0, AlignSelf::Stretch), (30.0, 100.0, AlignSelf::Stretch)]);
    d.relayout_subtree(frame);
    assert_eq!(heights(&d, frame, &kids), (100.0, vec![100.0, 100.0]));
}

#[test]
fn a_plain_sibling_sizes_the_line_and_the_frame_shrinks_back_with_it() {
    let (mut d, frame, kids) = hugging_row(&[(30.0, 100.0, AlignSelf::Auto), (30.0, 20.0, AlignSelf::Stretch)]);
    d.relayout_subtree(frame);
    assert_eq!(heights(&d, frame, &kids), (100.0, vec![100.0, 100.0]));

    // the stretch child now stores 100; shrinking the plain sibling must still pull the frame down
    d.get_mut(kids[0]).unwrap().size.y = 50.0;
    d.relayout_subtree(frame);
    assert_eq!(heights(&d, frame, &kids), (50.0, vec![50.0, 50.0]));
}
