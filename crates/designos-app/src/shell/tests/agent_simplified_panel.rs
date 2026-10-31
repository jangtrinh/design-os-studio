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
        assert_eq!(r.height(), crate::theme::TARGET_MIN, "{name} is 24 px high");
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
    s.shell.app.terminal.open_approval_fixture("agents-approval-command", None, crate::agent_permissions::AgentPermissionMode::Ask);
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
fn a_single_tab_with_a_dot_shows_its_full_title_at_the_default_panel_width() {
    for status in [AgentTabStatus::WaitingForInput(Some("Needs approval".into())), AgentTabStatus::Error(Some("503".into()))] {
        let mut s = session_sim();
        let tab_id = s.shell.app.terminal.tabs[0].id;
        let title = s.shell.app.terminal.tabs[0].title.clone();
        *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = status.clone();
        frames(&mut s, 3);
        assert!(recorded(&s, &format!("agents-tab-dot-{tab_id}")).is_some(), "{status:?} carries the dot");
        let tab = recorded(&s, "agents-left-tab-select").expect("the tab");
        let plus = recorded(&s, "agents-new-chat-btn").expect("the + button");
        assert!(plus.left() - tab.right() > 100.0, "the header has room to spare: tab {tab:?}, + {plus:?}");
        let texts = painted(&s);
        assert!(texts.contains(&title), "{status:?}: the lone tab shows {title:?} in full, not elided: {texts:?}");
        finish(s);
    }
}

#[test]
fn the_agents_picker_capture_panel_opens_the_agent_menu_and_the_agents_panel_does_not() {
    for (panel, menu_open) in [("agents", false), ("agents-picker", true)] {
        let mut s = Sim::with_starter(Options { panel: Some(panel.into()), ..Options::default() });
        s.use_stub_agent_clis();
        s.shell.app.terminal.session_registry.last_cli = None;
        settle_probes(&mut s);
        assert!(s.shell.app.terminal.picker_open, "--panel {panel} shows New chat");
        assert_eq!(s.shell.app.terminal.runtime_dropdown_open, menu_open, "--panel {panel}: the agent menu is open only for agents-picker");
        assert_eq!(recorded(&s, "agents-agent-menu").is_some(), menu_open, "--panel {panel}: the menu is drawn only when it is open");
        finish(s);
    }
}

