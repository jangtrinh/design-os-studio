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
    page_flow_library_commands_are_still_refused();
}

#[test]
fn page_flow_library_commands_are_still_refused() {
    let (doc, page, _) = full_doc(1, 0);
    for (cmd, expected) in [
        (Command::InsertPage { index: 0, nodes: Vec::new() }, "Adding a page"),
        (Command::RemovePage { id: page }, "Removing a page"),
        (Command::SetFlows { flows: Vec::new() }, "Editing prototype flows"),
        (Command::SetGuides { page, guides: Vec::new() }, "Editing guides"),
        (Command::SetPrototypeDevice { device: Default::default() }, "Editing the prototype device"),
        (Command::SetCheckDesignsSettings(Default::default()), "Editing check designs settings"),
    ] {
        let err = live_ops_for_command(&doc, &cmd).unwrap_err();
        assert!(err.contains(expected), "expected '{expected}' in '{err}'");
    }
}

#[test]
fn a_bind_on_the_scoped_canvas_merges_with_unchanged_ids() {
    let (mut full, page, tops) = full_doc(2, 2);
    full.variable_collections.insert("col:user".into(), collection("col:user"));
    full.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut agent = scoped(&full, page, 2);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();

    let mut frame = agent.get(tops[0]).unwrap().clone();
    frame.variables.bindings.insert("fills/0".into(), "col:user/v".into());
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Update { node: Box::new(frame) });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(full.get(tops[0]).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("col:user/v"));
    assert_eq!(full.get(tops[0]).unwrap().children.len(), 2, "frame kept its children");
}

#[test]
fn a_variable_edit_on_the_scoped_canvas_merges_without_duplicates() {
    let (mut full, page, _) = full_doc(1, 0);
    full.variable_collections.insert("col:user".into(), collection("col:user"));
    full.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();

    let mut vars = agent.variables.clone();
    vars.get_mut("col:user/v").unwrap().values.insert("m".into(), crate::VariableValue::Number(42.0));
    let colls = agent.variable_collections.clone();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetVariables { variables: vars, collections: colls });

    let mut state = LivePatchState::default();
    let report = replay(&mut full, ops, &mut state);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.tokens_applied, 1);
    assert_eq!(full.variables.len(), 1);
    assert_eq!(full.variables["col:user/v"].values.get("m"), Some(&crate::VariableValue::Number(42.0)));
}

#[test]
fn a_style_edit_on_the_scoped_canvas_merges_without_duplicates() {
    let (mut full, page, _) = full_doc(1, 0);
    full.style_meta.insert("S:1".into(), crate::StyleMeta { name: "Brand/Primary".into(), ..Default::default() });
    let mut agent = scoped(&full, page, 1);
    agent.styles = full.styles.clone();
    agent.style_meta = full.style_meta.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();

    let mut styles = agent.styles.clone();
    styles.insert("S:1".into(), Style::Fill(vec![crate::Paint::solid(crate::Color::rgb(10, 20, 30))]));
    let mut meta = agent.style_meta.clone();
    meta.get_mut("S:1").unwrap().description = "Updated by agent".into();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetStyles { styles, style_meta: meta });

    let mut state = LivePatchState::default();
    let report = replay(&mut full, ops, &mut state);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.tokens_applied, 1);
    assert_eq!(full.styles.len(), 1);
    assert_eq!(full.styles["S:1"], Style::Fill(vec![crate::Paint::solid(crate::Color::rgb(10, 20, 30))]));
    assert_eq!(full.style_meta["S:1"].description, "Updated by agent");
}

#[test]
fn replaying_a_token_patch_twice_changes_nothing() {
    let (mut full, page, _) = full_doc(1, 0);
    full.variable_collections.insert("col:user".into(), collection("col:user"));
    full.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();
    agent.styles = full.styles.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();

    let mut vars = agent.variables.clone();
    vars.get_mut("col:user/v").unwrap().values.insert("m".into(), crate::VariableValue::Number(7.0));
    let mut styles = agent.styles.clone();
    styles.insert("S:2".into(), Style::Fill(Vec::new()));
    let colls = agent.variable_collections.clone();
    let meta = agent.style_meta.clone();
    agent_exec(
        &mut agent,
        &mut hist,
        &mut ops,
        Command::Batch(vec![Command::SetVariables { variables: vars, collections: colls }, Command::SetStyles { styles, style_meta: meta }]),
    );

    let mut state = LivePatchState::default();
    let first = replay(&mut full, ops.clone(), &mut state);
    assert_eq!(first.tokens_applied, 2);
    let snapshot = (full.variables.clone(), full.styles.clone());

    let second = replay(&mut full, ops, &mut state);
    assert_eq!(second.applied, 0);
    assert_eq!(second.duplicates, 2);
    assert_eq!(second.tokens_duplicates, 2);
    assert_eq!((full.variables, full.styles), snapshot);
}

