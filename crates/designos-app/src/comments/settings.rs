//! The "Your name" app setting, stored in `settings.json` in the library root.
//!
//! The app had no settings store, so this is a small JSON object; keys it does not
//! know are preserved so other settings can live beside `author`.

use std::path::Path;

pub const FILE: &str = "settings.json";

/// The OS user name, or "You" when it cannot be determined.
pub fn os_user_name() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "You".into())
}

fn read(root: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(root.join(FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// The saved author name, if one was set.
pub fn load_author(root: &Path) -> Option<String> {
    read(root).get("author").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// A boolean app setting stored beside the author name.
pub fn load_flag(root: &Path, key: &str) -> Option<bool> {
    read(root).get(key).and_then(|v| v.as_bool())
}

pub fn save_flag(root: &Path, key: &str, value: bool) -> Result<(), String> {
    let mut map = read(root);
    map.insert(key.into(), value.into());
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

/// A string choice setting stored beside the author name.
pub fn load_choice(root: &Path, key: &str) -> Option<String> {
    read(root).get(key).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[allow(dead_code)]
pub fn save_choice(root: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut map = read(root);
    let value = value.trim();
    if value.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.into(), value.into());
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

/// Saves the author name (an empty name resets to the OS user name).
pub fn save_author(root: &Path, name: &str) -> Result<(), String> {
    let mut map = read(root);
    let name = name.trim();
    if name.is_empty() {
        map.remove("author");
    } else {
        map.insert("author".into(), name.into());
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

/// Loads saved custom CLI entries (`custom_clis`) from Studio's `settings.json` (`#652` §4).
pub fn load_custom_clis(root: &Path) -> Vec<crate::agent_cli_catalog::CustomCliEntry> {
    read(root).get("custom_clis").and_then(|v| serde_json::from_value::<Vec<crate::agent_cli_catalog::CustomCliEntry>>(v.clone()).ok()).unwrap_or_default()
}

/// Saves `entries` under `"custom_clis"` in Studio's `settings.json`, preserving all other keys.
pub fn save_custom_clis(root: &Path, entries: &[crate::agent_cli_catalog::CustomCliEntry]) -> Result<(), String> {
    let mut map = read(root);
    let val = serde_json::to_value(entries).map_err(|e| e.to_string())?;
    map.insert("custom_clis".into(), val);
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

/// Adds or updates a custom CLI entry in Studio's `settings.json` (shared across all files, `#652` §3 rule 7).
pub fn add_custom_cli(root: &Path, label: &str, command: &str) -> Result<Vec<crate::agent_cli_catalog::CustomCliEntry>, String> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err("Command cannot be empty".into());
    }
    let lbl = label.trim();
    let resolved_label = if lbl.is_empty() { cmd.split_whitespace().next().unwrap_or(cmd).to_string() } else { lbl.to_string() };
    let mut entries = load_custom_clis(root);
    if let Some(existing) = entries.iter_mut().find(|e| e.command == cmd && e.label == resolved_label) {
        existing.command = cmd.to_string();
    } else {
        entries.push(crate::agent_cli_catalog::CustomCliEntry { label: resolved_label, command: cmd.to_string() });
    }
    save_custom_clis(root, &entries)?;
    Ok(entries)
}

/// Loads `agent.in_progress_stale_minutes` from `settings.json` (clamped to `1..=1440`, default `15`)
/// and applies it to [`designos_doc::set_in_progress_stale_minutes`] (E15-7 §3.3).
pub fn load_in_progress_stale_minutes(root: &Path) -> u32 {
    let map = read(root);
    let raw = map
        .get("agent.in_progress_stale_minutes")
        .or_else(|| map.get("agent").and_then(|v| v.get("in_progress_stale_minutes")))
        .and_then(|v| v.as_u64())
        .map(|v| v.min(u64::from(u32::MAX)) as u32);
    let clamped = raw
        .map(|m| m.clamp(designos_doc::IN_PROGRESS_MIN_STALE_MINUTES, designos_doc::IN_PROGRESS_MAX_STALE_MINUTES))
        .unwrap_or(designos_doc::IN_PROGRESS_DEFAULT_STALE_MINUTES);
    designos_doc::set_in_progress_stale_minutes(Some(clamped));
    clamped
}

/// Saves `agent.in_progress_stale_minutes` (clamped to `1..=1440`) to `settings.json` and updates [`designos_doc`].
pub fn save_in_progress_stale_minutes(root: &Path, minutes: u32) -> Result<u32, String> {
    let clamped = minutes.clamp(designos_doc::IN_PROGRESS_MIN_STALE_MINUTES, designos_doc::IN_PROGRESS_MAX_STALE_MINUTES);
    let mut map = read(root);
    map.insert("agent.in_progress_stale_minutes".into(), serde_json::Value::from(clamped));
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))?;
    designos_doc::set_in_progress_stale_minutes(Some(clamped));
    Ok(clamped)
}

/// Loads the user's chosen default skill names (`skills.default`) from `settings.json`.
pub fn load_default_skills(root: &Path) -> Vec<String> {
    read(root)
        .get("skills")
        .and_then(|s| s.get("default"))
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

/// Saves the user's chosen default skill names (`skills.default`) into `settings.json`,
/// preserving all other keys in `settings.json` and any other sub-keys under `skills`.
pub fn save_default_skills(root: &Path, names: &[String]) -> Result<(), String> {
    let mut map = read(root);
    let clean: Vec<serde_json::Value> = names.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| serde_json::Value::String(s.to_string())).collect();
    let mut skills_obj = map.remove("skills").and_then(|v| v.as_object().cloned()).unwrap_or_default();
    skills_obj.insert("default".into(), serde_json::Value::Array(clean));
    map.insert("skills".into(), serde_json::Value::Object(skills_obj));
    let _ = std::fs::create_dir_all(root);
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

/// Loads the persisted Pages section list height (`pages_height`) from `settings.json`, if set and finite.
pub fn load_pages_height(root: &Path) -> Option<f32> {
    read(root).get("pages_height").and_then(|v| v.as_f64()).map(|v| v as f32).filter(|v| v.is_finite() && *v > 0.0)
}

/// Saves or clears (`None`) the Pages section list height (`pages_height`) in `settings.json`,
/// preserving all other keys.
pub fn save_pages_height(root: &Path, height: Option<f32>) -> Result<(), String> {
    let mut map = read(root);
    match height.filter(|v| v.is_finite() && *v > 0.0) {
        Some(h) => {
            if let Some(num) = serde_json::Number::from_f64(f64::from(h)) {
                map.insert("pages_height".into(), serde_json::Value::Number(num));
            }
        }
        None => {
            map.remove("pages_height");
        }
    }
    let _ = std::fs::create_dir_all(root);
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&root.join(FILE), text.as_bytes()).map_err(|e| format!("Cannot save settings: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("designos_settings_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn author_roundtrip_keeps_other_keys() {
        let d = tmp("a");
        assert_eq!(load_author(&d), None);
        std::fs::write(d.join(FILE), r#"{"theme":"dark","author":"old"}"#).unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("old"));
        save_author(&d, "  Kir ").unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("Kir"));
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join(FILE)).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        save_author(&d, "").unwrap();
        assert_eq!(load_author(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn broken_settings_file_is_ignored() {
        let d = tmp("b");
        std::fs::write(d.join(FILE), "{ nope").unwrap();
        assert_eq!(load_author(&d), None);
        save_author(&d, "Ann").unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("Ann"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn os_user_is_never_empty() {
        assert!(!os_user_name().is_empty());
    }

    /// A `settings.json` saved by v0.6.0, when the terminal dock still existed (#838). Studio never wrote the dock's open
    /// state or height to it; keys a file may still carry from an older build are ignored, kept and never rejected.
    #[test]
    fn a_v060_settings_file_still_loads_after_the_dock_was_removed() {
        let d = tmp("v060");
        std::fs::write(
            d.join(FILE),
            r#"{
                "author": "Kir",
                "agent_permission_mode": "ask",
                "agent.in_progress_stale_minutes": 30,
                "custom_clis": [{ "label": "My CLI", "command": "mycli --flag" }],
                "skills": { "default": ["es-designer"] },
                "dock_open": true,
                "dock_height": 360
            }"#,
        )
        .unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("Kir"));
        assert_eq!(crate::agent_permissions::load_permission_mode(&d), crate::agent_permissions::AgentPermissionMode::Ask);
        assert_eq!(load_in_progress_stale_minutes(&d), 30);
        assert_eq!(load_custom_clis(&d).len(), 1);
        assert_eq!(load_default_skills(&d), vec!["es-designer".to_string()]);

        save_author(&d, "Ann").unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("Ann"));
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join(FILE)).unwrap()).unwrap();
        assert_eq!(v["dock_open"], true, "a key an older build wrote survives a save");
        assert_eq!(v["dock_height"], 360);
        designos_doc::set_in_progress_stale_minutes(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn in_progress_stale_minutes_roundtrip_and_clamp() {
        let d = tmp("stale");
        assert_eq!(load_in_progress_stale_minutes(&d), 15);
        assert_eq!(save_in_progress_stale_minutes(&d, 45).unwrap(), 45);
        assert_eq!(load_in_progress_stale_minutes(&d), 45);
        assert_eq!(save_in_progress_stale_minutes(&d, 0).unwrap(), 1);
        assert_eq!(load_in_progress_stale_minutes(&d), 1);
        assert_eq!(save_in_progress_stale_minutes(&d, 99_999).unwrap(), 1440);
        assert_eq!(load_in_progress_stale_minutes(&d), 1440);
        designos_doc::set_in_progress_stale_minutes(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pages_height_roundtrip_and_clear() {
        let d = tmp("pages_h");
        assert_eq!(load_pages_height(&d), None);
        save_pages_height(&d, Some(96.0)).unwrap();
        assert_eq!(load_pages_height(&d), Some(96.0));
        save_pages_height(&d, None).unwrap();
        assert_eq!(load_pages_height(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
