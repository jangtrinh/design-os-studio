//! The New chat state owns its composer and starts its own agent. A Send there never resumes an old session, never
//! writes into the tab it covers, and never queues a draft while no agent can start. Send is drawn enabled only when
//! something can be sent, and the agent pill follows a CLI that is installed after the menu closed.

use super::agent_simplified_panel::{finish, frames, new_chat_sim, painted, recorded, session_sim, settle_probes};
use super::*;
use crate::agent_cli_catalog::{CliProbe, SignIn};
use crate::agent_composer::{RefuseReason, SendOutcome};
use crate::agent_sessions::{SessionEntry, SessionEntryStatus};
use crate::terminal::{AgentCliPreset, AgentTabStatus};

const ALL_CLIS: [AgentCliPreset; 6] =
    [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode];

/// An earlier Grok session that the registry can resume.
fn old_grok_session() -> SessionEntry {
    let now = crate::agent_sessions::unix_timestamp_secs();
    SessionEntry {
        tab_id: 77,
        title: "Old Grok chat".into(),
        cli: "grok".into(),
        session_id: None,
        created_at: now - 60,
        last_active_at: now - 30,
        status: SessionEntryStatus::Exited,
        extra: Default::default(),
    }
}

/// The Agents panel in the New chat state with no agent CLI installed: the pill reads `Choose agent`.
fn no_agent_sim() -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.session_registry.last_cli = None;
    for preset in ALL_CLIS {
        s.shell.app.terminal.picker.cache.set_probe(preset, CliProbe { installed: false, version: None, signed_in: SignIn::Unknown });
    }
    s.cmd_key(Key::Backtick);
    settle_probes(&mut s);
    assert!(painted(&s).iter().any(|t| t == "Choose agent"), "precondition: no agent can start: {:?}", painted(&s));
    s
}

fn install(s: &Sim, preset: AgentCliPreset) {
    s.shell.app.terminal.picker.cache.set_probe(preset, CliProbe { installed: true, version: Some("1.0.0".into()), signed_in: SignIn::Yes });
}

fn type_in_composer(s: &mut Sim, text: &str) {
    let input = recorded(s, "agents-composer-input").expect("the composer is drawn");
    s.click(input.center());
    s.type_text(text);
}

fn selected_frame(s: &mut Sim) {
    let page = s.shell.app.page;
    let fid = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, designos_doc::Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
    s.shell.app.selection = vec![fid];
}

#[test]
fn send_in_new_chat_starts_the_pill_agent_and_never_resumes_an_old_session() {
    let mut s = new_chat_sim();
    s.shell.app.terminal.session_registry.sessions.push(old_grok_session());
    frames(&mut s, 2);
    assert!(painted(&s).iter().any(|t| t == "Ask Claude Code…  @ to mention a layer"), "the pill names Claude Code: {:?}", painted(&s));
    type_in_composer(&mut s, "draw a hero");
    s.key(Key::Enter);
    frames(&mut s, 3);
    let terminal = &s.shell.app.terminal;
    let tabs: Vec<(AgentCliPreset, bool)> = terminal.tabs.iter().map(|t| (t.spawn_preset, t.is_resumed)).collect();
    assert_eq!(tabs, vec![(AgentCliPreset::Claude, false)], "Send starts the pill's agent and resumes nothing");
    let composer = &terminal.tabs[0].composer;
    assert!(composer.pending_send.is_some() || composer.send_count > 0, "the message went to the new Claude tab: {:?}", composer.pending_send);
    assert!(terminal.session_registry.sessions.iter().any(|e| e.tab_id == 77 && e.status == SessionEntryStatus::Exited), "the old session is left alone");
    finish(s);
}

