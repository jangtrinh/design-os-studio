//! The simplified Agents panel: one header row, an empty New chat body, and a composer that carries the agent pill, the
//! attach menu, the permission mode and Send. Every test builds its own `Sim` over stub agent CLIs.

use super::*;
use crate::agent_cli_catalog::{CliProbe, SignIn};
use crate::terminal::{AgentCliPreset, AgentTabStatus, TERMINAL_HEADER_H};

pub(super) fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// Waits for the background version probes of the stub CLIs, so the agent pill and the menu rows stop changing.
pub(super) fn settle_probes(s: &mut Sim) {
    for _ in 0..400 {
        s.frame(vec![]);
        if !s.shell.app.terminal.picker.cache.is_probing() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    frames(s, 3);
}

/// The Agents panel in the New chat state: no session, stub agent CLIs, Claude Code is the pill's agent.
pub(super) fn new_chat_sim() -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.session_registry.last_cli = None;
    s.cmd_key(Key::Backtick);
    assert!(s.app().terminal.tabs.is_empty(), "no session yet");
    settle_probes(&mut s);
    s
}

/// The Agents panel over one running stub Claude tab, the terminal holding the keyboard.
pub(super) fn session_sim() -> Sim {
    let mut s = new_chat_sim();
    s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    s.shell.app.terminal.viewport_focused = false;
    frames(&mut s, 3);
    s
}

pub(super) fn finish(mut s: Sim) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

pub(super) fn recorded(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

pub(super) fn painted(s: &Sim) -> Vec<String> {
    crate::lint::scan_shapes(&s.shapes).0.into_iter().map(|t| t.text).collect()
}

fn panel_top(s: &Sim) -> f32 {
    recorded(s, "agents-terminal-rect").expect("the Agents panel recorded its area").top()
}

const RETIRED_STRIP_KEYS: [&str; 7] = [
    "agents-sessions-btn",
    "agents-header-skills",
    "agent-selection-chip-left",
    "agent-mention-btn-left",
    "agents-needs-approval-pill",
    "agents-header-turn-status",
    "agents-header-usage",
];

const RETIRED_COMPOSER_KEYS: [&str; 6] =
    ["agents-composer-book", "agents-composer-prd", "agents-composer-skills", "agents-composer-attach", "agents-composer-plus", "agents-composer-context"];

#[test]
fn agents_new_chat_has_at_most_17_interactive_widgets() {
    let mut s = new_chat_sim();
    s.ctx.all_styles_mut(|st| st.debug.show_interactive_widgets = true);
    frames(&mut s, 4);
    let snap = crate::layout_snapshot::capture(&s.ctx, &s.shell.app.ui.regions);
    let named: usize = snap.named.values().filter(|w| w.region == "left_panel").map(|w| w.count).sum();
    let unnamed = snap.unnamed.iter().filter(|w| w.region == "left_panel").count();
    let total = named + unnamed;
    eprintln!("interactive widgets in the New chat panel: {total} ({named} named, {unnamed} unnamed)");
    assert!(total <= 17, "the New chat panel holds {total} widgets (33 before the simplification)");
    // The same count over the committed snapshot of the gate's own `i-agents` case, which holds a few more rects of the window.
    let blessed: crate::layout_snapshot::LayoutSnapshot =
        serde_json::from_str(include_str!("../../../tests/layout-baseline/i-agents.layout.json")).expect("the blessed i-agents snapshot parses");
    let blessed_total = blessed.named.values().filter(|w| w.region == "left_panel").map(|w| w.count).sum::<usize>()
        + blessed.unnamed.iter().filter(|w| w.region == "left_panel").count();
    assert!(blessed_total <= 17, "the blessed i-agents case holds {blessed_total} widgets");
    for key in [
        "agents-new-chat-tab",
        "agents-new-chat-btn",
        "agents-more-btn",
        "agents-composer-add-btn",
        "agents-composer-agent-pill",
        "agents-composer-mode-btn",
        "agents-composer-send",
    ] {
        assert!(snap.named.get(key).is_some_and(|w| w.region == "left_panel"), "{key} is a control of the panel: {:?}", snap.named.keys().collect::<Vec<_>>());
    }
    finish(s);
}

fn has_version(text: &str) -> bool {
    let b = text.as_bytes();
    (0..b.len()).any(|i| {
        let mut j = i;
        let mut groups = 0;
        while j < b.len() && b[j].is_ascii_digit() {
            let start = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j == start {
                break;
            }
            groups += 1;
            if groups == 3 {
                return true;
            }
            if j < b.len() && b[j] == b'.' && j + 1 < b.len() && b[j + 1].is_ascii_digit() {
                j += 1;
            } else {
                break;
            }
        }
        false
    })
}

#[test]
fn no_agent_row_paints_a_version() {
    let mut s = new_chat_sim();
    for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok] {
        s.shell.app.terminal.picker.cache.set_probe(preset, CliProbe { installed: true, version: Some("1.0.0".into()), signed_in: SignIn::Yes });
    }
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.terminal.picker.menu_more_open = true;
    frames(&mut s, 3);
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Claude Code"), "the menu is open: {texts:?}");
    assert!(!texts.iter().any(|t| has_version(t)), "no row paints a version: {texts:?}");
    finish(s);
}