#[test]
fn mode_control_sets_the_permission_mode() {
    let mut s = new_chat_sim();
    assert!(s.app().lib.is_none(), "the window has no library, so nothing is saved to disk");
    let mode = recorded(&s, "agents-composer-mode-btn").unwrap();
    s.click(mode.center());
    frames(&mut s, 2);
    assert!(recorded(&s, "agents-composer-mode-menu").is_some(), "the menu opens");
    assert!(painted(&s).iter().any(|t| t == "Applies to new agent tabs."), "the menu says which tabs the mode reaches: {:?}", painted(&s));
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
    let too_long = "x".repeat(crate::agent_composer::MAX_COMPOSER_CHARS + 1);
    s.shell.app.terminal.set_composer_draft(too_long.clone());
    let outcome = s.shell.app.terminal.send_to_agent(&too_long, None);
    assert_eq!(outcome, crate::agent_composer::SendOutcome::Refused(crate::agent_composer::RefuseReason::TooLong), "the refusal comes from the send path");
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

/// A running stub Claude tab, mid-turn, with the terminal holding the keyboard.
fn working_session() -> Sim {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Working(None);
    frames(&mut s, 2);
    s
}

fn pty_bytes_since(s: &Sim, before: usize) -> Vec<u8> {
    s.shell.app.terminal.tabs[0].written_bytes[before..].to_vec()
}

fn turn_is_working(s: &Sim) -> bool {
    matches!(*s.shell.app.terminal.tabs[0].agent_status.lock().unwrap(), AgentTabStatus::Working(_))
}

/// A point in the terminal viewport that no menu covers.
fn viewport_point(s: &Sim) -> egui::Pos2 {
    let vp = recorded(s, "agents-terminal-viewport").expect("the panel recorded its viewport");
    egui::pos2(vp.center().x, vp.top() + 40.0)
}

#[test]
fn attach_and_mode_menus_own_the_keyboard() {
    type Open = fn(&crate::terminal::TerminalState) -> bool;
    let menus: [(&str, Open, bool); 3] = [
        ("agents-composer-add-btn", |t| t.attach_menu_open, true),
        ("agents-composer-mode-btn", |t| t.mode_menu_open, true),
        ("agents-composer-agent-pill", |t| t.runtime_dropdown_open, false),
    ];
    for (anchor, is_open, takes_enter) in menus {
        let mut s = working_session();
        let a = recorded(&s, anchor).unwrap_or_else(|| panic!("{anchor} is drawn"));
        s.click(a.center());
        frames(&mut s, 2);
        assert!(is_open(&s.shell.app.terminal), "{anchor} opens its menu");
        assert!(s.shell.app.terminal.overlay_owns_keyboard, "{anchor}: an open menu owns the keyboard");

        // The terminal would take these keys if the menu did not own them.
        s.shell.app.terminal.viewport_focused = true;
        let before = s.shell.app.terminal.tabs[0].written_bytes.len();
        s.key(Key::ArrowDown);
        s.key(Key::ArrowUp);
        if takes_enter {
            s.key(Key::Enter);
        }
        assert!(pty_bytes_since(&s, before).is_empty(), "{anchor}: arrows and Enter go to the menu, not the PTY: {:?}", pty_bytes_since(&s, before));
        assert!(is_open(&s.shell.app.terminal), "{anchor}: the menu is still open");

        // Esc closes the menu and stops nothing.
        s.shell.app.terminal.viewport_focused = true;
        s.key(Key::Escape);
        frames(&mut s, 2);
        assert!(!is_open(&s.shell.app.terminal), "{anchor}: Esc closes the menu");
        assert!(pty_bytes_since(&s, before).is_empty(), "{anchor}: Esc writes no byte to the PTY: {:?}", pty_bytes_since(&s, before));
        assert!(turn_is_working(&s), "{anchor}: Esc did not stop the running turn");

        // A press outside closes it.
        s.click(a.center());
        frames(&mut s, 2);
        assert!(is_open(&s.shell.app.terminal), "{anchor}: reopened");
        s.click(viewport_point(&s));
        frames(&mut s, 2);
        assert!(!is_open(&s.shell.app.terminal), "{anchor}: a press outside closes the menu");

        // Control: with the menu closed, the keys reach the terminal again.
        let before = s.shell.app.terminal.tabs[0].written_bytes.len();
        s.type_text("z");
        frames(&mut s, 2);
        assert!(String::from_utf8_lossy(&pty_bytes_since(&s, before)).contains('z'), "{anchor}: the terminal hears typing again");
        finish(s);
    }
}

#[test]
fn skills_popover_closes_on_esc_and_an_outside_press_and_returns_the_keyboard() {
    let mut s = working_session();
    s.shell.app.terminal.skills.open_picker();
    frames(&mut s, 3);
    assert!(recorded(&s, "agents-skills-picker-popover").is_some(), "the popover is drawn");
    assert!(s.shell.app.terminal.overlay_owns_keyboard, "an open popover owns the keyboard");

    s.shell.app.terminal.viewport_focused = true;
    let before = s.shell.app.terminal.tabs[0].written_bytes.len();
    s.key(Key::Escape);
    frames(&mut s, 2);
    assert!(!s.shell.app.terminal.skills.picker_open, "Esc closes the popover");
    assert!(pty_bytes_since(&s, before).is_empty(), "Esc writes no byte to the PTY: {:?}", pty_bytes_since(&s, before));
    assert!(turn_is_working(&s), "Esc did not stop the running turn");

    s.shell.app.terminal.skills.open_picker();
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.skills.picker_open, "reopened");
    s.click(viewport_point(&s));
    frames(&mut s, 2);
    assert!(!s.shell.app.terminal.skills.picker_open, "a press outside closes the popover");

    let before = s.shell.app.terminal.tabs[0].written_bytes.len();
    s.type_text("z");
    frames(&mut s, 2);
    assert!(String::from_utf8_lossy(&pty_bytes_since(&s, before)).contains('z'), "the terminal hears typing again");
    finish(s);
}

#[test]
fn the_skills_popover_opened_by_a_click_survives_that_click() {
    let mut s = session_sim();
    let add = recorded(&s, "agents-composer-add-btn").unwrap();
    s.click(add.center());
    frames(&mut s, 2);
    let skills = recorded(&s, "agents-composer-skills-item").expect("the Skills… item");
    s.mv(skills.center());
    s.one_frame().press().release().send();
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.skills.picker_open, "the press that picks Skills… is not an outside press");
    finish(s);
}

