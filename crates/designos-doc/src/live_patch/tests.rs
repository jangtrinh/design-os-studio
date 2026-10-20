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

fn collection(id: &str) -> crate::VariableCollection {
    crate::VariableCollection {
        extensions: Default::default(),
        id: id.into(),
        name: id.into(),
        modes: vec![crate::VariableMode { id: "m".into(), name: "Default".into() }],
        default_mode: "m".into(),
        variables: vec![format!("{id}/v")],
        key: None,
        source_library_key: None,
    }
}

fn variable(id: &str, collection: &str) -> crate::Variable {
    crate::Variable {
        extensions: Default::default(),
        mode_extensions: Default::default(),
        id: id.into(),
        name: "v".into(),
        collection: collection.into(),
        kind: crate::VariableKind::Number,
        values: [("m".to_string(), crate::VariableValue::Number(1.0))].into(),
        description: String::new(),
        scopes: Vec::new(),
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
    }
}

#[test]
fn variables_the_agent_adds_merge_into_the_editors_and_undo_removes_only_those() {
    // The editor holds a collection of its own that the agent's scoped copy lacks.
    let (mut full, page, _) = full_doc(1, 0);
    full.variable_collections.insert("user".into(), collection("user"));
    full.variables.insert("user/v".into(), variable("user/v", "user"));
    let user_data = (full.variables.clone(), full.variable_collections.clone());
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut ops = Vec::new();

    let id = agent.alloc_id();
    let mut vars = agent.variables.clone();
    vars.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    let mut colls = agent.variable_collections.clone();
    colls.insert("easeui".into(), collection("easeui"));
    let batch = Command::Batch(vec![
        Command::SetVariables { variables: vars, collections: colls },
        Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 4.0, 4.0))] },
    ]);
    agent_exec(&mut agent, &mut hist, &mut ops, batch);
    assert!(matches!(&ops[0], LiveOp::Variables { collections, variables, removed_collections, removed_variables }
        if collections.len() == 1 && variables.len() == 1 && removed_collections.is_empty() && removed_variables.is_empty()));

    let mut state = LivePatchState::default();
    let report = replay(&mut full, ops.clone(), &mut state);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.applied, 2);
    assert!(full.variable_collections.contains_key("easeui") && full.variables.contains_key("easeui/v"));
    assert!(full.variable_collections.contains_key("user") && full.variables.contains_key("user/v"), "the editor's own variables survive");

    // A repeated delivery changes nothing.
    let again = replay(&mut full, ops, &mut state);
    assert_eq!(again.applied, 0);
    assert_eq!(again.duplicates, 2);

    // Undoing on the agent removes exactly what the agent added.
    let undo = hist_peek(&hist);
    let mut undo_ops = live_ops_for_command(&agent, &undo).unwrap();
    hist.undo(&mut agent).unwrap();
    assert!(matches!(undo_ops.iter().find(|o| matches!(o, LiveOp::Variables { .. })),
        Some(LiveOp::Variables { removed_collections, removed_variables, collections, variables })
            if removed_collections == &["easeui".to_string()] && removed_variables == &["easeui/v".to_string()] && collections.is_empty() && variables.is_empty()));
    let report = replay(&mut full, std::mem::take(&mut undo_ops), &mut state);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!((full.variables.clone(), full.variable_collections.clone()), user_data);
}

/// The agent applies a spec that adds `easeui`/`easeui/v` on a scoped copy and then undoes it;
/// returns the ops of both steps.
fn agent_adds_then_undoes_easeui(full: &Document, page: NodeId) -> (Vec<LiveOp>, Vec<LiveOp>) {
    let mut agent = scoped(full, page, 1);
    let mut hist = History::default();
    let mut apply_ops = Vec::new();
    let id = agent.alloc_id();
    let mut vars = agent.variables.clone();
    vars.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    let mut colls = agent.variable_collections.clone();
    colls.insert("easeui".into(), collection("easeui"));
    let batch = Command::Batch(vec![
        Command::SetVariables { variables: vars, collections: colls },
        Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 4.0, 4.0))] },
    ]);
    agent_exec(&mut agent, &mut hist, &mut apply_ops, batch);
    let undo_ops = live_ops_for_command(&agent, &hist_peek(&hist)).unwrap();
    (apply_ops, undo_ops)
}

