//! Which Codex server requests get Approve/Deny buttons, and what the card says (#930).
//!
//! A card is built from structured JSON-RPC fields only, never from terminal output, and only when every field the
//! card needs is there. Anything else (a form, a network approval, a request from a sub-agent) is left to the
//! terminal's own menu: the tab shows the "Open tab" card for it.

use super::item_index::ItemIndex;
use super::protocol::{method, CardKind};
use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;
use serde_json::Value;
use std::collections::HashSet;

/// Why a request gets no buttons.
#[derive(Debug, Clone, PartialEq)]
pub enum OpenTabReason {
    /// The tab is not in Ask mode.
    NotAsk,
    /// The request's thread is not a root thread Studio has joined.
    UnknownThread,
    /// The same request already has a card (a replay).
    Duplicate,
    /// A method with no card: permissions, user-input questions, anything new.
    Unsupported,
    /// A field the card needs is missing.
    Incomplete,
    /// A field says this is more than a plain one-shot approval: a form, a network rule, standard input, a grant root.
    Disqualified,
    /// The item the request is about (the MCP tool call, the changed files) is not known.
    ItemUnknown,
}

/// What the Agents panel shows for one request, and what it needs to answer it.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexCard {
    /// `cx-<generation>-<request id as JSON>`, unique per app-server start.
    pub key: String,
    /// The request id exactly as Codex sent it; the reply carries it back.
    pub raw_id: Value,
    pub kind: CardKind,
    pub title: String,
    pub summary: String,
    /// The summary was cut at the shared limit.
    pub truncated: bool,
    pub thread_id: String,
    pub turn_id: String,
}

pub enum Mapped {
    Card(CodexCard),
    OpenTab(OpenTabReason),
}

pub struct MapContext<'a> {
    pub generation: u64,
    /// The tab's title (`Codex CLI`), the subject of the card title.
    pub tab_title: &'a str,
    pub ask_mode: bool,
    pub joined_roots: &'a HashSet<String>,
    pub seen: &'a HashSet<String>,
    pub items: &'a ItemIndex,
}

pub fn card_key(generation: u64, id: &Value) -> String {
    format!("cx-{generation}-{id}")
}

/// Control characters become spaces; text over the shared summary limit is cut, and the flag says so.
fn finish_summary(raw: &str) -> (String, bool) {
    let text: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let truncated = text.chars().count() > MAX_INPUT_SUMMARY_CHARS;
    (if truncated { text.chars().take(MAX_INPUT_SUMMARY_CHARS).collect() } else { text }, truncated)
}

/// A value that says something: not null, not an empty list, object or string.
fn is_set(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::String(text) => !text.is_empty(),
        _ => true,
    }
}

/// A form that asks nothing, raised for an MCP tool call: the shape of a plain tool approval.
fn is_plain_tool_form(params: &Value) -> bool {
    params["mode"] == "form"
        && params["_meta"]["codex_approval_kind"] == "mcp_tool_call"
        && params["requestedSchema"]["properties"].as_object().is_some_and(|fields| fields.is_empty())
}

/// `(thread, turn)` when `method` is a plain MCP tool approval that would get a card but for one thing: no call of that
/// server is known to be running in the turn (the item started before Studio joined). One look at the turn's items
/// can name the tool.
pub fn needs_item_lookup(ctx: &MapContext, id: &Value, method_name: &str, params: &Value) -> Option<(String, String)> {
    let (thread_id, turn_id) = (params["threadId"].as_str()?, params["turnId"].as_str()?);
    let server = params["serverName"].as_str().filter(|name| !name.is_empty())?;
    let wanted = method_name == method::MCP_ELICITATION && ctx.ask_mode && is_plain_tool_form(params) && ctx.joined_roots.contains(thread_id);
    (wanted && !ctx.seen.contains(&card_key(ctx.generation, id)) && ctx.items.mcp_calls(thread_id, turn_id, server) == 0)
        .then(|| (thread_id.to_string(), turn_id.to_string()))
}