#[test]
fn the_skills_chip_opens_and_closes_the_popover() {
    let mut s = session_sim();
    s.shell.app.terminal.skills.set_selected_skills(vec!["design-system-rhythm".into()]);
    frames(&mut s, 3);
    let chip = recorded(&s, "agents-composer-chip-skills").expect("the skills chip");
    let _release = {
        s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
        crate::widgets::RecordDisabled::begin()
    };
    s.click(chip.center());
    frames(&mut s, 2);
    assert!(s.shell.app.terminal.skills.picker_open, "the chip opens the popover");
    s.click(chip.center());
    frames(&mut s, 2);
    assert!(!s.shell.app.terminal.skills.picker_open, "the chip, its own anchor, closes it");
    finish(s);
}

#[test]
fn a_composer_menu_closes_from_its_own_anchor() {
    type Open = fn(&crate::terminal::TerminalState) -> bool;
    let menus: [(&str, Open); 3] = [
        ("agents-composer-add-btn", |t| t.attach_menu_open),
        ("agents-composer-mode-btn", |t| t.mode_menu_open),
        ("agents-composer-agent-pill", |t| t.runtime_dropdown_open),
    ];
    for (anchor, is_open) in menus {
        let mut s = new_chat_sim();
        let a = recorded(&s, anchor).unwrap_or_else(|| panic!("{anchor} is drawn"));
        s.click(a.center());
        frames(&mut s, 2);
        assert!(is_open(&s.shell.app.terminal), "{anchor}: the first press opens the menu");
        s.click(a.center());
        frames(&mut s, 2);
        assert!(!is_open(&s.shell.app.terminal), "{anchor}: the second press closes it");

        // A slow frame hands over the press and the release together, with no rects stored, as in a release build.
        s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
        let _release = crate::widgets::RecordDisabled::begin();
        s.mv(a.center());
        s.one_frame().press().release().send();
        frames(&mut s, 2);
        assert!(is_open(&s.shell.app.terminal), "{anchor}: opens in one frame");
        s.one_frame().press().release().send();
        frames(&mut s, 2);
        assert!(!is_open(&s.shell.app.terminal), "{anchor}: closes in one frame");
        finish(s);
    }
}

#[test]
fn new_chat_from_the_keyboard_closes_every_menu_and_popover() {
    let mut s = session_sim();
    s.shell.app.terminal.attach_menu_open = true;
    s.shell.app.terminal.mode_menu_open = true;
    s.shell.app.terminal.skills.open_picker();
    s.ctx.memory_mut(|m| m.request_focus(egui::Id::new("agents-new-chat-btn")));
    frames(&mut s, 2);
    // No pointer press: only a keyboard activation reaches the button.
    s.key(Key::Enter);
    frames(&mut s, 2);
    let t = &s.shell.app.terminal;
    assert!(t.picker_open, "Enter on + starts New chat");
    assert!(!t.attach_menu_open && !t.mode_menu_open && !t.skills.picker_open, "and closes every menu and popover");
    finish(s);
}

#[test]
fn stop_reads_disabled_while_it_is_stopping() {
    use crate::widgets::kit::{button_look, Hierarchy, State};
    let mut s = working_session();
    let fill_of = |s: &Sim, rect: Rect| -> Vec<egui::Color32> {
        s.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Rect(r) if r.rect.center().distance(rect.center()) < 1.0 && (r.rect.width() - rect.width()).abs() < 1.0 => Some(r.fill),
                _ => None,
            })
            .collect()
    };
    let stop = recorded(&s, "agents-header-stop-btn").expect("Stop is drawn on a running turn");
    let disabled = button_look(Hierarchy::Destructive, State::Disabled).fill;
    assert!(!fill_of(&s, stop).contains(&disabled), "an idle Stop is enabled");
    s.click(stop.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.tabs[0].stop_state.is_some(), "the click started a stop");
    let stop = recorded(&s, "agents-header-stop-btn").expect("Stop stays while stopping");
    assert!(painted(&s).iter().any(|t| t == "Stopping…"), "{:?}", painted(&s));
    assert!(fill_of(&s, stop).contains(&disabled), "Stopping… is drawn with the disabled look: {:?}", fill_of(&s, stop));
    finish(s);
}