#[test]
fn a_bind_to_a_deleted_variable_is_skipped_with_a_reason() {
    let (mut full, page, tops) = full_doc(1, 0);
    full.variable_collections.insert("col:user".into(), collection("col:user"));
    full.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();
    // User deletes the variable in the editor before the agent's bind replays.
    full.variables.remove("col:user/v");

    let mut hist = History::default();
    let mut ops = Vec::new();
    let mut frame = agent.get(tops[0]).unwrap().clone();
    frame.variables.bindings.insert("fills/0".into(), "col:user/v".into());
    agent_exec(&mut agent, &mut hist, &mut ops, Command::Update { node: Box::new(frame) });

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(report.applied, 0);
    assert_eq!(report.duplicates, 0);
    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].contains("variable 'col:user/v' not found"), "{:?}", report.skipped);
    assert!(full.get(tops[0]).unwrap().variables.bindings.is_empty(), "no dangling binding may land");
}

#[test]
fn token_ops_are_one_undo_step_with_their_patch() {
    let (mut full, page, tops) = full_doc(1, 0);
    full.variable_collections.insert("col:user".into(), collection("col:user"));
    full.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut agent = scoped(&full, page, 1);
    agent.variable_collections = full.variable_collections.clone();
    agent.variables = full.variables.clone();
    agent.styles = full.styles.clone();
    let mut agent_hist = History::default();
    let mut ops = Vec::new();

    let mut vars = agent.variables.clone();
    vars.get_mut("col:user/v").unwrap().values.insert("m".into(), crate::VariableValue::Number(12.0));
    let mut styles = agent.styles.clone();
    styles.insert("S:new".into(), Style::Fill(Vec::new()));
    let mut frame = agent.get(tops[0]).unwrap().clone();
    frame.name = "Tokenized Frame".into();
    frame.variables.bindings.insert("width".into(), "col:user/v".into());
    frame.style_refs.insert("fill".into(), "S:new".into());
    let colls = agent.variable_collections.clone();
    let meta = agent.style_meta.clone();
    agent_exec(
        &mut agent,
        &mut agent_hist,
        &mut ops,
        Command::Batch(vec![
            Command::SetVariables { variables: vars, collections: colls },
            Command::SetStyles { styles, style_meta: meta },
            Command::Update { node: Box::new(frame) },
        ]),
    );

    let mut state = LivePatchState::default();
    let mut cmds = Vec::new();
    let report = replay_live_patch(&mut full, &LivePatch::new(ops), &mut state, &mut |d, c| {
        cmds.push(c.clone());
        c.apply(d).map(|_| ())
    })
    .unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.applied, 3);
    assert_eq!(report.tokens_applied, 2);

    // Committing the patch as a single batch creates 1 undo entry.
    let mut full_before = full_doc(1, 0).0;
    full_before.variable_collections.insert("col:user".into(), collection("col:user"));
    full_before.variables.insert("col:user/v".into(), variable("col:user/v", "col:user"));
    let mut editor_hist = History::default();
    editor_hist.exec(&mut full_before, Command::Batch(cmds)).unwrap();
    assert!(editor_hist.can_undo());
    assert!(editor_hist.undo(&mut full_before).unwrap());
    assert!(!editor_hist.can_undo());
    assert_eq!(full_before.variables["col:user/v"].values.get("m"), Some(&crate::VariableValue::Number(1.0)));
    assert!(!full_before.styles.contains_key("S:new"));
}