#[test]
fn new_chat_state_has_its_own_composer() {
    let mut s = session_sim();
    {
        let covered = &mut s.shell.app.terminal.tabs[0].composer;
        covered.send_count = 2;
        covered.record_sent("earlier");
        covered.draft = "half".into();
    }
    frames(&mut s, 2);
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open, "+ opens the New chat state");
    assert_eq!(s.shell.app.terminal.composer_draft(), "", "the New chat composer is empty, not the covered tab's draft");
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Ask Claude Code…  @ to mention a layer"), "New chat reads Ask, not Reply: {texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("Reply to")), "the covered tab's Reply hint is not shown: {texts:?}");

    s.type_text("new task");
    s.key(Key::Enter);
    frames(&mut s, 3);
    let tabs = &s.shell.app.terminal.tabs;
    assert_eq!(tabs.len(), 2, "Send started a second tab");
    let covered = &tabs[0].composer;
    assert_eq!(covered.send_count, 2, "the covered tab counted no send");
    assert_eq!(covered.draft, "half", "the covered tab keeps its draft");
    assert_eq!(covered.history.front().map(String::as_str), Some("earlier"), "and its history");
    assert_eq!(tabs[1].composer.history.front().map(String::as_str), Some("new task"), "the new message belongs to the new tab");
    finish(s);
}

#[test]
fn enter_with_no_startable_agent_keeps_the_draft_and_queues_nothing() {
    let mut s = no_agent_sim();
    type_in_composer(&mut s, "hello");
    s.key(Key::Enter);
    frames(&mut s, 3);
    {
        let c = s.shell.app.terminal.active_composer();
        assert_eq!(c.draft, "hello", "Enter did nothing: the draft stays");
        assert!(c.pending_send.is_none(), "and nothing is queued for a later agent: {:?}", c.pending_send);
        assert!(c.history.is_empty(), "and nothing is recorded as sent");
        assert_eq!(c.last_outcome, Some(SendOutcome::Refused(RefuseReason::NoAgent)));
    }
    assert!(s.shell.app.terminal.tabs.is_empty(), "no tab started");

    // The user installs Codex and starts it from the menu: the draft waits for Send, it is not sent by itself.
    install(&s, AgentCliPreset::Codex);
    let pill = recorded(&s, "agents-composer-agent-pill").unwrap();
    s.click(pill.center());
    frames(&mut s, 3);
    let row = recorded(&s, "agents-dropdown-codex").expect("the Codex row is offered");
    s.click(row.center());
    frames(&mut s, 3);
    let tabs = &s.shell.app.terminal.tabs;
    assert_eq!(tabs.iter().map(|t| t.spawn_preset).collect::<Vec<_>>(), vec![AgentCliPreset::Codex]);
    assert!(tabs[0].composer.pending_send.is_none(), "nothing was sent to Codex: {:?}", tabs[0].composer.pending_send);
    assert_eq!(tabs[0].composer.draft, "hello", "the draft is waiting in the new tab's composer");
    finish(s);
}

#[test]
fn a_disabled_send_changes_nothing_when_pressed() {
    let mut s = no_agent_sim();
    type_in_composer(&mut s, "hello");
    frames(&mut s, 2);
    let send = recorded(&s, "agents-composer-send").expect("Send is drawn");
    s.click(send.center());
    frames(&mut s, 2);
    let c = s.shell.app.terminal.active_composer();
    assert_eq!(c.last_outcome, None, "a disabled Send is not a refused send: the press did nothing");
    assert_eq!((c.draft.as_str(), c.send_count), ("hello", 0));
    finish(s);
}

#[test]
fn send_is_enabled_only_when_something_can_be_sent() {
    let mut s = session_sim();
    selected_frame(&mut s);
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    frames(&mut s, 3);
    assert!(recorded(&s, "agent-selection-chip").is_some(), "the selection chip is drawn");
    let send = recorded(&s, "agents-composer-send").unwrap();
    s.click(send.center());
    frames(&mut s, 2);
    let c = s.shell.app.terminal.active_composer();
    assert_eq!(c.last_outcome, None, "the selection chip alone is not a message: Send is disabled and the press does nothing");
    assert_eq!(c.send_count, 0);

    type_in_composer(&mut s, "make it blue");
    frames(&mut s, 2);
    let send = recorded(&s, "agents-composer-send").unwrap();
    s.click(send.center());
    frames(&mut s, 2);
    assert_eq!(s.shell.app.terminal.tabs[0].composer.send_count, 1, "with text Send sends");
    finish(s);
}

