//! The user's `designos` tool list, kept where an agent cannot write.
//!
//! The list (`designos.enabled_tools` and its `known_tools` snapshot) is a permission: it decides which `designos-mcp` tools an
//! agent may call. The session folder is the agent's working folder, so a list read from the `mcp-config.json` there is a list
//! the agent can rewrite, and Studio would carry the rewrite into every copy it makes (the server's config, the rules the agent is
//! shown, the bootstrap block). So the list the agent obeys is the one Studio keeps in its private folder:
//!
//! ```text
//! <data dir>/designos/ap/policy/<sid>/   0700  `<sid>` is the channel's hash of the session folder's path
//!   tool-policy.json                     0600  the list as it stood when the first agent of the folder started
//!   server-config.json                   0600  the MCP config whose path `DESIGNOS_MCP_CONFIG` names, built from that list
//! ```
//!
//! It sits beside the approval channels, not in one: a channel goes when its last tab closes, and a library session folder outlives
//! that, so a list kept in a channel would be read again from the folder's own copy at the next start.
//!
//! Until an agent has started in the folder the user's own `mcp-config.json` is the list (nobody but the user writes it yet).
//! [`capture`] copies it at the first start. A session folder an older build left, with a list the user wrote by hand, is captured
//! the same way at its first start after the update.
//!
//! After that the list can only be narrowed: the list in force is the kept one intersected with the list in the folder's
//! `mcp-config.json`, and when that is narrower than the kept one, [`resolve`] writes it back as the kept one (a ratchet, so a
//! restriction made after the first start cannot be undone). Editing the file by hand is the only way the user sets the list, so a
//! tool taken out of it takes effect at once; an agent that takes tools out of it only harms itself. A tool put back is ignored
//! (and told to the user), as is a deleted `designos` key, a forged `known_tools` snapshot or a file that does not parse: each reads
//! as "no narrowing", never as "every tool".
//!
//! `designos-mcp` enables every tool when its config is missing or does not parse, so a start that cannot keep a restricted list
//! private is refused ([`capture`]), never run with the folder's own copy.
//!
//! The limit: an agent that can write Studio's data folder can write these files too. The channel module's header says the same
//! of the approval hook: a process that runs unsandboxed as the user can already rewrite the agent's own settings.

use crate::agent_approval_channel as channel;
use designos_mcp::rules::{reconcile_designos_block, DesignosConfigBlock};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Folder of the private root that holds one folder per session.
const POLICY_DIR_NAME: &str = "policy";

/// What Studio kept: the list as the user wrote it.
const POLICY_FILE_NAME: &str = "tool-policy.json";

/// The MCP config the server and the CLIs read, built from the kept list.
const SERVER_CONFIG_FILE_NAME: &str = "server-config.json";

/// The error of a start that must not go ahead because the list cannot be kept private or the kept one cannot be read.
#[derive(Debug)]
struct Refusal(String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refusal {}

fn refuse(cause: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        Refusal(format!("Studio could not keep your designos tool list where the agent can't change it ({cause}), so the agent was not started.")),
    )
}

/// The refusal of a start whose kept list is damaged: the cause, then what to do (the folder to delete, so the folder's own
/// `mcp-config.json` is read at the next start as it was at the first).
fn refuse_damaged(session_dir: &Path, path: &Path, cause: impl std::fmt::Display) -> io::Error {
    let reset = policy_dir(session_dir).unwrap_or_else(|| path.to_path_buf());
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        Refusal(format!(
            "Studio could not read the designos tool list it keeps for this session ({}: {cause}), so the agent was not started. \
             To reset it, delete the folder {} and start the agent again: the list in the session's mcp-config.json is then used as it was at the first start.",
            path.display(),
            reset.display()
        )),
    )
}

/// Whether `error` is a start refused for the reasons of [`refuse`].
pub fn is_refusal(error: &io::Error) -> bool {
    error.get_ref().is_some_and(|inner| inner.is::<Refusal>())
}

/// `<private root>/policy/<sid>` for `session_dir`; `None` without a private root, or when it would land inside the session folder.
pub fn policy_dir(session_dir: &Path) -> Option<PathBuf> {
    let dir = channel::private_root()?.join(POLICY_DIR_NAME).join(channel::session_id(session_dir));
    (!dir.starts_with(session_dir)).then_some(dir)
}

/// The file `DESIGNOS_MCP_CONFIG` names for `session_dir`, when Studio can keep one outside it.
pub fn server_config_path(session_dir: &Path) -> Option<PathBuf> {
    policy_dir(session_dir).map(|dir| dir.join(SERVER_CONFIG_FILE_NAME))
}