#[test]
fn old_reader_reports_the_new_op_as_unreadable() {
    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum OldLiveOp {
        Insert { parent: NodeId },
        Update { id: NodeId },
        Reparent { id: NodeId },
        Remove { id: NodeId },
        Comments { removed: Vec<String> },
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    struct OldLivePatch {
        ops: Vec<OldLiveOp>,
    }

    let mut styles_set = std::collections::BTreeMap::new();
    styles_set.insert("S:1".to_string(), Style::Fill(Vec::new()));
    let patch = LivePatch::new(vec![LiveOp::Styles { styles_set, styles_removed: Vec::new(), meta_set: Default::default(), meta_removed: Vec::new() }]);
    let line = serde_json::to_vec(&patch).unwrap();
    let old_res = serde_json::from_slice::<OldLivePatch>(&line);
    assert!(old_res.is_err(), "an older reader without the Styles variant must reject the line as unreadable");
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

#[test]
fn styles_the_agent_adds_merge_into_the_editors_and_undo_removes_only_those() {
    let (mut full, page, _) = full_doc(1, 1);
    let user_styles = (full.styles.clone(), full.style_meta.clone());
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut ops = Vec::new();

    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), crate::Style::Fill(vec![crate::Paint::solid(crate::Color::rgb(10, 20, 30))]));
    let mut meta = agent.style_meta.clone();
    meta.insert(
        "s_agent".into(),
        crate::StyleMeta {
            name: "Agent/Fill".into(),
            key: None,
            description: String::new(),
            property: crate::StyleProperty::Fill,
            bindings: Default::default(),
            ..Default::default()
        },
    );
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetStyles { styles, style_meta: meta });

    let mut state = LivePatchState::default();
    let r = replay(&mut full, ops, &mut state);
    assert_eq!(r.applied, 1);
    assert!(full.styles.contains_key("s_agent"));
    assert_eq!(full.styles.len(), 2, "editor style and agent style both present");

    let undo = hist_peek(&hist);
    let undo_ops = live_ops_for_command(&agent, &undo).unwrap();
    hist.undo(&mut agent).unwrap();
    let _ = replay(&mut full, undo_ops, &mut state);
    assert_eq!((full.styles.clone(), full.style_meta.clone()), user_styles);
}

fn red_fill(r: u8) -> Style {
    Style::Fill(vec![crate::Paint::solid(crate::Color::rgb(r, 0, 0))])
}

/// Runs a style-only `SetStyles` on the agent copy, keeping its style metadata.
fn set_styles(agent: &mut Document, hist: &mut History, ops: &mut Vec<LiveOp>, styles: BTreeMap<String, Style>) {
    let style_meta = agent.style_meta.clone();
    agent_exec(agent, hist, ops, Command::SetStyles { styles, style_meta });
}

#[test]
fn an_agent_upsert_of_an_existing_editor_style_replays_and_changes_it() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    agent.styles = full.styles.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("S:1".into(), red_fill(255));
    set_styles(&mut agent, &mut hist, &mut ops, styles);

    let report = replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(report.applied, 1);
    assert_eq!(report.duplicates, 0);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(full.styles["S:1"], red_fill(255), "last write wins on an existing editor style");
}

#[test]
fn a_user_edit_after_an_agent_style_write_is_preserved_with_a_skipped_reason() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();

    let mut ops1 = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(10));
    set_styles(&mut agent, &mut hist, &mut ops1, styles);
    assert_eq!(replay(&mut full, ops1, &mut state).applied, 1);

    // The agent updates its own style: applied.
    let mut ops2 = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(20));
    set_styles(&mut agent, &mut hist, &mut ops2, styles);
    let r2 = replay(&mut full, ops2, &mut state);
    assert_eq!((r2.applied, r2.duplicates), (1, 0));
    assert_eq!(full.styles["s_agent"], red_fill(20));

    // The user edits it; a later agent write must not overwrite it, and says why.
    full.styles.insert("s_agent".into(), red_fill(99));
    let mut ops3 = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(30));
    set_styles(&mut agent, &mut hist, &mut ops3, styles);
    let r3 = replay(&mut full, ops3, &mut state);
    assert_eq!(full.styles["s_agent"], red_fill(99), "the user's value stays");
    assert_eq!(r3.applied, 0);
    assert_eq!(r3.duplicates, 0, "a refused write is not a duplicate");
    assert_eq!(r3.skipped.len(), 1, "{:?}", r3.skipped);
    assert!(r3.skipped[0].contains("s_agent"), "{:?}", r3.skipped);
}

#[test]
fn a_style_the_agent_created_before_a_canvas_reload_can_still_be_updated() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();
    let mut ops1 = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(10));
    set_styles(&mut agent, &mut hist, &mut ops1, styles);
    let _ = replay(&mut full, ops1, &mut state);

    let mut ops2 = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(20));
    set_styles(&mut agent, &mut hist, &mut ops2, styles);
    let mut reloaded = LivePatch::new(ops2);
    reloaded.base = Some(7);
    let r = replay_live_patch(&mut full, &reloaded, &mut state, &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    assert_eq!(r.applied, 1, "{:?}", r.skipped);
    assert_eq!(full.styles["s_agent"], red_fill(20));
}

