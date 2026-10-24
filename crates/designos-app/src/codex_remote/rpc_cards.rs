//! The cards of one tab (#930): which server requests got buttons, which are done, and what a click writes.
//!
//! A request without buttons is remembered (a replay is ignored) and reported, but never answered: Studio replies
//! only to a card it showed with buttons, and only after a click.

use super::card_mapping::{card_key, map_request, needs_item_lookup, MapContext, Mapped, OpenTabReason};
use super::item_index::ItemIndex;
use super::protocol::{answer_result, response_frame, CardKind, Decision};
use super::rpc_state::Action;
use super::session_folder::SessionFolder;
use super::CodexEvent;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A card with buttons that has not been answered or withdrawn.
struct Live {
    raw_id: Value,
    kind: CardKind,
    thread_id: String,
    turn_id: String,
}

pub struct Cards {
    generation: u64,
    tab_title: String,
    ask_mode: Arc<AtomicBool>,
    session: SessionFolder,
    items: ItemIndex,
    /// Keys of requests already carded or left to the terminal.
    seen: HashSet<String>,
    live: HashMap<String, Live>,
}

impl Cards {
    pub fn new(generation: u64, tab_title: String, ask_mode: Arc<AtomicBool>, session: SessionFolder) -> Self {
        Cards { generation, tab_title, ask_mode, session, items: ItemIndex::default(), seen: HashSet::new(), live: HashMap::new() }
    }

    pub fn record_item(&mut self, thread_id: &str, turn_id: &str, item: &Value) {
        self.items.record(thread_id, turn_id, item);
    }

    fn context<'a>(&'a self, roots: &'a HashSet<String>) -> MapContext<'a> {
        MapContext {
            generation: self.generation,
            tab_title: &self.tab_title,
            ask_mode: self.ask_mode.load(Ordering::Relaxed),
            joined_roots: roots,
            seen: &self.seen,
            items: &self.items,
            session: &self.session,
        }
    }

    /// `(thread, turn)` to look up when this request would be a tool approval but no running call is known.
    pub fn lookup_needed(&self, roots: &HashSet<String>, id: &Value, method: &str, params: &Value) -> Option<(String, String)> {
        needs_item_lookup(&self.context(roots), id, method, params)
    }

    /// Takes in the items a `thread/items/list` answered with, keeping any item already seen live.
    pub fn seed(&mut self, thread_id: &str, turn_id: &str, items: &Value) {
        for item in items.as_array().into_iter().flatten() {
            self.items.record_if_absent(thread_id, turn_id, item);
        }
    }

    /// Cards the request, or leaves it to the terminal. Nothing is written to the server either way.
    pub fn request(&mut self, roots: &HashSet<String>, id: &Value, method: &str, params: &Value) -> Vec<Action> {
        match map_request(&self.context(roots), id, method, params) {
            Mapped::Card(card) => {
                self.seen.insert(card.key.clone());
                self.live.insert(
                    card.key.clone(),
                    Live { raw_id: card.raw_id.clone(), kind: card.kind, thread_id: card.thread_id.clone(), turn_id: card.turn_id.clone() },
                );
                vec![Action::Emit(CodexEvent::Card(card))]
            }
            Mapped::OpenTab(OpenTabReason::Duplicate) => Vec::new(),
            Mapped::OpenTab(why) => {
                let key = card_key(self.generation, id);
                self.seen.insert(key.clone());
                vec![Action::Emit(CodexEvent::OpenTabWait { key, why })]
            }
        }
    }

    /// Remembers a request as over, so it never gets a card: one held for an items lookup may be resolved, or its turn
    /// may end, before the lookup answers.
    pub fn mark_over(&mut self, request_id: &Value) {
        self.seen.insert(card_key(self.generation, request_id));
    }

    /// The server resolved a request (answered in the terminal, withdrawn, or its turn ended).
    pub fn resolved(&mut self, request_id: &Value) -> Vec<Action> {
        let key = card_key(self.generation, request_id);
        self.live.remove(&key);
        self.mark_over(request_id);
        vec![Action::Emit(CodexEvent::Resolved { key })]
    }

    /// A turn ended: its items and its open cards are done.
    pub fn turn_completed(&mut self, thread_id: &str, turn_id: &str) -> Vec<Action> {
        self.items.finish_turn(thread_id, turn_id);
        let mut done: Vec<String> =
            self.live.iter().filter(|(_, card)| card.thread_id == thread_id && card.turn_id == turn_id).map(|(key, _)| key.clone()).collect();
        done.sort();
        done.into_iter()
            .map(|key| {
                self.live.remove(&key);
                Action::Emit(CodexEvent::Resolved { key })
            })
            .collect()
    }

    /// A click. The card is removed first, so a second click, or a click after the request was resolved, writes nothing.
    pub fn answer(&mut self, key: &str, decision: Decision) -> Vec<Action> {
        self.live.remove(key).map_or_else(Vec::new, |card| vec![Action::Send(response_frame(&card.raw_id, answer_result(card.kind, decision)))])
    }
}
