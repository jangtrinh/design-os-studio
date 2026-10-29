//! Acceptance and edge-case GUI tests for `#1173` (`US-13`):
//! when an agent turn touches many layers and the user clicks "Show all", the terminal viewport
//! and the "changed layers" card scroll independently, the card is capped at 50% of the panel
//! height with pinned header + actions ("Revert", "Keep", "Show less"), and scroll state is
//! isolated per agent tab and per turn.

use super::*;
use crate::agent_turn_marks::{TurnMarkLine, TurnPhase};
use crate::app::RailItem;
use crate::terminal::AgentCliPreset;
use designos_doc::live_patch::{LiveOp, LivePatch};
use designos_doc::{Node, Rect as DRect};
use std::time::Instant;

fn recorded(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

/// Runs one egui frame with the window height chosen so the left panel (`inner` in `panels::left::show`)
/// is `panel_h` logical pixels tall.
fn frame_at_panel_height(s: &mut Sim, panel_h: f32, events: Vec<Event>) {
    let show_title_bar = s.app().tabs.always_visible && s.shell.shot.is_none() && s.app().proto.present.is_none();
    let show_tab_bar = s.app().tabs.is_bar_visible(s.app().home.is_some()) && s.app().proto.present.is_none();
    let chrome_top = (if show_title_bar { TOP_TITLE_BAR_HEIGHT } else { 0.0 }) + (if show_tab_bar { crate::tabs::TOP_TAB_BAR_HEIGHT } else { 0.0 });
    s.t += 0.05;
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, panel_h + chrome_top))),
        time: Some(s.t),
        events,
        modifiers: s.mods,
        ..Default::default()
    };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let out = ctx.run_ui(raw, |ui| shell.draw(ui));
    s.cursor = out.platform_output.cursor_icon;
    s.shapes = out.shapes;
    s.accesskit_update = out.platform_output.accesskit_update;
    s.shell.app.settle_paste_jobs();
}

/// Returns `(term_rect, viewport_rect)` painted by `terminal::show_left_with_doc` in the last frame.
fn terminal_rects(s: &Sim) -> (Rect, Rect) {
    let rects: Vec<Rect> = s
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Rect(r) if r.fill == crate::terminal::TERM_BG_VIEWPORT => Some(r.rect),
            _ => None,
        })
        .collect();
    assert!(rects.len() >= 2, "expected term_rect and viewport_rect with TERM_BG_VIEWPORT fill, got {}", rects.len());
    (rects[0], rects[1])
}

/// Returns the vertical bounds of the left panel (`left_rect` / `inner`).
fn panel_rect(s: &Sim) -> Rect {
    let handle = s.rect_of("left-panel-resize-handle");
    let (term_rect, _) = terminal_rects(s);
    Rect::from_min_max(Pos2::new(term_rect.left(), handle.top()), Pos2::new(term_rect.right(), handle.bottom()))
}

fn wheel_event(dy: f32) -> Event {
    Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, dy), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE }
}

fn scroll_at(s: &mut Sim, panel_h: f32, pos: Pos2, dy: f32) {
    s.pos = pos;
    frame_at_panel_height(s, panel_h, vec![Event::PointerMoved(pos), wheel_event(dy)]);
}

fn click_at(s: &mut Sim, panel_h: f32, pos: Pos2) {
    s.pos = pos;
    frame_at_panel_height(s, panel_h, vec![Event::PointerMoved(pos)]);
    frame_at_panel_height(s, panel_h, vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: s.mods }]);
    frame_at_panel_height(s, panel_h, vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: s.mods }]);
    frame_at_panel_height(s, panel_h, vec![]);
}

