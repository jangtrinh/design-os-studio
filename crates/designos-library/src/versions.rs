//! Version history of library files.
//!
//! Layout under [`Library::root`]:
//!
//! ```text
//! versions/<file-id>/<n>.designos   a full snapshot of the document
//! versions/<file-id>/index.json    [{n, label, description, auto, created, size}]
//! ```
//!
//! Versions are either *named* (saved by the user, "Save to version history…")
//! or *auto* (saved while editing and when closing a file). Named versions are
//! always kept; auto versions are thinned by [`retention`] every time a new
//! auto version is written. Deleting a version moves it to the trash folder,
//! it is never hard-deleted except by [`Library::empty_trash`].
//!
//! The index and snapshots are written atomically, so a crash mid-write never
//! leaves a truncated file. A missing or damaged index is rebuilt from the
//! snapshot files.

use crate::store::{now_ms, write_atomic};
use crate::{Library, LibraryError, Result};
use designos_doc::Document;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

const INDEX: &str = "index.json";
const HOUR: u64 = 3_600_000;
const DAY: u64 = 24 * HOUR;
/// Auto versions older than this are dropped.
pub const AUTO_MAX_AGE: u64 = 30 * DAY;

/// One saved snapshot of a file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileVersion {
    /// Sequence number, 1-based and never reused.
    pub n: u32,
    /// Name given by the user; `None` for plain auto versions.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Saved automatically (autosave, close, before a restore).
    #[serde(default)]
    pub auto: bool,
    /// Milliseconds since the Unix epoch.
    pub created: u64,
    /// Size of the snapshot in bytes.
    #[serde(default)]
    pub size: u64,
}

impl FileVersion {
    /// Kept forever: saved by the user, or an auto version that was later named.
    pub fn is_named(&self) -> bool {
        !self.auto || self.label.as_deref().is_some_and(|l| !l.trim().is_empty())
    }
}

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Which auto versions the retention policy no longer keeps.
///
/// - the last hour: every auto version is kept;
/// - up to one day old: the newest auto version of each clock hour;
/// - up to 30 days old: the newest auto version of each calendar day (UTC);
/// - older than that: dropped.
///
/// Named versions are never returned. Order of `entries` does not matter.
pub fn retention(entries: &[FileVersion], now: u64) -> Vec<u32> {
    use std::collections::BTreeMap;
    let mut newest: BTreeMap<(u8, u64), (u64, u32)> = BTreeMap::new();
    let mut drop = Vec::new();
    for e in entries.iter().filter(|e| !e.is_named()) {
        let age = now.saturating_sub(e.created);
        let bucket = if age < HOUR {
            continue;
        } else if age < DAY {
            (0u8, e.created / HOUR)
        } else if age < AUTO_MAX_AGE {
            (1u8, e.created / DAY)
        } else {
            drop.push(e.n);
            continue;
        };
        let slot = newest.entry(bucket).or_insert((e.created, e.n));
        if (e.created, e.n) > *slot {
            drop.push(slot.1);
            *slot = (e.created, e.n);
        } else if (e.created, e.n) < *slot {
            drop.push(e.n);
        }
    }
    drop.sort_unstable();
    drop.dedup();
    drop
}

impl Library {
    fn versions_dir(&self, file: &str) -> PathBuf {
        self.path("versions").join(file)
    }

    fn version_path(&self, file: &str, n: u32) -> PathBuf {
        self.versions_dir(file).join(format!("{n}.designos"))
    }

    fn read_index(&self, file: &str) -> Vec<FileVersion> {
        let dir = self.versions_dir(file);
        if let Some(v) = std::fs::read_to_string(dir.join(INDEX)).ok().and_then(|t| serde_json::from_str::<Vec<FileVersion>>(&t).ok()) {
            return v;
        }
        // Rebuild from the snapshots that are there.
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        rd.flatten()
            .filter_map(|e| {
                let path = e.path();
                let n: u32 = path.file_name()?.to_str()?.strip_suffix(".designos")?.parse().ok()?;
                let meta = e.metadata().ok()?;
                let created = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
                Some(FileVersion { n, label: None, description: None, auto: true, created, size: meta.len() })
            })
            .collect()
    }

    fn write_index(&self, file: &str, entries: &[FileVersion]) -> Result<()> {
        write_atomic(&self.versions_dir(file).join(INDEX), serde_json::to_string_pretty(entries)?.as_bytes())
    }