fn policy_path(session_dir: &Path) -> Option<PathBuf> {
    policy_dir(session_dir).map(|dir| dir.join(POLICY_FILE_NAME))
}

/// Makes the private root and the policy folder of `session_dir`, owner only, and returns the latter.
fn prepare_dir(session_dir: &Path) -> io::Result<PathBuf> {
    let dir = policy_dir(session_dir).ok_or_else(|| io::Error::other("no Studio-private data folder outside the folders agents can write"))?;
    let root = channel::private_root().ok_or_else(|| io::Error::other("no Studio-private data folder outside the folders agents can write"))?;
    let policy_root = root.join(POLICY_DIR_NAME);
    for private in [&root, &policy_root, &dir] {
        channel::create_private_dir_all(private)?;
        channel::require_private_dir(private)?;
    }
    Ok(dir)
}

/// Removes everything kept for `session_dir`. Called where the session folder itself is deleted, never when only its channel goes.
pub fn remove(session_dir: &Path) {
    if let Some(dir) = policy_dir(session_dir) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("the kept tool list {what}"))
}

/// What the policy file says: the `designos` block, or `None` for every tool. Anything else is refused, because
/// `reconcile_designos_block` reads a list it cannot parse as "every tool".
fn parse_kept(raw: &str) -> io::Result<Option<Value>> {
    let value: Value = serde_json::from_str(raw).map_err(|e| invalid(&format!("is not JSON: {e}")))?;
    let block = value.as_object().ok_or_else(|| invalid("is not a JSON object"))?.get("designos").ok_or_else(|| invalid("has no designos key"))?;
    let Some(fields) = block.as_object() else {
        return if block.is_null() { Ok(None) } else { Err(invalid("has a designos value that is neither null nor an object")) };
    };
    let strings = |key: &str, required: bool| -> io::Result<()> {
        match fields.get(key) {
            None if required => Err(invalid(&format!("has no {key}"))),
            None => Ok(()),
            Some(Value::Array(items)) if items.iter().all(Value::is_string) => Ok(()),
            Some(_) => Err(invalid(&format!("has a {key} that is not a list of tool names"))),
        }
    };
    strings("enabled_tools", true)?;
    strings("known_tools", false)?;
    Ok(Some(block.clone()))
}

/// The list kept for `session_dir`: `None` before the first start captured one, `Some(None)` for a captured default (every tool).
fn load(session_dir: &Path) -> io::Result<Option<Option<Value>>> {
    let Some(path) = policy_path(session_dir) else {
        return Ok(None);
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => parse_kept(&raw).map(Some).map_err(|e| refuse_damaged(session_dir, &path, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(refuse_damaged(session_dir, &path, e)),
    }
}

/// The list as the `designos` rules read it, with the tools newer than its `known_tools` snapshot switched on.
fn reconcile_block(block: &Value) -> Option<DesignosConfigBlock> {
    reconcile_designos_block(Some(&json!({ "designos": block }).to_string()))
}

/// The names of every registered tool.
fn all_tool_names() -> &'static BTreeSet<String> {
    static NAMES: OnceLock<BTreeSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| designos_mcp::server::tools().into_iter().map(|tool| tool.name.to_string()).collect())
}

/// The tools a list enables (`None` = every tool).
fn enabled_of(custom: Option<&DesignosConfigBlock>) -> BTreeSet<String> {
    custom.map_or_else(|| all_tool_names().clone(), |custom| custom.enabled.clone())
}

/// The list that enables exactly `enabled`, as `reconcile_designos_block` would write it (`None` when that is every tool).
fn list_of(enabled: &BTreeSet<String>) -> Option<DesignosConfigBlock> {
    reconcile_block(&json!({ "enabled_tools": enabled, "known_tools": all_tool_names() }))
}

/// What the lists of a session say now.
struct Plan {
    /// The list in force.
    custom: Option<DesignosConfigBlock>,
    /// The new kept list, when the folder's list is narrower than the kept one.
    ratchet_to: Option<Option<Value>>,
    /// The folder's list enables a tool the kept one does not.
    widened: bool,
}