fn finish(mut s: Sim) {
    let dir = s.app().terminal.session_dir.clone();
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
    s.shell.app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

fn spawn_claude_tab_with_output(s: &mut Sim, output_lines: usize) -> usize {
    #[cfg(unix)]
    let idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("spawn stub claude tab");
    #[cfg(not(unix))]
    let idx = {
        let i = s.shell.app.terminal.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell tab");
        s.shell.app.terminal.tabs[i].preset = AgentCliPreset::Claude;
        s.shell.app.terminal.tabs[i].title = format!("Claude Code #{}", i + 1);
        i
    };
    if output_lines > 0 {
        let mut buf = String::new();
        for i in 1..=output_lines {
            buf.push_str(&format!("CLAUDE_OUTPUT_LINE_{i:03}\r\n"));
        }
        let mut parser = s.shell.app.terminal.tabs[idx].parser.lock().expect("lock parser");
        parser.process(buf.as_bytes());
        parser.set_scrollback(0);
    }
    idx
}

fn apply_agent_layer_turn(s: &mut Sim, row_count: usize, prefix: &str) -> Vec<NodeId> {
    let page = s.app().page;
    let mut ids = Vec::with_capacity(row_count);
    for i in 0..row_count {
        let id = s.shell.app.doc.alloc_id();
        let mut node = Node::rectangle(id, DRect::new((i % 10) as f32 * 24.0, (i / 10) as f32 * 24.0, 20.0, 16.0));
        node.name = format!("{prefix} Layer {:03}", i + 1);
        s.shell.app.doc.insert(page, node).expect("insert layer");
        ids.push(id);
    }
    let ops: Vec<LiveOp> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let mut set = serde_json::Map::new();
            set.insert("name".into(), format!("{prefix} Layer {:03} (edited)", i + 1).into());
            LiveOp::Update { id, set, unset: Vec::new() }
        })
        .collect();
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[LivePatch::new(ops)]);
    ids
}

fn setup_panel_with_turn(panel_h: f32, output_lines: usize, card_rows: usize, expand: bool) -> (Sim, Vec<NodeId>) {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.ui.rail = Some(RailItem::Agents);
    s.shell.app.ui.left_collapsed = false;
    s.shell.app.terminal.side_panel_pty_open = true;
    s.shell.app.terminal.chat_mode = false;
    s.shell.app.terminal.active_runtime = AgentCliPreset::Claude;
    spawn_claude_tab_with_output(&mut s, output_lines);
    frame_at_panel_height(&mut s, panel_h, vec![]);

    let ids = apply_agent_layer_turn(&mut s, card_rows, "Turn1");
    frame_at_panel_height(&mut s, panel_h, vec![]);

    if expand && card_rows > crate::agent_turn_card::MAX_PREVIEW_LAYERS {
        let show_all = s.rect_of("agent-turn-show-all");
        click_at(&mut s, panel_h, show_all.center());
        frame_at_panel_height(&mut s, panel_h, vec![]);
    }
    if let Some(tab) = s.shell.app.terminal.tabs.first() {
        if let Ok(mut parser) = tab.parser.lock() {
            parser.set_scrollback(0);
        }
    }
    frame_at_panel_height(&mut s, panel_h, vec![]);
    assert!((panel_rect(&s).height() - panel_h).abs() < 1.0, "expected panel height {panel_h}, got {}", panel_rect(&s).height());
    (s, ids)
}

#[test]
fn ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 300, 82, true);
    let (_term_rect, vp_rect) = terminal_rects(&s);
    let sb_before = s.app().terminal.tabs[0].parser.lock().unwrap().screen().scrollback();
    assert_eq!(sb_before, 0, "precondition: terminal starts at bottom of output (scrollback == 0)");

    scroll_at(&mut s, 900.0, vp_rect.center(), 96.0);
    let sb_after = s.app().terminal.tabs[0].parser.lock().unwrap().screen().scrollback();
    assert!(sb_after > 0, "AC-01: wheel over terminal viewport with 82-row expanded card must scroll terminal into scrollback, got scrollback={sb_after}");
    finish(s);
}

#[test]
fn ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 300, 82, true);
    let p_rect = panel_rect(&s);
    let card_rect = s.rect_of("agent-turn-card");

    for _ in 0..12 {
        scroll_at(&mut s, 900.0, card_rect.center(), -320.0);
    }

    let card_rect = s.rect_of("agent-turn-card");
    let row_81 = recorded(&s, "agent-turn-row-81").expect("AC-02: Row 82 (idx=81) must be visible after scrolling to the bottom of the card");
    let pill = s.rect_of("agent-turn-count-pill");
    let revert = s.rect_of("agent-turn-revert");
    let keep = s.rect_of("agent-turn-keep");
    let show_less = recorded(&s, "agent-turn-show-less").expect("AC-02: Show less row must be rendered when 82-row card is expanded");

    assert!(
        p_rect.contains_rect(row_81) && card_rect.contains_rect(row_81),
        "AC-02: Row 82 (idx=81) {row_81:?} must sit inside card {card_rect:?} and panel {p_rect:?}"
    );
    for (name, r) in [("header count pill", pill), ("Revert", revert), ("Keep", keep), ("Show less", show_less)] {
        assert!(p_rect.contains_rect(r) && card_rect.contains_rect(r), "AC-02: {name} {r:?} must stay visible inside card {card_rect:?} and panel {p_rect:?}");
    }
    assert!(row_81.bottom() <= show_less.top() + 0.5, "AC-02: Row 82 bottom ({}) must stay above Show less top ({})", row_81.bottom(), show_less.top());
    let sb = s.app().terminal.tabs[0].parser.lock().unwrap().screen().scrollback();
    assert_eq!(sb, 0, "R2/AC-02: wheel over the card list must scroll only the card list, leaving terminal scrollback at 0");
    finish(s);
}

