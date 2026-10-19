//! End-to-end round trip: the editor publishes its live canvas, the real `designos-mcp` session
//! edits it, and the editor picks the edit up through `poll_live_agent_bridge`.
//!
//! Above 25,000 nodes the editor publishes a scoped canvas (top-level frames without children,
//! styles or comments). An agent edit made on that scoped copy must land in the full document as
//! an insert/update of the touched nodes only.

use crate::app::App;
use crate::terminal::TerminalState;
use designos_doc::{Comment, CommentAnchor, CommentThread, Document, Node, NodeId, NodeSpec, Rect, Style, StyleMeta, Vec2};
use serde_json::json;

const SCREENS: usize = 140;
const RECTS_PER_SCREEN: usize = 180;

struct Fixture {
    doc: Document,
    page: NodeId,
    toolbar: NodeId,
    screens: Vec<NodeId>,
}

/// A page with a Hug auto-layout toolbar and 140 screens of 180 layers (> 25,000 nodes),
/// one shared style and one comment thread.
fn big_fixture() -> Fixture {
    let mut doc = Document::new("Big Live Fixture");
    let page = doc.add_page("Screens");
    let toolbar_spec: NodeSpec = serde_json::from_value(json!({
        "kind": "frame",
        "name": "Toolbar/Default",
        "width_sizing": "hug",
        "height_sizing": "hug",
        "layout": { "direction": "row", "gap": 8.0, "padding": [12.0, 12.0, 12.0, 12.0] },
        "children": [
            { "kind": "rectangle", "name": "Tool A", "width": 120.0, "height": 32.0 },
            { "kind": "rectangle", "name": "Tool B", "width": 120.0, "height": 32.0 },
            { "kind": "rectangle", "name": "Tool C", "width": 120.0, "height": 32.0 }
        ]
    }))
    .expect("toolbar spec");
    let toolbar = doc.apply_spec(page, &toolbar_spec).expect("toolbar");
    doc.relayout_subtree(toolbar);
    let mut screens = Vec::with_capacity(SCREENS);
    for s in 0..SCREENS {
        let sid = doc.alloc_id();
        let x = (s % 14) as f32 * 1600.0;
        let y = 200.0 + (s / 14) as f32 * 1000.0;
        doc.insert(page, Node::frame(sid, Rect::new(x, y, 1440.0, 900.0))).expect("screen");
        doc.get_mut(sid).expect("screen").name = format!("Screen {s}");
        for r in 0..RECTS_PER_SCREEN {
            let rid = doc.alloc_id();
            let rx = (r % 18) as f32 * 80.0;
            let ry = (r / 18) as f32 * 90.0;
            doc.insert(sid, Node::rectangle(rid, Rect::new(rx, ry, 64.0, 64.0))).expect("rect");
        }
        screens.push(sid);
    }
    doc.styles.insert("S:heading".into(), Style::Fill(Vec::new()));
    doc.style_meta.insert("S:heading".into(), StyleMeta { name: "Heading/H1".into(), ..Default::default() });
    doc.comments.push(CommentThread {
        id: "thread-1".into(),
        anchor: CommentAnchor { page, node: Some(screens[0]), offset: Vec2::new(4.0, 4.0) },
        messages: vec![Comment {
            id: "m1".into(),
            author: "Jang".into(),
            author_id: None,
            author_color: None,
            text: "Keep this".into(),
            created: "2026-10-16T23:00:00Z".into(),
            edited: None,
        }],
        resolved: false,
    });
    assert!(doc.nodes.len() > 25_000, "fixture must take the scoped-canvas path");
    Fixture { doc, page, toolbar, screens }
}

