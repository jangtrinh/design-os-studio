//! Codex approvals in the Agents panel (#930): each tab's link to `codex app-server`, the cards it raised, and the
//! card as the panel paints it.
//!
//! Nothing here writes to a PTY. A card is answered only as the JSON-RPC response to its request id, through the tab's
//! [`CodexLink`](crate::codex_remote::CodexLink); a tab without a link keeps the #1195 banner.

use crate::agent_approval_card::{card_button_width, paint_card_button, CardAction};
use crate::codex_remote::card_mapping::CodexCard;
use crate::codex_remote::protocol::CardKind;
pub use crate::codex_remote::protocol::Decision;
use crate::codex_remote::remote::{CodexRemote, StartRequest};
use crate::codex_remote::rpc_client::Timing;
use crate::codex_remote::{gate, launch, shell_env};
use crate::codex_remote::{CodexEvent, CodexLink, FallbackReason, UnavailableReason};
use crate::terminal::{STATUS_BAD_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT};
use crate::theme;
use crate::widgets;
use egui::text::LayoutJob;
use egui::{Align2, Id, Pos2, Rect, Stroke, StrokeKind, TextFormat, Ui, Vec2};
use portable_pty::CommandBuilder;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// The panel's context, so a link event wakes an idle window (the client thread calls it after every event).
static REPAINT: OnceLock<egui::Context> = OnceLock::new();

/// Remembers the context to wake. The first one wins: the app has one window.
pub fn register_repaint(ctx: &egui::Context) {
    let _ = REPAINT.set(ctx.clone());
}

/// The `Wake` handed to a tab's client.
pub fn wake() -> crate::codex_remote::rpc_client::Wake {
    std::sync::Arc::new(|| {
        if let Some(ctx) = REPAINT.get() {
            ctx.request_repaint();
        }
    })
}

/// Arrival order across tabs: the card that came first is shown first.
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// A card waiting for a click, with its place in the queue.
#[derive(Clone, Debug)]
pub struct PendingCard {
    pub seq: u64,
    pub card: CodexCard,
}

/// One Codex tab's side of the card path.
#[derive(Default)]
pub struct CodexTab {
    link: Option<CodexLink>,
    /// Why the tab runs without cards (`None` while it has a link, or for a tab that never asked for one).
    pub fallback: Option<FallbackReason>,
    /// The server went away, or never answered, with the tab on the card path.
    unavailable: Option<UnavailableReason>,
    pub cards: Vec<PendingCard>,
    /// Requests that wait for the terminal's own menu: no buttons, no reply.
    pub waits: Vec<String>,
    /// The link dropped while a request was pending: its card is void and the request waits in the terminal.
    pub link_lost: bool,
}

impl CodexTab {
    /// A tab with a live link.
    pub fn linked(link: CodexLink) -> Self {
        CodexTab { link: Some(link), ..Self::default() }
    }

    /// A tab that runs as before, for `why`.
    pub fn fell_back(why: FallbackReason) -> Self {
        CodexTab { fallback: Some(why), ..Self::default() }
    }

    /// A tab that shows these cards with no server behind them: the `--panel agents-codex-*` previews.
    pub fn preview(cards: Vec<CodexCard>, link_lost: bool, fallback: Option<FallbackReason>) -> Self {
        let cards = cards.into_iter().map(|card| PendingCard { seq: NEXT_SEQ.fetch_add(1, Ordering::Relaxed), card }).collect();
        CodexTab { cards, link_lost, fallback, ..Self::default() }
    }

    pub fn has_link(&self) -> bool {
        self.link.is_some()
    }

    /// A card with buttons, a request that waits in the terminal, or a lost link waits on the user.
    pub fn is_waiting(&self) -> bool {
        !self.cards.is_empty() || !self.waits.is_empty() || self.link_lost
    }

