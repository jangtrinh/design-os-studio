//! The Codex card path on a platform without Unix sockets (`#930`): there is no link, so every Codex tab keeps the #1195
//! banner. The names match `agent_codex_cards.rs`, which the terminal code calls.

use crate::agent_approval_card::CardAction;
use egui::{Rect, Ui};
use portable_pty::CommandBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Deny,
}

#[derive(Clone, Debug)]
pub struct StubCard {
    pub key: String,
}

#[derive(Clone, Debug)]
pub struct PendingCard {
    pub seq: u64,
    pub card: StubCard,
}

#[derive(Default)]
pub struct CodexTab {
    pub cards: Vec<PendingCard>,
    pub link_lost: bool,
}

impl CodexTab {
    pub fn preview(_cards: Vec<StubCard>, link_lost: bool, _fallback: Option<()>) -> Self {
        CodexTab { link_lost, ..Self::default() }
    }
    pub fn is_waiting(&self) -> bool {
        false
    }
    pub fn poll(&mut self) -> bool {
        false
    }
    pub fn answer(&mut self, _key: &str, _decision: Decision) -> bool {
        false
    }
    pub fn clear(&mut self) {}
    pub fn banner_caption(&self) -> Option<String> {
        None
    }
}

pub enum CodexView {
    Card { tab_id: usize, tab_title: Option<String>, pending: Box<PendingCard>, total: usize },
    LinkLost { tab_id: usize, in_active_terminal: bool },
}

pub struct StartParams<'a> {
    pub tab_id: usize,
    pub session_id: &'a str,
    pub session_dir: &'a std::path::Path,
    pub card_command: Option<&'a str>,
    pub platform_supported: bool,
}

pub fn start_for_tab(_cmd: &mut CommandBuilder, _params: &StartParams) -> CodexTab {
    CodexTab::default()
}

pub fn paint_codex_card(_ui: &mut Ui, _viewport_rect: Rect, _view: &CodexView, _surface_id: &'static str) -> (Option<CardAction>, f32) {
    (None, 0.0)
}

pub fn warm(_cmd: &CommandBuilder, _session_id: &str) {}
pub fn forget(_session_id: &str) {}
pub fn register_repaint(_ctx: &egui::Context) {}
