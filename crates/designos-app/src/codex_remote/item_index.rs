//! The MCP tool calls and file changes Studio has seen start (#930).
//!
//! An approval request carries no tool name or file path of its own: they come from the item that started just
//! before the request. Items are kept by (thread, item id) until their turn ends.

use serde_json::Value;
use std::collections::HashMap;

enum Tracked {
    Mcp { server: String, tool: String, arguments: Value, in_progress: bool },
    FileChange { paths: Vec<String> },
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
            (Some("fileChange"), ..) => Tracked::FileChange {
                paths: item["changes"].as_array().into_iter().flatten().filter_map(|change| change["path"].as_str().map(String::from)).collect(),
            },
            _ => return,
        };
        self.items.insert((thread_id.into(), id.into()), (turn_id.into(), tracked));
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

    /// The paths of the file change `item_id`, when it is known.
    pub fn file_paths(&self, thread_id: &str, item_id: &str) -> Option<&[String]> {
        match self.items.get(&(thread_id.to_string(), item_id.to_string())) {
            Some((_, Tracked::FileChange { paths })) => Some(paths),
            _ => None,
        }
    }
}
