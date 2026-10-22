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

/// Reads the user-level `"auto_fix"` setting from `settings.json` in `root`, returning
/// `(settings, unreadable_or_malformed)`. Missing `settings.json` returns `(default, false)`
/// without touching the file.
pub fn load_auto_fix_checked(root: &Path) -> (designos_doc::AutoFixSettings, bool) {
    let path = root.join(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (designos_doc::AutoFixSettings::default(), false),
        Err(_) => return (designos_doc::AutoFixSettings::default(), true),
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (designos_doc::AutoFixSettings::default(), true);
    };
    let Some(obj) = val.as_object() else {
        return (designos_doc::AutoFixSettings::default(), true);
    };
    let Some(raw) = obj.get("auto_fix") else {
        return (designos_doc::AutoFixSettings::default(), false);
    };
    if raw.is_null() {
        return (designos_doc::AutoFixSettings::default(), false);
    }
    match serde_json::from_value::<designos_doc::AutoFixSettings>(raw.clone()) {
        Ok(s) => (s, false),
        Err(_) => (designos_doc::AutoFixSettings::default(), true),
    }
}

/// Saves the user-level `"auto_fix"` setting into `settings.json`, preserving unknown keys.
pub fn save_auto_fix(root: &Path, settings: &designos_doc::AutoFixSettings) -> Result<(), String> {
    let mut map = read(root);
    let val = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    map.insert("auto_fix".into(), val);
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
        assert!(load_auto_fix_checked(&d).1);
        save_author(&d, "Ann").unwrap();
        assert_eq!(load_author(&d).as_deref(), Some("Ann"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn auto_fix_roundtrip_keeps_other_keys() {
        let d = tmp("af");
        assert_eq!(load_auto_fix_checked(&d), (designos_doc::AutoFixSettings::default(), false));
        std::fs::write(d.join(FILE), r#"{"author":"Kir"}"#).unwrap();
        let mut s = designos_doc::AutoFixSettings { mode: designos_doc::AutoFixMode::ReportOnly, rules: Default::default() };
        s.set_rule("hardcoded-value", false);
        save_auto_fix(&d, &s).unwrap();
        assert_eq!(load_auto_fix_checked(&d), (s, false));
        assert_eq!(load_author(&d).as_deref(), Some("Kir"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn os_user_is_never_empty() {
        assert!(!os_user_name().is_empty());
    }
}
