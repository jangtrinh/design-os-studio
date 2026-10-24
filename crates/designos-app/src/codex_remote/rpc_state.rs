//! The client's state machine (#930): frames in, actions out. No socket, no clock, no thread, so every rule below is
//! a plain function of what the server sent.
//!
//! Studio writes only what this module and `rpc_cards` build: `initialize`, `initialized`, `thread/loaded/list`,
//! `thread/read`, `thread/resume` (the join), `thread/items/list`, and the reply to a card Studio showed with buttons,
//! after a click. A server request without buttons gets no reply at all, not even an error.

use super::gate::{judge_semver, VersionVerdict};
use super::protocol::*;
use super::rpc_cards::Cards;
use super::rpc_threads::Threads;
use super::session_folder::SessionFolder;
use super::{CodexEvent, Originator, UnavailableReason};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub enum Action {
    Send(Value),
    Emit(CodexEvent),
    /// Call [`Machine::on_retry`] for this thread after the retry delay.
    Retry {
        thread_id: String,
    },
    /// End the session without telling the UI it was lost.
    Close,
}

/// A server request held while Studio looks up the turn's items.
struct Held {
    id: Value,
    method: String,
    params: Value,
}

/// A request of Studio that waits for its reply.
enum Pending {
    Initialize,
    LoadedList,
    Read(String),
    Resume(String),
    Items(Held),
}

pub struct Machine {
    next_id: i64,
    pending: HashMap<i64, Pending>,
    threads: Threads,
    cards: Cards,
}

fn unavailable(shown: &str) -> Vec<Action> {
    vec![Action::Emit(CodexEvent::Unavailable(UnavailableReason::ServerVersion(shown.to_string()))), Action::Close]
}

impl Machine {
    pub fn new(generation: u64, tab_title: String, ask_mode: Arc<AtomicBool>, session: SessionFolder, max_attempts: u32) -> Self {
        Machine { next_id: 0, pending: HashMap::new(), threads: Threads::new(max_attempts), cards: Cards::new(generation, tab_title, ask_mode, session) }
    }

    fn ask(&mut self, method: &str, params: Value, pending: Pending) -> Action {
        self.next_id += 1;
        self.pending.insert(self.next_id, pending);
        Action::Send(request_frame(self.next_id, method, params))
    }

    fn read(&mut self, thread_id: &str) -> Option<Action> {
        self.threads.begin_read(thread_id).then(|| self.ask(method::THREAD_READ, read_params(thread_id), Pending::Read(thread_id.to_string())))
    }

    pub fn start(&mut self) -> Vec<Action> {
        vec![self.ask(method::INITIALIZE, initialize_params(), Pending::Initialize)]
    }

    pub fn on_frame(&mut self, frame: &Value) -> Vec<Action> {
        match classify(frame) {
            Inbound::Response { id, outcome } => self.pending.remove(&id).map_or_else(Vec::new, |pending| self.on_response(pending, outcome)),
            Inbound::Request { id, method, params } => self.on_request(id, method, params),
            Inbound::Notification { method, params } => self.on_notification(method, params),
            Inbound::Other => Vec::new(),
        }
    }

    fn on_response(&mut self, pending: Pending, outcome: Result<&Value, i64>) -> Vec<Action> {
        match (pending, outcome) {
            (Pending::Initialize, Ok(result)) => self.on_initialized(result),
            (Pending::Initialize, Err(_)) => unavailable("unknown"),
            (Pending::LoadedList, Ok(result)) => {
                result["data"].as_array().into_iter().flatten().filter_map(Value::as_str).filter_map(|id| self.read(id)).collect()
            }
            (Pending::Read(_), Ok(result)) => self.learn(&result["thread"]),
            (Pending::Read(id), Err(_)) => {
                self.threads.read_failed(&id);
                Vec::new()
            }
            (Pending::Resume(id), Ok(result)) => {
                self.threads.joined(&id, &result["thread"]["status"]);
                // The state at join is sent in any case: notices seen before the join may be what the UI missed.
                [Action::Emit(CodexEvent::Joined { thread_id: id })].into_iter().chain(self.waiting(true)).collect()
            }
            (Pending::Resume(id), Err(code)) => {
                if self.threads.join_failed(&id, code) {
                    vec![Action::Retry { thread_id: id }]
                } else {
                    Vec::new()
                }
            }
            (Pending::Items(held), outcome) => {
                if let Ok(result) = outcome {
                    self.cards.seed(held.params["threadId"].as_str().unwrap_or_default(), held.params["turnId"].as_str().unwrap_or_default(), &result["data"]);
                }
                self.cards.request(&self.threads.joined_roots(), &held.id, &held.method, &held.params)
            }
            _ => Vec::new(),
        }
    }