/// Before an agent has started in the folder its own list is the list. After, it is the kept list intersected with the folder's.
fn plan(session_dir: &Path, session_config: Option<&str>) -> io::Result<Plan> {
    let folder = reconcile_designos_block(session_config);
    let Some(kept) = load(session_dir)? else {
        return Ok(Plan { custom: folder, ratchet_to: None, widened: false });
    };
    let kept = kept.as_ref().and_then(reconcile_block);
    let (kept_tools, folder_tools) = (enabled_of(kept.as_ref()), enabled_of(folder.as_ref()));
    let in_force: BTreeSet<String> = kept_tools.intersection(&folder_tools).cloned().collect();
    let custom = list_of(&in_force);
    let narrower = in_force.len() < kept_tools.len();
    Ok(Plan { ratchet_to: narrower.then(|| custom.as_ref().map(|custom| custom.block.clone())), widened: !folder_tools.is_subset(&kept_tools), custom })
}

/// The tool list of `session_dir` (see [`resolve`]), without keeping anything.
pub fn effective(session_dir: &Path, session_config: Option<&str>) -> io::Result<Option<DesignosConfigBlock>> {
    plan(session_dir, session_config).map(|plan| plan.custom)
}

/// The tool list of `session_dir`, and the sentence to tell the user when the folder's list asked for tools the kept one has off.
/// A folder's list narrower than the kept one becomes the kept one, so what the user took out stays out.
pub fn resolve(session_dir: &Path, session_config: Option<&str>) -> io::Result<Resolved> {
    // The check and the write are one step: two writers must not leave the wider of two lists.
    static KEEPING: Mutex<()> = Mutex::new(());
    let _keeping = KEEPING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let plan = plan(session_dir, session_config)?;
    if let (Some(block), Some(policy)) = (&plan.ratchet_to, policy_path(session_dir)) {
        let bytes = serde_json::to_vec(&json!({ "designos": block })).map_err(refuse)?;
        channel::write_owner_only_file(&policy, &bytes, false).map_err(refuse)?;
    }
    let notice = plan.widened.then(|| {
        let reset =
            policy_dir(session_dir).map_or_else(String::new, |dir| format!(" To widen it, delete the folder {} and start the agent again.", dir.display()));
        format!("The tool list of a session that has started can only be narrowed by editing its mcp-config.json: tools put back in it stay off.{reset}")
    });
    Ok(Resolved { custom: plan.custom, notice })
}

/// The result of [`resolve`].
pub struct Resolved {
    pub custom: Option<DesignosConfigBlock>,
    /// What to tell the user, when the folder's list asked for tools that stay off.
    pub notice: Option<String>,
}

/// The MCP config text that holds just the list: what the server reads if the full config is not written yet.
fn list_only_config(block: Option<&Value>) -> Vec<u8> {
    serde_json::to_vec(&block.map_or_else(|| json!({}), |block| json!({ "designos": block }))).unwrap_or_else(|_| b"{}".to_vec())
}

/// Called when an agent is about to start in `session_dir`: keeps its tool list (`session_config` is the text of the session's own
/// `mcp-config.json`) unless it already was kept at an earlier start. Fails when the list cannot be kept private or what was kept is
/// not readable, and the start then does not go ahead. A session on the default needs no private folder.
pub fn capture(session_dir: &Path, session_config: Option<&str>) -> io::Result<()> {
    if channel::private_root().is_none() {
        return match reconcile_designos_block(session_config) {
            None => Ok(()),
            Some(_) => Err(refuse("there is no Studio-private data folder outside the folders agents can write")),
        };
    }
    let dir = prepare_dir(session_dir).map_err(refuse)?;
    let policy = dir.join(POLICY_FILE_NAME);
    let server_config = dir.join(SERVER_CONFIG_FILE_NAME);
    let kept = load(session_dir)?;
    let block = match &kept {
        Some(block) => block.clone(),
        None => {
            let block = reconcile_designos_block(session_config).map(|custom| custom.block);
            let bytes = serde_json::to_vec(&json!({ "designos": block })).map_err(refuse)?;
            channel::write_owner_only_file(&policy, &bytes, false).map_err(refuse)?;
            block
        }
    };
    // From here the server has a config that holds the list, even if the full one that replaces it a moment later cannot be written.
    if kept.is_none() || !server_config.exists() {
        channel::write_owner_only_file(&server_config, &list_only_config(block.as_ref()), false).map_err(refuse)?;
    }
    Ok(())
}

