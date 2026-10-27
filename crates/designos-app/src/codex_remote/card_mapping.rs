//! Which Codex server requests get Approve/Deny buttons, and what the card says (#930).
//!
//! A card is built from structured JSON-RPC fields only, never from terminal output, and only when every field the
//! card needs is there. Anything else (a form, a network approval, a request from a sub-agent) is left to the
//! terminal's own menu: the tab shows the "Open tab" card for it.

use super::exec_cards::{describe_command, describe_file_change};
use super::file_changes::FileLine;
use super::item_index::ItemIndex;
use super::protocol::{method, CardKind};
use super::session_folder::SessionFolder;
use crate::approval_text;
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
    /// A command whose text is cut at the shared limit: the tail is hidden, so it gets no Approve.
    CommandCut,
    /// A tool approval from an MCP server other than `designos`.
    OtherServer,
    /// The request's own copy of the tool arguments differs from the arguments of the running call.
    PayloadMismatch,
    /// A command with a character outside printable ASCII (a tab, a line break, a space look-alike, an invisible or
    /// combining character, a look-alike letter): shown, it would not read the way the shell parses it.
    CommandNotShownExactly,
    /// A changed file's path that would not read on the card as it is (a control or invisible character, a space look-alike,
    /// a combining mark).
    PathNotShownExactly,
    /// A file-change summary that the shared limit would cut.
    ChangeCut,
    /// A command that runs outside the tab's session folder (the model chose another `cwd`) or in an environment other
    /// than the local one.
    CommandOutsideSession,
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
    /// What the panel paints besides the title (boxed: the card travels through event queues).
    pub detail: Box<CardDetail>,
}

/// The structured fields the card is painted from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardDetail {
    /// A command card: the folder the command runs in (the tab's session folder, which the mapping checked).
    pub cwd: Option<String>,
    /// A tool card: the cleaned tool name, and the call's arguments as compact JSON (`None` when it has none).
    pub tool: Option<String>,
    pub input: Option<String>,
    /// A file-change card: one entry per changed path, in the order of `summary`.
    pub files: Vec<FileLine>,
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
    /// The tab's session folder: a command card needs the command to run there.
    pub session: &'a SessionFolder,
}

/// The one MCP server whose tool approvals get buttons.
const DESIGNOS_SERVER: &str = "designos";

pub fn card_key(generation: u64, id: &Value) -> String {
    format!("cx-{generation}-{id}")
}

/// A value that says something: not null, not an empty list, object or string.
pub(super) fn is_set(value: &Value) -> bool {
    !(value.is_null() || value.as_array().is_some_and(Vec::is_empty) || value.as_object().is_some_and(|f| f.is_empty()) || value.as_str() == Some(""))
}

/// A form that asks nothing, raised for an MCP tool call: the shape of a plain tool approval.
fn is_plain_tool_form(params: &Value) -> bool {
    params["mode"] == "form"
        && params["_meta"]["codex_approval_kind"] == "mcp_tool_call"
        && params["requestedSchema"]["properties"].as_object().is_some_and(|fields| fields.is_empty())
}

/// `(thread, turn)` when `method` is a plain `designos` tool approval that would get a card but for one thing: no call is
/// known to be running in the turn (it started before Studio joined). One look at the turn's items can name the tool.
pub fn needs_item_lookup(ctx: &MapContext, id: &Value, method_name: &str, params: &Value) -> Option<(String, String)> {
    let (thread_id, turn_id) = (params["threadId"].as_str()?, params["turnId"].as_str()?);
    let server = params["serverName"].as_str().filter(|name| *name == DESIGNOS_SERVER)?;
    let wanted = method_name == method::MCP_ELICITATION && ctx.ask_mode && is_plain_tool_form(params) && ctx.joined_roots.contains(thread_id);
    (wanted && !ctx.seen.contains(&card_key(ctx.generation, id)) && ctx.items.mcp_calls(thread_id, turn_id, server) == 0)
        .then(|| (thread_id.to_string(), turn_id.to_string()))
}

/// The same payload: equal values, with "no arguments" written as null or as `{}` counting as one.
fn same_payload(copy: &Value, arguments: &Value) -> bool {
    copy == arguments || (!is_set(copy) && !is_set(arguments))
}

/// `server · key: value, ...` from the call's own arguments (values as compact JSON): the payload that will be sent.
fn describe_arguments(server: &str, arguments: &Value) -> String {
    let pairs: Vec<String> = match arguments {
        Value::Object(fields) => fields.iter().map(|(key, value)| format!("{key}: {value}")).collect(),
        Value::Null => Vec::new(),
        other => vec![other.to_string()],
    };
    if pairs.is_empty() {
        server.to_string()
    } else {
        format!("{server} · {}", pairs.join(", "))
    }
}

/// `(tool, summary, arguments as JSON)` of an MCP tool approval. The summary comes from the running call's `arguments`, never from the
/// request's display hints, and the request's own copy of the arguments (when it has one) must agree with them.
fn describe_mcp(ctx: &MapContext, thread_id: &str, turn_id: &str, params: &Value) -> Result<(String, String, Option<String>), OpenTabReason> {
    if !is_plain_tool_form(params) {
        return Err(OpenTabReason::Disqualified);
    }
    let server = params["serverName"].as_str().filter(|name| !name.is_empty()).ok_or(OpenTabReason::Incomplete)?;
    if server != DESIGNOS_SERVER {
        return Err(OpenTabReason::OtherServer);
    }
    let mut calls = ctx.items.in_progress_mcp(thread_id, turn_id, server);
    let (tool, arguments) = if calls.len() == 1 { calls.remove(0) } else { return Err(OpenTabReason::ItemUnknown) };
    if params["_meta"].get("tool_params").is_some_and(|copy| !same_payload(copy, arguments)) {
        return Err(OpenTabReason::PayloadMismatch);
    }
    let input = is_set(arguments).then(|| approval_text::clean(&arguments.to_string()));
    Ok((tool.to_string(), describe_arguments(server, arguments), input))
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
    let (mut cwd, mut tool, mut input, mut files) = (None, None, None, Vec::new());
    let described = match kind {
        CardKind::McpTool => describe_mcp(ctx, thread_id, turn_id, params).map(|(name, summary, arguments)| {
            let title = format!("{tab} wants to use design:os {}", approval_text::clean(&name));
            tool = Some(approval_text::clean(&name));
            input = arguments;
            (title, summary)
        }),
        CardKind::Command => describe_command(ctx, params).map(|summary| {
            cwd = params["cwd"].as_str().map(str::to_string);
            (format!("{tab} wants to run a command"), summary)
        }),
        CardKind::FileChange => describe_file_change(ctx, thread_id, params).map(|(summary, changed)| {
            let title = format!("{tab} wants to change {} file{}", changed.len(), if changed.len() == 1 { "" } else { "s" });
            files = changed;
            (title, summary)
        }),
    };
    match described {
        Ok((title, raw_summary)) => {
            let (summary, truncated) = approval_text::finish(&raw_summary);
            if truncated && kind == CardKind::Command {
                // The tail of a cut command is hidden, and an approved command may run outside the sandbox.
                return reason(OpenTabReason::CommandCut);
            }
            let input = input.map(|text| approval_text::finish(&text).0);
            Mapped::Card(CodexCard {
                key,
                raw_id: id.clone(),
                kind,
                title,
                summary,
                truncated,
                thread_id: thread_id.into(),
                turn_id: turn_id.into(),
                detail: Box::new(CardDetail { cwd, tool, input, files }),
            })
        }
        Err(why) => reason(why),
    }
}