#[test]
fn a_working_session_row_spins_in_the_info_colour() {
    let mut s = working_session();
    let more = recorded(&s, "agents-more-btn").unwrap();
    s.click(more.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.sessions_popover_open, "the sessions popover is open");
    let spinner_colours: Vec<egui::Color32> = s
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::LineSegment { stroke, .. } => Some(stroke.color),
            _ => None,
        })
        .collect();
    assert!(spinner_colours.contains(&crate::theme::INFO), "the working spinner is INFO: {spinner_colours:?}");
    assert!(!spinner_colours.contains(&crate::theme::WARNING), "and not WARNING");
    finish(s);
}

#[test]
fn chips_wear_the_kit_tag_look() {
    use crate::widgets::kit::{tag_look, State};
    let mut s = session_sim();
    s.shell.app.terminal.active_composer_mut().attach_prd = true;
    frames(&mut s, 3);
    let chip = recorded(&s, "agents-composer-chip-prd").expect("the PRD chip");
    let look = tag_look(State::Default);
    let rects: Vec<&egui::epaint::RectShape> = s
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Rect(r) if (r.rect.center() - chip.center()).length() < 1.0 && (r.rect.width() - chip.width()).abs() < 1.0 => Some(r),
            _ => None,
        })
        .collect();
    assert!(
        rects.iter().any(|r| r.fill == look.fill && r.corner_radius == crate::theme::cr(crate::theme::RADIUS_CONTROL)),
        "the chip is a kit tag with the control radius: {rects:?}"
    );
    let label = s.shapes.iter().find_map(|cs| match &cs.shape {
        egui::epaint::Shape::Text(t) if chip.contains(t.pos) && t.galley.job.text == "PRD" => Some(t),
        _ => None,
    });
    let label = label.expect("the PRD label is painted inside the chip");
    assert_eq!(label.galley.job.sections[0].format.font_id.size, crate::theme::FONT_SIZE_TEXT_XS, "the label is text-xs");
    finish(s);
}

#[test]
fn the_skills_popover_treats_the_strip_of_a_detached_chip_as_outside() {
    let mut s = working_session();
    s.shell.app.terminal.skills.set_selected_skills(vec!["design-system-rhythm".into()]);
    frames(&mut s, 3);
    let chip = recorded(&s, "agents-composer-chip-skills").expect("the skills chip is drawn");
    s.shell.app.terminal.skills.set_selected_skills(Vec::new());
    frames(&mut s, 3);
    assert!(recorded(&s, "agents-composer-chip-skills").is_some_and(|r| r == chip), "the recorded rect is left over from the last frame the chip was drawn");
    s.shell.app.terminal.skills.open_picker();
    frames(&mut s, 3);
    let popover = recorded(&s, "agents-skills-picker-popover").expect("the popover is drawn");
    let press = [0.15f32, 0.5, 0.85]
        .into_iter()
        .map(|fx| egui::pos2(chip.left() + chip.width() * fx, chip.bottom() - 3.0))
        .find(|p| !popover.contains(*p))
        .expect("a point of the old chip strip lies outside the popover");
    s.shell.app.terminal.viewport_focused = true;
    s.click(press);
    frames(&mut s, 2);
    assert!(!s.shell.app.terminal.skills.picker_open, "a press where the chip used to be closes the popover");
    assert!(!s.shell.app.terminal.overlay_owns_keyboard, "and the terminal hears the keyboard again");
    finish(s);
}

/// Starts `n` more stub Claude tabs, so the strip has more tabs than room.
fn spawn_more_tabs(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    }
    s.shell.app.terminal.viewport_focused = false;
    frames(s, 3);
}

/// How many recorded widgets carry `name` in the left panel, from the layout snapshot (a rect record keeps only the last one).
fn drawn_count(s: &Sim, name: &str) -> usize {
    let snap = crate::layout_snapshot::capture(&s.ctx, &s.shell.app.ui.regions);
    snap.named.get(name).map_or(0, |w| w.count)
}

#[test]
fn the_new_chat_tab_is_the_last_tab_hidden() {
    let mut s = session_sim();
    spawn_more_tabs(&mut s, 9);
    let before = drawn_count(&s, "agents-left-tab-select");
    assert!(before < 10, "ten tabs do not all fit the strip: {before} drawn");
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open);
    let new_chat = recorded(&s, "agents-new-chat-tab").expect("the New chat tab is drawn though the session tabs overflow");
    let more = recorded(&s, "agents-more-btn").unwrap();
    assert!(new_chat.right() <= more.left(), "inside the strip: {new_chat:?} before {more:?}");
    assert!(drawn_count(&s, "agents-left-tab-select") >= 1, "at least one session tab is still reachable on the strip");
    assert!(drawn_count(&s, "agents-left-tab-select") < before, "the session tabs gave way to it ({before} before)");
    let tab = recorded(&s, "agents-left-tab-select").expect("a session tab");
    assert!(tab.right() <= new_chat.left(), "New chat comes after the session tabs");
    finish(s);
}