    /// Takes the events the client queued since the last call. Returns whether anything the panel shows changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(event) = self.link.as_ref().and_then(CodexLink::next_event) {
            changed = true;
            match event {
                CodexEvent::Card(card) => {
                    if !self.cards.iter().any(|pending| pending.card.key == card.key) {
                        self.cards.push(PendingCard { seq: NEXT_SEQ.fetch_add(1, Ordering::Relaxed), card });
                    }
                }
                CodexEvent::OpenTabWait { key, .. } => {
                    if !self.waits.contains(&key) {
                        self.waits.push(key);
                    }
                }
                CodexEvent::Resolved { key } => {
                    self.cards.retain(|pending| pending.card.key != key);
                    self.waits.retain(|wait| *wait != key);
                    if !self.is_waiting() {
                        self.link_lost = false;
                    }
                }
                CodexEvent::Unavailable(why) => {
                    self.link = None;
                    self.unavailable = Some(why);
                }
                CodexEvent::Disconnected => {
                    // The cards are void, and the requests behind them still wait in the terminal.
                    self.link_lost = self.is_waiting();
                    self.cards.clear();
                    self.waits.clear();
                    self.link = None;
                }
                CodexEvent::Originator(_) | CodexEvent::ThreadReady | CodexEvent::Joined { .. } | CodexEvent::Waiting { .. } => {
                    changed = false;
                }
            }
        }
        changed
    }

    /// The one reply to a click: the JSON-RPC response to the request, and the card is gone. A key that is not a card
    /// of this tab does nothing.
    pub fn answer(&mut self, key: &str, decision: Decision) -> bool {
        let Some(at) = self.cards.iter().position(|pending| pending.card.key == key) else { return false };
        self.cards.remove(at);
        if let Some(link) = &self.link {
            link.answer(key, decision);
        }
        true
    }

    /// The tab ended or its status left the wait: nothing it showed is valid any more. The link ends with it.
    pub fn clear(&mut self) {
        self.cards.clear();
        self.waits.clear();
        self.link_lost = false;
        self.link = None;
    }

    /// The caption of the #1195 banner for a tab that is not on the card path (the reason, when it is one the user can act on).
    pub fn banner_caption(&self) -> Option<String> {
        match (&self.fallback, &self.unavailable) {
            (Some(FallbackReason::VersionOutsideRange(version)), _) => Some(format!("Codex {version} \u{b7} structured approvals need 0.164.x")),
            (Some(FallbackReason::PlatformUnsupported), _) => Some("Structured approvals need macOS".to_string()),
            (_, Some(UnavailableReason::ServerVersion(version))) => Some(format!("Codex {version} \u{b7} structured approvals need 0.164.x")),
            _ => None,
        }
    }
}

/// What a tab asks of [`start_for_tab`].
pub struct StartParams<'a> {
    pub tab_id: usize,
    pub session_id: &'a str,
    pub session_dir: &'a std::path::Path,
    /// Today's launch command of the tab, when it is a new Ask-mode Codex session that may take the card path.
    pub card_command: Option<&'a str>,
    /// The card path runs on macOS only.
    pub platform_supported: bool,
}

/// Decides how a Codex tab starts, once its PTY builder is complete. With the card path, the builder is changed to start
/// the TUI as `codex --remote` and the returned tab holds the link. Any other case leaves the builder as it is, so the tab
/// starts exactly as before, and the returned tab says why.
pub fn start_for_tab(cmd: &mut CommandBuilder, params: &StartParams) -> CodexTab {
    // A value Studio inherited must never steer a tab that is not on the card path.
    cmd.env_remove(launch::ENV_CODEX_BIN);
    cmd.env_remove(launch::ENV_CODEX_SOCKET);
    let Some(fallback) = params.card_command else { return CodexTab::default() };
    if !params.platform_supported {
        return CodexTab::fell_back(FallbackReason::PlatformUnsupported);
    }
    let shell = shell_env::tab_shell(cmd);
    let tab_env = match shell_env::lookup(params.session_id, cmd, &shell) {
        shell_env::EnvLookup::Pending => return CodexTab::fell_back(FallbackReason::VersionUnknown),
        shell_env::EnvLookup::Ready(env) => env.overlaid_with(cmd),
    };
    let Some(codex_bin) = tab_env.path().and_then(gate::resolve_in_path) else { return CodexTab::fell_back(FallbackReason::VersionUnknown) };
    let channel_dir = crate::agent_approval_channel::prepare_channel_dir(params.session_dir).ok();
    let started = CodexRemote::start(StartRequest {
        codex_bin: &codex_bin,
        channel_dir: channel_dir.as_deref(),
        tab_id: params.tab_id,
        pty: cmd,
        env: &tab_env,
        wake: wake(),
        timing: Timing::PRODUCTION,
    });
    match started {
        Ok(remote) => {
            cmd.env(launch::ENV_CODEX_BIN, &codex_bin);
            cmd.env(launch::ENV_CODEX_SOCKET, &remote.rendezvous);
            if let Some(last) = cmd.get_argv_mut().last_mut() {
                *last = launch::tui_launch_command(fallback).into();
            }
            CodexTab::linked(remote.link)
        }
        Err(why) => CodexTab::fell_back(why),
    }
}

/// The session ended: its captured environment is not needed any more.
pub fn forget(session_id: &str) {
    shell_env::forget(session_id);
}

