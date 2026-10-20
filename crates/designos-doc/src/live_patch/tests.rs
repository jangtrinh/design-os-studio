use super::*;
use crate::{Command, Comment, CommentAnchor, CommentThread, Document, History, Node, NodeId, Rect, Style, Vec2};

/// A page with `frames` top-level frames of `kids` rectangles each, one style, one comment.
fn full_doc(frames: usize, kids: usize) -> (Document, NodeId, Vec<NodeId>) {
    let mut d = Document::new("full");
    let page = d.add_page("Page");
    let mut tops = Vec::new();
    for f in 0..frames {
        let fid = d.alloc_id();
        d.insert(page, Node::frame(fid, Rect::new(f as f32 * 500.0, 0.0, 400.0, 300.0))).unwrap();
        for k in 0..kids {
            let kid = d.alloc_id();
            d.insert(fid, Node::rectangle(kid, Rect::new(k as f32 * 10.0, 0.0, 8.0, 8.0))).unwrap();
        }
        tops.push(fid);
    }
    d.styles.insert("S:1".into(), Style::Fill(Vec::new()));
    d.comments.push(thread("t-1", page, "keep"));
    (d, page, tops)
}

fn thread(id: &str, page: NodeId, text: &str) -> CommentThread {
    CommentThread {
        id: id.into(),
        anchor: CommentAnchor { page, node: None, offset: Vec2::new(1.0, 1.0) },
        messages: vec![Comment {
            id: format!("{id}-m"),
            author: "a".into(),
            author_id: None,
            author_color: None,
            text: text.into(),
            created: "2026-10-16T00:00:00Z".into(),
            edited: None,
        }],
        resolved: false,
    }
}

/// What the editor shares for large files: the page and its first `cap` frames, without
/// children, styles or comments.
fn scoped(full: &Document, page: NodeId, cap: usize) -> Document {
    let mut s = Document::new("scoped");
    let mut p = full.get(page).unwrap().clone();
    p.children.truncate(cap);
    for c in &p.children {
        let mut n = full.get(*c).unwrap().clone();
        n.children.clear();
        s.nodes.insert(n.id, n);
    }
    s.nodes.insert(page, p);
    s.pages.push(page);
    s.reserve_ids(NodeId(full.nodes.keys().next_back().unwrap().0));
    s
}

/// Runs `cmd` on the agent copy the way the MCP engine does, collecting the ops.
fn agent_exec(agent: &mut Document, hist: &mut History, ops: &mut Vec<LiveOp>, cmd: Command) {
    ops.extend(live_ops_for_command(agent, &cmd).unwrap());
    hist.exec(agent, cmd).unwrap();
}

fn replay(doc: &mut Document, ops: Vec<LiveOp>, state: &mut LivePatchState) -> LivePatchReport {
    replay_live_patch(doc, &LivePatch::new(ops), state, &mut |d, c| c.apply(d).map(|_| ())).unwrap()
}

#[test]
fn agent_edits_on_a_scoped_copy_replay_without_losing_the_rest() {
    let (mut full, page, tops) = full_doc(6, 5);
    let mut agent = scoped(&full, page, 4);
    let mut hist = History::default();
    let mut ops = Vec::new();

    // Insert a card on top of the page (the agent sees only 4 of 6 frames).
    let card = agent.alloc_id();
    let label = agent.alloc_id();
    let mut card_node = Node::frame(card, Rect::new(0.0, 900.0, 200.0, 100.0));
    card_node.children = vec![label];
    let mut label_node = Node::rectangle(label, Rect::new(0.0, 0.0, 10.0, 10.0));
    label_node.parent = Some(card);
    let index = agent.get(page).unwrap().children.len();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Insert { parent: page, index, nodes: vec![card_node, label_node] });

    // Rename a frame the agent holds without children.
    let mut renamed = agent.get(tops[1]).unwrap().clone();
    renamed.name = "Renamed".into();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Update { node: Box::new(renamed) });

    // A layout pass on the partial copy shrinks a frame; it is not a command and must not leak.
    agent.get_mut(tops[2]).unwrap().size = Vec2::new(0.0, 0.0);

    // Add a comment: the agent's copy has none, so its SetComments carries only the new thread.
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetComments { comments: vec![thread("t-2", page, "new")] });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.inserted_roots, vec![card]);
    for t in &tops {
        assert_eq!(full.get(*t).unwrap().children.len(), 5, "frame {t:?} lost its children");
    }
    assert_eq!(full.get(tops[2]).unwrap().size, Vec2::new(400.0, 300.0));
    assert_eq!(full.get(tops[1]).unwrap().name, "Renamed");
    assert_eq!(full.get(page).unwrap().children.last(), Some(&card), "the card goes on top, past frames the agent never saw");
    assert_eq!(full.get(card).unwrap().children, vec![label]);
    assert_eq!(full.styles.len(), 1);
    assert_eq!(full.comments.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["t-1", "t-2"]);
}