#[test]
fn an_agent_undo_never_removes_variables_the_editor_already_had() {
    // The editor's file already holds the collection (an earlier preset created it) and layers use it.
    let (mut full, page, tops) = full_doc(1, 0);
    full.variable_collections.insert("easeui".into(), collection("easeui"));
    full.variables.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    full.get_mut(tops[0]).unwrap().variables.bindings.insert("fills/0".into(), "easeui/v".into());
    let before = (full.variables.clone(), full.variable_collections.clone());
    let (apply_ops, undo_ops) = agent_adds_then_undoes_easeui(&full, page);

    let mut state = LivePatchState::default();
    replay(&mut full, apply_ops, &mut state);
    replay(&mut full, undo_ops, &mut state);

    assert_eq!((full.variables.clone(), full.variable_collections.clone()), before, "the editor's own collection must survive the agent's undo");
}

#[test]
fn an_agent_apply_never_overwrites_a_variable_the_user_changed() {
    let (mut full, page, _) = full_doc(1, 0);
    full.variable_collections.insert("easeui".into(), collection("easeui"));
    full.variables.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();

    // The user edits `easeui/v` in the editor while the agent applies a spec that already has `easeui/v`
    // and therefore does not touch it.
    let mut edited = variable("easeui/v", "easeui");
    edited.values.insert("m".into(), crate::VariableValue::Number(99.0));
    full.variables.insert("easeui/v".into(), edited.clone());

    let mut hist = History::default();
    let mut apply_ops = Vec::new();
    let id = agent.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 4.0, 4.0));
    node.variables.bindings.insert("fills/0".into(), "easeui/v".into());
    let batch = Command::Batch(vec![
        Command::SetVariables { variables: agent.variables.clone(), collections: agent.variable_collections.clone() },
        Command::Insert { parent: page, index: 0, nodes: vec![node] },
    ]);
    agent_exec(&mut agent, &mut hist, &mut apply_ops, batch);
    assert!(
        !apply_ops.iter().any(|op| matches!(op, LiveOp::Variables { .. })),
        "an apply that did not change an existing variable must not emit LiveOp::Variables"
    );

    replay(&mut full, apply_ops, &mut LivePatchState::default());

    assert_eq!(full.variables.get("easeui/v"), Some(&edited), "the user's edited value must stay");
}

#[test]
fn agent_set_variables_updates_existing_variable_on_replay() {
    let (mut full, page, _) = full_doc(1, 0);
    full.variable_collections.insert("c1".into(), collection("c1"));
    full.variables.insert("c1/primary".into(), variable("c1/primary", "c1"));

    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();

    let mut hist = History::default();
    let mut ops = Vec::new();
    let mut vars = agent.variables.clone();
    vars.get_mut("c1/primary").unwrap().values.insert("m".into(), crate::VariableValue::Number(42.0));
    let colls = agent.variable_collections.clone();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetVariables { variables: vars, collections: colls });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(report.applied, 1);
    assert_eq!(report.duplicates, 0);
    assert_eq!(full.variables["c1/primary"].values.get("m"), Some(&crate::VariableValue::Number(42.0)));
}

#[test]
fn agent_adding_variable_to_existing_collection_unions_member_list_on_replay() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut coll = collection("c1");
    coll.variables = vec!["c1/v1".into()];
    full.variable_collections.insert("c1".into(), coll);
    full.variables.insert("c1/v1".into(), variable("c1/v1", "c1"));

    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();

    // Meanwhile the editor also has `c1/editor_only` in `c1`.
    full.variable_collections.get_mut("c1").unwrap().variables.push("c1/editor_only".into());
    full.variables.insert("c1/editor_only".into(), variable("c1/editor_only", "c1"));

    let mut hist = History::default();
    let mut ops = Vec::new();
    let mut vars = agent.variables.clone();
    vars.insert("c1/v2".into(), variable("c1/v2", "c1"));
    let mut colls = agent.variable_collections.clone();
    colls.get_mut("c1").unwrap().variables.push("c1/v2".into());
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetVariables { variables: vars, collections: colls });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(report.applied, 1);
    assert!(full.variables.contains_key("c1/v2"));
    assert_eq!(
        full.variable_collections["c1"].variables,
        vec!["c1/v1".to_string(), "c1/editor_only".to_string(), "c1/v2".to_string()],
        "existing collection member list must union editor members with newly added agent members"
    );
}

