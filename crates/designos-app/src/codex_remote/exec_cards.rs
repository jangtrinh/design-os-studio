//! The text of command and file-change cards (#930): what Approve will run or change, shown exactly or not at all.

use super::card_mapping::{is_set, MapContext, OpenTabReason};
use super::file_changes::Changes;
use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;
use crate::approval_text;
use serde_json::Value;

/// The command line exactly as sent. Never `commandActions`, which is a lossy display summary (`ls | xargs rm -rf` comes
/// out as `ls`). Only printable ASCII reads on the card the way the shell parses it, so any other character (a tab, a line
/// break, a space look-alike, an invisible or combining character, a look-alike letter) means no card: the terminal shows
/// the command in full.
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
    if !approval_text::reads_as_ascii_exactly(text) {
        return Err(OpenTabReason::CommandNotShownExactly);
    }
    Ok(text.to_string())
}

/// What a file-change approval does, one `delete`, `move ... \u{2192} ...`, `add` or `edit` line per change (destructive ones
/// first), when it asks for nothing beyond those files, every change is of a kind Studio describes, every path reads as it
/// is, and the cut at the shared limit hides no part of a delete or a move.
pub(super) fn describe_file_change(ctx: &MapContext, thread_id: &str, params: &Value) -> Result<String, OpenTabReason> {
    if is_set(&params["grantRoot"]) {
        return Err(OpenTabReason::Disqualified);
    }
    let item_id = params["itemId"].as_str().ok_or(OpenTabReason::Incomplete)?;
    match ctx.items.file_changes(thread_id, item_id) {
        Some(Changes::Described(described)) if !described.lines.is_empty() => {
            let destructive_end = described.lines[..described.destructive].join(", ").chars().count();
            if destructive_end > MAX_INPUT_SUMMARY_CHARS {
                return Err(OpenTabReason::ChangeCut);
            }
            Ok(described.lines.join(", "))
        }
        Some(Changes::UnknownKind) => Err(OpenTabReason::Disqualified),
        Some(Changes::PathNotShownExactly) => Err(OpenTabReason::PathNotShownExactly),
        _ => Err(OpenTabReason::ItemUnknown),
    }
}