#[test]
fn new_chat_paints_no_sample_prompt_and_no_suggestion_text() {
    let s = new_chat_sim();
    let texts = painted(&s);
    for banned in ["What's next?", "Review this frame", "Try", "Type @ to mention layers.", "No agent CLI found. Install one to start."] {
        assert!(!texts.iter().any(|t| t == banned), "{banned:?} is painted: {texts:?}");
    }
    for key in ["agents-sample-prompt-0", "agents-empty-attach-hint", "agents-new-session-picker", "agents-no-cli-line"] {
        assert!(recorded(&s, key).is_none(), "{key} is recorded");
    }
    finish(s);
}

#[test]
fn agents_header_is_one_row() {
    let s = session_sim();
    let top = panel_top(&s);
    let tab = recorded(&s, "agents-left-tab-select").expect("the session tab");
    let plus = recorded(&s, "agents-new-chat-btn").expect("new chat");
    let more = recorded(&s, "agents-more-btn").expect("more");
    for (name, r) in [("tab", tab), ("new chat", plus), ("more", more)] {
        assert!(r.top() >= top - 0.01 && r.bottom() <= top + TERMINAL_HEADER_H + 0.01, "{name} {r:?} lies inside the {TERMINAL_HEADER_H} px header at {top}");
        assert_eq!(r.height(), crate::theme::TARGET_MIN, "{name} is 24 px high (R10.1)");
        assert!((r.center().y - tab.center().y).abs() < 0.5, "{name} shares the tab's vertical centre");
    }
    for key in RETIRED_STRIP_KEYS {
        assert!(recorded(&s, key).is_none(), "{key} is gone");
    }
    finish(s);
}

#[test]
fn composer_toolbar_is_plus_pill_mode_send() {
    let s = new_chat_sim();
    let bx = recorded(&s, "agents-composer-box").expect("composer box");
    let rects: Vec<Rect> = ["agents-composer-add-btn", "agents-composer-agent-pill", "agents-composer-mode-btn", "agents-composer-send"]
        .iter()
        .map(|k| recorded(&s, k).unwrap_or_else(|| panic!("{k} is drawn")))
        .collect();
    for r in &rects {
        assert_eq!(r.height(), crate::theme::CONTROL_XS, "every toolbar control is xs 32: {r:?}");
        assert!(bx.contains_rect(*r), "{r:?} lies inside the composer box {bx:?}");
    }
    for pair in rects.windows(2) {
        assert!(pair[0].right() <= pair[1].left() + 0.01, "left to right: {:?} then {:?}", pair[0], pair[1]);
    }
    for key in RETIRED_COMPOSER_KEYS {
        assert!(recorded(&s, key).is_none(), "{key} is gone");
    }
    assert_eq!(bx.height(), 66.0, "one line and nothing else is 66 px");
    finish(s);
}

#[test]
fn context_row_is_gone() {
    let mut s = session_sim();
    let page = s.shell.app.page;
    let fid = s.shell.app.doc.alloc_id();
    let node = designos_doc::Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0));
    s.shell.app.doc.insert(page, node).unwrap();
    s.shell.app.selection = vec![fid];
    let tab_id = s.shell.app.terminal.active_tab().unwrap().id;
    *s.shell.app.terminal.active_tab().unwrap().agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
    frames(&mut s, 3);
    for key in RETIRED_STRIP_KEYS {
        assert!(recorded(&s, key).is_none(), "{key} is gone");
    }
    let chip = recorded(&s, "agent-selection-chip").expect("the selection chip is drawn");
    let bx = recorded(&s, "agents-composer-box").expect("composer box");
    assert!(bx.contains_rect(chip), "the selection chip {chip:?} lives inside the composer box {bx:?}");
    assert!(recorded(&s, &format!("agents-tab-dot-{tab_id}")).is_some(), "the tab carries the dot while the session needs the user");
    finish(s);
}

