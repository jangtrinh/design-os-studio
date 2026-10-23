//! The Codex app-server protocol (0.164, stable subset): method names, frame builders, the fixed replies, and the
//! readers of the few fields the client looks at. Frames are `serde_json::Value`s; Codex omits the `jsonrpc` member
//! and so does Studio.

use serde_json::{json, Value};

/// The `clientInfo.name` Studio initializes with. The first client to initialize a server names it process-wide, so
/// the `userAgent` that comes back tells Studio whether the TUI or Studio went first.
pub const CLIENT_NAME: &str = "designos-studio";
/// The error code of `thread/resume` while the thread has no rollout file yet (before its first prompt).
pub const NO_ROLLOUT_CODE: i64 = -32600;

pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "initialized";
    pub const THREAD_LOADED_LIST: &str = "thread/loaded/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const THREAD_RESUME: &str = "thread/resume";
    pub const THREAD_ITEMS_LIST: &str = "thread/items/list";
    pub const THREAD_STARTED: &str = "thread/started";
    pub const THREAD_STATUS_CHANGED: &str = "thread/status/changed";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_COMPLETED: &str = "item/completed";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const REQUEST_RESOLVED: &str = "serverRequest/resolved";
    pub const MCP_ELICITATION: &str = "mcpServer/elicitation/request";
    pub const COMMAND_APPROVAL: &str = "item/commandExecution/requestApproval";
    pub const FILE_APPROVAL: &str = "item/fileChange/requestApproval";
}

/// What a card asks about. It decides the shape of the reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    McpTool,
    Command,
    FileChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Deny,
}

pub fn request_frame(id: i64, method: &str, params: Value) -> Value {
    json!({"id": id, "method": method, "params": params})
}

pub fn notification_frame(method: &str) -> Value {
    json!({"method": method})
}

/// The reply to a server request, carrying the request's own id (a number or a string) back untouched.
pub fn response_frame(id: &Value, result: Value) -> Value {
    json!({"id": id, "result": result})
}

pub fn initialize_params() -> Value {
    json!({"clientInfo": {"name": CLIENT_NAME, "title": "DESIGN:OS Studio", "version": env!("CARGO_PKG_VERSION")}, "capabilities": {"experimentalApi": false}})
}

/// Join a thread without replaying its history (`excludeTurns`: without it the server sends a deprecation notice).
/// These are the only keys Studio sends: it never changes a thread's model, approval or sandbox settings.
pub fn resume_params(thread_id: &str) -> Value {
    json!({"threadId": thread_id, "excludeTurns": true})
}

pub fn read_params(thread_id: &str) -> Value {
    json!({"threadId": thread_id})
}

/// The newest items of one turn, for a request whose item Studio did not see start.
pub fn items_list_params(thread_id: &str, turn_id: &str) -> Value {
    json!({"threadId": thread_id, "turnId": turn_id, "sortDirection": "desc", "limit": 50})
}

/// The one-shot answers a card button sends. There is no "for the session", "always" or amendment variant here: those
/// stay in the terminal. Deny declines and the turn goes on; Studio never cancels (which would interrupt the turn).
pub fn answer_result(kind: CardKind, decision: Decision) -> Value {
    let word = if decision == Decision::Approve { "accept" } else { "decline" };
    match kind {
        CardKind::McpTool => json!({"action": word, "content": null}),
        CardKind::Command | CardKind::FileChange => json!({"decision": word}),
    }
}

/// A frame from the server, by shape.
pub enum Inbound<'a> {
    Request {
        id: &'a Value,
        method: &'a str,
        params: &'a Value,
    },
    Notification {
        method: &'a str,
        params: &'a Value,
    },
    /// The answer to one of Studio's requests: the result, or the JSON-RPC error code.
    Response {
        id: i64,
        outcome: Result<&'a Value, i64>,
    },
    Other,
}

static NO_PARAMS: Value = Value::Null;

pub fn classify(frame: &Value) -> Inbound<'_> {
    let params = frame.get("params").unwrap_or(&NO_PARAMS);
    match (frame.get("method").and_then(Value::as_str), frame.get("id")) {
        (Some(method), Some(id)) => Inbound::Request { id, method, params },
        (Some(method), None) => Inbound::Notification { method, params },
        (None, Some(id)) => match (id.as_i64(), frame.get("result"), frame.get("error")) {
            (Some(id), Some(result), _) => Inbound::Response { id, outcome: Ok(result) },
            (Some(id), None, Some(error)) => Inbound::Response { id, outcome: Err(error.get("code").and_then(Value::as_i64).unwrap_or(0)) },
            _ => Inbound::Other,
        },
        (None, None) => Inbound::Other,
    }
}

/// `(originator, version)` of `<originator>/<X.Y.Z> (<os>) ...`, the `userAgent` of an `initialize` reply.
pub fn parse_user_agent(user_agent: &str) -> Option<(&str, &str)> {
    let (originator, rest) = user_agent.split_once('/')?;
    let version = rest.split_whitespace().next()?;
    (!originator.is_empty()).then_some((originator, version))
}

/// What Studio reads from a thread object (`thread/started`, `thread/read`, a `thread/resume` reply).
pub struct ThreadFacts {
    pub id: String,
    /// A thread no other thread spawned. Sub-agent threads are not roots.
    pub root: bool,
    pub active: bool,
    pub waiting_on_approval: bool,
}

pub fn thread_facts(thread: &Value) -> Option<ThreadFacts> {
    let (active, waiting_on_approval) = status_facts(&thread["status"]);
    Some(ThreadFacts {
        id: thread.get("id")?.as_str()?.to_string(),
        root: thread.get("parentThreadId").is_none_or(Value::is_null),
        active,
        waiting_on_approval,
    })
}

/// `(active, waitingOnApproval)` of a thread status: `{"type":"active","activeFlags":[...]}`.
pub fn status_facts(status: &Value) -> (bool, bool) {
    let active = status["type"] == "active";
    let waiting = active && status["activeFlags"].as_array().is_some_and(|flags| flags.iter().any(|flag| flag == "waitingOnApproval"));
    (active, waiting)
}
