//! Shared writer-boundary command normalization (`writer_command`) for headless writers
//! (`designos-mcp` `Engine::exec` and `live_patch` replay).
//!
//! `Command::apply` is shared by undo, redo, `Tx::cmd`, and main-component instance sync, so
//! writer-time rewrites that add companion child or slot updates run here before a writer records
//! live ops or pushes the command onto `History`.

use crate::{is_instance_slot, AutoLayout, Command, Document, Node, NodeId, OverrideField};
use std::collections::BTreeSet;

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
    match cmd {
        Command::Update { .. } => update_boundary_step(doc, cmd),
        other => slot_content_marked(doc, other),
    }
}

/// Emits companion [`Command::Update`]s for un-renamed instances when a main component
/// or component set is renamed (`#1193`).
pub fn companion_component_rename_updates(doc: &Document, target_id: NodeId, old_name: &str, new_name: &str) -> Vec<Command> {
    if old_name == new_name {
        return Vec::new();
    }
    let Some(stored) = doc.get(target_id) else {
        return Vec::new();
    };
    let is_standalone_component = matches!(stored.kind, crate::NodeKind::Component(_))
        && !stored.parent.and_then(|pid| doc.get(pid)).is_some_and(|p| matches!(p.kind, crate::NodeKind::ComponentSet(_)));
    if !is_standalone_component && !matches!(stored.kind, crate::NodeKind::ComponentSet(_)) {
        return Vec::new();
    }

    let mut updated_ids = BTreeSet::from([target_id]);
    let mut updates = Vec::new();
    let comp_ids = crate::components_of(doc, target_id);
    for inst_id in crate::instances_of_any(doc, &comp_ids) {
        let Some(inst) = doc.get(inst_id) else { continue };
        if !inst.component.overrides.contains(&OverrideField::Name) && inst.name == old_name && updated_ids.insert(inst_id) {
            let mut updated = Node { children: Vec::new(), ..inst.clone() };
            updated.name = new_name.to_string();
            updates.push(Command::Update { node: Box::new(updated) });
        }
    }

    updates
}

fn update_boundary_step(doc: &Document, cmd: Command) -> WriterCommand {
    let Command::Update { mut node } = cmd else {
        return WriterCommand::single(cmd);
    };
    let Some(stored) = doc.get(node.id) else {
        return WriterCommand::single(Command::Update { node });
    };
    if matches!(node.kind, crate::NodeKind::Instance { .. }) && stored.name != node.name {
        node.component.overrides.insert(OverrideField::Name);
    }
    let rename_updates = companion_component_rename_updates(doc, node.id, &stored.name, &node.name);
    let mut wc = direction_flip_step(doc, Command::Update { node });
    if rename_updates.is_empty() {
        return wc;
    }
    for r_cmd in rename_updates {
        if let Command::Update { node: ref r_node } = r_cmd {
            if let Some(Command::Update { node: existing }) = wc.parts.iter_mut().find(|p| matches!(p, Command::Update { node } if node.id == r_node.id)) {
                existing.name = r_node.name.clone();
                continue;
            }
        }
        wc.parts.push(r_cmd);
    }
    if wc.parts.len() == 1 {
        WriterCommand::single(wc.parts.pop().unwrap())
    } else {
        WriterCommand { cmd: Command::Batch(wc.parts.clone()), parts: wc.parts }
    }
}