#[test]
fn a_repeated_patch_changes_nothing() {
    let (mut full, page, tops) = full_doc(2, 1);
    let mut agent = full.clone();
    let mut ops = Vec::new();
    let id = agent.alloc_id();
    agent_exec(
        &mut agent,
        &mut History::default(),
        &mut ops,
        Command::Insert { parent: tops[0], index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 4.0, 4.0))] },
    );
    let mut state = LivePatchState::default();
    replay(&mut full, ops.clone(), &mut state);
    let again = replay(&mut full, ops, &mut state);
    assert_eq!(again.applied, 0);
    assert_eq!(again.duplicates, 1);
    assert_eq!(full.get(tops[0]).unwrap().children.len(), 2);
    assert_eq!(full.get(page).unwrap().children.len(), 2);
}

#[test]
fn ids_the_editor_reused_meanwhile_get_fresh_ones() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = full.clone();
    // The user draws a layer in the editor; it takes the id the agent will also allocate.
    let taken = full.alloc_id();
    full.insert(page, Node::rectangle(taken, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();

    let mut hist = History::default();
    let mut ops = Vec::new();
    let id = agent.alloc_id();
    assert_eq!(id, taken);
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Insert { parent: page, index: 1, nodes: vec![Node::frame(id, Rect::new(0.0, 0.0, 9.0, 9.0))] });
    let mut moved = agent.get(id).unwrap().clone();
    moved.name = "Agent frame".into();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Update { node: Box::new(moved) });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    let fresh = report.inserted_roots[0];
    assert_ne!(fresh, taken);
    assert_eq!(full.get(taken).unwrap().size, Vec2::new(1.0, 1.0), "the user's layer is untouched");
    assert_eq!(full.get(fresh).unwrap().name, "Agent frame", "later ops follow the new id");
}

#[test]
fn remapped_ids_stop_applying_once_the_agent_reloads_the_editor_canvas() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = full.clone();
    let taken = full.alloc_id();
    full.insert(page, Node::rectangle(taken, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    let id = agent.alloc_id();
    assert_eq!(id, taken);
    let mut ops = Vec::new();
    agent_exec(
        &mut agent,
        &mut History::default(),
        &mut ops,
        Command::Insert { parent: page, index: 1, nodes: vec![Node::frame(id, Rect::new(0.0, 0.0, 9.0, 9.0))] },
    );
    let mut state = LivePatchState::default();
    let on = |base: u64, ops: Vec<LiveOp>| LivePatch::new(ops).with_base(Some(base));
    let exec = &mut |d: &mut Document, c: Command| c.apply(d).map(|_| ());
    let fresh = replay_live_patch(&mut full, &on(1, ops), &mut state, exec).unwrap().inserted_roots[0];
    let rename = |name: &str| vec![LiveOp::Update { id: taken, set: [("name".to_string(), serde_json::json!(name))].into_iter().collect(), unset: Vec::new() }];

    // Same canvas: the agent's id still means its own frame.
    replay_live_patch(&mut full, &on(1, rename("agent frame")), &mut state, exec).unwrap();
    assert_eq!(full.get(fresh).unwrap().name, "agent frame");
    // After a reload the agent holds the editor's ids, so the same id is the user's layer.
    replay_live_patch(&mut full, &on(2, rename("user layer")), &mut state, exec).unwrap();
    assert_eq!(full.get(taken).unwrap().name, "user layer");
    assert_eq!(full.get(fresh).unwrap().name, "agent frame");
}

#[test]
fn removes_and_moves_replay_on_the_full_subtree() {
    let (mut full, page, tops) = full_doc(3, 2);
    let mut agent = full.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();
    let kid = agent.get(tops[0]).unwrap().children[0];
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Reparent { id: kid, parent: tops[2], index: 0 });
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Remove { id: tops[1] });
    // Undo of the remove arrives as an insert of the subtree it brought back.
    let undo = hist_peek(&hist);
    ops.extend(live_ops_for_command(&agent, &undo).unwrap());
    hist.undo(&mut agent).unwrap();

    replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(full.get(kid).unwrap().parent, Some(tops[2]));
    assert_eq!(full.get(tops[2]).unwrap().children[0], kid);
    assert_eq!(full.get(tops[1]).unwrap().children.len(), 2);
    assert_eq!(full.get(page).unwrap().children, tops);
}