#[test]
fn an_agent_removal_of_an_existing_editor_style_applies() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    agent.styles = full.styles.clone();
    let mut hist = History::default();
    let mut ops = Vec::new();
    set_styles(&mut agent, &mut hist, &mut ops, BTreeMap::new());
    let r = replay(&mut full, ops, &mut LivePatchState::default());
    assert_eq!(r.applied, 1);
    assert!(!full.styles.contains_key("S:1"), "a plain agent delete applies");
}

#[test]
fn later_edit_to_agent_introduced_variable_replays_while_user_edit_is_preserved() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();

    // 1. Agent introduces easeui/v = 16.0
    let mut ops1 = Vec::new();
    let mut v16 = variable("easeui/v", "easeui");
    v16.values.insert("m".into(), crate::VariableValue::Number(16.0));
    let mut vars1 = agent.variables.clone();
    vars1.insert("easeui/v".into(), v16);
    let mut colls1 = agent.variable_collections.clone();
    colls1.insert("easeui".into(), collection("easeui"));
    agent_exec(&mut agent, &mut hist, &mut ops1, Command::SetVariables { variables: vars1, collections: colls1 });
    let r1 = replay(&mut full, ops1, &mut state);
    assert_eq!(r1.applied, 1);
    assert_eq!(full.variables["easeui/v"].values["m"], crate::VariableValue::Number(16.0));

    // 2. Agent updates easeui/v from 16.0 to 24.0 in the same session
    let mut ops2 = Vec::new();
    let mut v24 = variable("easeui/v", "easeui");
    v24.values.insert("m".into(), crate::VariableValue::Number(24.0));
    let mut vars2 = agent.variables.clone();
    vars2.insert("easeui/v".into(), v24);
    let colls2 = agent.variable_collections.clone();
    agent_exec(&mut agent, &mut hist, &mut ops2, Command::SetVariables { variables: vars2, collections: colls2 });
    let r2 = replay(&mut full, ops2, &mut state);
    assert_eq!(r2.applied, 1, "follow-up edit to agent-introduced variable must apply, not be dropped as duplicate");
    assert_eq!(r2.duplicates, 0);
    assert_eq!(full.variables["easeui/v"].values["m"], crate::VariableValue::Number(24.0));

    // 3. User edits easeui/v in the editor to 99.0; a later agent edit to 32.0 must NOT overwrite it
    full.variables.get_mut("easeui/v").unwrap().values.insert("m".into(), crate::VariableValue::Number(99.0));
    let mut ops3 = Vec::new();
    let mut v32 = variable("easeui/v", "easeui");
    v32.values.insert("m".into(), crate::VariableValue::Number(32.0));
    let mut vars3 = agent.variables.clone();
    vars3.insert("easeui/v".into(), v32);
    let colls3 = agent.variable_collections.clone();
    agent_exec(&mut agent, &mut hist, &mut ops3, Command::SetVariables { variables: vars3, collections: colls3 });
    let r3 = replay(&mut full, ops3, &mut state);
    assert_eq!(r3.duplicates, 0, "a refused write is not a duplicate");
    assert_eq!(r3.skipped.len(), 1, "{:?}", r3.skipped);
    assert!(r3.skipped[0].contains("easeui/v"), "{:?}", r3.skipped);
    assert_eq!(full.variables["easeui/v"].values["m"], crate::VariableValue::Number(99.0));
}

/// Undoes the agent's latest history entry and replays its live ops onto the editor document.
fn agent_undo_and_replay(agent: &mut Document, hist: &mut History, full: &mut Document, state: &mut LivePatchState) -> LivePatchReport {
    let undo_ops = live_ops_for_command(agent, &hist_peek(hist)).unwrap();
    hist.undo(agent).unwrap();
    replay(full, undo_ops, state)
}