#[test]
fn an_attached_prd_alone_sends_the_prd_reference() {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    s.shell.app.terminal.active_composer_mut().attach_prd = true;
    frames(&mut s, 3);
    let before = s.shell.app.terminal.tabs[0].written_bytes.len();
    let send = recorded(&s, "agents-composer-send").unwrap();
    s.click(send.center());
    frames(&mut s, 3);
    let tab = &s.shell.app.terminal.tabs[0];
    let written = String::from_utf8_lossy(&tab.written_bytes[before..]).into_owned();
    assert!(written.contains("@prd"), "the PRD reference was sent: {written:?}");
    assert!(!written.contains("@prd "), "with no trailing space: {written:?}");
    assert!(!tab.composer.attach_prd, "the chip is spent");
    assert!(tab.composer.history.is_empty(), "an attachment-only message adds no empty history entry: {:?}", tab.composer.history);
    finish(s);
}

#[test]
fn the_pill_and_placeholder_follow_a_cli_installed_after_the_menu_closed() {
    let mut s = no_agent_sim();
    assert!(painted(&s).iter().any(|t| t == "No agent CLI found. Open the agent menu to install one."), "{:?}", painted(&s));
    // Open and close the menu once, so a cache that only the menu refreshes cannot explain the result.
    let pill = recorded(&s, "agents-composer-agent-pill").unwrap();
    s.click(pill.center());
    frames(&mut s, 2);
    s.key(Key::Escape);
    frames(&mut s, 3);
    assert!(!s.shell.app.terminal.runtime_dropdown_open, "the menu is closed");

    install(&s, AgentCliPreset::Codex);
    frames(&mut s, 3);
    let texts = painted(&s);
    assert!(texts.iter().any(|t| t == "Codex"), "the pill names the installed agent: {texts:?}");
    assert!(texts.iter().any(|t| t == "Ask Codex…  @ to mention a layer"), "and so does the placeholder: {texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("No agent CLI found")), "the no-CLI line is gone: {texts:?}");
    assert!(!texts.iter().any(|t| t == "Choose agent"), "{texts:?}");
    finish(s);
}

#[test]
fn a_covered_tabs_still_running_notice_stays_out_of_new_chat() {
    let mut s = session_sim();
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    let box_before = recorded(&s, "agents-composer-box").expect("the New chat composer box").height();
    let reserved_before = crate::agent_composer::composer_reserved_height(&s.shell.app.terminal);
    s.shell.app.terminal.tabs[0].still_running_notice = Some(crate::agent_stop::STILL_RUNNING_NOTICE.to_string());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open, "New chat is still the state on screen");
    assert!(!painted(&s).iter().any(|t| t.starts_with("Still running")), "the covered tab's notice is not painted in New chat: {:?}", painted(&s));
    assert_eq!(recorded(&s, "agents-composer-box").unwrap().height(), box_before, "and it does not size the New chat box");
    assert_eq!(crate::agent_composer::composer_reserved_height(&s.shell.app.terminal), reserved_before, "nor the room the panel reserves for it");

    let tab = recorded(&s, "agents-left-tab-select").expect("the covered session tab");
    s.click(tab.center());
    frames(&mut s, 3);
    assert!(!s.shell.app.terminal.picker_open);
    assert!(painted(&s).iter().any(|t| t.starts_with("Still running")), "back on its tab the notice shows: {:?}", painted(&s));
    finish(s);
}