#[test]
fn a_variable_op_without_an_id_is_refused() {
    let (mut full, _, _) = full_doc(1, 0);
    let before = full.variables.clone();
    let op = LiveOp::Variables { collections: Vec::new(), variables: vec![variable("", "c")], removed_collections: Vec::new(), removed_variables: Vec::new() };
    let report = replay(&mut full, vec![op], &mut LivePatchState::default());
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(full.variables, before);
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

#[test]
fn live_patch_update_flip() {
    use crate::{writer_command, AlignSelf, AutoLayout, FrameProps, LayoutDirection, NodeKind, Sizing};

    let build_unsolved = || {
        let mut doc = Document::new("live_flip");
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
        doc.insert(fid, child).unwrap();
        (doc, fid, cid)
    };

    // Direct flip through writer_command
    let (mut direct_doc, fid, cid) = build_unsolved();
    let mut flipped_frame = direct_doc.get(fid).unwrap().clone();
    flipped_frame.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().direction = LayoutDirection::Vertical;
    let w = writer_command(&direct_doc, Command::Update { node: Box::new(flipped_frame.clone()) });
    w.cmd.apply(&mut direct_doc).unwrap();
    direct_doc.relayout_subtree(fid);
    direct_doc.flush_pending_relayout();

    // Replay a single LiveOp::Update carrying only the frame's updated `kind` (as when the agent
    // edited a scoped copy that did not hold the frame's children).
    let (mut replay_doc, r_fid, r_cid) = build_unsolved();
    let mut set = serde_json::Map::new();
    let serialized = serde_json::to_value(&flipped_frame).unwrap();
    set.insert("kind".into(), serialized["kind"].clone());
    let report = replay(&mut replay_doc, vec![LiveOp::Update { id: r_fid, set, unset: Vec::new() }], &mut LivePatchState::default());
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.needs_layout, vec![r_fid]);
    replay_doc.relayout_subtree(r_fid);
    replay_doc.flush_pending_relayout();

    assert_eq!(replay_doc.get(r_fid).unwrap(), direct_doc.get(fid).unwrap());
    assert_eq!(replay_doc.get(r_cid).unwrap(), direct_doc.get(cid).unwrap());
    assert_eq!(replay_doc.get(r_cid).unwrap().layout_child.align_self, AlignSelf::Auto);
    assert_eq!(replay_doc.get(r_cid).unwrap().layout_child.vertical, Sizing::Fill);
}