#[test]
fn hidden_tab_that_needs_the_user_puts_the_dot_on_more() {
    let mut s = session_sim();
    spawn_more_tabs(&mut s, 9);
    let last = s.shell.app.terminal.tabs.len() - 1;
    let last_id = s.shell.app.terminal.tabs[last].id;
    s.shell.app.terminal.active_tab = 0;
    frames(&mut s, 3);
    assert!(drawn_count(&s, "agents-left-tab-select") < 10, "the last tab is hidden");
    assert!(recorded(&s, &format!("agents-tab-dot-{last_id}")).is_none(), "and has no dot of its own on the strip");
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 2);
    assert!(recorded(&s, "agents-more-dot").is_none(), "while every hidden tab is quiet, ⋯ has no dot");

    *s.shell.app.terminal.tabs[last].agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    let dot = recorded(&s, "agents-more-dot").expect("a hidden tab waits: ⋯ carries the dot");
    let more = recorded(&s, "agents-more-btn").unwrap();
    assert!(more.contains(dot.center()), "the dot sits on ⋯: {dot:?} in {more:?}");
    // The tooltip counts the tabs that wait.
    s.mv(more.center() + egui::vec2(0.0, 1.0));
    s.mv(more.center());
    frames(&mut s, 30);
    assert!(painted(&s).iter().any(|t| t == "1 waiting"), "the tooltip of ⋯ counts the waiting tab: {:?}", painted(&s));
    finish(s);
}

#[test]
fn stop_stays_in_the_header_and_stop_all_is_in_more() {
    let mut s = working_session();
    let top = panel_top(&s);
    let stop = recorded(&s, "agents-header-stop-btn").expect("Stop is drawn while a turn runs");
    assert_eq!(stop.height(), crate::theme::TARGET_MIN, "Stop is 24 high");
    assert!(stop.top() >= top - 0.01 && stop.bottom() <= top + TERMINAL_HEADER_H + 0.01, "and lies in the header row: {stop:?}");
    assert!(recorded(&s, "agents-header-stop-all-btn").is_none(), "there is no Stop all in the header");
    let more = recorded(&s, "agents-more-btn").unwrap();
    s.click(more.center());
    frames(&mut s, 3);
    assert!(!painted(&s).iter().any(|t| t == "Stop all"), "one running tab does not offer Stop all: {:?}", painted(&s));
    s.key(Key::Escape);
    frames(&mut s, 2);

    s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("a second stub claude");
    *s.shell.app.terminal.tabs[1].agent_status.lock().unwrap() = AgentTabStatus::Working(None);
    s.shell.app.terminal.viewport_focused = false;
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    assert!(recorded(&s, "agents-header-stop-all-btn").is_none(), "two running tabs still draw no Stop all in the header");
    let more = recorded(&s, "agents-more-btn").unwrap();
    s.click(more.center());
    frames(&mut s, 3);
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Stop all"), "⋯ lists Stop all: {texts:?}");
    assert!(texts.iter().any(|t| t == "2 running"), "with the caption 2 running: {texts:?}");
    finish(s);
}

fn node_labels(s: &mut Sim, frames_n: usize) -> std::collections::HashMap<egui::accesskit::NodeId, (egui::accesskit::Role, String)> {
    let mut seen = std::collections::HashMap::new();
    for _ in 0..frames_n {
        s.frame(vec![]);
        if let Some(update) = s.accesskit_update.as_ref() {
            for (id, node) in &update.nodes {
                if let Some(label) = node.label() {
                    seen.insert(*id, (node.role(), label.to_string()));
                }
            }
        }
    }
    seen
}

