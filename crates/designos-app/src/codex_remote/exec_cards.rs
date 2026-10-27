//! The text of command and file-change cards (#930): what Approve will run or change, shown exactly or not at all.

use super::card_mapping::{is_set, MapContext, OpenTabReason};
use super::file_changes::{Changes, FileLine};
use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;
use crate::approval_text;
use serde_json::Value;

/// The command line exactly as sent. Never `commandActions`, which is a lossy display summary (`ls | xargs rm -rf` comes
/// out as `ls`). Only printable ASCII reads on the card the way the shell parses it, so any other character (a tab, a line
/// break, a space look-alike, an invisible or combining character, a look-alike letter) means no card: the terminal shows
/// the command in full.
///
/// The card does not show where the command runs, and the model chooses that (`workdir`, `environment_id`). So there is a
/// card only when the request's `cwd` is the tab's session folder itself and the environment is the local one; `cwd` is
/// compared as the string it is, never resolved.
pub(super) fn describe_command(ctx: &MapContext, params: &Value) -> Result<String, OpenTabReason> {
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
    let cwd = params["cwd"].as_str().filter(|cwd| !cwd.is_empty()).ok_or(OpenTabReason::Incomplete)?;
    if !approval_text::reads_as_ascii_exactly(text) {
        return Err(OpenTabReason::CommandNotShownExactly);
    }
    let local = matches!(params.get("environmentId"), None | Some(Value::Null)) || params["environmentId"] == "local";
    if !local || !ctx.session.is(cwd) {
        return Err(OpenTabReason::CommandOutsideSession);
    }
    Ok(text.to_string())
}

/// What a file-change approval does, one `delete`, `move ... \u{2192} ...`, `add` or `edit` line per change (destructive ones
/// first), when it asks for nothing beyond those files, every change is of a kind Studio describes, every path reads as it
/// is, and the summary fits the shared limit: a cut would drop the last changes, and an edit of a shell startup file is as
/// serious as a delete.
pub(super) fn describe_file_change(ctx: &MapContext, thread_id: &str, params: &Value) -> Result<(String, Vec<FileLine>), OpenTabReason> {
    if is_set(&params["grantRoot"]) {
        return Err(OpenTabReason::Disqualified);
    }
    let item_id = params["itemId"].as_str().ok_or(OpenTabReason::Incomplete)?;
    match ctx.items.file_changes(thread_id, item_id) {
        Some(Changes::Described(lines, files)) if !lines.is_empty() => {
            let summary = lines.join(", ");
            if summary.chars().count() > MAX_INPUT_SUMMARY_CHARS {
                return Err(OpenTabReason::ChangeCut);
            }
            Ok((summary, files.clone()))
        }
        Some(Changes::UnknownKind) => Err(OpenTabReason::Disqualified),
        Some(Changes::PathNotShownExactly) => Err(OpenTabReason::PathNotShownExactly),
        _ => Err(OpenTabReason::ItemUnknown),
    }
}
