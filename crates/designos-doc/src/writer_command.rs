//! Shared writer-boundary command normalization (`writer_command`) for headless writers
//! (`designos-mcp` `Engine::exec` and `live_patch` replay).
//!
//! `Command::apply` is shared by undo, redo, `Tx::cmd`, and main-component instance sync, so
//! writer-time rewrites that add companion child or slot updates run here before a writer records
//! live ops or pushes the command onto `History`.

use crate::{AutoLayout, Command, Document, Node};

/// Normalized command produced by [`writer_command`].
///
/// `cmd` is the command to execute and push onto the undo stack (a [`Command::Batch`] when a
/// boundary step expanded the write, otherwise the original command), and `parts` lists the same
/// commands un-batched in execution order. Because the parts are independent of one another (each
/// targets a distinct node and none reads fields changed by an earlier part), live-patch recording
/// can inspect each part against the unmodified [`Document`] via
/// [`crate::live_patch::live_ops_for_independent`] without cloning the scene graph.
#[derive(Clone, Debug)]
pub struct WriterCommand {
    pub cmd: Command,
    pub parts: Vec<Command>,
}

impl WriterCommand {
    fn single(cmd: Command) -> Self {
        Self { cmd: cmd.clone(), parts: vec![cmd] }
    }
}

/// Applies writer-boundary normalization steps to a command about to be executed by a headless
/// writer (`Engine::exec`, live-patch replay).
pub fn writer_command(doc: &Document, cmd: Command) -> WriterCommand {
    direction_flip_step(doc, cmd)
}