#[test]
fn agents_panel_labels_follow_the_new_controls() {
    use egui::accesskit::Role;
    let mut s = session_sim();
    crate::agent_a11y::enable_accesskit(&s.ctx);
    let page = s.shell.app.page;
    let fid = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, designos_doc::Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
    s.shell.app.selection = vec![fid];
    s.shell.app.terminal.active_composer_mut().attach_prd = true;
    s.shell.app.terminal.skills.set_selected_skills(vec!["design-system-rhythm".into()]);
    let labels = node_labels(&mut s, 6);
    let named = |id: egui::Id| labels.get(&id.accesskit_id()).cloned();
    let expect: [(egui::Id, Role, &str); 8] = [
        (egui::Id::new("agents-new-chat-btn"), Role::Button, "New chat"),
        (egui::Id::new("agents-more-btn"), Role::Button, "More"),
        (egui::Id::new("agents-composer-add-btn"), Role::Button, "Attach context"),
        (egui::Id::new("agents-composer-agent-pill"), Role::Button, "Agent: Claude Code"),
        (egui::Id::new("agents-composer-mode-btn"), Role::Button, "Permissions: Ask first"),
        (egui::Id::new(("agents-composer-chip-prd", 0usize, "dismiss")), Role::Button, "Remove PRD"),
        (egui::Id::new(("agents-composer-chip-skills", 0usize, "dismiss")), Role::Button, "Detach skills"),
        (egui::Id::new(("agents-composer-chip-selection", 0usize, "dismiss")), Role::Button, "Dismiss selection"),
    ];
    for (id, role, name) in expect {
        assert_eq!(named(id), Some((role, name.to_string())), "{name}");
    }
    assert_eq!(named(egui::Id::new("agents-composer-send")).map(|(_, n)| n).as_deref(), Some("Send"), "Send keeps its name");
    for retired in ["agents-runtime-add-btn", "agents-composer-plus", "agents-composer-book", "agents-composer-prd"] {
        assert_eq!(named(egui::Id::new(retired)), None, "{retired} names no widget any more");
    }

    // The New chat state names its tab.
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    let labels = node_labels(&mut s, 4);
    assert_eq!(labels.get(&egui::Id::new("agents-new-chat-tab").accesskit_id()).cloned(), Some((Role::Tab, "New chat".to_string())));
    finish(s);
}

#[test]
fn selection_chip_dismiss_hides_it_until_the_selection_changes_and_a_click_sends() {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    let page = s.shell.app.page;
    let hero = s.shell.app.doc.alloc_id();
    let title = s.shell.app.doc.alloc_id();
    for (id, name) in [(hero, "Hero"), (title, "Title")] {
        let mut node = designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0));
        node.name = name.into();
        s.shell.app.doc.insert(page, node).unwrap();
    }
    s.shell.app.selection = vec![hero];
    frames(&mut s, 3);
    assert!(painted(&s).iter().any(|t| t == "Hero"), "the chip names the selected layer: {:?}", painted(&s));

    // A click on the label sends the selection to the agent: the tab's PTY hears it.
    let chip = recorded(&s, "agent-selection-chip").expect("the selection chip");
    let before = s.shell.app.terminal.tabs[0].written_bytes.len();
    s.click(egui::pos2(chip.left() + 12.0, chip.center().y));
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.tabs[0].written_bytes.len() > before, "the label click wrote the selection to the PTY");

    // The cross dismisses it, and it stays away for the same selection.
    let cross = egui::pos2(chip.right() - crate::theme::TARGET_MIN * 0.5, chip.center().y);
    s.click(cross);
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    frames(&mut s, 3);
    assert!(recorded(&s, "agent-selection-chip").is_none(), "× hides the chip");
    s.shell.app.selection = vec![hero];
    frames(&mut s, 3);
    assert!(recorded(&s, "agent-selection-chip").is_none(), "and it stays hidden for the same selection");
    s.shell.app.selection = vec![hero, title];
    frames(&mut s, 3);
    assert!(recorded(&s, "agent-selection-chip").is_some(), "a different selection brings it back");
    finish(s);
}

#[test]
fn selected_menu_rows_draw_a_check_icon_not_a_glyph() {
    let mut s = session_sim();
    let mode = recorded(&s, "agents-composer-mode-btn").unwrap();
    s.click(mode.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.mode_menu_open, "the mode menu is open");
    assert!(recorded(&s, "agents-composer-mode-ask").is_some(), "its rows are drawn");
    assert!(!painted(&s).iter().any(|t| t.contains('✓')), "the selected mode shows an icon, no check glyph: {:?}", painted(&s));
    s.key(Key::Escape);
    frames(&mut s, 2);
    let pill = recorded(&s, "agents-composer-agent-pill").unwrap();
    s.click(pill.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.runtime_dropdown_open, "the agent menu is open");
    assert!(!painted(&s).iter().any(|t| t.contains('✓')), "and so does the agent that is running: {:?}", painted(&s));
    finish(s);
}
