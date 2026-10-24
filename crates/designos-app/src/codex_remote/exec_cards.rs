//! The text of command and file-change cards (#930): what Approve will run or change, shown exactly or not at all.

use super::card_mapping::{is_set, MapContext, OpenTabReason};
use super::item_index::Changes;
use crate::approval_text;
use serde_json::Value;

/// The command line exactly as sent. Never `commandActions`, which is a lossy display summary (`ls | xargs rm -rf` comes
/// out as `ls`). The card shows the text after sanitizing, so a command the sanitizer would change (a line break, a control
/// or invisible character, which can decide what runs) is not a card: the terminal shows it in full. A tab, shown as a
/// space, is the one change allowed.
pub(super) fn describe_command(params: &Value) -> Result<String, OpenTabReason> {
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
    let text = params["command"].as_str().unwrap_or_default();
    if approval_text::clean(text).trim().is_empty() {
        return Err(OpenTabReason::Incomplete);
    }
    if !approval_text::is_shown_exactly(text) {
        return Err(OpenTabReason::CommandNotShownExactly);
    }
    Ok(text.to_string())
}

/// What a file-change approval does, one `add`, `edit`, `delete` or `move ... → ...` per change, when it asks for nothing
/// beyond those files and every change is of a kind Studio describes.
pub(super) fn describe_file_change(ctx: &MapContext, thread_id: &str, params: &Value) -> Result<String, OpenTabReason> {
    if is_set(&params["grantRoot"]) {
        return Err(OpenTabReason::Disqualified);
    }
    let item_id = params["itemId"].as_str().ok_or(OpenTabReason::Incomplete)?;
    match ctx.items.file_changes(thread_id, item_id) {
        Some(Changes::Described(changes)) if !changes.is_empty() => Ok(changes.join(", ")),
        Some(Changes::Unreadable) => Err(OpenTabReason::Disqualified),
        _ => Err(OpenTabReason::ItemUnknown),
    }
}