/// `(tool, summary)` of an MCP tool approval.
fn describe_mcp(ctx: &MapContext, thread_id: &str, turn_id: &str, params: &Value) -> Result<(String, String), OpenTabReason> {
    if !is_plain_tool_form(params) {
        return Err(OpenTabReason::Disqualified);
    }
    let server = params["serverName"].as_str().filter(|name| !name.is_empty()).ok_or(OpenTabReason::Incomplete)?;
    let mut calls = ctx.items.in_progress_mcp(thread_id, turn_id, server);
    let (tool, arguments) = if calls.len() == 1 { calls.remove(0) } else { return Err(OpenTabReason::ItemUnknown) };
    let pairs: Vec<String> = match params["_meta"]["tool_params_display"].as_array() {
        Some(entries) => entries
            .iter()
            .filter_map(|entry| {
                let label = entry["display_name"].as_str().or(entry["name"].as_str())?;
                let value = entry.get("value")?;
                Some(format!("{label}: {}", value.as_str().map_or_else(|| value.to_string(), str::to_string)))
            })
            .collect(),
        None => Some(arguments).filter(|arguments| is_set(arguments)).map(Value::to_string).into_iter().collect(),
    };
    Ok((tool.to_string(), if pairs.is_empty() { server.to_string() } else { format!("{server} · {}", pairs.join(", ")) }))
}

/// The text of a plain command approval: the one action's command, or else the whole command line.
fn describe_command(params: &Value) -> Result<String, OpenTabReason> {
    let plain = matches!(params["kind"].as_str(), None | Some("command"))
        && !is_set(&params["networkApprovalContext"])
        && !is_set(&params["proposedNetworkPolicyAmendments"]);
    // The decisions Codex offers ride on the wire (not in the stable schema); when they are listed, accept must be one.
    let accepts = match params.get("availableDecisions") {
        None | Some(Value::Null) => true,
        Some(Value::Array(list)) => list.iter().any(|decision| decision == "accept"),
        Some(_) => false,
    };
    if !plain || !accepts {
        return Err(OpenTabReason::Disqualified);
    }
    let single_action = params["commandActions"].as_array().filter(|actions| actions.len() == 1).and_then(|actions| actions[0]["command"].as_str());
    single_action.or(params["command"].as_str()).filter(|text| !text.trim().is_empty()).map(str::to_string).ok_or(OpenTabReason::Incomplete)
}

/// The changed paths of a file-change approval that asks for nothing beyond those files.
fn describe_file_change(ctx: &MapContext, thread_id: &str, params: &Value) -> Result<String, OpenTabReason> {
    if is_set(&params["grantRoot"]) {
        return Err(OpenTabReason::Disqualified);
    }
    let item_id = params["itemId"].as_str().ok_or(OpenTabReason::Incomplete)?;
    ctx.items.file_paths(thread_id, item_id).filter(|paths| !paths.is_empty()).map(|paths| paths.join(", ")).ok_or(OpenTabReason::ItemUnknown)
}

/// Decides whether a server request gets a card.
pub fn map_request(ctx: &MapContext, id: &Value, method_name: &str, params: &Value) -> Mapped {
    let kind = match method_name {
        method::MCP_ELICITATION => CardKind::McpTool,
        method::COMMAND_APPROVAL => CardKind::Command,
        method::FILE_APPROVAL => CardKind::FileChange,
        _ => return Mapped::OpenTab(OpenTabReason::Unsupported),
    };
    let reason = Mapped::OpenTab;
    if !ctx.ask_mode {
        return reason(OpenTabReason::NotAsk);
    }
    let Some(thread_id) = params["threadId"].as_str().filter(|thread| ctx.joined_roots.contains(*thread)) else { return reason(OpenTabReason::UnknownThread) };
    let key = card_key(ctx.generation, id);
    if ctx.seen.contains(&key) {
        return reason(OpenTabReason::Duplicate);
    }
    let Some(turn_id) = params["turnId"].as_str() else { return reason(OpenTabReason::Incomplete) };
    let tab = ctx.tab_title;
    let described = match kind {
        CardKind::McpTool => {
            describe_mcp(ctx, thread_id, turn_id, params).map(|(tool, summary)| (format!("{tab} wants to run {}", finish_summary(&tool).0), summary))
        }
        CardKind::Command => describe_command(params).map(|summary| (format!("{tab} wants to run a command"), summary)),
        CardKind::FileChange => describe_file_change(ctx, thread_id, params).map(|summary| (format!("{tab} wants to change files"), summary)),
    };
    match described {
        Ok((title, raw_summary)) => {
            let (summary, truncated) = finish_summary(&raw_summary);
            Mapped::Card(CodexCard { key, raw_id: id.clone(), kind, title, summary, truncated, thread_id: thread_id.into(), turn_id: turn_id.into() })
        }
        Err(why) => reason(why),
    }
}
