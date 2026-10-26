//! Integration tests for #1193: canvas frame label stays in sync with Layers, Properties,
//! and the Agents tab selection chip across user, agent/MCP (`designos_update_node` / `doc-patch.jsonl`),
//! undo, redo, and Revert on the agent's Revert/Keep card.

use super::*;
use crate::agent_selection_chip::SelectionSummary;
use crate::live_sync_tests::unique_tab_id;
use designos_doc::{Document, Node, NodeId, NodeKind, PageHydrator, Rect as DRect, Vec2 as DVec2};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct McpHarness {
    session: designos_mcp::server::Session,
    dir: PathBuf,
    workspace: PathBuf,
}

impl McpHarness {
    fn attach(s: &mut Sim) -> Self {
        s.use_stub_agent_clis();
        let tab_id = unique_tab_id();
        s.shell.app.tabs.instance_id = format!("1193-{tab_id}");
        let sid = s.shell.app.tabs.active_session_id();
        s.shell.app.terminal.bind_session(&sid);
        let dir = s.shell.app.terminal.session_dir.clone();
        let workspace = std::env::temp_dir().join(format!("designos-1193-ws-{tab_id}"));
        std::fs::create_dir_all(&workspace).unwrap();
        crate::live_patch_apply::publish_live_context(&mut s.shell.app);
        let session = designos_mcp::server::Session::with_session_dir(&workspace, &dir).expect("bind in-process designos-mcp session");
        Self { session, dir, workspace }
    }

    fn sync_from_editor(&mut self, s: &mut Sim) {
        crate::live_patch_apply::publish_live_context(&mut s.shell.app);
        self.session.bridge.last_canvas_mtime = None;
        self.session.sync_from_live_canvas_if_newer();
    }

    fn cleanup(self, s: &mut Sim) {
        s.shell.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

fn collect_texts_in_rect(shapes: &[egui::epaint::ClippedShape], target: egui::Rect, out: &mut Vec<String>) {
    for cs in shapes {
        collect_shape_texts(&cs.shape, target, out);
    }
}

fn collect_shape_texts(shape: &egui::epaint::Shape, target: egui::Rect, out: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(ts) => {
            if target.expand(2.0).contains(ts.pos) {
                out.push(ts.galley.text().to_string());
            }
        }
        egui::epaint::Shape::Vec(inner) => {
            for s in inner {
                collect_shape_texts(s, target, out);
            }
        }
        _ => {}
    }
}

/// Reads the actual painted label text emitted by `canvas::draw::overlays` for `node_id` on the last frame.
fn painted_canvas_label(s: &Sim, node_id: NodeId) -> Option<String> {
    let labels = crate::canvas::draw::label_rects(s.app(), s.app().canvas_rect);
    let (_, label_rect) = labels.into_iter().find(|(id, _)| *id == node_id)?;
    let mut texts = Vec::new();
    collect_texts_in_rect(&s.shapes, label_rect, &mut texts);
    texts.into_iter().next()
}

/// Sets up a clean single-page document in `Sim`, draws a rectangle with `Tool::Rectangle`,
/// and converts it to a component with `Ctrl+Alt+K` (`create_component`), returning `(comp_id, rect_id)`.
fn setup_rectangle_component_via_ctrl_alt_k(s: &mut Sim) -> (NodeId, NodeId) {
    let mut doc = Document::new("Issue 1193");
    let page = doc.add_page("Page 1");
    s.shell.app.replace_document(doc, None);
    s.shell.app.page = page;
    s.frame(vec![]);

    // Step 1: Draw a rectangle ("Rectangle 1") on the canvas.
    s.key(Key::R);
    assert_eq!(s.app().tool, Tool::Rectangle);
    let (a, b) = (s.screen(120.0, 120.0), s.screen(280.0, 220.0));
    s.mods = Modifiers::COMMAND;
    s.drag(a, b);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);

    let rect_id = s.by_name("Rectangle 1");
    assert!(matches!(s.app().doc.get(rect_id).unwrap().kind, NodeKind::Rectangle));

    // Step 2: Convert the rectangle to a component the way Ctrl+Alt+K does.
    let cmd_alt = Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
    s.mods = cmd_alt;
    s.key(Key::K);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);

    let page_children = s.app().doc.get(page).unwrap().children.clone();
    assert_eq!(page_children.len(), 1, "page should have one top-level component");
    let comp_id = page_children[0];
    assert!(matches!(s.app().doc.get(comp_id).unwrap().kind, NodeKind::Component(_)));
    assert_eq!(s.app().doc.get(comp_id).unwrap().children, vec![rect_id]);
    assert_eq!(painted_canvas_label(s, comp_id).as_deref(), Some("Rectangle 1"));
    (comp_id, rect_id)
}