/// A session id no other test in any running test process uses (tests run in parallel).
pub(crate) fn unique_tab_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    9_000_000 + (std::process::id() as u64 % 100_000) * 100 + NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[test]
fn mcp_apply_spec_on_scoped_canvas_keeps_full_document() {
    let Fixture { mut doc, page, toolbar, screens } = big_fixture();
    // Threads the scoped canvas cannot resolve: on a layer of a screen past the shared frames,
    // and on another page.
    let far_rect = doc.get(screens[SCREENS - 1]).unwrap().children[0];
    let mut on_far_rect = user_thread("thread-2", page, "On a hidden layer");
    on_far_rect.anchor.node = Some(far_rect);
    doc.comments.push(on_far_rect);
    let other_page = doc.add_page("Other");
    doc.comments.push(user_thread("thread-3", other_page, "On another page"));
    let comments_before = doc.comments.clone();
    let node_count = doc.nodes.len();
    let toolbar_size = doc.get(toolbar).unwrap().size;
    let toolbar_children = doc.get(toolbar).unwrap().children.clone();
    let page_children = doc.get(page).unwrap().children.clone();
    let renamed = screens[7];

    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    app.view.zoom = 0.75;
    app.view.offset = Vec2::new(-120.0, 40.0);
    let (zoom, offset) = (app.view.zoom, app.view.offset);
    let session_dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-sync-ws-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();

    // 1. The editor publishes its live canvas for the agent.
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);

    // 2. The real MCP session edits that canvas: one apply_spec and one update_node.
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("mcp session");
    let applied = session
        .call(
            "designos_apply_spec",
            json!({ "spec": { "name": "Agent Pricing Card", "preset": "easeui-card", "children": [
                { "kind": "text", "name": "Price", "text": "$49" }
            ] } }),
        )
        .expect("apply_spec");
    assert_eq!(applied["parent_id"], page.0);
    let card = NodeId(applied["root_node_id"].as_u64().expect("root id"));
    session.call("designos_update_node", json!({ "node_id": renamed.0, "name": "Screen 7 (agent)", "x": 123.0 })).expect("update_node");

    // 3. The editor picks the edits up the way the running app does.
    let actions = app.terminal.poll_live_agent_bridge();
    assert!(!actions.is_empty(), "the editor must see the agent's edits");
    crate::shell::execute_terminal_actions(&mut app, actions);

    // Every screen keeps its layers; the Hug toolbar keeps its size and children.
    let d = &app.doc;
    for sid in &screens {
        assert_eq!(d.get(*sid).map(|n| n.children.len()), Some(RECTS_PER_SCREEN), "screen {sid:?} lost its layers");
    }
    assert_eq!(d.get(toolbar).unwrap().children, toolbar_children, "toolbar lost its children");
    assert_eq!(d.get(toolbar).unwrap().size, toolbar_size, "toolbar was resized by the agent's scoped relayout");
    assert_eq!(d.styles.len(), 1, "shared styles were dropped");
    assert_eq!(d.style_meta.len(), 1, "style metadata was dropped");
    assert_eq!(d.comments, comments_before, "comment threads changed");

    // The new card sits on top of the page with its text; the renamed screen kept its layers.
    let page_now = d.get(page).unwrap().children.clone();
    assert_eq!(page_now.len(), page_children.len() + 1);
    assert_eq!(page_now.last(), Some(&card), "the agent's card must be the topmost layer of the page");
    assert_eq!(d.get(card).unwrap().parent, Some(page));
    let mut texts = Vec::new();
    d.walk(card, |n, _| {
        if n.name == "Price" {
            texts.push(n.id)
        }
    });
    assert_eq!(texts.len(), 1, "the card's text layer is missing");
    let r = d.get(renamed).unwrap();
    assert_eq!(r.name, "Screen 7 (agent)");
    assert_eq!(r.transform.tx, 123.0);
    assert_eq!(r.children.len(), RECTS_PER_SCREEN);
    let added = d.nodes.len() - node_count;
    assert!(added >= 2, "card subtree must be inserted, got {added} new nodes");

    // The card's EaseUI colours point at variables the editor now holds, so a theme change recolours it.
    let card_fill_binding = d.get(card).unwrap().variables.bindings.get("fills/0").cloned().expect("the card fill is bound");
    assert!(d.variables.contains_key(&card_fill_binding), "the editor lacks the variable {card_fill_binding} the card is bound to");
    assert!(d.variable_collections.contains_key("easeui-deprecated"), "the EaseUI (deprecated) collection did not reach the editor");

    // The view did not jump (no fit-to-screen on an agent edit).
    assert_eq!(app.view.zoom, zoom);
    assert_eq!(app.view.offset, offset);

    // Polling again applies nothing twice.
    let again = app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut app, again);
    assert_eq!(app.doc.get(page).unwrap().children.len(), page_children.len() + 1, "agent edit was applied twice");
    assert_eq!(app.doc.nodes.len(), node_count + added);

    // The agent keeps working on what it just made: the editor's next canvas must carry the
    // selected card whole, or the MCP no longer sees the card's layers.
    let price = texts[0];
    session.call("designos_update_node", json!({ "node_id": price.0, "text": "$59" })).expect("follow-up edit on the new card");
    let follow_up = app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut app, follow_up);
    let price_text = |d: &Document| match &d.get(price).unwrap().kind {
        designos_doc::NodeKind::Text(t) => t.content.clone(),
        _ => String::new(),
    };
    assert_eq!(price_text(&app.doc), "$59");
    assert_eq!(app.doc.get(card).unwrap().children.len(), 1, "the card kept its layers");
    assert!(app.hist.undo(&mut app.doc).expect("undo"));
    assert_eq!(price_text(&app.doc), "$49");

    // Each agent tool call is its own undo step, however the polls happened to group them:
    // the rename and the card arrived in one poll but undo one at a time.
    assert!(app.hist.undo(&mut app.doc).expect("undo"));
    assert_eq!(app.doc.get(renamed).unwrap().name, "Screen 7", "the rename is its own undo step");
    assert!(app.doc.get(card).is_some(), "undoing the rename must not undo the card");
    assert!(app.hist.undo(&mut app.doc).expect("undo"), "the card is its own undo step");
    assert_eq!(app.doc.nodes.len(), node_count);
    assert!(app.doc.get(card).is_none());
    assert!(app.doc.variable_collections.is_empty() && app.doc.variables.is_empty(), "undoing the card must remove the variables it brought");
    assert_eq!(app.doc.get(renamed).unwrap().name, "Screen 7");
    assert_eq!(app.doc.get(page).unwrap().children, page_children);

    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
    let _ = std::fs::remove_dir_all(&workspace);
}