#[test]
fn agent_updates_its_own_variable_and_undo_preserves_user_edits() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();

    // 1. The agent creates collection `easeui` and variable `easeui/v` (value 1.0).
    let mut vars1 = agent.variables.clone();
    vars1.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    let mut colls1 = agent.variable_collections.clone();
    colls1.insert("easeui".into(), collection("easeui"));
    let mut ops1 = Vec::new();
    agent_exec(&mut agent, &mut hist, &mut ops1, Command::SetVariables { variables: vars1, collections: colls1.clone() });
    assert_eq!(replay(&mut full, ops1, &mut state).applied, 1);

    // 2. The agent updates its own variable to 24.0 in a second tool call.
    let mut vars2 = agent.variables.clone();
    let mut updated = variable("easeui/v", "easeui");
    updated.values.insert("m".into(), crate::VariableValue::Number(24.0));
    vars2.insert("easeui/v".into(), updated.clone());
    let mut ops2 = Vec::new();
    agent_exec(&mut agent, &mut hist, &mut ops2, Command::SetVariables { variables: vars2, collections: colls1 });
    assert_eq!(replay(&mut full, ops2, &mut state).applied, 1);
    assert_eq!(full.variables.get("easeui/v"), Some(&updated));

    // 3. The user edits the variable in the editor to 99.0.
    let mut user_edited = updated.clone();
    user_edited.values.insert("m".into(), crate::VariableValue::Number(99.0));
    full.variables.insert("easeui/v".into(), user_edited.clone());

    // 4. The agent undoes both steps; neither may touch the user's value.
    let r = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.variables.get("easeui/v"), Some(&user_edited), "undoing the agent's update must not overwrite the user's edit");
    assert!(r.skipped.iter().any(|s| s.contains("easeui/v")), "{:?}", r.skipped);
    let r = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.variables.get("easeui/v"), Some(&user_edited), "undoing the agent's creation must not delete a variable the user edited");
    assert!(r.skipped.iter().any(|s| s.contains("easeui/v")), "{:?}", r.skipped);
    assert!(full.variable_collections.contains_key("easeui"), "the collection still holds the user's variable");
}

#[test]
fn agent_updates_its_own_collection_and_undo_preserves_user_edits() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();

    // 1. The agent creates collection `easeui`.
    let mut colls1 = agent.variable_collections.clone();
    colls1.insert("easeui".into(), collection("easeui"));
    let mut ops1 = Vec::new();
    let vars = agent.variables.clone();
    agent_exec(&mut agent, &mut hist, &mut ops1, Command::SetVariables { variables: vars, collections: colls1.clone() });
    assert_eq!(replay(&mut full, ops1, &mut state).applied, 1);

    // 2. The agent renames its own collection in a second tool call.
    let mut colls2 = colls1;
    colls2.get_mut("easeui").unwrap().name = "Agent name".into();
    let mut ops2 = Vec::new();
    let vars = agent.variables.clone();
    agent_exec(&mut agent, &mut hist, &mut ops2, Command::SetVariables { variables: vars, collections: colls2 });
    assert_eq!(replay(&mut full, ops2, &mut state).applied, 1);
    assert_eq!(full.variable_collections["easeui"].name, "Agent name");

    // 3. The user renames it in the editor.
    full.variable_collections.get_mut("easeui").unwrap().name = "User name".into();

    // 4. The agent undoes both steps; the user's collection survives untouched.
    agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.variable_collections["easeui"].name, "User name", "undoing the agent's rename must not overwrite the user's");
    let r = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.variable_collections["easeui"].name, "User name", "undoing the agent's creation must not delete a collection the user renamed");
    assert!(r.skipped.iter().any(|s| s.contains("easeui")), "{:?}", r.skipped);
}

/// Creates collection `easeui` (member `easeui/v`) through the agent and replays it.
fn agent_creates_easeui(full: &mut Document, agent: &mut Document, hist: &mut History, state: &mut LivePatchState) {
    let mut vars = agent.variables.clone();
    vars.insert("easeui/v".into(), variable("easeui/v", "easeui"));
    let mut colls = agent.variable_collections.clone();
    colls.insert("easeui".into(), collection("easeui"));
    let mut ops = Vec::new();
    agent_exec(agent, hist, &mut ops, Command::SetVariables { variables: vars, collections: colls });
    assert_eq!(replay(full, ops, state).applied, 1);
}

/// The agent adds `easeui/v2` to its own collection and replays it.
fn agent_adds_second_easeui_variable(full: &mut Document, agent: &mut Document, hist: &mut History, state: &mut LivePatchState) -> LivePatchReport {
    let mut vars = agent.variables.clone();
    vars.insert("easeui/v2".into(), variable("easeui/v2", "easeui"));
    let mut colls = agent.variable_collections.clone();
    colls.get_mut("easeui").unwrap().variables.push("easeui/v2".into());
    let mut ops = Vec::new();
    agent_exec(agent, hist, &mut ops, Command::SetVariables { variables: vars, collections: colls });
    replay(full, ops, state)
}