/// Writes the MCP config `DESIGNOS_MCP_CONFIG` names (see [`server_config_path`]), owner only.
pub fn write_server_config(session_dir: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = prepare_dir(session_dir)?;
    channel::write_owner_only_file(&dir.join(SERVER_CONFIG_FILE_NAME), bytes, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_approval_channel::without_private_root;

    const FULL: &str = r#"{"designos":{"enabled_tools":["designos_get_document"]}}"#;
    const FORGED: &str = r#"{"designos":{"enabled_tools":[],"known_tools":[]}}"#;

    fn session(tag: &str) -> PathBuf {
        std::env::temp_dir().join("designos-sessions").join(format!("tool-policy-{tag}-{}", std::process::id()))
    }

    fn enabled(session_dir: &Path, session_config: Option<&str>) -> Option<Vec<String>> {
        effective(session_dir, session_config).expect("a readable list").map(|custom| custom.enabled.into_iter().collect())
    }

    #[test]
    fn the_list_is_the_users_until_an_agent_starts_and_the_kept_one_after() {
        let dir = session("kept");
        remove(&dir);
        assert_eq!(enabled(&dir, Some(FULL)), Some(vec!["designos_get_document".to_string()]));
        capture(&dir, Some(FULL)).expect("kept");
        // Whatever widens the folder's copy reads as no narrowing.
        for forged in [FORGED, "{}", "not json", r#"{"designos":{"enabled_tools":["designos_get_document","designos_create_node"]}}"#] {
            assert_eq!(enabled(&dir, Some(forged)), Some(vec!["designos_get_document".to_string()]), "{forged}");
            capture(&dir, Some(forged)).expect("a second start keeps what the first kept");
        }
        assert_eq!(enabled(&dir, None), Some(vec!["designos_get_document".to_string()]));
        remove(&dir);
    }

    #[test]
    fn the_kept_list_outlives_the_approval_channel_of_the_folder() {
        let dir = session("channel");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        channel::prepare_channel_dir(&dir).expect("a channel");
        channel::remove_channel(&dir);
        assert_eq!(enabled(&dir, Some("{}")), Some(vec!["designos_get_document".to_string()]), "the channel's removal does not reopen the list");
        remove(&dir);
        assert_eq!(enabled(&dir, Some("{}")), None, "removing what was kept is what resets it");
    }

    #[test]
    fn a_default_session_is_kept_as_the_default() {
        let dir = session("default");
        remove(&dir);
        capture(&dir, None).expect("kept");
        assert_eq!(enabled(&dir, Some("{}")), None);
        assert_eq!(enabled(&dir, Some("not json")), None);
        let config: Value =
            serde_json::from_str(&std::fs::read_to_string(server_config_path(&dir).expect("a path")).expect("the server's config")).expect("JSON");
        assert!(config.get("designos").is_none(), "{config}");
        remove(&dir);
    }

    #[test]
    fn a_restricted_list_cannot_be_kept_without_a_private_folder_and_the_default_does_not_need_one() {
        let dir = session("no-root");
        without_private_root(|| {
            assert!(server_config_path(&dir).is_none());
            capture(&dir, None).expect("the default needs nothing kept");
            capture(&dir, Some("not json")).expect("a list that is not read is the default");
            let refused = capture(&dir, Some(FULL)).expect_err("a restricted list has nowhere to be kept");
            assert!(is_refusal(&refused), "{refused}");
        });
    }

    #[test]
    fn a_kept_list_that_cannot_be_read_refuses_the_start_instead_of_reading_the_folders_copy() {
        let dir = session("malformed");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        let policy = policy_path(&dir).expect("a path");
        for malformed in [
            "not json",
            "[]",
            "{}",
            r#"{"designos":"all"}"#,
            r#"{"designos":{}}"#,
            r#"{"designos":{"enabled_tools":"all"}}"#,
            r#"{"designos":{"enabled_tools":["designos_get_document",7]}}"#,
            r#"{"designos":{"enabled_tools":[],"known_tools":{}}}"#,
        ] {
            std::fs::write(&policy, malformed).expect("a damaged policy");
            let refused = capture(&dir, Some(FORGED)).expect_err(malformed);
            assert!(is_refusal(&refused), "{malformed}: {refused}");
            assert!(effective(&dir, Some(FORGED)).is_err(), "{malformed}: the folder's copy is not used instead");
        }
        remove(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn what_is_kept_sits_outside_the_session_folder_and_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = session("modes");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        write_server_config(&dir, b"{}").expect("written");
        let kept = policy_dir(&dir).expect("a folder");
        assert!(!kept.starts_with(&dir) && !server_config_path(&dir).expect("a path").starts_with(&dir));
        assert_eq!(std::fs::metadata(&kept).expect("a folder").permissions().mode() & 0o777, 0o700);
        for name in [POLICY_FILE_NAME, SERVER_CONFIG_FILE_NAME] {
            assert_eq!(std::fs::metadata(kept.join(name)).expect("a file").permissions().mode() & 0o777, 0o600, "{name}");
        }
        remove(&dir);
        assert!(!kept.exists());
    }

    #[test]
    fn the_server_has_a_config_that_holds_the_list_as_soon_as_it_is_kept() {
        let dir = session("list-only");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        let raw = std::fs::read_to_string(server_config_path(&dir).expect("a path")).expect("the server's config");
        let config = designos_mcp::rules::EnabledToolsConfig::from_mcp_config_json(&raw);
        assert!(config.is_tool_enabled("designos_get_document") && !config.is_tool_enabled("designos_create_node"));
        remove(&dir);
    }

    fn kept_enabled(dir: &Path) -> Option<Vec<String>> {
        let raw = std::fs::read_to_string(policy_path(dir).expect("a path")).expect("the kept list");
        let block = parse_kept(&raw).expect("a readable kept list");
        block.as_ref().and_then(reconcile_block).map(|custom| custom.enabled.into_iter().collect())
    }

    const TWO: &str = r#"{"designos":{"enabled_tools":["designos_get_document","designos_list_nodes"]}}"#;

    #[test]
    fn a_tool_taken_out_after_the_first_start_stays_out_and_is_kept() {
        let dir = session("ratchet");
        remove(&dir);
        capture(&dir, Some(TWO)).expect("kept");
        let narrowed = resolve(&dir, Some(FULL)).expect("resolved");
        assert_eq!(narrowed.custom.map(|custom| custom.enabled.into_iter().collect::<Vec<_>>()), Some(vec!["designos_get_document".to_string()]));
        assert!(narrowed.notice.is_none());
        assert_eq!(kept_enabled(&dir), Some(vec!["designos_get_document".to_string()]), "the narrower list is what is kept now");
        // The agent puts the tool back: the kept list, not the folder's copy, decides.
        let widened = resolve(&dir, Some(TWO)).expect("resolved");
        assert_eq!(widened.custom.map(|custom| custom.enabled.into_iter().collect::<Vec<_>>()), Some(vec!["designos_get_document".to_string()]));
        assert!(widened.notice.is_some());
        assert_eq!(kept_enabled(&dir), Some(vec!["designos_get_document".to_string()]));
        remove(&dir);
    }

    #[test]
    fn a_default_session_can_be_narrowed_by_hand_and_the_narrowing_is_kept() {
        let dir = session("from-default");
        remove(&dir);
        capture(&dir, None).expect("kept");
        let resolved = resolve(&dir, Some(FULL)).expect("resolved");
        assert!(resolved.notice.is_none());
        assert_eq!(kept_enabled(&dir), Some(vec!["designos_get_document".to_string()]));
        for forged in ["{}", "not json", FORGED, TWO] {
            assert_eq!(enabled(&dir, Some(forged)), Some(vec!["designos_get_document".to_string()]), "{forged}");
        }
        remove(&dir);
    }

    #[test]
    fn a_wider_folder_list_changes_nothing_kept_and_says_so_once_it_is_rewritten() {
        let dir = session("widen");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        let before = std::fs::read_to_string(policy_path(&dir).expect("a path")).expect("the kept list");
        for wider in [TWO, "{}", FORGED, "not json"] {
            let resolved = resolve(&dir, Some(wider)).expect("resolved");
            let notice = resolved.notice.expect(wider);
            assert!(notice.contains("can only be narrowed") && notice.contains(&policy_dir(&dir).expect("a folder").display().to_string()), "{notice}");
            assert_eq!(std::fs::read_to_string(policy_path(&dir).expect("a path")).expect("the kept list"), before, "{wider}");
        }
        // What the sync writes back is the list in force, which says nothing more to the user.
        assert!(resolve(&dir, Some(FULL)).expect("resolved").notice.is_none());
        remove(&dir);
    }

    #[test]
    fn a_damaged_kept_list_refuses_with_the_folder_to_delete() {
        let dir = session("damaged-message");
        remove(&dir);
        capture(&dir, Some(FULL)).expect("kept");
        std::fs::write(policy_path(&dir).expect("a path"), "not json").expect("a damaged list");
        let refused = capture(&dir, None).expect_err("refused");
        let message = refused.to_string();
        assert!(
            is_refusal(&refused) && message.contains("delete the folder") && message.contains(&policy_dir(&dir).expect("a folder").display().to_string()),
            "{message}"
        );
        remove(&dir);
    }
}