/// Starts the version probe and the environment capture for a session ahead of its first Codex tab, so that tab finds
/// both answered. `cmd` is any builder with the session's environment.
pub fn warm(cmd: &CommandBuilder, session_id: &str) {
    let shell = shell_env::tab_shell(cmd);
    if let shell_env::EnvLookup::Ready(env) = shell_env::lookup(session_id, cmd, &shell) {
        if let Some(bin) = env.path().and_then(gate::resolve_in_path) {
            gate::warm(&bin, &env.overlaid_with(cmd));
        }
    }
}

/// What the Agents panel shows for Codex's own approvals.
pub enum CodexView {
    /// A request with Approve and Deny.
    Card { tab_id: usize, tab_title: Option<String>, pending: Box<PendingCard>, total: usize },
    /// The link dropped while a request waits.
    LinkLost { tab_id: usize, in_active_terminal: bool },
}

const BODY_PAD: f32 = theme::INSET_BUTTON;

/// A laid-out line of the card body.
struct Body {
    galley: std::sync::Arc<egui::Galley>,
}

fn body_job(text: String, color: egui::Color32, width: f32, mono: bool) -> LayoutJob {
    let font = if mono { theme::font_mono() } else { theme::font_caption() };
    let line_height = if mono { theme::LINE_BODY } else { theme::LINE_CAPTION };
    let mut job = LayoutJob::simple(text, font.clone(), color, width);
    for section in &mut job.sections {
        section.format = TextFormat { font_id: font.clone(), color, line_height: Some(line_height), ..Default::default() };
    }
    // Break inside a word when it does not fit, so a long path or command is wrapped and never cut.
    job.wrap.break_anywhere = true;
    job
}

fn lay_out(ui: &Ui, job: LayoutJob) -> Body {
    Body { galley: ui.fonts_mut(|f| f.layout_job(job)) }
}