/// Slot-content override marking (#582): when a writer command (`Insert`, `Reparent`, `Remove`,
/// or a `Batch` containing them) mutates the direct child list of an instance slot container
/// ([`crate::is_instance_slot`]) that does not yet carry [`OverrideField::SlotContent`], prepends
/// a [`Command::Update`] on the slot adding [`OverrideField::SlotContent`] to its
/// `component.overrides`.
pub fn slot_content_marked(doc: &Document, cmd: Command) -> WriterCommand {
    let mut marked = BTreeSet::new();
    match cmd {
        Command::Batch(cmds) => {
            let mut slot_updates = Vec::new();
            let mut expanded = Vec::with_capacity(cmds.len());
            for sub in &cmds {
                append_slot_marked_single(doc, sub.clone(), &mut marked, &mut slot_updates, &mut expanded);
            }
            let mut parts = slot_updates;
            parts.push(Command::Batch(cmds));
            WriterCommand { cmd: Command::Batch(expanded), parts }
        }
        other => {
            let mut slot_updates = Vec::new();
            let mut parts = Vec::with_capacity(2);
            append_slot_marked_single(doc, other, &mut marked, &mut slot_updates, &mut parts);
            if parts.len() == 1 {
                let single = parts.pop().unwrap();
                WriterCommand::single(single)
            } else {
                WriterCommand { cmd: Command::Batch(parts.clone()), parts }
            }
        }
    }
}

fn append_slot_marked_single(doc: &Document, cmd: Command, marked: &mut BTreeSet<NodeId>, slot_updates: &mut Vec<Command>, out: &mut Vec<Command>) {
    match cmd {
        Command::Insert { ref parent, .. } => {
            maybe_mark_slot(doc, *parent, marked, slot_updates, out);
            out.push(cmd);
        }
        Command::Reparent { ref id, ref parent, .. } => {
            if let Some(src_parent) = doc.get(*id).and_then(|n| n.parent) {
                maybe_mark_slot(doc, src_parent, marked, slot_updates, out);
            }
            maybe_mark_slot(doc, *parent, marked, slot_updates, out);
            out.push(cmd);
        }
        Command::Remove { ref id } => {
            if let Some(parent) = doc.get(*id).and_then(|n| n.parent) {
                maybe_mark_slot(doc, parent, marked, slot_updates, out);
            }
            out.push(cmd);
        }
        Command::Update { ref node } => {
            if node.component.overrides.contains(&OverrideField::SlotContent) {
                marked.insert(node.id);
            }
            out.push(cmd);
        }
        Command::Batch(cmds) => {
            for sub in cmds {
                append_slot_marked_single(doc, sub, marked, slot_updates, out);
            }
        }
        other => out.push(other),
    }
}

