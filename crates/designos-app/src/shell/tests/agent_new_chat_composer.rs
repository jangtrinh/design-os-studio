//! The New chat state owns its composer and starts its own agent. A Send there never resumes an old session, never
//! writes into the tab it covers, and never queues a draft while no agent can start. Send is drawn enabled only when
//! something can be sent, and the agent pill follows a CLI that is installed after the menu closed.

use super::agent_simplified_panel::{finish, frames, new_chat_sim, painted, recorded, session_sim, settle_probes};
use super::*;
use crate::agent_cli_catalog::{CliProbe, SignIn};
use crate::agent_composer::{RefuseReason, SendOutcome};
use crate::agent_sessions::{SessionEntry, SessionEntryStatus};
use crate::terminal::AgentCliPreset;

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