/// Direction-flip normalization (#583): when a `Command::Update` changes a frame's auto-layout
/// horizontality (`AutoLayout::primary_is_horizontal` differs between the stored frame and the
/// updated frame), emits explicit child `Command::Update`s for direct children whose
/// `stretched.last_parent_horiz` is `None` and whose `layout_child` is changed by
/// `LayoutChild::apply_parent_direction_change(old_horiz, new_horiz)`.
fn direction_flip_step(doc: &Document, cmd: Command) -> WriterCommand {
    let Command::Update { node } = cmd else {
        return WriterCommand::single(cmd);
    };
    let Some(stored) = doc.get(node.id) else {
        return WriterCommand::single(Command::Update { node });
    };
    let old_horiz = stored.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).map(AutoLayout::primary_is_horizontal);
    let new_horiz = node.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).map(AutoLayout::primary_is_horizontal);

    let (Some(old_h), Some(new_h)) = (old_horiz, new_horiz) else {
        return WriterCommand::single(Command::Update { node });
    };
    if old_h == new_h {
        return WriterCommand::single(Command::Update { node });
    }

    let mut parts = Vec::with_capacity(1 + stored.children.len());
    parts.push(Command::Update { node });

    for &cid in &stored.children {
        let Some(child) = doc.get(cid) else { continue };
        if child.layout_child.stretched.last_parent_horiz.is_some() {
            continue;
        }
        let mut lc = child.layout_child;
        lc.apply_parent_direction_change(old_h, new_h);
        if lc != child.layout_child {
            let mut updated = Node { children: Vec::new(), ..child.clone() };
            updated.layout_child = lc;
            parts.push(Command::Update { node: Box::new(updated) });
        }
    }

    if parts.len() == 1 {
        WriterCommand::single(parts.pop().unwrap())
    } else {
        WriterCommand { cmd: Command::Batch(parts.clone()), parts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        create_instance, instances_of, sync_instances, AlignSelf, AutoLayout, FrameProps, History, LayoutDirection, NodeId, NodeKind, Rect, Sizing, Vec2,
    };

    /// Builds a document with an unsolved horizontal auto-layout frame (200x120) and one child
    /// (50x30) carrying the imported counter-axis stretch pair (`vertical = Fill`,
    /// `align_self = Stretch`, `stretched.last_parent_horiz = None`).
    fn unsolved_horiz_frame_with_pair_child() -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("flip");
        let page = doc.add_page("Page");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 120.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Horizontal;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
        frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        doc.insert(page, frame).unwrap();

        let cid = doc.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 50.0, 30.0));
        child.layout_child.horizontal = Sizing::Fixed;
        child.layout_child.vertical = Sizing::Fill;
        child.layout_child.align_self = AlignSelf::Stretch;
        assert_eq!(child.layout_child.stretched.last_parent_horiz, None);
        doc.insert(fid, child).unwrap();

        (doc, fid, cid)
    }

    fn direction_update(doc: &Document, frame_id: NodeId, dir: LayoutDirection) -> Command {
        let mut updated = doc.get(frame_id).unwrap().clone();
        updated.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().direction = dir;
        Command::Update { node: Box::new(updated) }
    }

    #[test]
    fn flip_before_first_solve_leaves_one_signal() {
        let (mut doc, fid, cid) = unsolved_horiz_frame_with_pair_child();
        let w = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Vertical));
        assert_eq!(w.parts.len(), 2, "expected parent update + 1 child update");
        w.cmd.apply(&mut doc).unwrap();
        doc.relayout_subtree(fid);
        doc.flush_pending_relayout();

        let child = doc.get(cid).unwrap();
        assert_eq!(child.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(child.layout_child.vertical, Sizing::Fill);
        assert_eq!(child.layout_child.horizontal, Sizing::Fixed);
        // Width stays fixed at 50px (does not stretch to 200px along the new counter axis),
        // while height fills the 120px vertical primary axis.
        assert_eq!(child.size, Vec2::new(50.0, 120.0));
    }

    #[test]
    fn flip_back_is_symmetric() {
        let (mut doc, fid, cid) = unsolved_horiz_frame_with_pair_child();
        let w1 = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Vertical));
        w1.cmd.apply(&mut doc).unwrap();
        doc.relayout_subtree(fid);
        doc.flush_pending_relayout();

        let w2 = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Horizontal));
        w2.cmd.apply(&mut doc).unwrap();
        doc.relayout_subtree(fid);
        doc.flush_pending_relayout();

        let child = doc.get(cid).unwrap();
        assert_eq!(child.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(child.layout_child.vertical, Sizing::Fill);
        assert_eq!(child.layout_child.horizontal, Sizing::Fixed);
        assert_eq!(child.size, Vec2::new(50.0, 120.0));

        // Symmetric starting from Vertical with horizontal = Fill + align_self = Stretch:
        let mut doc_v = Document::new("flip_v");
        let page = doc_v.add_page("Page");
        let vfid = doc_v.alloc_id();
        let mut vframe = Node::frame(vfid, Rect::new(0.0, 0.0, 200.0, 120.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Vertical;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
        vframe.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        doc_v.insert(page, vframe).unwrap();

        let vcid = doc_v.alloc_id();
        let mut vchild = Node::rectangle(vcid, Rect::new(0.0, 0.0, 50.0, 30.0));
        vchild.layout_child.horizontal = Sizing::Fill;
        vchild.layout_child.vertical = Sizing::Fixed;
        vchild.layout_child.align_self = AlignSelf::Stretch;
        doc_v.insert(vfid, vchild).unwrap();

        let wv = writer_command(&doc_v, direction_update(&doc_v, vfid, LayoutDirection::Horizontal));
        wv.cmd.apply(&mut doc_v).unwrap();
        doc_v.relayout_subtree(vfid);
        doc_v.flush_pending_relayout();

        let vchild_after = doc_v.get(vcid).unwrap();
        assert_eq!(vchild_after.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(vchild_after.layout_child.horizontal, Sizing::Fill);
        assert_eq!(vchild_after.layout_child.vertical, Sizing::Fixed);
        assert_eq!(vchild_after.size, Vec2::new(200.0, 30.0));
    }

    #[test]
    fn undo_restores_pair() {
        let (mut doc, fid, cid) = unsolved_horiz_frame_with_pair_child();
        let mut hist = History::default();
        let w = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Vertical));
        hist.exec(&mut doc, w.cmd).unwrap();
        doc.relayout_subtree(fid);
        doc.flush_pending_relayout();

        assert!(hist.undo(&mut doc).unwrap());
        doc.flush_pending_relayout();

        let frame = doc.get(fid).unwrap();
        assert_eq!(frame.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().direction, LayoutDirection::Horizontal);
        let child = doc.get(cid).unwrap();
        assert_eq!(child.layout_child.vertical, Sizing::Fill);
        assert_eq!(child.layout_child.align_self, AlignSelf::Stretch);
    }

    #[test]
    fn redo_after_undo_matches_the_first_flip() {
        let (mut doc, fid, cid) = unsolved_horiz_frame_with_pair_child();
        let mut hist = History::default();
        let w = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Vertical));
        hist.exec(&mut doc, w.cmd).unwrap();
        doc.relayout_subtree(fid);
        doc.flush_pending_relayout();

        let first_frame = doc.get(fid).unwrap().clone();
        let first_child = doc.get(cid).unwrap().clone();
        let first_depth = hist.undo_depth();

        assert!(hist.undo(&mut doc).unwrap());
        doc.flush_pending_relayout();
        assert!(hist.redo(&mut doc).unwrap());
        doc.flush_pending_relayout();

        assert_eq!(hist.undo_depth(), first_depth);
        assert_eq!(doc.get(fid).unwrap(), &first_frame);
        assert_eq!(doc.get(cid).unwrap(), &first_child);
    }

    #[test]
    fn solved_child_not_double_rewritten() {
        let (mut unsolved_doc, u_fid, u_cid) = unsolved_horiz_frame_with_pair_child();
        let wu = writer_command(&unsolved_doc, direction_update(&unsolved_doc, u_fid, LayoutDirection::Vertical));
        wu.cmd.apply(&mut unsolved_doc).unwrap();
        unsolved_doc.relayout_subtree(u_fid);
        unsolved_doc.flush_pending_relayout();

        let (mut solved_doc, s_fid, s_cid) = unsolved_horiz_frame_with_pair_child();
        // Solve once before flipping so `stretched.last_parent_horiz` is `Some(true)`.
        solved_doc.relayout_subtree(s_fid);
        solved_doc.flush_pending_relayout();
        assert_eq!(solved_doc.get(s_cid).unwrap().layout_child.stretched.last_parent_horiz, Some(true));

        let ws = writer_command(&solved_doc, direction_update(&solved_doc, s_fid, LayoutDirection::Vertical));
        assert_eq!(ws.parts.len(), 1, "solved child has Some(last_parent_horiz) so boundary must not emit a duplicate child update");
        ws.cmd.apply(&mut solved_doc).unwrap();
        solved_doc.relayout_subtree(s_fid);
        solved_doc.flush_pending_relayout();

        let u_child = unsolved_doc.get(u_cid).unwrap();
        let s_child = solved_doc.get(s_cid).unwrap();
        assert_eq!(s_child.layout_child.align_self, u_child.layout_child.align_self);
        assert_eq!(s_child.layout_child.vertical, u_child.layout_child.vertical);
        assert_eq!(s_child.layout_child.horizontal, u_child.layout_child.horizontal);
        assert_eq!(s_child.layout_child.stretched.last_parent_horiz, u_child.layout_child.stretched.last_parent_horiz);
        assert_eq!(s_child.size, u_child.size);
    }

    #[test]
    fn sync_after_a_main_direction_flip_is_unaffected() {
        let mut doc = Document::new("sync");
        let page = doc.add_page("Page");
        let comp = doc.alloc_id();
        let mut comp_node = Node::frame(comp, Rect::new(0.0, 0.0, 200.0, 120.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Horizontal;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
        comp_node.kind = NodeKind::Component(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        doc.insert(page, comp_node).unwrap();

        let main_child = doc.alloc_id();
        let mut mc = Node::rectangle(main_child, Rect::new(0.0, 0.0, 50.0, 30.0));
        mc.layout_child.horizontal = Sizing::Fixed;
        mc.layout_child.vertical = Sizing::Fill;
        mc.layout_child.align_self = AlignSelf::Stretch;
        doc.insert(comp, mc).unwrap();

        create_instance(&mut doc, comp, page, Vec2::new(300.0, 0.0)).apply(&mut doc).unwrap();
        let inst = instances_of(&doc, comp)[0];
        let inst_child = doc.get(inst).unwrap().children[0];

        // Flip the main component's frame through the writer boundary, then run sync_instances.
        let w = writer_command(&doc, direction_update(&doc, comp, LayoutDirection::Vertical));
        w.cmd.apply(&mut doc).unwrap();

        let sync_cmd = sync_instances(&mut doc, comp);
        match &sync_cmd {
            Command::Batch(cmds) => {
                // One update for the instance root and one for its mirrored child; no extra boundary rewrite.
                assert_eq!(cmds.len(), 2);
            }
            other => panic!("expected Command::Batch from sync_instances, got {other:?}"),
        }
        sync_cmd.apply(&mut doc).unwrap();

        let main_c = doc.get(main_child).unwrap();
        let inst_c = doc.get(inst_child).unwrap();
        assert_eq!(inst_c.layout_child, main_c.layout_child);
        assert_eq!(inst_c.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(inst_c.layout_child.vertical, Sizing::Fill);
    }

    #[test]
    fn same_horizontality_untouched() {
        let (doc, fid, _) = unsolved_horiz_frame_with_pair_child();
        let w = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Grid));
        assert_eq!(w.parts.len(), 1);
        match &w.cmd {
            Command::Update { node } => {
                assert_eq!(node.id, fid);
                assert_eq!(node.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().direction, LayoutDirection::Grid);
            }
            other => panic!("expected unchanged Command::Update for Horizontal -> Grid, got {other:?}"),
        }
    }

    #[test]
    fn children_without_the_pair_emit_no_update() {
        let mut doc = Document::new("plain");
        let page = doc.add_page("Page");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 120.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Horizontal;
        frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        doc.insert(page, frame).unwrap();

        // Child 1: plain Fixed/Fixed/Auto
        let c1 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c1, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();

        // Child 2: Stretch only (no Fill) — edge case 4: untouched, keeps stretching the new counter axis
        let c2 = doc.alloc_id();
        let mut stretch_only = Node::rectangle(c2, Rect::new(0.0, 0.0, 40.0, 20.0));
        stretch_only.layout_child.align_self = AlignSelf::Stretch;
        doc.insert(fid, stretch_only).unwrap();

        let w = writer_command(&doc, direction_update(&doc, fid, LayoutDirection::Vertical));
        assert_eq!(w.parts.len(), 1, "plain and stretch-only children must not emit child updates");
        match &w.cmd {
            Command::Update { node } => assert_eq!(node.id, fid),
            other => panic!("expected single Command::Update when no child needed normalization, got {other:?}"),
        }
    }
}
