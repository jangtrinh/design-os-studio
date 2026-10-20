//! `<session_dir>/run.json`: which skills were attached to which run, append-only (`RD-4` §3.3 and §8.5).
//!
//! A run starts when skills are attached to a tab: its first composer send, and again after the selected skill set
//! changed. Each start appends one entry to `runs`; nothing already in the file is rewritten, so a run keeps the exact
//! versions it was given even after the set changes or a built-in skill ships a new version.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Provenance record of one skill attached to a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedSkillRecord {
    pub name: String,
    pub version: String,
    pub sha256: String,
}

/// One run start: the skills attached to one agent tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSkillRecord {
    pub session_id: String,
    /// The agent tab (`TerminalTab::id`); several tabs of one session share the file.
    pub tab_id: usize,
    /// The CLI that received the skills: its binary name, or `custom` for an "Other CLI…" command.
    pub runtime: String,
    pub skills: Vec<AttachedSkillRecord>,
}

/// Structure of `<session_dir>/run.json`: run starts, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSkillManifest {
    pub runs: Vec<RunSkillRecord>,
}

fn run_path(session_dir: &Path) -> PathBuf {
    session_dir.join("run.json")
}

/// Reads `<session_dir>/run.json`; `None` when it is missing or is not a manifest.
#[cfg(test)]
pub fn read_run_manifest(session_dir: &Path) -> Option<RunSkillManifest> {
    serde_json::from_str(&std::fs::read_to_string(run_path(session_dir)).ok()?).ok()
}

/// Appends `record` to `<session_dir>/run.json` and returns the file's path. A file that exists but is not a manifest
/// (hand-edited, or written by another version) is kept as `run.unreadable.json` instead of being overwritten.
pub fn append_run_record(session_dir: &Path, record: RunSkillRecord) -> Result<PathBuf, String> {
    let _ = std::fs::create_dir_all(session_dir);
    let path = run_path(session_dir);
    let mut manifest = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<RunSkillManifest>(&raw).unwrap_or_else(|_| {
            let kept = session_dir.join("run.unreadable.json");
            let _ = std::fs::rename(&path, kept);
            RunSkillManifest::default()
        }),
        Err(_) => RunSkillManifest::default(),
    };
    manifest.runs.push(record);
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    designos_library::write_atomic(&path, &bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_run_record_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(tab_id: usize, runtime: &str, names: &[&str]) -> RunSkillRecord {
        RunSkillRecord {
            session_id: "session-1".into(),
            tab_id,
            runtime: runtime.into(),
            skills: names.iter().map(|n| AttachedSkillRecord { name: (*n).into(), version: "1.0.0".into(), sha256: "ab".repeat(32) }).collect(),
        }
    }

    #[test]
    fn a_new_run_is_appended_and_the_earlier_one_stays_byte_for_byte() {
        let dir = temp_dir("append");
        append_run_record(&dir, record(1, "codex", &["design-system-rhythm"])).unwrap();
        let first = read_run_manifest(&dir).unwrap();
        append_run_record(&dir, record(1, "codex", &["design-system-rhythm", "component-craft"])).unwrap();
        append_run_record(&dir, record(2, "claude", &["accessibility-wcag"])).unwrap();

        let all = read_run_manifest(&dir).unwrap();
        assert_eq!(all.runs.len(), 3);
        assert_eq!(all.runs[0], first.runs[0], "the earlier run is untouched");
        assert_eq!(all.runs[1].skills.len(), 2);
        assert_eq!((all.runs[2].tab_id, all.runs[2].runtime.as_str(), all.runs[2].skills.len()), (2, "claude", 1), "another tab's run is appended after it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_not_a_manifest_is_kept_aside_not_overwritten() {
        let dir = temp_dir("unreadable");
        let legacy = r#"{"session_id":"old","runtime":"claude","skills":[{"name":"a","version":"1","sha256":"00"}]}"#;
        std::fs::write(dir.join("run.json"), legacy).unwrap();

        append_run_record(&dir, record(1, "codex", &["component-craft"])).unwrap();

        assert_eq!(std::fs::read_to_string(dir.join("run.unreadable.json")).unwrap(), legacy, "the old file is preserved");
        assert_eq!(read_run_manifest(&dir).unwrap().runs.len(), 1, "a fresh manifest starts with the new run");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