#[test]
fn repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);

    // Clicking the component on the canvas selects `rect_id` (because `selects_through` is true for `Component`).
    s.click(s.screen(180.0, 160.0));
    assert_eq!(s.app().selection, vec![rect_id]);
    let mut mcp = McpHarness::attach(&mut s);

    // Step 3: Agent reads `context.json` and renames the selected component node to "Card" via `designos_update_node`.
    let ctx_bytes = std::fs::read(mcp.dir.join("context.json")).expect("read context.json");
    let ctx_json: serde_json::Value = serde_json::from_slice(&ctx_bytes).expect("parse context.json");
    let target_id = ctx_json["selected_ids"][0].as_u64().or_else(|| ctx_json["selection"][0]["id"].as_u64()).unwrap_or(rect_id.0);
    assert_eq!(target_id, rect_id.0);

    mcp.session.call("designos_update_node", json!({ "node_id": target_id, "name": "Card" })).expect("designos_update_node");

    // Step 4: Next frame polls `doc-patch.jsonl` and replays the live patch with no user interaction.
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    s.frame(vec![]);

    // Properties header, Agents tab selection chip, Layers rows, and Canvas frame label must all read "Card".
    let selected_name = s.app().doc.get(s.app().selection[0]).unwrap().name.as_str();
    let chip_text = SelectionSummary::from_doc(&s.app().doc, s.app().page, &s.app().selection).chip_text().unwrap_or_default();
    assert_eq!(selected_name, "Card", "Properties header reads selected node name");
    assert_eq!(chip_text, "Page 1 · 1 selected · Card", "Agents tab header selection chip");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Card", "inner shape layer name");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"), "canvas frame label above component must update from 'Rectangle 1' to 'Card'");
    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Card", "component frame layer name");

    mcp.cleanup(&mut s);
}

#[test]
fn user_rename_on_ctrl_alt_k_component_keeps_canvas_label_layers_and_properties_in_sync() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);

    // 1. Click component on canvas (selects `rect_id`) and rename via `App::rename`.
    s.click(s.screen(180.0, 160.0));
    assert_eq!(s.app().selection, vec![rect_id]);
    s.shell.app.rename(rect_id, "Card");
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Card");
    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Card");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"));
    assert_eq!(SelectionSummary::from_doc(&s.app().doc, s.app().page, &s.app().selection).chip_text().as_deref(), Some("Page 1 · 1 selected · Card"));

    // 2. Rename the outer `comp_id` while `rect_id` remains selected: Properties, chip, and canvas label must all read "Hero Card".
    s.shell.app.rename(comp_id, "Hero Card");
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Hero Card");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Hero Card");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Hero Card"));
    assert_eq!(SelectionSummary::from_doc(&s.app().doc, s.app().page, &s.app().selection).chip_text().as_deref(), Some("Page 1 · 1 selected · Hero Card"));
}

#[test]
fn undo_and_redo_after_component_rename_keep_canvas_label_layers_and_properties_in_sync() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);
    let mut mcp = McpHarness::attach(&mut s);

    s.click(s.screen(180.0, 160.0));
    mcp.sync_from_editor(&mut s);

    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Card" })).expect("agent rename");
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    s.frame(vec![]);
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"));

    // Undo restores "Rectangle 1" across canvas label, Layers, Properties, and selection chip.
    s.shell.app.undo();
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Rectangle 1");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Rectangle 1");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Rectangle 1"));
    assert_eq!(SelectionSummary::from_doc(&s.app().doc, s.app().page, &s.app().selection).chip_text().as_deref(), Some("Page 1 · 1 selected · Rectangle 1"));

    // Redo re-applies "Card" across all surfaces.
    s.shell.app.redo();
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Card");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Card");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"));
    assert_eq!(SelectionSummary::from_doc(&s.app().doc, s.app().page, &s.app().selection).chip_text().as_deref(), Some("Page 1 · 1 selected · Card"));

    mcp.cleanup(&mut s);
}