    /// Saves a snapshot of `doc` as a new version of `file_id`.
    ///
    /// `label` names the version; `auto` marks it as automatic. Saving an auto version
    /// also applies the retention policy.
    pub fn save_version(&self, file_id: &str, doc: &Document, label: Option<String>, auto: bool) -> Result<FileVersion> {
        self.save_version_described(file_id, doc, label, None, auto)
    }

    /// [`Library::save_version`] with a description (the "Save to version history" dialog).
    pub fn save_version_described(&self, file_id: &str, doc: &Document, label: Option<String>, description: Option<String>, auto: bool) -> Result<FileVersion> {
        self.save_version_at(file_id, doc, label, description, auto, now_ms())
    }

    /// Like `save_version_described` with an explicit clock (tests of the retention policy).
    pub fn save_version_at(
        &self,
        file_id: &str,
        doc: &Document,
        label: Option<String>,
        description: Option<String>,
        auto: bool,
        now: u64,
    ) -> Result<FileVersion> {
        if file_id.is_empty() || file_id.contains(['/', '\\']) {
            return Err(LibraryError::Invalid("Invalid file id".into()));
        }
        let _g = lock();
        let mut entries = self.read_index(file_id);
        let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let json = doc.to_json()?;
        let on_disk_max = std::fs::read_dir(self.versions_dir(file_id))
            .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".designos")?.parse::<u32>().ok()).max().unwrap_or(0))
            .unwrap_or(0);
        let trashed_max = std::fs::read_dir(self.path("trash"))
            .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str()?.strip_prefix(&format!("{file_id}-v"))?.parse::<u32>().ok()).max().unwrap_or(0))
            .unwrap_or(0);
        let n = entries.iter().map(|e| e.n).max().unwrap_or(0).max(on_disk_max).max(trashed_max) + 1;
        let v = FileVersion { n, label: clean(label), description: clean(description), auto, created: now, size: json.len() as u64 };
        write_atomic(&self.version_path(file_id, n), json.as_bytes())?;
        entries.push(v.clone());
        if auto {
            let doomed = retention(&entries, now);
            for d in &doomed {
                if *d != n {
                    let _ = std::fs::remove_file(self.version_path(file_id, *d));
                }
            }
            entries.retain(|e| e.n == n || !doomed.contains(&e.n));
        }
        self.write_index(file_id, &entries)?;
        Ok(v)
    }

    /// The versions of a file, newest first.
    pub fn list_versions(&self, file_id: &str) -> Vec<FileVersion> {
        let _g = lock();
        let mut v = self.read_index(file_id);
        v.retain(|e| self.version_path(file_id, e.n).exists());
        v.sort_by(|a, b| b.created.cmp(&a.created).then(b.n.cmp(&a.n)));
        v
    }

    pub fn load_version(&self, file_id: &str, n: u32) -> Result<Document> {
        let text = std::fs::read_to_string(self.version_path(file_id, n)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LibraryError::NotFound(format!("version {n} of {file_id}"))
            } else {
                e.into()
            }
        })?;
        Ok(Document::from_json(&text)?)
    }

    /// Gives a version a name and/or description (an empty name clears it).
    pub fn rename_version(&self, file_id: &str, n: u32, label: Option<String>, description: Option<String>) -> Result<()> {
        let _g = lock();
        let mut entries = self.read_index(file_id);
        let e = entries.iter_mut().find(|e| e.n == n).ok_or_else(|| LibraryError::NotFound(format!("version {n} of {file_id}")))?;
        let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        e.label = clean(label);
        if description.is_some() {
            e.description = clean(description);
        }
        self.write_index(file_id, &entries)
    }

    /// Moves a version to the trash (`trash/<file-id>-v<n>/`) and drops it from the index.
    pub fn delete_version(&self, file_id: &str, n: u32) -> Result<()> {
        let _g = lock();
        let mut entries = self.read_index(file_id);
        let i = entries.iter().position(|e| e.n == n).ok_or_else(|| LibraryError::NotFound(format!("version {n} of {file_id}")))?;
        let dir = self.path("trash").join(format!("{file_id}-v{n}"));
        std::fs::create_dir_all(&dir)?;
        write_atomic(&dir.join("version.json"), serde_json::to_string_pretty(&entries[i])?.as_bytes())?;
        let src = self.version_path(file_id, n);
        if src.exists() {
            std::fs::rename(src, dir.join("version.designos"))?;
        }
        entries.remove(i);
        self.write_index(file_id, &entries)
    }

    /// Version storage key for a branch (`<file-id>~<branch-id>`), keeping branch history
    /// under `versions/<file-id>~<branch-id>/` separate from main's version index.
    pub fn branch_version_key(file_id: &str, branch_id: &str) -> String {
        format!("{file_id}~{branch_id}")
    }

    /// Saves a snapshot of `doc` as a new version on `branch_id` under `versions/<file-id>~<branch-id>/`.
    pub fn save_branch_version(&self, file_id: &str, branch_id: &str, doc: &Document, label: Option<String>, auto: bool) -> Result<FileVersion> {
        self.save_branch_version_described(file_id, branch_id, doc, label, None, auto)
    }

    /// [`Self::save_branch_version`] with a description.
    pub fn save_branch_version_described(
        &self,
        file_id: &str,
        branch_id: &str,
        doc: &Document,
        label: Option<String>,
        description: Option<String>,
        auto: bool,
    ) -> Result<FileVersion> {
        let key = Self::branch_version_key(file_id, branch_id);
        self.save_version_described(&key, doc, label, description, auto)
    }

    /// Versions saved on `branch_id`, newest first.
    pub fn list_branch_versions(&self, file_id: &str, branch_id: &str) -> Vec<FileVersion> {
        let key = Self::branch_version_key(file_id, branch_id);
        self.list_versions(&key)
    }

    /// Loads version `n` of `branch_id`.
    pub fn load_branch_version(&self, file_id: &str, branch_id: &str, n: u32) -> Result<Document> {
        let key = Self::branch_version_key(file_id, branch_id);
        self.load_version(&key, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect};

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("designos_versions_{}_{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            Self(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn doc_with(rects: usize) -> Document {
        let mut d = Document::new("v");
        let page = d.add_page("Page 1");
        for _ in 0..rects {
            let id = d.alloc_id();
            d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        }
        d
    }

    fn v(n: u32, age: u64, auto: bool, label: Option<&str>, now: u64) -> FileVersion {
        FileVersion { n, label: label.map(str::to_string), description: None, auto, created: now - age, size: 1 }
    }

    const NOW: u64 = 100 * DAY + 13 * HOUR + 30 * 60_000;

    #[test]
    fn saves_lists_and_loads_versions() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        let a = lib.save_version("f1", &doc_with(1), None, true).unwrap();
        let b = lib.save_version("f1", &doc_with(3), Some("  Milestone ".into()), false).unwrap();
        assert_eq!((a.n, b.n), (1, 2));
        assert_eq!(b.label.as_deref(), Some("Milestone"));
        assert!(b.size > a.size);
        let list = lib.list_versions("f1");
        assert_eq!(list.iter().map(|e| e.n).collect::<Vec<_>>(), [2, 1], "newest first");
        assert_eq!(lib.load_version("f1", 1).unwrap().nodes.len(), 2);
        assert_eq!(lib.load_version("f1", 2).unwrap().nodes.len(), 4);
        assert!(lib.load_version("f1", 9).is_err());
        assert!(lib.list_versions("other").is_empty());
        assert!(t.0.join("versions/f1/index.json").exists() && t.0.join("versions/f1/2.designos").exists());
        assert!(!t.0.join("versions/f1/index.json.tmp").exists(), "atomic writes leave no temp file");
    }

    #[test]
    fn index_has_the_documented_fields() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        lib.save_version("f", &doc_with(1), Some("x".into()), false).unwrap();
        let text = std::fs::read_to_string(t.0.join("versions/f/index.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let e = &json[0];
        assert_eq!((e["n"].as_u64(), e["label"].as_str(), e["auto"].as_bool()), (Some(1), Some("x"), Some(false)));
        assert!(e["created"].as_u64().unwrap() > 0 && e["size"].as_u64().unwrap() > 0);
    }

    #[test]
    fn delete_moves_to_trash_and_numbers_are_not_reused() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        lib.save_version("f", &doc_with(1), None, true).unwrap();
        lib.save_version("f", &doc_with(2), None, true).unwrap();
        lib.delete_version("f", 2).unwrap();
        assert_eq!(lib.list_versions("f").len(), 1);
        assert!(!t.0.join("versions/f/2.designos").exists());
        assert!(t.0.join("trash/f-v2/version.designos").exists(), "restorable from the trash");
        assert!(lib.delete_version("f", 2).is_err());
        let next = lib.save_version("f", &doc_with(1), None, true).unwrap();
        assert_eq!(next.n, 3);
        assert!(lib.list_trash().is_empty(), "version leftovers are not listed as files");
    }

    #[test]
    fn rename_names_a_version_and_keeps_it() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        lib.save_version("f", &doc_with(1), None, true).unwrap();
        assert!(!lib.list_versions("f")[0].is_named());
        lib.rename_version("f", 1, Some("Final".into()), Some("shipped".into())).unwrap();
        let e = &lib.list_versions("f")[0];
        assert!(e.is_named());
        assert_eq!((e.label.as_deref(), e.description.as_deref()), (Some("Final"), Some("shipped")));
        lib.rename_version("f", 1, Some("  ".into()), None).unwrap();
        assert!(!lib.list_versions("f")[0].is_named());
    }

    #[test]
    fn a_damaged_index_is_rebuilt_from_the_snapshots() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        lib.save_version("f", &doc_with(1), Some("a".into()), false).unwrap();
        lib.save_version("f", &doc_with(2), None, true).unwrap();
        std::fs::write(t.0.join("versions/f/index.json"), "{ nope").unwrap();
        let list = lib.list_versions("f");
        assert_eq!(list.len(), 2);
        assert_eq!(lib.save_version("f", &doc_with(1), None, true).unwrap().n, 3);
    }

    #[test]
    fn retention_keeps_everything_in_the_last_hour() {
        let es: Vec<_> = (1..=10).map(|i| v(i, i as u64 * 5 * 60_000, true, None, NOW)).collect();
        assert!(es.iter().all(|e| NOW - e.created < HOUR));
        assert!(retention(&es, NOW).is_empty());
    }

    #[test]
    fn retention_thins_to_one_per_hour_for_a_day() {
        // three auto versions inside the same clock hour 3h ago, plus one in another hour
        let base = (NOW / HOUR - 3) * HOUR;
        let es = vec![
            FileVersion { n: 1, label: None, description: None, auto: true, created: base + 5 * 60_000, size: 1 },
            FileVersion { n: 2, label: None, description: None, auto: true, created: base + 20 * 60_000, size: 1 },
            FileVersion { n: 3, label: None, description: None, auto: true, created: base + 50 * 60_000, size: 1 },
            FileVersion { n: 4, label: None, description: None, auto: true, created: base - HOUR + 10 * 60_000, size: 1 },
        ];
        assert_eq!(retention(&es, NOW), vec![1, 2], "the newest of the hour survives");
    }

    #[test]
    fn retention_thins_to_one_per_day_for_a_month_and_drops_older() {
        let day = (NOW / DAY - 5) * DAY;
        let es = vec![
            FileVersion { n: 1, label: None, description: None, auto: true, created: day + HOUR, size: 1 },
            FileVersion { n: 2, label: None, description: None, auto: true, created: day + 9 * HOUR, size: 1 },
            FileVersion { n: 3, label: None, description: None, auto: true, created: day - DAY + HOUR, size: 1 },
            v(4, 31 * DAY, true, None, NOW),
        ];
        assert_eq!(retention(&es, NOW), vec![1, 4]);
    }

    #[test]
    fn named_versions_are_always_kept() {
        let es = vec![
            v(1, 40 * DAY, false, Some("Release"), NOW),
            v(2, 40 * DAY, true, Some("Renamed auto"), NOW),
            v(3, 41 * DAY, false, None, NOW),
            v(4, 40 * DAY, true, None, NOW),
        ];
        assert_eq!(retention(&es, NOW), vec![4]);
    }

    #[test]
    fn saving_an_auto_version_prunes_old_ones_on_disk() {
        let t = Tmp::new();
        let lib = Library::open(&t.0).unwrap();
        let now = NOW;
        lib.save_version_at("f", &doc_with(1), None, None, true, now - 40 * DAY).unwrap();
        lib.save_version_at("f", &doc_with(1), Some("Keep".into()), None, false, now - 40 * DAY).unwrap();
        lib.save_version_at("f", &doc_with(1), None, None, true, now - 5 * 60_000).unwrap();
        let ns: Vec<_> = lib.list_versions("f").iter().map(|e| e.n).collect();
        assert_eq!(ns, [3, 2], "the 40 day old auto version is gone");
        assert!(!t.0.join("versions/f/1.designos").exists());
        assert!(t.0.join("versions/f/2.designos").exists());
    }
}