#[test]
fn placeholder_carries_the_at_hint() {
    let s = new_chat_sim();
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Ask Claude Code…  @ to mention a layer"), "New chat placeholder: {texts:?}");
    finish(s);

    let mut s = session_sim();
    s.shell.app.terminal.active_tab_mut().unwrap().composer.send_count = 1;
    frames(&mut s, 2);
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Reply to Claude Code…"), "after the first send: {texts:?}");
    finish(s);

    let mut s = new_chat_sim();
    s.shell.app.terminal.spawn_tab(AgentCliPreset::Shell, None).expect("shell tab");
    frames(&mut s, 3);
    let texts = painted(&s);
    assert_eq!(texts.iter().filter(|t| t.as_str() == "Select an agent tab").count(), 1, "a shell tab says it once: {texts:?}");
    finish(s);
}

#[test]
fn plus_starts_new_chat_and_send_starts_the_pill_agent() {
    let mut s = session_sim();
    let before: Vec<u8> = s.shell.app.terminal.tabs[0].written_bytes.clone();
    let now = crate::agent_sessions::unix_timestamp_secs();
    s.shell.app.terminal.session_registry.sessions.push(crate::agent_sessions::SessionEntry {
        tab_id: 77,
        title: "Old Grok chat".into(),
        cli: "grok".into(),
        session_id: None,
        created_at: now - 60,
        last_active_at: now - 30,
        status: crate::agent_sessions::SessionEntryStatus::Exited,
        extra: Default::default(),
    });
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open, "+ opens the New chat state");
    assert_eq!(s.shell.app.terminal.tabs.len(), 1, "and spawns nothing");
    assert!(recorded(&s, "agents-new-chat-tab").is_some(), "the New chat tab is selected");
    s.type_text("make it blue");
    assert_eq!(s.shell.app.terminal.composer_draft(), "make it blue", "+ focuses the composer, so the letters land in it");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert_eq!(s.shell.app.terminal.tabs.len(), 2, "Send starts the pill's agent in a new tab");
    assert_eq!(s.shell.app.terminal.tabs[1].spawn_preset, AgentCliPreset::Claude, "the pill's agent, not the resumable Grok session");
    assert!(!s.shell.app.terminal.tabs[1].is_resumed);
    assert!(s.shell.app.terminal.tabs[1].composer.pending_send.is_some() || s.shell.app.terminal.tabs[1].composer.send_count > 0, "with the text queued");
    assert_eq!(s.shell.app.terminal.tabs[0].written_bytes, before, "the old tab's PTY received no bytes");
    finish(s);
}

#[test]
fn tab_click_clears_picker_open_and_new_chat_card_reads_open_tab() {
    let mut s = new_chat_sim();
    s.shell.app.terminal.open_approval_fixture("agents-approval-command", None);
    frames(&mut s, 3);
    assert!(recorded(&s, "agent-approval-approve").is_some(), "on the live tab the card has Approve");
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    // A rect stays in the context until it is drawn again, so drop them to see what the next frames draw.
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open);
    assert!(recorded(&s, "agent-approval-open-tab").is_some(), "the New chat state covers the tab: the card reads Open tab");
    assert!(recorded(&s, "agent-approval-approve").is_none(), "and has no Approve");
    let tab = recorded(&s, "agents-left-tab-select").expect("the session tab");
    s.click(tab.center());
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    assert!(!s.shell.app.terminal.picker_open, "a tab click clears the New chat state");
    assert!(recorded(&s, "agent-approval-approve").is_some(), "Approve returns");
    finish(s);
}

#[test]
fn tab_dot_only_when_the_session_needs_the_user() {
    let mut s = session_sim();
    let tab_id = s.shell.app.terminal.active_tab().unwrap().id;
    let dot_key = format!("agents-tab-dot-{tab_id}");
    let dot_circle = |s: &Sim, fill: egui::Color32| {
        s.shapes.iter().any(|cs| matches!(&cs.shape, egui::epaint::Shape::Circle(c) if (c.radius - 4.0).abs() < 0.05 && c.fill == fill && c.center.y < 40.0))
    };
    for quiet in [AgentTabStatus::Working(None), AgentTabStatus::Done, AgentTabStatus::Idle, AgentTabStatus::Stopped] {
        *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = quiet.clone();
        frames(&mut s, 2);
        assert!(recorded(&s, &dot_key).is_none(), "{quiet:?} draws no dot");
    }
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
    frames(&mut s, 2);
    assert!(recorded(&s, &dot_key).is_some() && dot_circle(&s, crate::theme::ICON_BRAND_PRIMARY), "waiting draws one brand dot");
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Error(Some("503".into()));
    frames(&mut s, 2);
    assert!(dot_circle(&s, crate::theme::DANGER), "a failed session draws the danger dot");
    assert!(!dot_circle(&s, crate::theme::ICON_BRAND_PRIMARY), "and never two dots");
    finish(s);
}