fn hist_peek(h: &History) -> Command {
    h.peek_undo().cloned().expect("undo entry")
}

#[test]
fn edits_the_editor_cannot_replay_are_refused() {
    let (doc, _, _) = full_doc(1, 0);
    let err = live_ops_for_command(&doc, &Command::SetFlows { flows: Default::default() }).unwrap_err();
    assert!(err.contains("prototype flows"), "{err}");
}

#[test]
fn sibling_hints_survive_different_views_of_the_parent() {
    let ids: Vec<NodeId> = (1..=6).map(NodeId).collect();
    let seen = &ids[..4];
    assert_eq!(SiblingHint::at(seen, 4).index_in(&ids), 6, "on top of a truncated list stays on top");
    assert_eq!(SiblingHint::at(seen, 0).index_in(&ids), 0);
    assert_eq!(SiblingHint::at(seen, 2).index_in(&ids), 2);
    assert_eq!(SiblingHint { after: Some(NodeId(99)), before: Some(NodeId(3)) }.index_in(&ids), 2);
}

#[test]
fn agent_comments_keep_the_editor_threads_and_follow_their_new_ids() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = full.clone();
    let mut hist = History::default();
    // Meanwhile the user starts `t-2` in the editor, the id the agent is about to use.
    full.comments.push(thread("t-2", page, "user"));

    let mut add = Vec::new();
    let mut with_t2 = agent.comments.clone();
    with_t2.push(thread("t-2", page, "agent"));
    agent_exec(&mut agent, &mut hist, &mut add, Command::SetComments { comments: with_t2 });
    let mut state = LivePatchState::default();
    replay(&mut full, add.clone(), &mut state);
    let texts = |d: &Document, id: &str| d.comments.iter().find(|t| t.id == id).map(|t| t.messages.iter().map(|m| m.text.clone()).collect::<Vec<_>>());
    assert_eq!(texts(&full, "t-1").unwrap(), ["keep"]);
    assert_eq!(texts(&full, "t-2").unwrap(), ["user"], "the user's thread was overwritten");
    assert_eq!(texts(&full, "t-2.2").unwrap(), ["agent"]);
    assert_eq!(replay(&mut full, add, &mut state).duplicates, 1, "a repeated patch adds nothing");
    assert_eq!(full.comments.len(), 3);

    // The agent replies in its `t-2` and resolves it; both land on `t-2.2`. A reply id the user's
    // own message took meanwhile gets a fresh one.
    full.comments.iter_mut().find(|t| t.id == "t-2.2").unwrap().messages.push(Comment {
        id: "t-2-r".into(),
        author: "u".into(),
        author_id: None,
        author_color: None,
        text: "user reply".into(),
        created: "2026-10-16T00:01:00Z".into(),
        edited: None,
    });
    let mut changed = agent.comments.clone();
    let mine = changed.iter_mut().find(|t| t.id == "t-2").unwrap();
    mine.messages.push(Comment {
        id: "t-2-r".into(),
        author: "a".into(),
        author_id: None,
        author_color: None,
        text: "agent reply".into(),
        created: "2026-10-16T00:02:00Z".into(),
        edited: None,
    });
    mine.resolved = true;
    let mut ops = Vec::new();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetComments { comments: changed });
    replay(&mut full, ops, &mut state);
    let t = full.comments.iter().find(|t| t.id == "t-2.2").unwrap();
    assert!(t.resolved);
    assert_eq!(t.messages.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), ["agent", "user reply", "agent reply"]);
    assert_eq!(t.messages.iter().map(|m| m.id.as_str()).collect::<std::collections::BTreeSet<_>>().len(), 3);
    assert!(!full.comments.iter().find(|t| t.id == "t-2").unwrap().resolved, "the user's thread stays open");
}