fn maybe_mark_slot(doc: &Document, slot_id: NodeId, marked: &mut BTreeSet<NodeId>, slot_updates: &mut Vec<Command>, out: &mut Vec<Command>) {
    if marked.contains(&slot_id) || !is_instance_slot(doc, slot_id) {
        return;
    }
    let Some(slot) = doc.get(slot_id) else { return };
    if slot.component.overrides.contains(&OverrideField::SlotContent) {
        marked.insert(slot_id);
        return;
    }
    marked.insert(slot_id);
    let mut updated = Node { children: Vec::new(), ..slot.clone() };
    updated.component.overrides.insert(OverrideField::SlotContent);
    let update = Command::Update { node: Box::new(updated) };
    slot_updates.push(update.clone());
    out.push(update);
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
        convert_to_slot, create_instance, instances_of,
        live_patch::{replay_live_patch, LiveOp, LivePatch, LivePatchState, SiblingHint},
        sync_instances, AlignSelf, AutoLayout, FrameProps, History, LayoutDirection, NodeId, NodeKind, Rect, Sizing, Vec2,
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

    /// Builds a document with a `Card` component containing a `BodySlot` slot container (and an
    /// optional default child inside `BodySlot`), plus one instance of `Card`.
    fn component_with_instance_slot(with_default_child: bool) -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut doc = Document::new("slots");
        let page = doc.add_page("Page");
        let comp = doc.alloc_id();
        let mut comp_node = Node::new(comp, NodeKind::Component(FrameProps::default()));
        comp_node.name = "Card".into();
        comp_node.size = Vec2::new(200.0, 120.0);
        doc.insert(page, comp_node).unwrap();

        let main_slot = doc.alloc_id();
        let mut slot_frame = Node::frame(main_slot, Rect::new(10.0, 10.0, 180.0, 100.0));
        slot_frame.name = "BodySlot".into();
        doc.insert(comp, slot_frame).unwrap();

        if with_default_child {
            let def_child = doc.alloc_id();
            let mut dc = Node::rectangle(def_child, Rect::new(0.0, 0.0, 40.0, 20.0));
            dc.name = "DefaultItem".into();
            doc.insert(main_slot, dc).unwrap();
        }

        convert_to_slot(&mut doc, main_slot).apply(&mut doc).unwrap();

        create_instance(&mut doc, comp, page, Vec2::new(240.0, 0.0)).apply(&mut doc).unwrap();
        let inst = instances_of(&doc, comp)[0];
        let inst_slot = doc.get(inst).unwrap().children[0];
        assert!(is_instance_slot(&doc, inst_slot));
        assert!(!doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));

        (doc, comp, main_slot, inst, inst_slot)
    }

    #[test]
    fn live_patch_insert_is_marked_in_studio() {
        let (mut doc, comp, _main_slot, _inst, inst_slot) = component_with_instance_slot(true);
        let default_mirror = doc.get(inst_slot).unwrap().children[0];

        // 1. Replay a standalone LiveOp::Insert into the instance slot (without a preceding LiveOp::Update).
        let custom_id = doc.alloc_id();
        let mut custom = Node::rectangle(custom_id, Rect::new(0.0, 0.0, 60.0, 24.0));
        custom.name = "CustomBadge".into();
        let patch = LivePatch::new(vec![LiveOp::Insert { parent: inst_slot, at: SiblingHint::default(), nodes: vec![custom] }]);
        let report = replay_live_patch(&mut doc, &patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert!(doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));

        // Main component edit + sync_instances must keep CustomBadge in the slot.
        doc.get_mut(comp).unwrap().size.x = 260.0;
        sync_instances(&mut doc, comp).apply(&mut doc).unwrap();
        assert!(doc.get(inst_slot).unwrap().children.contains(&custom_id));

        // 2. Replay a LiveOp::Remove of a default mirror child on a fresh unmarked instance slot.
        let (mut doc_rem, comp_rem, _, _, inst_slot_rem) = component_with_instance_slot(true);
        let mirror_child = doc_rem.get(inst_slot_rem).unwrap().children[0];
        let rem_patch = LivePatch::new(vec![LiveOp::Remove { id: mirror_child }]);
        replay_live_patch(&mut doc_rem, &rem_patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert!(doc_rem.get(inst_slot_rem).unwrap().component.overrides.contains(&OverrideField::SlotContent));
        sync_instances(&mut doc_rem, comp_rem).apply(&mut doc_rem).unwrap();
        assert!(doc_rem.get(inst_slot_rem).unwrap().children.is_empty(), "removed default mirror must stay deleted after sync");

        // 3. Replay a LiveOp::Reparent moving a node out of one unmarked instance slot into another.
        let (mut doc_rep, _comp_rep, _, _, inst_slot_a) = component_with_instance_slot(true);
        let page = doc_rep.pages[0];
        let comp_id = doc_rep.get(inst_slot_a).unwrap().component.source.and_then(|s| doc_rep.get(s)).and_then(|n| n.parent).unwrap();
        create_instance(&mut doc_rep, comp_id, page, Vec2::new(500.0, 0.0)).apply(&mut doc_rep).unwrap();
        let inst_b = instances_of(&doc_rep, comp_id)[1];
        let inst_slot_b = doc_rep.get(inst_b).unwrap().children[0];
        let moving_child = doc_rep.get(inst_slot_a).unwrap().children[0];
        assert_eq!(moving_child, default_mirror);
        let rep_patch = LivePatch::new(vec![LiveOp::Reparent { id: moving_child, parent: inst_slot_b, at: SiblingHint::default() }]);
        replay_live_patch(&mut doc_rep, &rep_patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert!(doc_rep.get(inst_slot_a).unwrap().component.overrides.contains(&OverrideField::SlotContent), "source slot marked");
        assert!(doc_rep.get(inst_slot_b).unwrap().component.overrides.contains(&OverrideField::SlotContent), "destination slot marked");
    }

    #[test]
    fn undo_of_marked_insert_clears_the_mark() {
        let (mut doc, _comp, _main_slot, _inst, inst_slot) = component_with_instance_slot(false);
        let mut hist = History::default();

        let cid = doc.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 50.0, 20.0));
        child.name = "Inserted".into();

        let w = writer_command(&doc, Command::Insert { parent: inst_slot, index: 0, nodes: vec![child] });
        assert_eq!(w.parts.len(), 2, "expected slot Update + Insert");
        hist.exec(&mut doc, w.cmd).unwrap();

        assert_eq!(doc.get(inst_slot).unwrap().children, vec![cid]);
        assert!(doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));
        assert_eq!(hist.undo_depth(), 1, "mark + insert must be a single undo step");

        assert!(hist.undo(&mut doc).unwrap());
        assert!(doc.get(inst_slot).unwrap().children.is_empty());
        assert!(!doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent), "undo must clear SlotContent mark");

        assert!(hist.redo(&mut doc).unwrap());
        assert_eq!(doc.get(inst_slot).unwrap().children, vec![cid]);
        assert!(doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent), "redo must restore SlotContent mark");
    }

    #[test]
    fn idempotent_second_insert() {
        let (mut doc, _comp, _main_slot, _inst, inst_slot) = component_with_instance_slot(false);

        let c1 = doc.alloc_id();
        let n1 = Node::rectangle(c1, Rect::new(0.0, 0.0, 30.0, 20.0));
        let c2 = doc.alloc_id();
        let n2 = Node::rectangle(c2, Rect::new(0.0, 30.0, 30.0, 20.0));

        // Two inserts in one batch -> only one Update on the slot.
        let batch = Command::Batch(vec![
            Command::Insert { parent: inst_slot, index: 0, nodes: vec![n1] },
            Command::Insert { parent: inst_slot, index: 1, nodes: vec![n2] },
        ]);
        let w = writer_command(&doc, batch);
        let update_count = w.parts.iter().filter(|c| matches!(c, Command::Update { node } if node.id == inst_slot)).count();
        assert_eq!(update_count, 1, "two inserts in one batch must emit only one slot Update");
        assert_eq!(w.parts.len(), 2, "parts must hold the slot Update followed by Command::Batch(original sub-commands)");
        assert!(matches!(&w.parts[1], Command::Batch(sub) if sub.len() == 2));
        assert!(matches!(&w.cmd, Command::Batch(all) if all.len() == 3));
        w.cmd.apply(&mut doc).unwrap();

        // A subsequent insert into the already-marked slot emits no Update at all.
        let c3 = doc.alloc_id();
        let n3 = Node::rectangle(c3, Rect::new(0.0, 60.0, 30.0, 20.0));
        let w2 = writer_command(&doc, Command::Insert { parent: inst_slot, index: 2, nodes: vec![n3] });
        assert_eq!(w2.parts.len(), 1, "already-marked slot must not emit another Update");
        assert!(matches!(w2.cmd, Command::Insert { .. }));
    }

    #[test]
    fn sync_does_not_mark() {
        let (mut doc, comp, main_slot, _inst, inst_slot) = component_with_instance_slot(false);

        // Inserting into the main component's own slot container must NOT mark the main slot.
        let main_child = doc.alloc_id();
        let mut mc = Node::rectangle(main_child, Rect::new(0.0, 0.0, 40.0, 20.0));
        mc.name = "MainDefault".into();
        let w_main = writer_command(&doc, Command::Insert { parent: main_slot, index: 0, nodes: vec![mc] });
        assert_eq!(w_main.parts.len(), 1, "main-component slot is not an instance slot and must not be marked");
        w_main.cmd.apply(&mut doc).unwrap();
        assert!(!doc.get(main_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));

        // Running sync_instances copies the new default child into the instance slot without marking SlotContent.
        sync_instances(&mut doc, comp).apply(&mut doc).unwrap();
        assert_eq!(doc.get(inst_slot).unwrap().children.len(), 1);
        assert!(!doc.get(inst_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent), "sync_instances must not mark SlotContent");
    }
}