#[test]
fn mode_control_sets_the_permission_mode() {
    let mut s = new_chat_sim();
    assert!(s.app().lib.is_none(), "the window has no library, so nothing is saved to disk");
    let mode = recorded(&s, "agents-composer-mode-btn").unwrap();
    s.click(mode.center());
    frames(&mut s, 2);
    assert!(recorded(&s, "agents-composer-mode-menu").is_some(), "the menu opens");
    let skip = recorded(&s, "agents-composer-mode-skip").expect("Skip permissions row");
    s.click(skip.center());
    frames(&mut s, 3);
    assert_eq!(s.shell.app.terminal.permission_mode, crate::agent_permissions::AgentPermissionMode::Skip);
    assert!(painted(&s).iter().any(|t| t == "Skip permissions"), "the label follows the mode: {:?}", painted(&s));
    finish(s);
}

#[test]
fn chrome_focus_band_is_the_header_only() {
    let mut s = session_sim();
    let top = panel_top(&s);
    let tab = recorded(&s, "agents-left-tab-select").expect("the session tab");
    let x = tab.right() + 12.0;
    s.click(egui::pos2(x, top + TERMINAL_HEADER_H + 12.0));
    assert!(s.shell.app.terminal.viewport_focused && !s.shell.app.terminal.chrome_focused, "a press under the header is the viewport");
    s.click(egui::pos2(x, top + TERMINAL_HEADER_H * 0.5));
    assert!(s.shell.app.terminal.chrome_focused && !s.shell.app.terminal.viewport_focused, "a press in the header is chrome");
    finish(s);
}

#[test]
fn no_status_line_under_the_composer() {
    let mut s = new_chat_sim();
    s.shell.app.terminal.set_composer_draft("x".repeat(crate::agent_composer::MAX_COMPOSER_CHARS + 1));
    let _ = s.shell.app.terminal.send_to_agent("x", None);
    s.shell.app.terminal.active_composer_mut().notice = Some("Message is too long.".into());
    s.shell.app.terminal.active_composer_mut().last_outcome = Some(crate::agent_composer::SendOutcome::Refused(crate::agent_composer::RefuseReason::TooLong));
    frames(&mut s, 3);
    let bx = recorded(&s, "agents-composer-box").expect("composer box");
    let note = recorded(&s, "agents-composer-notice").expect("a refusal is captioned");
    assert!(bx.contains_rect(note), "the caption {note:?} lies inside the box {bx:?}");
    finish(s);
}

#[test]
fn agents_viewport_spans_header_to_composer() {
    let s = new_chat_sim();
    let top = panel_top(&s);
    let viewport = recorded(&s, "agents-terminal-viewport").expect("the panel recorded its viewport");
    let composer = recorded(&s, "agents-composer-box").expect("composer box");
    assert_eq!(viewport.top(), top + TERMINAL_HEADER_H, "the viewport starts at the header's bottom, no context strip between (60 before)");
    assert!(viewport.bottom() <= composer.top() + 0.01, "and ends above the composer: {viewport:?} {composer:?}");
    assert!(composer.top() - viewport.bottom() <= crate::theme::SPACE_INLINE + 0.01, "with nothing but the gap between them (no receipt row)");
    finish(s);
}

#[test]
fn sign_in_shows_only_on_a_row_known_signed_out_and_runs_the_cli_login() {
    let mut s = new_chat_sim();
    let probe = |installed, signed_in| CliProbe { installed, version: Some("1.0.0".into()), signed_in };
    s.shell.app.terminal.picker.cache.set_probe(AgentCliPreset::Claude, probe(true, SignIn::No));
    s.shell.app.terminal.picker.cache.set_probe(AgentCliPreset::Codex, probe(true, SignIn::Unknown));
    s.shell.app.terminal.picker.cache.set_probe(AgentCliPreset::Grok, probe(true, SignIn::Yes));
    s.shell.app.terminal.picker.cache.set_probe(AgentCliPreset::Gemini, probe(true, SignIn::Unknown));
    s.shell.app.terminal.runtime_dropdown_open = true;
    frames(&mut s, 3);
    let texts = painted(&s);
    assert_eq!(texts.iter().filter(|t| t.as_str() == "Sign in").count(), 1, "Sign in only on the row Studio knows is signed out: {texts:?}");
    for banned in ["Sign-in unknown", "Signed in", "Not signed in", "Not installed"] {
        assert!(!texts.iter().any(|t| t == banned), "{banned:?} is painted: {texts:?}");
    }
    let sign_in = recorded(&s, "agents-menu-sign-in-claude").expect("Claude's Sign in");
    s.click(sign_in.center());
    frames(&mut s, 3);
    assert!(!s.shell.app.terminal.runtime_dropdown_open, "the menu closes");
    let titles: Vec<String> = s.shell.app.terminal.tabs.iter().map(|t| t.title.clone()).collect();
    assert!(titles.iter().any(|t| t.contains("auth login")), "a plain command tab runs the CLI's own sign-in: {titles:?}");
    finish(s);
}