#[test]
fn a_variable_the_agent_adds_joins_a_collection_the_user_added_a_member_to() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();
    agent_creates_easeui(&mut full, &mut agent, &mut hist, &mut state);

    // The user adds a variable to the collection in Studio (member list and variable).
    full.variables.insert("easeui/u1".into(), variable("easeui/u1", "easeui"));
    full.variable_collections.get_mut("easeui").unwrap().variables.push("easeui/u1".into());

    let r = agent_adds_second_easeui_variable(&mut full, &mut agent, &mut hist, &mut state);
    assert!(full.variables.contains_key("easeui/v2"));
    assert_eq!(
        full.variable_collections["easeui"].variables,
        ["easeui/v", "easeui/u1", "easeui/v2"],
        "the user's members and order stay, the agent's new member joins: {:?}",
        r.skipped
    );
}

#[test]
fn a_variable_the_agent_adds_joins_a_collection_the_user_renamed() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();
    agent_creates_easeui(&mut full, &mut agent, &mut hist, &mut state);

    full.variable_collections.get_mut("easeui").unwrap().name = "User name".into();

    let r = agent_adds_second_easeui_variable(&mut full, &mut agent, &mut hist, &mut state);
    assert_eq!(full.variable_collections["easeui"].name, "User name", "the user's rename stays");
    assert!(full.variable_collections["easeui"].variables.contains(&"easeui/v2".to_string()), "{:?}", full.variable_collections["easeui"].variables);
    assert_eq!(r.skipped.len(), 1, "{:?}", r.skipped);
    assert!(r.skipped[0].contains("easeui") && r.skipped[0].contains("name"), "{:?}", r.skipped);

    // The agent's creation is later undone: the renamed collection survives with the user's name.
    let undo = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.variable_collections["easeui"].name, "User name", "{:?}", undo.skipped);
}

#[test]
fn a_collection_kept_because_a_variable_still_uses_it_says_so() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();
    agent_creates_easeui(&mut full, &mut agent, &mut hist, &mut state);

    // A variable of the user's names the collection; the collection itself is untouched.
    full.variables.insert("easeui/u1".into(), variable("easeui/u1", "easeui"));

    let r = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert!(full.variable_collections.contains_key("easeui"));
    assert_eq!(r.duplicates, 0, "a kept collection is not a duplicate");
    assert!(r.skipped.iter().any(|s| s.contains("easeui") && s.contains("easeui/u1")), "{:?}", r.skipped);
}

#[test]
fn an_agent_forward_delete_of_its_own_variable_is_refused_after_a_user_edit() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();
    agent_creates_easeui(&mut full, &mut agent, &mut hist, &mut state);

    let mut edited = full.variables["easeui/v"].clone();
    edited.values.insert("m".into(), crate::VariableValue::Number(99.0));
    full.variables.insert("easeui/v".into(), edited.clone());

    // A forward delete (not an undo) carries the same ops as an undo of the creation.
    let mut colls = agent.variable_collections.clone();
    colls.get_mut("easeui").unwrap().variables.clear();
    let mut ops = Vec::new();
    agent_exec(&mut agent, &mut hist, &mut ops, Command::SetVariables { variables: BTreeMap::new(), collections: colls });
    let r = replay(&mut full, ops, &mut state);
    assert_eq!(full.variables.get("easeui/v"), Some(&edited), "the user's edit survives the agent's delete");
    assert!(r.skipped.iter().any(|s| s.contains("easeui/v")), "{:?}", r.skipped);
}

#[test]
fn an_agent_undo_of_its_style_keeps_a_style_the_user_edited_since() {
    let (mut full, page, _) = full_doc(1, 0);
    let mut agent = scoped(&full, page, 1);
    let mut hist = History::default();
    let mut state = LivePatchState::default();

    let mut ops = Vec::new();
    let mut styles = agent.styles.clone();
    styles.insert("s_agent".into(), red_fill(10));
    set_styles(&mut agent, &mut hist, &mut ops, styles);
    assert_eq!(replay(&mut full, ops, &mut state).applied, 1);

    full.styles.insert("s_agent".into(), red_fill(99));

    let r = agent_undo_and_replay(&mut agent, &mut hist, &mut full, &mut state);
    assert_eq!(full.styles.get("s_agent"), Some(&red_fill(99)), "undo must not delete a style the user edited");
    assert_eq!(r.duplicates, 0);
    assert!(r.skipped.iter().any(|s| s.contains("s_agent")), "{:?}", r.skipped);
}