    fn on_initialized(&mut self, result: &Value) -> Vec<Action> {
        let Some((originator, version)) = parse_user_agent(result["userAgent"].as_str().unwrap_or_default()) else { return unavailable("unknown") };
        let first = if originator == CLIENT_NAME { Originator::Studio } else { Originator::Tui };
        let mut actions = vec![Action::Emit(CodexEvent::Originator(first))];
        match judge_semver(version) {
            VersionVerdict::Supported => {
                actions.push(Action::Send(notification_frame(method::INITIALIZED)));
                actions.push(self.ask(method::THREAD_LOADED_LIST, json!({}), Pending::LoadedList));
            }
            VersionVerdict::Outside(shown) => actions.extend(unavailable(&shown)),
            VersionVerdict::Unknown => actions.extend(unavailable(version)),
        }
        actions
    }

    /// The `Waiting` event, when the state over all threads changed (or always, with `force`).
    fn waiting(&mut self, force: bool) -> Option<Action> {
        self.threads.report(force).map(|(approval, active)| Action::Emit(CodexEvent::Waiting { approval, active }))
    }

    /// Joins a working root thread that has no join under way.
    fn join(&mut self, thread_id: &str) -> Option<Action> {
        self.threads.begin_join(thread_id).then(|| self.ask(method::THREAD_RESUME, resume_params(thread_id), Pending::Resume(thread_id.to_string())))
    }

    /// Takes in a thread object (`thread/started`, `thread/read`): is it a root, is it working, is it the TUI's.
    fn learn(&mut self, thread: &Value) -> Vec<Action> {
        let Some(facts) = thread_facts(thread) else { return Vec::new() };
        let id = facts.id.clone();
        let first_root = self.threads.learn(facts);
        first_root.then_some(Action::Emit(CodexEvent::ThreadReady)).into_iter().chain(self.join(&id)).chain(self.waiting(false)).collect()
    }

    pub fn on_retry(&mut self, thread_id: &str) -> Vec<Action> {
        self.threads.retry_due(thread_id);
        self.join(thread_id).into_iter().collect()
    }

    fn on_notification(&mut self, name: &str, params: &Value) -> Vec<Action> {
        let (thread_id, turn_id) = (params["threadId"].as_str(), params["turnId"].as_str().or(params["turn"]["id"].as_str()));
        match (name, thread_id, turn_id) {
            (method::THREAD_STARTED, ..) => self.learn(&params["thread"]),
            (method::THREAD_STATUS_CHANGED, Some(thread_id), _) => {
                if !self.threads.set_status(thread_id, &params["status"]) {
                    // A thread not learned yet: its object carries the parent and the status, so read it.
                    return self.read(thread_id).into_iter().collect();
                }
                self.join(thread_id).into_iter().chain(self.waiting(false)).collect()
            }
            (method::ITEM_STARTED | method::ITEM_COMPLETED, Some(thread_id), Some(turn_id)) => {
                self.cards.record_item(thread_id, turn_id, &params["item"]);
                Vec::new()
            }
            (method::TURN_COMPLETED, Some(thread_id), Some(turn_id)) => {
                for pending in self.pending.values() {
                    if let Pending::Items(held) = pending {
                        if held.params["threadId"] == thread_id && held.params["turnId"] == turn_id {
                            self.cards.mark_over(&held.id);
                        }
                    }
                }
                self.cards.turn_completed(thread_id, turn_id)
            }
            (method::REQUEST_RESOLVED, ..) => params.get("requestId").map_or_else(Vec::new, |request_id| self.cards.resolved(request_id)),
            _ => Vec::new(),
        }
    }

    fn on_request(&mut self, id: &Value, method_name: &str, params: &Value) -> Vec<Action> {
        let roots = self.threads.joined_roots();
        let Some((thread_id, turn_id)) = self.cards.lookup_needed(&roots, id, method_name, params) else {
            return self.cards.request(&roots, id, method_name, params);
        };
        let held = Held { id: id.clone(), method: method_name.to_string(), params: params.clone() };
        vec![self.ask(method::THREAD_ITEMS_LIST, items_list_params(&thread_id, &turn_id), Pending::Items(held))]
    }

    /// A click on a card.
    pub fn answer(&mut self, key: &str, decision: Decision) -> Vec<Action> {
        self.cards.answer(key, decision)
    }
}