/// A user thread with a given id and one message.
fn user_thread(id: &str, page: NodeId, text: &str) -> CommentThread {
    CommentThread {
        id: id.into(),
        anchor: CommentAnchor { page, node: None, offset: Vec2::new(10.0, 10.0) },
        messages: vec![Comment {
            id: format!("{id}-1"),
            author: "Jang".into(),
            author_id: None,
            author_color: None,
            text: text.into(),
            created: "2026-10-17T09:00:00Z".into(),
            edited: None,
        }],
        resolved: false,
    }
}

#[test]
fn agent_comments_never_overwrite_user_threads() {
    let Fixture { mut doc, page, screens, .. } = big_fixture();
    doc.comments.push(user_thread("t1", page, "User thread"));
    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let session_dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-sync-ws-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("mcp session");
    let replay = |app: &mut App| {
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(app, actions);
    };
    let thread = |app: &App, id: &str| app.doc.comments.iter().find(|t| t.id == id).cloned();
    let texts = |t: &CommentThread| t.messages.iter().map(|m| m.text.clone()).collect::<Vec<_>>();

    // 1. The agent adds a thread while the user's `t1` already exists.
    let added = session.call("designos_add_comment", json!({ "page_id": page.0, "x": 5.0, "y": 5.0, "text": "Agent note" })).expect("add_comment");
    replay(&mut app);
    assert_eq!(texts(&thread(&app, "t1").expect("user t1")), ["User thread"], "the user's t1 was overwritten");
    let agent_id = added["id"].as_str().unwrap().to_string();
    let agent_threads: Vec<_> = app.doc.comments.iter().filter(|t| texts(t) == ["Agent note"]).collect();
    assert_eq!(agent_threads.len(), 1, "the agent's thread must exist once");
    assert_ne!(agent_threads[0].id, "t1");

    // 2. The user adds `t4` after the agent read the canvas; the agent's next thread is also `t4`,
    // and it replies there before the editor polls. Both calls land on the agent's thread.
    app.doc.comments.push(user_thread("t4", page, "User race"));
    let raced = session.call("designos_add_comment", json!({ "page_id": page.0, "x": 6.0, "y": 6.0, "text": "Agent race" })).expect("add_comment");
    assert_eq!(raced["id"], "t4", "precondition: the agent picks the id the user just took");
    session.call("designos_reply_comment", json!({ "thread_id": "t4", "text": "Agent follow-up" })).expect("reply");
    replay(&mut app);
    assert_eq!(texts(&thread(&app, "t4").unwrap()), ["User race"], "the user's t4 was overwritten");
    let moved = app.doc.comments.iter().find(|t| texts(t).first().is_some_and(|m| m == "Agent race")).expect("agent thread kept under a new id").id.clone();
    assert_ne!(moved, "t4");
    assert_eq!(texts(&thread(&app, &moved).unwrap()), ["Agent race", "Agent follow-up"]);

    // A layer edit makes the editor republish its canvas. The agent's next call reloads it and
    // from then on sees the editor's ids: `t4` is the user's thread, and resolving it must
    // resolve the user's thread, not the one the agent's old `t4` was moved to.
    session.call("designos_update_node", json!({ "node_id": screens[0].0, "name": "Screen 0 (agent)" })).expect("update_node");
    replay(&mut app);
    session.call("designos_resolve_comment", json!({ "thread_id": "t4", "resolved": true })).expect("resolve");
    let agent_view = |session: &designos_mcp::server::Session, id: &str| session.engine.doc.comments.iter().find(|t| t.id == id).cloned();
    assert_eq!(texts(&agent_view(&session, "t4").expect("agent sees t4")), ["User race"], "precondition: the agent reloaded the editor's canvas");
    replay(&mut app);
    assert!(thread(&app, "t4").unwrap().resolved, "the agent resolved the user's t4");
    assert!(!thread(&app, &moved).unwrap().resolved, "the edit was redirected to the agent's old thread");

    // 3. The user and the agent both reply to t1; both replies survive.
    let mut t1 = thread(&app, "t1").unwrap();
    t1.messages.push(Comment {
        id: "t1-2".into(),
        author: "Jang".into(),
        author_id: None,
        author_color: None,
        text: "User reply".into(),
        created: "2026-10-17T09:05:00Z".into(),
        edited: None,
    });
    if let Some(t) = app.doc.comments.iter_mut().find(|t| t.id == "t1") {
        *t = t1;
    }
    session.call("designos_reply_comment", json!({ "thread_id": "t1", "text": "Agent reply" })).expect("reply to t1");
    replay(&mut app);
    assert_eq!(texts(&thread(&app, "t1").unwrap()), ["User thread", "User reply", "Agent reply"]);
    let ids: Vec<_> = thread(&app, "t1").unwrap().messages.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "message ids must stay unique: {ids:?}");
    assert!(app.doc.comments.iter().any(|t| t.id == agent_id || texts(t) == ["Agent note"]));

    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn agent_undo_after_a_canvas_reload_never_touches_user_layers() {
    let Fixture { doc, page, screens, .. } = big_fixture();
    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let session_dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-sync-ws-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("mcp session");
    let replay = |app: &mut App| {
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(app, actions);
    };

    // The user draws a layer that takes the id the agent allocates next. It sits at the bottom of
    // the page, inside the frames the scoped canvas shares.
    let user_layer = app.doc.alloc_id();
    let mut user_rect = Node::rectangle(user_layer, Rect::new(10.0, 10.0, 40.0, 40.0));
    user_rect.name = "User layer".into();
    designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![user_rect] }.apply(&mut app.doc).unwrap();
    let created = session.call("designos_create_node", json!({ "kind": "rectangle", "name": "Agent layer", "width": 20.0, "height": 20.0 })).expect("create");
    assert_eq!(created["node"]["id"].as_u64(), Some(user_layer.0), "precondition: the agent picks the id the user just took");
    replay(&mut app);
    let agent_layer = *app.doc.get(page).unwrap().children.last().unwrap();
    assert_ne!(agent_layer, user_layer);
    assert_eq!(app.doc.get(agent_layer).unwrap().name, "Agent layer");

    // The editor republished its canvas; the agent's next call reloads it with the editor's ids.
    // Its undo history refers to its old ids, so undoing must not touch the user's layer.
    session.call("designos_get_document", json!({})).expect("read");
    assert!(session.engine.doc.get(user_layer).is_some_and(|n| n.name == "User layer"), "precondition: the agent reloaded the editor's canvas");
    let undo = session.call("designos_undo", json!({})).expect("undo");
    replay(&mut app);
    assert_eq!(app.doc.get(user_layer).map(|n| n.name.as_str()), Some("User layer"), "the agent's undo deleted the user's layer");
    assert_eq!(undo["changed"], false);
    assert!(undo["reason"].as_str().is_some_and(|r| r.contains("Studio")), "undo must say why it did nothing: {undo}");
    assert!(app.doc.get(screens[0]).is_some_and(|n| n.children.len() == RECTS_PER_SCREEN));

    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
    let _ = std::fs::remove_dir_all(&workspace);
}