#[test]
fn ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts() {
    for panel_h in [400.0_f32, 900.0, 2000.0] {
        for rows in [5usize, 82, 300] {
            let (s, _ids) = setup_panel_with_turn(panel_h, 60, rows, true);
            let p_rect = panel_rect(&s);
            let (term_rect, _vp_rect) = terminal_rects(&s);
            let card_rect = s.rect_of("agent-turn-card");
            let min_term_h = (panel_h * 0.5).max(8.0 * crate::theme::LINE_BODY);
            let card_outer_h = card_rect.height() + crate::theme::INSET_FRAME * 2.0;

            assert!(
                term_rect.height() + 0.5 >= min_term_h,
                "AC-03 (panel_h={panel_h}, rows={rows}): term_rect.height()={} must be >= {min_term_h}",
                term_rect.height()
            );
            assert!(
                card_outer_h <= panel_h - min_term_h + 0.5,
                "AC-03 (panel_h={panel_h}, rows={rows}): card outer height {card_outer_h} must leave at least {min_term_h} for terminal in {panel_h} px panel"
            );
            assert!(
                card_rect.bottom() + crate::theme::INSET_FRAME <= p_rect.bottom() + 0.5,
                "AC-03 (panel_h={panel_h}, rows={rows}): card_rect.bottom()={} + inset must not overflow panel bottom {}",
                card_rect.bottom(),
                p_rect.bottom()
            );
            finish(s);
        }
    }
}

#[test]
fn ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 60, 82, false);
    let collapsed_card_outer_h = s.rect_of("agent-turn-card").height() + crate::theme::INSET_FRAME * 2.0;

    let show_all = s.rect_of("agent-turn-show-all");
    click_at(&mut s, 900.0, show_all.center());
    let (expanded_term, _) = terminal_rects(&s);
    assert!((expanded_term.height() - 450.0).abs() < 1.0, "precondition: expanded 82-row card caps terminal at 450 px");

    // Click Show less -> on the very next frame after release, terminal gets back panel_h - collapsed_card_outer_h with no gap.
    let show_less = recorded(&s, "agent-turn-show-less").expect("AC-04: Show less must be present when expanded");
    click_at(&mut s, 900.0, show_less.center());
    let (collapsed_term, _) = terminal_rects(&s);
    let collapsed_card = s.rect_of("agent-turn-card");
    let expected_term_h = 900.0 - collapsed_card_outer_h;
    assert!(
        (collapsed_term.height() - expected_term_h).abs() < 1.0,
        "AC-04: Show less must restore terminal height to {expected_term_h} on the next frame, got {}",
        collapsed_term.height()
    );
    let card_outer_top = collapsed_card.top() - crate::theme::INSET_FRAME;
    assert!(
        (collapsed_term.bottom() - card_outer_top).abs() < 0.5,
        "AC-04: no empty gap between terminal bottom ({}) and card outer top ({card_outer_top})",
        collapsed_term.bottom()
    );

    // Expand again, then click Keep -> on the very next frame after release, terminal gets full 900 px height.
    let show_all = s.rect_of("agent-turn-show-all");
    click_at(&mut s, 900.0, show_all.center());
    let keep = s.rect_of("agent-turn-keep");
    click_at(&mut s, 900.0, keep.center());
    let (full_term, _) = terminal_rects(&s);
    let p_rect = panel_rect(&s);
    assert!(
        (full_term.height() - 900.0).abs() < 1.0 && (full_term.bottom() - p_rect.bottom()).abs() < 0.5,
        "AC-04: Keep must restore terminal to full 900 px panel height on the next frame without gap, got {}",
        full_term.height()
    );
    finish(s);
}