#[test]
fn revert_on_agent_turn_card_restores_old_name_on_canvas_label_in_fast_and_fallback_paths() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);
    let mut mcp = McpHarness::attach(&mut s);

    let inst_id = s.shell.app.insert_instance(comp_id, DVec2::new(400.0, 170.0)).expect("instance");
    s.frame(vec![]);
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Rectangle 1"));
    mcp.sync_from_editor(&mut s);

    // --- Part A: Fast-undo Revert on the agent's Revert/Keep card ---
    let now = std::time::Instant::now();
    let patch_off = s.app().terminal.live_patch.current_offset();
    s.shell.app.terminal.active_turn_ledger_mut().on_status_change(true, patch_off, now);

    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Card" })).expect("agent rename");
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    let end_off = s.app().terminal.live_patch.current_offset();
    s.shell.app.terminal.active_turn_ledger_mut().on_status_change(false, end_off, now);
    s.frame(vec![]);

    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"));
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Card"));

    let turn_id = s.app().terminal.active_turn_ledger().visible_turn(now).expect("completed turn card").id;
    crate::agent_turn_card::apply_card_action(&mut s.shell.app, 0, crate::agent_turn_card::TurnCardAction::Revert(turn_id));
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Rectangle 1");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Rectangle 1");
    assert_eq!(s.app().doc.get(inst_id).unwrap().name, "Rectangle 1");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Rectangle 1"));
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Rectangle 1"));

    // --- Part B: Fallback per-node Revert (when a subsequent user edit invalidates fast undo) ---
    mcp.sync_from_editor(&mut s);
    let patch_off2 = s.app().terminal.live_patch.current_offset();
    s.shell.app.terminal.active_turn_ledger_mut().on_status_change(true, patch_off2, now);

    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Card V2" })).expect("second agent rename");
    let actions2 = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions2);
    let end_off2 = s.app().terminal.live_patch.current_offset();
    s.shell.app.terminal.active_turn_ledger_mut().on_status_change(false, end_off2, now);
    s.frame(vec![]);
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card V2"));
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Card V2"));

    // User performs an unrelated edit so `cur_depth != end_depth` (forcing fallback snapshot revert).
    let page = s.app().page;
    let extra_id = s.shell.app.doc.alloc_id();
    let extra_rect = Node::rectangle(extra_id, DRect::new(10.0, 400.0, 40.0, 40.0));
    s.shell.app.hist.exec(&mut s.shell.app.doc, designos_doc::Command::Insert { parent: page, index: 2, nodes: vec![extra_rect] }).unwrap();
    s.frame(vec![]);

    let turn_id2 = s.app().terminal.active_turn_ledger().visible_turn(now).expect("second completed turn card").id;
    crate::agent_turn_card::apply_card_action(&mut s.shell.app, 0, crate::agent_turn_card::TurnCardAction::Revert(turn_id2));
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Rectangle 1");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Rectangle 1");
    assert_eq!(s.app().doc.get(inst_id).unwrap().name, "Rectangle 1");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Rectangle 1"));
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Rectangle 1"));

    mcp.cleanup(&mut s);
}

#[test]
fn renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names() {
    let mut s = Sim::new();
    let (comp_id, _rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);
    let inst_default = s.shell.app.insert_instance(comp_id, DVec2::new(400.0, 170.0)).expect("default instance");
    let inst_custom = s.shell.app.insert_instance(comp_id, DVec2::new(600.0, 170.0)).expect("custom instance");
    s.shell.app.rename(inst_custom, "Custom Badge");
    s.frame(vec![]);

    assert_eq!(painted_canvas_label(&s, inst_default).as_deref(), Some("Rectangle 1"));
    assert_eq!(painted_canvas_label(&s, inst_custom).as_deref(), Some("Custom Badge"));

    let mut mcp = McpHarness::attach(&mut s);
    mcp.session.call("designos_update_node", json!({ "node_id": comp_id.0, "name": "Card" })).expect("rename main component");
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    s.frame(vec![]);

    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"));
    assert_eq!(painted_canvas_label(&s, inst_default).as_deref(), Some("Card"), "un-overridden instance label follows main component");
    assert_eq!(painted_canvas_label(&s, inst_custom).as_deref(), Some("Custom Badge"), "overridden instance label stays untouched");

    mcp.cleanup(&mut s);
}

#[test]
fn several_renames_in_one_transaction_show_only_the_final_name_on_canvas_label() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);
    let inst_id = s.shell.app.insert_instance(comp_id, DVec2::new(400.0, 170.0)).expect("instance");
    s.frame(vec![]);

    let mut mcp = McpHarness::attach(&mut s);
    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Draft 1" })).expect("rename 1");
    mcp.session.call("designos_update_node", json!({ "node_id": comp_id.0, "name": "Draft 2" })).expect("rename 2");
    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Final Card" })).expect("rename 3");

    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(comp_id).unwrap().name, "Final Card");
    assert_eq!(s.app().doc.get(rect_id).unwrap().name, "Final Card");
    assert_eq!(s.app().doc.get(inst_id).unwrap().name, "Final Card");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Final Card"));
    assert_eq!(painted_canvas_label(&s, inst_id).as_deref(), Some("Final Card"));

    mcp.cleanup(&mut s);
}