#[test]
fn apply_spec_library_import_with_styles_variables_images_and_slots_replays_and_remaps_ids() {
    use crate::{
        fixtures, instance_component, snapshot, BlendMode, Color, ImageScaleMode, LibraryRef, NodeKind, NodeSpec, Paint, PaintKind, PropValue, Style, Variable,
        VariableCollection, VariableKind, VariableMode, VariableValue,
    };
    use serde_json::json;

    let mut lib_doc = fixtures::card_with_slots();
    lib_doc.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(12, 34, 56))]));
    let coll = VariableCollection {
        id: "c1".into(),
        name: "Tokens".into(),
        modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
        default_mode: "m1".into(),
        variables: vec!["v1".into()],
        key: None,
        source_library_key: None,
        extensions: Default::default(),
    };
    lib_doc.variable_collections.insert("c1".into(), coll);
    lib_doc.variables.insert(
        "v1".into(),
        Variable {
            id: "v1".into(),
            name: "Pad".into(),
            collection: "c1".into(),
            kind: VariableKind::Number,
            values: [("m1".to_string(), VariableValue::Number(12.0))].into(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    lib_doc.images.insert("dot_hash".into(), vec![0x89, 0x50, 0x4E, 0x47]);
    let card_comp_id = lib_doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Card").unwrap().id;
    {
        let card = lib_doc.get_mut(card_comp_id).unwrap();
        card.style_refs.insert("fill".into(), "Brand".into());
        card.variables.bindings.insert("itemSpacing".into(), "v1".into());
        card.fills.push(Paint {
            kind: PaintKind::Image { image_ref: "dot_hash".into(), scale_mode: ImageScaleMode::Fill },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        });
    }
    let snap = snapshot(&lib_doc, "ui_kit", "UI Kit", 1);

    let (mut full, page, _) = full_doc(1, 0);
    full.library.enabled.push(LibraryRef { id: "ui_kit".into(), name: "UI Kit".into(), version: 1 });
    let mut agent = full.clone();
    let mut hist = History::default();

    // Editor creates local nodes whose IDs collide with the IDs the agent will allocate for
    // `library_page`, imported components, and placed instances.
    for _ in 0..8 {
        let eid = full.alloc_id();
        full.insert(page, Node::rectangle(eid, Rect::new(0.0, 0.0, 8.0, 8.0))).unwrap();
    }

    let spec: NodeSpec = serde_json::from_value(json!({
        "kind": "instance",
        "name": "MirroredCard",
        "component": "Card",
        "slots": {
            "body": [
                { "kind": "instance", "name": "MirroredRow", "component": "FormRow" }
            ],
            "footer": false
        }
    }))
    .unwrap();

    let (cmd, _report) = crate::apply_spec_command(&mut agent, page, &spec, std::slice::from_ref(&snap)).unwrap();
    let mut ops = Vec::new();
    agent_exec(&mut agent, &mut hist, &mut ops, cmd);

    let mut state = LivePatchState::default();
    let rep = replay(&mut full, ops, &mut state);
    assert!(rep.skipped.is_empty(), "no live ops may be skipped: {:?}", rep.skipped);

    let editor_lp = full.library_page.expect("editor must have library_page after replay");
    assert!(full.get(editor_lp).is_some(), "editor library_page node must exist in full.nodes");
    assert!(full.styles.contains_key("ui_kit/Brand"), "imported style must reach editor");
    assert!(full.variables.contains_key("ui_kit/v1"), "imported variable must reach editor");
    assert!(full.images.contains_key("dot_hash"), "imported image must reach editor");

    let editor_inst = full
        .nodes
        .values()
        .find(|n| matches!(n.kind, NodeKind::Instance { .. }) && n.name == "MirroredCard")
        .expect("MirroredCard instance must exist on editor");
    let editor_comp_id = instance_component(editor_inst).expect("MirroredCard must stay linked to a component");
    let editor_comp = full.get(editor_comp_id).expect("linked component must exist in editor.nodes");
    assert_eq!(editor_comp.parent, Some(editor_lp), "imported component must sit under editor's library_page");

    // Every PropValue::Slot on the imported component must reference a valid default child inside a slot container of `editor_comp`.
    for def in &editor_comp.component.props {
        if let PropValue::Slot(sids) = &def.default {
            for sid in sids {
                assert!(full.is_ancestor(editor_comp_id, *sid), "remapped PropValue::Slot({sid:?}) must be a descendant of editor_comp");
            }
        }
    }
    for cfg in editor_comp.component.slot_configs.values() {
        for &pref_id in &cfg.preferred {
            let pref = full.get(pref_id).expect("remapped slot_configs.preferred must exist on editor");
            assert_eq!(pref.parent, Some(editor_lp), "remapped preferred component must sit under editor's library_page");
        }
    }
    // Every direct slot container of `editor_inst` must have `component.source` pointing into `editor_comp`.
    for &cid in &editor_inst.children {
        let child = full.get(cid).unwrap();
        let src = child.component.source.expect("slot container on instance must keep component.source");
        assert!(editor_comp.children.contains(&src), "remapped component.source {src:?} must point to a child of editor_comp");
    }

    // Undo on agent and replay onto editor cleans up the instance, imported component, styles, vars, and images.
    let undo_cmd = hist_peek(&hist);
    let undo_ops = live_ops_for_command(&agent, &undo_cmd).unwrap();
    hist.undo(&mut agent).unwrap();
    let undo_rep = replay(&mut full, undo_ops, &mut state);
    assert!(undo_rep.skipped.is_empty(), "undo replay must not skip ops: {:?}", undo_rep.skipped);
    assert!(full.nodes.values().all(|n| n.name != "MirroredCard"));
    assert!(!full.styles.contains_key("ui_kit/Brand"));
    assert!(!full.variables.contains_key("ui_kit/v1"));
    assert!(!full.images.contains_key("dot_hash"));
}
