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
        messages: vec![Comment { id: "m1".into(), author: "Jang".into(), text: "Keep this".into(), created: "2026-10-16T23:00:00Z".into(), edited: None }],
        resolved: false,
    });
    assert!(doc.nodes.len() > 25_000, "fixture must take the scoped-canvas path");
    Fixture { doc, page, toolbar, screens }
}

fn unique_tab_id() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
    900_000 + (nanos % 100_000) + std::process::id() as u64
}

#[test]
fn mcp_apply_spec_on_scoped_canvas_keeps_full_document() {
    let Fixture { doc, page, toolbar, screens } = big_fixture();
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
    assert_eq!(d.comments.len(), 1, "comment threads were dropped");

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

    // One undo restores the document as it was before the agent edit.
    assert!(app.hist.undo(&mut app.doc).expect("undo"), "agent edit must be one undo step");
    assert_eq!(app.doc.nodes.len(), node_count);
    assert!(app.doc.get(card).is_none());
    assert_eq!(app.doc.get(renamed).unwrap().name, "Screen 7");
    assert_eq!(app.doc.get(page).unwrap().children, page_children);

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