#[test]
fn a_prompt_entry_point_leaves_the_composer_on_screen_alone() {
    // New chat over a live tab: Make design sends its own text to the tab, and the New chat draft survives.
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    s.shell.app.terminal.tabs[0].composer.draft = "covered draft".into();
    frames(&mut s, 2);
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    type_in_composer(&mut s, "my new chat draft");
    frames(&mut s, 2);
    s.shell.app.terminal.dispatch_figma_ai_prompt("Make Design", "pricing card", None);
    frames(&mut s, 3);
    let terminal = &s.shell.app.terminal;
    assert_eq!(terminal.unattached_composer.draft, "my new chat draft", "the New chat draft is not taken");
    assert!(terminal.unattached_composer.history.is_empty(), "and nothing is recorded in its history: {:?}", terminal.unattached_composer.history);
    assert_eq!(terminal.tabs[0].composer.draft, "covered draft", "the covered tab's draft is not taken either");
    assert!(
        terminal.tabs[0].composer.pending_send.as_deref().is_some_and(|p| p.contains("pricing card")) || terminal.tabs[0].composer.send_count > 0,
        "the prompt went to the covered tab: {:?}",
        terminal.tabs[0].composer.pending_send
    );
    finish(s);

    // A live tab on screen: the prompt does not clear what the user typed there, chips and PRD included.
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    selected_frame(&mut s);
    {
        let composer = &mut s.shell.app.terminal.tabs[0].composer;
        composer.draft = "half a thought".into();
        composer.attach_prd = true;
    }
    frames(&mut s, 3);
    let outcome = s.shell.app.terminal.send_to_agent("pricing card", None);
    assert!(matches!(outcome, SendOutcome::Written | SendOutcome::Queued), "the prompt is delivered: {outcome:?}");
    frames(&mut s, 2);
    let composer = &s.shell.app.terminal.tabs[0].composer;
    assert_eq!(composer.draft, "half a thought", "the draft is still there");
    assert!(composer.attach_prd, "and the PRD chip");
    let written = String::from_utf8_lossy(&s.shell.app.terminal.tabs[0].written_bytes).into_owned();
    assert!(!written.contains("@prd"), "the prompt carried no PRD tag of the composer: {written:?}");
    finish(s);
}

/// The prompt `specs.rs` builds for "Ask @agent about this Spec" and hands to `submit_chat_prompt`.
const SPEC_ASK: &str = "@prd @spec://pricing/prd (C-1) implement the pricing card";

fn layer_chip() -> crate::agent_composer::LayerChip {
    crate::agent_composer::LayerChip { node_id: designos_doc::NodeId(42), name: "Hero".into(), is_component: false, reference: "designos:node/42".into() }
}

#[test]
fn spec_ask_on_a_live_tab_sends_its_own_text_and_keeps_the_draft_the_chips_and_the_prd_chip() {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    {
        let composer = &mut s.shell.app.terminal.tabs[0].composer;
        composer.draft = "half a thought".into();
        composer.attach_prd = true;
        composer.chips.push(layer_chip());
    }
    frames(&mut s, 3);
    s.shell.app.terminal.submit_chat_prompt(SPEC_ASK, None);
    s.shell.app.terminal.flush_pending_enter_now();
    frames(&mut s, 2);
    let terminal = &s.shell.app.terminal;
    let composer = &terminal.tabs[0].composer;
    let written = String::from_utf8_lossy(&terminal.tabs[0].written_bytes).into_owned();
    assert!(written.contains("@spec://pricing/prd"), "the Ask reached the live session: {written:?}");
    assert_eq!(composer.draft, "half a thought", "the user's draft is still on screen, not deleted unsent");
    assert!(composer.attach_prd, "the PRD chip is still attached");
    assert_eq!(composer.chips.len(), 1, "and the layer chip");
    assert!(!written.contains("half a thought"), "the draft was not sent with the Ask: {written:?}");
    assert!(!written.contains("designos:node/42"), "the chip's References footer does not ride on the Ask: {written:?}");
    finish(s);
}

#[test]
fn spec_ask_over_new_chat_leaves_the_new_chat_draft_and_prd_chip_alone() {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    type_in_composer(&mut s, "my new chat draft");
    frames(&mut s, 2);
    s.shell.app.terminal.unattached_composer.attach_prd = true;
    assert!(s.shell.app.terminal.picker_open, "precondition: New chat is on screen");
    s.shell.app.terminal.submit_chat_prompt(SPEC_ASK, None);
    frames(&mut s, 3);
    let terminal = &s.shell.app.terminal;
    assert_eq!(terminal.unattached_composer.draft, "my new chat draft", "the New chat draft is not taken");
    assert!(terminal.unattached_composer.attach_prd, "nor its PRD chip");
    assert!(terminal.unattached_composer.history.is_empty(), "and nothing is recorded in its history: {:?}", terminal.unattached_composer.history);
    assert!(
        terminal.tabs[0].composer.pending_send.as_deref().is_some_and(|p| p.contains("@spec://pricing/prd")) || terminal.tabs[0].composer.send_count > 0,
        "the Ask went to the covered tab: {:?}",
        terminal.tabs[0].composer.pending_send
    );
    finish(s);
}