/// The same round trip on a real file: `DESIGNOS_LIVE_SYNC_FIG=<file.fig>` and, optionally,
/// `DESIGNOS_LIVE_SYNC_OUT=<result.designos>` to save the edited document for a screenshot run
/// (plus `<result>.card.png`, the agent's own render of the card on its next call).
#[test]
#[ignore = "needs a real design file in DESIGNOS_LIVE_SYNC_FIG"]
fn mcp_apply_spec_on_a_real_file_keeps_every_screen() {
    let Some(fig) = std::env::var_os("DESIGNOS_LIVE_SYNC_FIG").map(std::path::PathBuf::from) else { return };
    let mut app = App::new(Document::new("Untitled"));
    app.open_path(fig.clone());
    let page = app.page;
    let kids = |d: &Document| d.get(page).unwrap().children.iter().map(|c| (*c, d.get(*c).map_or(0, |n| n.children.len()))).collect::<Vec<_>>();
    let (before_nodes, before_tops, before_styles) = (app.doc.nodes.len(), kids(&app.doc), app.doc.styles.len());
    eprintln!("{}: {before_nodes} nodes, {} top-level frames, {before_styles} styles", fig.display(), before_tops.len());

    app.terminal = TerminalState::for_tab(unique_tab_id());
    let session_dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-sync-real-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("mcp session");
    let applied = session
        .call(
            "designos_apply_spec",
            json!({ "spec": { "name": "Agent Pricing Card", "preset": "easeui-card", "width": 480, "children": [
                { "kind": "text", "name": "Plan", "text": "Pro plan", "font_size": 20 },
                { "kind": "text", "name": "Price", "text": "$49 / month", "font_size": 40, "font_weight": 700 },
                { "kind": "frame", "name": "CTA", "preset": "easeui-button-primary", "children": [{ "kind": "text", "name": "Label", "text": "Upgrade" }] }
            ] } }),
        )
        .expect("apply_spec");
    let card = NodeId(applied["root_node_id"].as_u64().unwrap());
    let actions = app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut app, actions);

    let after_tops = kids(&app.doc);
    eprintln!("after: {} nodes, {} top-level frames, {} styles, card {card:?}", app.doc.nodes.len(), after_tops.len(), app.doc.styles.len());
    assert_eq!(&after_tops[..before_tops.len()], &before_tops[..], "a top-level frame lost layers");
    assert_eq!(after_tops.last().map(|t| t.0), Some(card));
    assert_eq!(app.doc.styles.len(), before_styles);

    // The agent's next calls still see the whole card it just made.
    let rendered = session.call("designos_render_png", json!({ "path": "card.png", "node_id": card.0 })).expect("render the new card");
    eprintln!("agent render of {card:?}: {} bytes", rendered["bytes"]);
    let mut price = None;
    app.doc.walk(card, |n, _| {
        if n.name == "Price" {
            price = Some(n.id)
        }
    });
    let price = price.expect("price layer");
    session.call("designos_update_node", json!({ "node_id": price.0, "text": "$59 / month" })).expect("follow-up edit on the card");
    let follow_up = app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut app, follow_up);
    assert!(matches!(&app.doc.get(price).unwrap().kind, designos_doc::NodeKind::Text(t) if t.content == "$59 / month"));
    assert_eq!(&kids(&app.doc)[..before_tops.len()], &before_tops[..], "a top-level frame lost layers on the follow-up edit");

    if let Some(out) = std::env::var_os("DESIGNOS_LIVE_SYNC_OUT").map(std::path::PathBuf::from) {
        std::fs::write(&out, app.doc.to_json().unwrap()).unwrap();
        std::fs::copy(workspace.join("card.png"), out.with_extension("card.png")).unwrap();
    }
    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn bind_then_merge_then_export_spec_round_trips() {
    use designos_doc::{Color, StyleProperty, TextStyle, Variable, VariableCollection, VariableKind, VariableMode, VariableValue};

    let Fixture { mut doc, screens, .. } = big_fixture();
    doc.variable_collections.insert(
        "col:brand".into(),
        VariableCollection {
            id: "col:brand".into(),
            name: "Brand".into(),
            modes: vec![VariableMode { id: "m:default".into(), name: "Default".into() }],
            default_mode: "m:default".into(),
            variables: vec!["var:brand-primary".into(), "var:card-pad".into()],
            key: None,
            source_library_key: None,
        },
    );
    doc.variables.insert(
        "var:brand-primary".into(),
        Variable {
            id: "var:brand-primary".into(),
            name: "brand/primary".into(),
            collection: "col:brand".into(),
            kind: VariableKind::Color,
            values: [("m:default".to_string(), VariableValue::Color(Color::rgb(0xFF, 0x6A, 0x3D)))].into(),
            description: String::new(),
            scopes: vec!["ALL_FILLS".into()],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        },
    );
    doc.variables.insert(
        "var:card-pad".into(),
        Variable {
            id: "var:card-pad".into(),
            name: "space/card-pad".into(),
            collection: "col:brand".into(),
            kind: VariableKind::Number,
            values: [("m:default".to_string(), VariableValue::Number(24.0))].into(),
            description: String::new(),
            scopes: vec!["CORNER_RADIUS".into(), "GAP".into()],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        },
    );
    doc.styles.insert("S:heading".into(), Style::Text(TextStyle { font_size: 28.0, font_weight: 700, ..Default::default() }));
    doc.style_meta.insert("S:heading".into(), StyleMeta { name: "Heading/H1".into(), property: StyleProperty::Text, ..Default::default() });
    assert!(doc.nodes.len() > 25_000, "must take the scoped canvas path");

    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let session_dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-sync-bind-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();

    // 1. Editor publishes the scoped canvas (which now carries variables & styles).
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);

    // 2. Agent binds `var:brand/primary`, `var:space/card-pad`, and `text_style: "Heading/H1"` on the scoped canvas.
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("mcp session");
    let applied = session
        .call(
            "designos_apply_spec",
            json!({
                "spec": {
                    "kind": "frame",
                    "name": "Token Bound Card",
                    "fill": "var:brand/primary",
                    "corner_radius": "var:space/card-pad",
                    "layout": { "direction": "column", "gap": "var:space/card-pad", "padding": [16.0, 16.0, 16.0, 16.0] },
                    "children": [
                        { "kind": "text", "name": "Hero Title", "text": "Scoped Tokens", "text_style": "Heading/H1" }
                    ]
                }
            }),
        )
        .expect("apply_spec with var: and text_style on scoped canvas");
    assert_eq!(applied["bindings_created"], 6);
    assert_eq!(applied["styles_linked"], 1);
    assert_eq!(applied["warnings"], json!([]));
    let card = NodeId(applied["root_node_id"].as_u64().expect("root_node_id"));

    // 3. Editor replays the live patch into the full 25,000+ node document.
    let actions = app.terminal.poll_live_agent_bridge();
    assert!(!actions.is_empty());
    crate::shell::execute_terminal_actions(&mut app, actions);

    // 4. Verify unchanged token ids, no duplicates, and round-trip via export_spec.
    let d = &app.doc;
    assert_eq!(d.variable_collections.len(), 1);
    assert_eq!(d.variables.len(), 2);
    assert_eq!(d.styles.len(), 1);
    assert_eq!(d.get(screens[0]).map(|n| n.children.len()), Some(RECTS_PER_SCREEN));
    let card_node = d.get(card).expect("merged card node");
    assert_eq!(card_node.variables.bindings.get("fills/0").map(String::as_str), Some("var:brand-primary"));
    assert_eq!(card_node.variables.bindings.get("topLeftRadius").map(String::as_str), Some("var:card-pad"));
    assert_eq!(card_node.variables.bindings.get("itemSpacing").map(String::as_str), Some("var:card-pad"));
    let title_id = card_node.children[0];
    let title_node = d.get(title_id).expect("merged title node");
    assert_eq!(title_node.style_refs.get("text").map(String::as_str), Some("S:heading"));

    let exported = d.export_spec(card).expect("export_spec");
    assert_eq!(exported.fill.as_deref(), Some("var:Brand/brand/primary"));
    assert_eq!(exported.children[0].text_style.as_deref(), Some("Heading/H1"));

    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
    let _ = std::fs::remove_dir_all(&workspace);
}