/// The lines of a file-change card: `verb path  +12 \u{2212}3`.
fn file_lines(card: &CodexCard) -> String {
    card.detail
        .files
        .iter()
        .map(|file| match file.counts {
            Some((added, removed)) => {
                let mut counts = String::new();
                if added > 0 {
                    counts.push_str(&format!(" +{added}"));
                }
                if removed > 0 {
                    counts.push_str(&format!(" \u{2212}{removed}"));
                }
                format!("{} {}{counts}", file.verb, file.path)
            }
            None => format!("{} {}", file.verb, file.path),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body text of a card: the full command, the changed paths, or the tool's input.
fn body_text(card: &CodexCard) -> String {
    match card.kind {
        CardKind::Command => card.summary.clone(),
        CardKind::FileChange => file_lines(card),
        CardKind::McpTool => card.detail.input.clone().unwrap_or_default(),
    }
}

/// Paints the Codex card pinned to the top of `viewport_rect`. Returns the click, if any, and the height the card
/// took, margins included, so the caller can lay the rest of the panel below it.
pub fn paint_codex_card(ui: &mut Ui, viewport_rect: Rect, view: &CodexView, surface_id: &'static str) -> (Option<CardAction>, f32) {
    let outer = theme::SPACE_ROW;
    let card_w = (viewport_rect.width() - outer - outer).max(theme::CONTROL_ROW);
    let inner_w = (card_w - theme::INSET_FRAME - theme::INSET_FRAME).max(theme::CONTROL_ROW);
    let body_w = (inner_w - BODY_PAD - BODY_PAD).max(theme::CONTROL_DENSE);

    let (title, title_color, body, cwd, queue) = match view {
        CodexView::Card { tab_title, pending, total, .. } => {
            let card = &pending.card;
            let title = match tab_title {
                Some(tab) => format!("{} ({tab})", card.title),
                None => card.title.clone(),
            };
            let text = body_text(card);
            let body = (!text.is_empty()).then(|| lay_out(ui, body_job(text, theme::TEXT, body_w, true)));
            let cwd = card.detail.cwd.as_ref().map(|cwd| lay_out(ui, body_job(format!("in {cwd}"), theme::TEXT_SECONDARY, inner_w, false)));
            (title, theme::TEXT, body, cwd, *total)
        }
        CodexView::LinkLost { .. } => {
            let text = "Answer in its terminal. The request is still waiting there.".to_string();
            ("Lost the link to Codex approvals".to_string(), STATUS_BAD_TEXT, Some(lay_out(ui, body_job(text, theme::TEXT, body_w, true))), None, 0)
        }
    };

    let body_h = body.as_ref().map_or(0.0, |b| b.galley.size().y + theme::INSET_CONTROL + theme::INSET_CONTROL);
    let mut card_h = theme::INSET_FRAME + theme::CONTROL_DENSE + theme::SPACE_ROW;
    if body.is_some() {
        card_h += body_h + theme::SPACE_ROW;
    }
    if let Some(cwd) = &cwd {
        card_h += cwd.galley.size().y + theme::SPACE_ROW;
    }
    card_h += theme::CONTROL_STANDARD + theme::INSET_FRAME;

    let card_rect = Rect::from_min_size(Pos2::new(viewport_rect.left() + outer, viewport_rect.top() + outer), Vec2::new(card_w, card_h));
    widgets::record(ui, "agent-approval-card", card_rect);
    let p = ui.painter().clone();
    p.rect_filled(card_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_TERTIARY);
    p.rect_stroke(card_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);

    let inner = card_rect.shrink(theme::INSET_FRAME);
    let header_r = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::CONTROL_DENSE));
    let badge_font = theme::font_caption_medium();
    let badge_w = widgets::text_width(&p, "CODEX", &badge_font) + theme::INSET_FRAME + theme::INSET_FRAME;
    let badge_r = Rect::from_min_size(header_r.min, Vec2::new(badge_w, theme::CONTROL_DENSE));
    p.rect_filled(badge_r, theme::cr_pill(theme::CONTROL_DENSE), STATUS_WARN_BG);
    p.rect_stroke(badge_r, theme::cr_pill(theme::CONTROL_DENSE), Stroke::new(theme::BORDER_WIDTH, STATUS_WARN_BORDER), StrokeKind::Inside);
    p.text(badge_r.center(), Align2::CENTER_CENTER, "CODEX", badge_font, STATUS_WARN_TEXT);

    let count_font = theme::font_caption();
    let count_reserve = if queue > 1 {
        let text = format!("1 of {queue}");
        let width = widgets::text_width(&p, &text, &count_font) + theme::SPACE_INLINE;
        p.text(Pos2::new(header_r.right(), header_r.center().y), Align2::RIGHT_CENTER, text, count_font, theme::TEXT_SECONDARY);
        width
    } else {
        0.0
    };
    let title_font = theme::font_body_medium();
    let title_x = badge_r.right() + theme::SPACE_INLINE;
    let title_w = (header_r.right() - count_reserve - title_x).max(theme::CONTROL_DENSE);
    let shown = widgets::elide(&p, &title, &title_font, title_w);
    p.text(Pos2::new(title_x, header_r.center().y), Align2::LEFT_CENTER, shown, title_font, title_color);

    let mut y = header_r.bottom() + theme::SPACE_ROW;
    if let Some(body) = &body {
        let rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), body_h));
        p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        p.galley(Pos2::new(rect.left() + BODY_PAD, rect.top() + theme::INSET_CONTROL), body.galley.clone(), theme::TEXT);
        widgets::record(ui, "agent-approval-summary", rect);
        y = rect.bottom() + theme::SPACE_ROW;
    }
    if let Some(cwd) = &cwd {
        p.galley(Pos2::new(inner.left(), y), cwd.galley.clone(), theme::TEXT_SECONDARY);
        widgets::record(ui, "agent-approval-cwd", Rect::from_min_size(Pos2::new(inner.left(), y), cwd.galley.size()));
        y += cwd.galley.size().y + theme::SPACE_ROW;
    }

    let action_top = y;
    let mut out = None;
    match view {
        CodexView::Card { tab_id, pending, .. } => {
            let key = pending.card.key.clone();
            let title = pending.card.title.clone();
            let approve_w = card_button_width(&p, "Approve");
            // Deny is made first, so Tab reaches it first, and it holds the default focus.
            let (_, deny) = paint_card_button(
                ui,
                &p,
                (surface_id, "agent-approval-deny"),
                "agent-approval-deny",
                inner.right() - approve_w - theme::SPACE_ROW,
                action_top,
                "Deny",
                false,
            );
            let (_, approve) =
                paint_card_button(ui, &p, (surface_id, "agent-approval-approve"), "agent-approval-approve", inner.right(), action_top, "Approve", true);
            widgets::labelled(ui, deny.id, egui::accesskit::Role::Button, &format!("Deny Codex request: {title}"));
            widgets::labelled(ui, approve.id, egui::accesskit::Role::Button, &format!("Approve Codex request: {title}"));
            give_default_focus(ui, &key, deny.id);
            if deny.clicked() {
                out = Some(CardAction::CodexDeny { tab_id: *tab_id, key });
            } else if approve.clicked() {
                out = Some(CardAction::CodexApprove { tab_id: *tab_id, key });
            }
        }
        CodexView::LinkLost { tab_id, in_active_terminal } => {
            let (label, name) = if *in_active_terminal { ("Show prompt", "agent-approval-show-prompt") } else { ("Open tab", "agent-approval-open-tab") };
            let (_, resp) = paint_card_button(ui, &p, (surface_id, name), name, inner.right(), action_top, label, false);
            let what = if *in_active_terminal { "Show the Codex prompt" } else { "Open the Codex tab waiting for approval" };
            widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, what);
            if resp.clicked() {
                out = Some(if *in_active_terminal { CardAction::ShowPrompt } else { CardAction::OpenTab(*tab_id) });
            }
        }
    }
    (out, card_h + outer + outer)
}