#[test]
fn the_next_new_chat_after_a_send_starts_empty() {
    let mut s = session_sim();
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    frames(&mut s, 2);
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    type_in_composer(&mut s, "second chat");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert_eq!(s.shell.app.terminal.tabs.len(), 2, "Send started the second chat");
    let plus = recorded(&s, "agents-new-chat-btn").unwrap();
    s.click(plus.center());
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker_open, "+ opens New chat again");
    assert_eq!(s.shell.app.terminal.composer_draft(), "", "the text moved with the new tab: the next New chat is empty");
    assert!(s.shell.app.terminal.unattached_composer.history.is_empty(), "and holds none of the sent text in its history");
    finish(s);
}

#[test]
fn the_pill_repaints_on_its_own_while_probes_run_and_names_the_agent_when_they_end() {
    use crate::agent_cli_catalog::{CatalogProbeCache, CommandOutput, RunCommand, PROBE_TIMEOUT};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Holds every probe until the test opens the gate, then answers as an installed, signed-in Claude Code.
    struct GatedRunner {
        open: Arc<AtomicBool>,
    }
    impl RunCommand for GatedRunner {
        fn is_installed(&self, binary: &str, _install_folders: &[std::path::PathBuf]) -> bool {
            let give_up = Instant::now() + Duration::from_secs(10);
            while !self.open.load(Ordering::SeqCst) && Instant::now() < give_up {
                std::thread::sleep(Duration::from_millis(5));
            }
            binary == "claude"
        }
        fn run(&self, argv: &[&str], _install_folders: &[std::path::PathBuf], _timeout: Duration) -> CommandOutput {
            match argv {
                ["claude", "--version"] => CommandOutput::ok("2.1.302 (Claude Code)\n"),
                ["claude", "auth", "status"] => CommandOutput::ok(r#"{"loggedIn": true}"#),
                _ => CommandOutput::exited(1, "", ""),
            }
        }
    }

    let gate = Arc::new(AtomicBool::new(false));
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.session_registry.last_cli = None;
    s.shell.app.terminal.picker.cache = CatalogProbeCache::new(Arc::new(GatedRunner { open: gate.clone() }), PROBE_TIMEOUT);
    // egui keeps the `file:line` of every repaint request of a pass, whatever its delay.
    let composer_asked = |s: &Sim| s.ctx.repaint_causes().iter().any(|cause| cause.to_string().contains("agent_composer.rs"));
    s.cmd_key(Key::Backtick);
    frames(&mut s, 3);
    assert!(s.shell.app.terminal.picker.cache.is_probing(), "the probes are held");
    assert!(composer_asked(&s), "while probes run, the composer asks for a repaint soon: {:?}", s.ctx.repaint_causes());
    assert!(!painted(&s).iter().any(|t| t == "Claude Code"), "nothing is known yet: {:?}", painted(&s));

    gate.store(true, Ordering::SeqCst);
    let give_up = Instant::now() + Duration::from_secs(10);
    while s.shell.app.terminal.picker.cache.is_probing() && Instant::now() < give_up {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!s.shell.app.terminal.picker.cache.is_probing(), "the probes finished");
    s.frame(vec![]);
    assert!(painted(&s).iter().any(|t| t == "Claude Code"), "the next frame, with no input, names the agent: {:?}", painted(&s));
    frames(&mut s, 2);
    assert!(!composer_asked(&s), "and the poll stops once they are done: {:?}", s.ctx.repaint_causes());
    finish(s);
}