#[test]
fn ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout() {
    let (s, _ids) = setup_panel_with_turn(900.0, 60, 3, false);
    assert!(recorded(&s, "scrollbar-track").is_none(), "AC-05: 3-row card must not render a scrollbar");
    assert!(recorded(&s, "agent-turn-show-all").is_none(), "AC-05: 3-row card must not render Show all");
    assert!(recorded(&s, "agent-turn-show-less").is_none(), "AC-05: 3-row card must not render Show less");

    let p_rect = panel_rect(&s);
    let (term_rect, _) = terminal_rects(&s);
    let card_rect = s.rect_of("agent-turn-card");
    // Base commit formula for 3 rows, from the theme's line heights and gaps:
    // pad*2 (16) + LINE_BODY + SPACE_FACT + LINE_CAPTION + SPACE_INLINE
    // + (3 * 24 + 2 * 4) (80) + SPACE_INLINE + CONTROL_STANDARD; the terminal takes the panel minus the card and 16.
    let card_height = 16.0
        + crate::theme::LINE_BODY
        + crate::theme::SPACE_FACT
        + crate::theme::LINE_CAPTION
        + crate::theme::SPACE_INLINE
        + 80.0
        + crate::theme::SPACE_INLINE
        + crate::theme::CONTROL_STANDARD;
    let terminal_height = 900.0 - card_height - 16.0;
    assert!((card_rect.height() - card_height).abs() < 0.5, "AC-05: 3-row card height must match the base formula ({card_height}), got {}", card_rect.height());
    assert!(
        (term_rect.height() - terminal_height).abs() < 0.5,
        "AC-05: 3-row terminal height must match the base formula ({terminal_height}), got {}",
        term_rect.height()
    );
    assert!(
        (term_rect.bottom() - (card_rect.top() - crate::theme::INSET_FRAME)).abs() < 0.5
            && (card_rect.bottom() + crate::theme::INSET_FRAME - p_rect.bottom()).abs() < 0.5,
        "AC-05: 3-row card and terminal rects must match base commit layout"
    );
    finish(s);

    // Contrast: an expanded 82-row card on the same 900 px panel DOES render the shared panel scrollbar.
    let (s_many, _ids_many) = setup_panel_with_turn(900.0, 60, 82, true);
    assert!(recorded(&s_many, "scrollbar-track").is_some(), "AC-05 contrast: expanded 82-row card must render panels::scrollbar");
    finish(s_many);
}

#[test]
fn edge_reverted_card_does_not_scroll() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 60, 82, true);
    assert!(recorded(&s, "agent-turn-show-less").is_some(), "precondition: expanded 82-row card shows Show less before Revert");
    let revert = s.rect_of("agent-turn-revert");
    click_at(&mut s, 900.0, revert.center());
    // Re-run a clean frame so temp rects reflect only the reverted 1-row card.
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);

    assert!(recorded(&s, "scrollbar-track").is_none(), "reverted 1-row card must not have a scrollbar");
    assert!(recorded(&s, "agent-turn-show-less").is_none(), "reverted 1-row card must not show Show less");
    let dismiss_before = s.rect_of("agent-turn-dismiss");
    let card_before = s.rect_of("agent-turn-card");
    scroll_at(&mut s, 900.0, card_before.center(), -240.0);
    let dismiss_after = s.rect_of("agent-turn-dismiss");
    let card_after = s.rect_of("agent-turn-card");
    assert_eq!(dismiss_before, dismiss_after, "reverted card must not scroll on wheel input");
    assert_eq!(card_before, card_after, "reverted card rect must remain unchanged on wheel input");
    finish(s);
}

#[test]
fn edge_new_turn_while_scrolled_resets_to_top_without_panic() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 60, 82, true);
    let row0_initial = s.rect_of("agent-turn-row-0");
    let card_rect = s.rect_of("agent-turn-card");

    for _ in 0..8 {
        scroll_at(&mut s, 900.0, card_rect.center(), -320.0);
    }
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);
    assert!(recorded(&s, "agent-turn-row-0").is_none(), "precondition: Row 0 must be scrolled off the top before the new turn arrives");

    // Complete Turn 1 and open Turn 2 with 12 layers, then expand Turn 2.
    {
        let ledger = s.shell.app.terminal.active_turn_ledger_mut();
        if let Some(last) = ledger.turns.last_mut() {
            last.state = crate::agent_turn_ledger::TurnState::Completed;
        }
        ledger.ingest_marks(
            vec![TurnMarkLine { v: crate::agent_turn_marks::TURN_MARK_VERSION, turn_mark: TurnPhase::Start, patch_bytes: 500, cli: Some("claude".into()) }],
            Instant::now(),
        );
    }
    let _turn2_ids = apply_agent_layer_turn(&mut s, 12, "Turn2");
    if let Some(last) = s.shell.app.terminal.active_turn_ledger_mut().turns.last_mut() {
        last.expanded = true;
    }
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);

    let row0_new = recorded(&s, "agent-turn-row-0").expect("new turn must start scrolled to the top with Row 0 visible");
    assert!(
        (row0_new.top() - row0_initial.top()).abs() < 1.0,
        "new turn Row 0 top ({}) must match top-of-list position ({})",
        row0_new.top(),
        row0_initial.top()
    );
    finish(s);
}

