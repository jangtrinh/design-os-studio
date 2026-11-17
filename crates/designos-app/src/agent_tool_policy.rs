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
//! [`capture`] copies it at the first start and every later reading uses the copy. A session folder an older build left, with a list
//! the user wrote by hand, is captured the same way at its first start after the update.
//!
//! `designos-mcp` enables every tool when its config is missing or does not parse, so a start that cannot keep a restricted list
//! private is refused ([`capture`]), never run with the folder's own copy.
//!
//! The limit: an agent that can write Studio's data folder can write these files too. The channel module's header says the same
//! of the approval hook: a process that runs unsandboxed as the user can already rewrite the agent's own settings.

use crate::agent_approval_channel as channel;
use designos_mcp::rules::{reconcile_designos_block, DesignosConfigBlock};
use serde_json::{json, Value};
use std::io;
use std::path::{Path, PathBuf};

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
        Ok(raw) => parse_kept(&raw).map(Some).map_err(|e| refuse(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(refuse(format!("{}: {e}", path.display()))),
    }
}

/// The list as the `designos` rules read it, with the tools newer than its `known_tools` snapshot switched on.
fn reconcile_block(block: &Value) -> Option<DesignosConfigBlock> {
    reconcile_designos_block(Some(&json!({ "designos": block }).to_string()))
}

/// The tool list of `session_dir`: the kept one once an agent has started there, otherwise the one the user wrote in the session's
/// `mcp-config.json` (`session_config`, the text of that file when it could be read).
pub fn effective(session_dir: &Path, session_config: Option<&str>) -> io::Result<Option<DesignosConfigBlock>> {
    Ok(match load(session_dir)? {
        Some(kept) => kept.as_ref().and_then(reconcile_block),
        None => reconcile_designos_block(session_config),
    })
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
        for forged in [FORGED, "{}", "not json", r#"{"designos":{"enabled_tools":["designos_create_node"]}}"#] {
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
        assert_eq!(enabled(&dir, Some(FULL)), None);
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
}
