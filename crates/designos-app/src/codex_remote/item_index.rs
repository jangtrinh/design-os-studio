//! The MCP tool calls and file changes Studio has seen start (#930).
//!
//! An approval request carries no tool name or file path of its own: they come from the item that started just
//! before the request. Items are kept by (thread, item id) until their turn ends.

use super::file_changes::Changes;
use serde_json::Value;
use std::collections::HashMap;

enum Tracked {
    Mcp { server: String, tool: String, arguments: Value, in_progress: bool },
    FileChange(Changes),
}

#[derive(Default)]
pub struct ItemIndex {
    items: HashMap<(String, String), (String, Tracked)>,
}

impl ItemIndex {
    /// Records `item` (from `item/started`, `item/completed` or a `thread/items/list` entry) of `turn_id`. Other item
    /// types are ignored; recording an item again replaces it (a completed call is no longer in progress).
    pub fn record(&mut self, thread_id: &str, turn_id: &str, item: &Value) {
        let Some(id) = item["id"].as_str() else { return };
        let tracked = match (item["type"].as_str(), item["server"].as_str(), item["tool"].as_str()) {
            (Some("mcpToolCall"), Some(server), Some(tool)) => {
                Tracked::Mcp { server: server.into(), tool: tool.into(), arguments: item["arguments"].clone(), in_progress: item["status"] == "inProgress" }
            }
            (Some("fileChange"), ..) => Tracked::FileChange(Changes::describe(item["changes"].as_array().map_or(&[][..], Vec::as_slice))),
            _ => return,
        };
        self.items.insert((thread_id.into(), id.into()), (turn_id.into(), tracked));
    }

    /// Like [`ItemIndex::record`], but an item already known is kept: a `thread/items/list` answer is older than what
    /// arrived live, and must not bring a completed call back to life.
    pub fn record_if_absent(&mut self, thread_id: &str, turn_id: &str, item: &Value) {
        let known = item["id"].as_str().is_some_and(|id| self.items.contains_key(&(thread_id.to_string(), id.to_string())));
        if !known {
            self.record(thread_id, turn_id, item);
        }
    }

    /// Drops every item of a finished turn.
    pub fn finish_turn(&mut self, thread_id: &str, turn_id: &str) {
        self.items.retain(|(thread, _), (turn, _)| !(thread == thread_id && turn == turn_id));
    }

    /// `(tool, arguments)` of every call of `server` in progress in that turn.
    pub fn in_progress_mcp(&self, thread_id: &str, turn_id: &str, server_name: &str) -> Vec<(&str, &Value)> {
        let calls = self.items.iter().filter(|((thread, _), (turn, _))| thread == thread_id && turn == turn_id);
        calls
            .filter_map(|(_, (_, tracked))| match tracked {
                Tracked::Mcp { server, tool, arguments, in_progress: true } if server == server_name => Some((tool.as_str(), arguments)),
                _ => None,
            })
            .collect()
    }

    /// How many MCP calls of `server` are in progress in that turn.
    pub fn mcp_calls(&self, thread_id: &str, turn_id: &str, server: &str) -> usize {
        self.in_progress_mcp(thread_id, turn_id, server).len()
    }

    /// What the file-change item `item_id` does, when the item is known.
    pub fn file_changes(&self, thread_id: &str, item_id: &str) -> Option<&Changes> {
        match self.items.get(&(thread_id.to_string(), item_id.to_string())) {
            Some((_, Tracked::FileChange(changes))) => Some(changes),
            _ => None,
        }
    }
}