#[derive(Debug)]
struct UnloadedInstancePageHydrator {
    page: NodeId,
    inst: NodeId,
    comp: NodeId,
    hydrated: bool,
}

impl PageHydrator for UnloadedInstancePageHydrator {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
        if page != self.page || self.hydrated {
            return false;
        }
        self.hydrated = true;
        if doc.get(self.inst).is_some_and(|n| n.children.is_empty()) {
            let kids = doc.get(self.comp).map(|n| n.children.clone()).unwrap_or_default();
            for (idx, cid) in kids.into_iter().enumerate() {
                if let Some(child) = doc.get(cid).cloned() {
                    let new_id = doc.alloc_id();
                    let mut copy = Node { id: new_id, parent: Some(self.inst), children: Vec::new(), ..child };
                    copy.component.source = Some(cid);
                    let _ = designos_doc::Command::Insert { parent: self.inst, index: idx, nodes: vec![copy] }.apply(doc);
                }
            }
        }
        true
    }

    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        self.hydrate_page(doc, self.page)
    }

    fn is_hydrated(&self, page: NodeId) -> bool {
        page != self.page || self.hydrated
    }
}

#[test]
fn off_screen_and_unloaded_page_renames_paint_updated_canvas_label_when_scrolled_or_switched_to() {
    let mut s = Sim::new();
    let (comp_id, rect_id) = setup_rectangle_component_via_ctrl_alt_k(&mut s);

    // Move `comp_id` far off-screen (5000, 5000) so it is culled from `label_rects` in the current viewport.
    let mut tx = crate::edit::Tx::new();
    if let Some(n) = tx.ed.node(&mut s.shell.app.doc, comp_id) {
        n.transform.tx = 5000.0;
        n.transform.ty = 5000.0;
    }
    s.shell.app.run_tx(tx);

    // Add a second unloaded page holding an unexpanded instance shell of `comp_id`.
    let page2 = s.shell.app.doc.add_page("Page 2");
    let inst_unloaded_id = s.shell.app.doc.alloc_id();
    let comp_fp = s.app().doc.get(comp_id).unwrap().kind.frame_props().cloned().unwrap_or_default();
    let mut unloaded_inst = Node::new(inst_unloaded_id, NodeKind::Instance { component: Some(comp_id), component_key: None, frame: comp_fp });
    unloaded_inst.name = "Rectangle 1".into();
    unloaded_inst.size = s.app().doc.get(comp_id).unwrap().size;
    unloaded_inst.transform = designos_doc::Affine::translate(100.0, 100.0);
    unloaded_inst.component.source = Some(comp_id);
    s.shell.app.doc.insert(page2, unloaded_inst).unwrap();
    s.shell.app.doc.page_hydrator =
        Some(Arc::new(Mutex::new(UnloadedInstancePageHydrator { page: page2, inst: inst_unloaded_id, comp: comp_id, hydrated: false })));
    s.frame(vec![]);

    assert!(painted_canvas_label(&s, comp_id).is_none(), "off-screen component is culled before scrolling");
    assert!(!s.app().doc.page_hydrator.as_ref().unwrap().lock().unwrap().is_hydrated(page2), "page2 starts unloaded");

    // Agent renames the off-screen component while page2 is still unloaded.
    let mut mcp = McpHarness::attach(&mut s);
    mcp.session.call("designos_update_node", json!({ "node_id": rect_id.0, "name": "Card" })).expect("rename off-screen component");
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    s.frame(vec![]);

    // Pan viewport to (5000, 5000) without bumping TileView::rev (CLAUDE.md §4 invariant 1).
    let rev_before_pan = s.app().canvas.tiles.rev();
    s.shell.app.view.offset = DVec2::new(4900.0, 4900.0);
    s.frame(vec![]);
    assert_eq!(s.app().canvas.tiles.rev(), rev_before_pan, "viewport-only pan must never bump TileView::rev");
    assert_eq!(painted_canvas_label(&s, comp_id).as_deref(), Some("Card"), "scrolled-to component paints new label");

    // Switch to the previously unloaded page2 and scroll to (0, 0): the hydrated instance paints "Card".
    s.shell.app.switch_page(page2);
    s.shell.app.view.offset = DVec2::ZERO;
    s.frame(vec![]);
    assert!(s.app().doc.page_hydrator.as_ref().unwrap().lock().unwrap().is_hydrated(page2));
    assert_eq!(painted_canvas_label(&s, inst_unloaded_id).as_deref(), Some("Card"), "instance on newly hydrated page paints new label");

    mcp.cleanup(&mut s);
}
