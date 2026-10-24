//! The threads of one app-server as Studio knows them (#930): which are root threads, which are working or waiting on
//! an approval, and which Studio has joined. Joining is how approval requests start reaching Studio.

use super::protocol::{status_facts, ThreadFacts, NO_ROLLOUT_CODE};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct Thread {
    root: bool,
    active: bool,
    waiting_on_approval: bool,
    joined: bool,
    resuming: bool,
    retry_queued: bool,
    attempts: u32,
}

pub struct Threads {
    known: HashMap<String, Thread>,
    /// Threads with a `thread/read` in flight.
    reading: HashSet<String>,
    ready: bool,
    max_attempts: u32,
    /// The last `(waiting on approval, active)` reported over all threads.
    reported: (bool, bool),
}

impl Threads {
    pub fn new(max_attempts: u32) -> Self {
        Threads { known: HashMap::new(), reading: HashSet::new(), ready: false, max_attempts, reported: (false, false) }
    }

    /// `true` when `thread_id` is unknown and nobody is reading it yet: the caller sends the `thread/read`.
    pub fn begin_read(&mut self, thread_id: &str) -> bool {
        !self.known.contains_key(thread_id) && self.reading.insert(thread_id.to_string())
    }

    pub fn read_failed(&mut self, thread_id: &str) {
        self.reading.remove(thread_id);
    }

    /// Takes in what a thread object says. `true` when this is the first root thread: the TUI's thread exists.
    pub fn learn(&mut self, facts: ThreadFacts) -> bool {
        self.reading.remove(&facts.id);
        let thread = self.known.entry(facts.id).or_default();
        (thread.root, thread.active, thread.waiting_on_approval) = (facts.root, facts.active, facts.waiting_on_approval);
        facts.root && !std::mem::replace(&mut self.ready, true)
    }

    /// A status change. `false` when the thread is not known (the caller reads it first).
    pub fn set_status(&mut self, thread_id: &str, status: &Value) -> bool {
        let Some(thread) = self.known.get_mut(thread_id) else { return false };
        (thread.active, thread.waiting_on_approval) = status_facts(status);
        if status["type"] == "notLoaded" {
            thread.joined = false;
        }
        if thread.active {
            // Every new active status starts the join over with a fresh count.
            thread.attempts = 0;
        }
        true
    }

    /// `true` when a working root thread is not joined and has no join under way: the caller sends the `thread/resume`.
    pub fn begin_join(&mut self, thread_id: &str) -> bool {
        match self.known.get_mut(thread_id) {
            Some(thread) if thread.root && thread.active && !thread.joined && !thread.resuming && !thread.retry_queued => {
                (thread.resuming, thread.attempts) = (true, thread.attempts + 1);
                true
            }
            _ => false,
        }
    }

    pub fn joined(&mut self, thread_id: &str, status: &Value) {
        if let Some(thread) = self.known.get_mut(thread_id) {
            (thread.joined, thread.resuming, thread.retry_queued) = (true, false, false);
            (thread.active, thread.waiting_on_approval) = status_facts(status);
        }
    }

    /// A join that failed. `true` when it is worth trying again: before the first prompt there is no rollout yet, and
    /// the cap per activation is not reached.
    pub fn join_failed(&mut self, thread_id: &str, code: i64) -> bool {
        let Some(thread) = self.known.get_mut(thread_id) else { return false };
        thread.resuming = false;
        thread.retry_queued = code == NO_ROLLOUT_CODE && thread.attempts < self.max_attempts;
        thread.retry_queued
    }

    pub fn retry_due(&mut self, thread_id: &str) {
        if let Some(thread) = self.known.get_mut(thread_id) {
            thread.retry_queued = false;
        }
    }

    /// The `(waiting on approval, active)` state over all threads, root and sub-agent, when it differs from the last one
    /// reported (or always, with `force`).
    pub fn report(&mut self, force: bool) -> Option<(bool, bool)> {
        let now = (self.known.values().any(|t| t.waiting_on_approval), self.known.values().any(|t| t.active));
        (force || now != self.reported).then(|| {
            self.reported = now;
            now
        })
    }

    /// The root threads Studio has joined.
    pub fn joined_roots(&self) -> HashSet<String> {
        self.known.iter().filter(|(_, thread)| thread.root && thread.joined).map(|(id, _)| id.clone()).collect()
    }
}