/// Deny takes the keyboard focus once, when a card first shows. Enter on a card that just appeared therefore never approves.
fn give_default_focus(ui: &Ui, key: &str, deny: Id) {
    let marker = Id::new(("codex-card-default-focus", key.to_string()));
    let given = ui.ctx().data(|d| d.get_temp::<bool>(marker)).unwrap_or(false);
    if !given {
        ui.ctx().memory_mut(|m| m.request_focus(deny));
        ui.ctx().data_mut(|d| d.insert_temp(marker, true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn builder() -> CommandBuilder {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg("codex");
        cmd.env(launch::ENV_CODEX_BIN, "/inherited/codex");
        cmd.env(launch::ENV_CODEX_SOCKET, "/inherited/socket");
        cmd
    }

    fn params(card_command: Option<&str>, platform_supported: bool) -> StartParams<'_> {
        StartParams { tab_id: 3, session_id: "930-start", session_dir: Path::new("/tmp/930-start"), card_command, platform_supported }
    }

    /// AC-6: a platform without the card path starts the tab as before, with the reason, and no inherited variable steers it.
    #[test]
    fn a_platform_without_the_card_path_starts_the_tab_as_before() {
        let mut cmd = builder();
        let before = cmd.get_argv().clone();
        let tab = start_for_tab(&mut cmd, &params(Some("codex --some-flag"), false));
        assert_eq!(tab.fallback, Some(FallbackReason::PlatformUnsupported));
        assert!(!tab.has_link() && !tab.is_waiting());
        assert_eq!(cmd.get_argv(), &before, "the launch command is today's, byte for byte");
        assert!(cmd.get_env(launch::ENV_CODEX_BIN).is_none() && cmd.get_env(launch::ENV_CODEX_SOCKET).is_none());
    }

    #[test]
    fn a_tab_that_did_not_ask_for_cards_has_no_state_and_no_inherited_wrapper_variables() {
        let mut cmd = builder();
        let tab = start_for_tab(&mut cmd, &params(None, true));
        assert!(tab.fallback.is_none() && !tab.has_link());
        assert!(cmd.get_env(launch::ENV_CODEX_BIN).is_none() && cmd.get_env(launch::ENV_CODEX_SOCKET).is_none());
    }

    #[test]
    fn the_banner_names_the_reason_only_when_the_user_can_act_on_it() {
        let caption = |why| CodexTab::fell_back(why).banner_caption();
        assert_eq!(caption(FallbackReason::VersionOutsideRange("0.162.1".into())).as_deref(), Some("Codex 0.162.1 \u{b7} structured approvals need 0.164.x"));
        assert_eq!(caption(FallbackReason::PlatformUnsupported).as_deref(), Some("Structured approvals need macOS"));
        assert_eq!(caption(FallbackReason::VersionUnknown), None);
        assert_eq!(caption(FallbackReason::SocketInUse), None);
    }

    #[test]
    fn a_click_on_a_card_that_is_gone_sends_nothing() {
        let (link, _events, sent) = CodexLink::for_test();
        let mut tab = CodexTab::linked(link);
        assert!(!tab.answer("cx-1-9", Decision::Approve));
        assert!(sent.try_recv().is_err());
    }

    /// AC-7: no code path of the card path writes to a PTY. The answer is a JSON-RPC response on the app-server socket.
    #[test]
    fn the_card_code_never_touches_a_pty() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![src.join("agent_codex_cards.rs")];
        for entry in std::fs::read_dir(src.join("codex_remote")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        assert!(files.len() > 10, "the scan reaches the module: {files:?}");
        let forbidden = ["write_bytes", "send_line", "take_writer", "MasterPty", "TerminalTab", "bracketed_paste", "raw_log"];
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            // The test module of this very file names the words it looks for.
            let code = text.split("#[cfg(test)]").next().unwrap();
            for word in forbidden {
                let in_code = code.lines().filter(|line| !line.trim_start().starts_with("//")).any(|line| line.contains(word));
                assert!(!in_code, "{} mentions {word}", file.display());
            }
        }
    }
}