#[test]
fn edge_clicking_row_after_scrolling_selects_correct_layer() {
    let (mut s, ids) = setup_panel_with_turn(900.0, 60, 82, true);
    let card_rect = s.rect_of("agent-turn-card");

    for _ in 0..12 {
        scroll_at(&mut s, 900.0, card_rect.center(), -320.0);
    }
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);

    let row_81 = recorded(&s, "agent-turn-row-81").expect("Row 82 (idx=81) must be visible after scrolling to end");
    assert!(panel_rect(&s).contains_rect(row_81), "Row 82 must be inside the 900 px panel before clicking");
    click_at(&mut s, 900.0, row_81.center());
    assert_eq!(s.app().selection, vec![ids[81]], "clicking scrolled Row 82 must select layer ids[81] with no offset error");
    finish(s);
}

#[test]
fn edge_scroll_state_isolated_per_tab_and_turn() {
    let (mut s, _ids_tab1) = setup_panel_with_turn(900.0, 60, 82, true);
    let row0_top = s.rect_of("agent-turn-row-0").top();
    let card_rect = s.rect_of("agent-turn-card");

    // Scroll Tab 1 Turn 1 to the bottom.
    for _ in 0..12 {
        scroll_at(&mut s, 900.0, card_rect.center(), -320.0);
    }
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);
    assert!(recorded(&s, "agent-turn-row-0").is_none(), "precondition: Tab 1 Turn 1 is scrolled past Row 0");
    assert!(recorded(&s, "agent-turn-row-81").is_some(), "precondition: Tab 1 Turn 1 shows Row 82 at bottom");

    // Spawn Tab 2, make it active, and create Turn 1 on Tab 2 (also id == 1, 82 rows, expanded).
    let tab2 = spawn_claude_tab_with_output(&mut s, 60);
    s.shell.app.terminal.active_tab = tab2;
    let _ids_tab2 = apply_agent_layer_turn(&mut s, 82, "Tab2Turn1");
    if let Some(turn) = s.shell.app.terminal.ledger_for_tab_mut(tab2).turns.last_mut() {
        assert_eq!(turn.id, 1, "precondition: Tab 2's first turn also has turn.id == 1");
        turn.expanded = true;
    }
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);

    let tab2_row0 = recorded(&s, "agent-turn-row-0").expect("Tab 2 Turn 1 must start at top (Row 0 visible), not inherit Tab 1 Turn 1 scroll");
    assert!((tab2_row0.top() - row0_top).abs() < 1.0, "Tab 2 Turn 1 Row 0 must sit at the top of the list");

    // Switch back to Tab 1 -> Tab 1 Turn 1 must still be scrolled to the bottom.
    s.shell.app.terminal.active_tab = 0;
    s.ctx.data_mut(|d| d.clear());
    frame_at_panel_height(&mut s, 900.0, vec![]);
    assert!(recorded(&s, "agent-turn-row-81").is_some(), "Tab 1 Turn 1 must retain its bottom scroll position after switching back from Tab 2");
    finish(s);
}

#[test]
fn a11y_list_region_announces_row_count_when_rows_off_screen() {
    let (mut s, _ids) = setup_panel_with_turn(900.0, 60, 82, true);
    crate::agent_a11y::enable_accesskit(&s.ctx);
    frame_at_panel_height(&mut s, 900.0, vec![]);

    let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
    let has_region = update.nodes.iter().any(|(_, node)| node.role() == egui::accesskit::Role::Region && node.label() == Some("82 changed layers"));
    assert!(
        has_region,
        "Accessibility: the changed-layers list region must announce its total row count (\"82 changed layers\") even when rows are off-screen"
    );
    finish(s);
}
