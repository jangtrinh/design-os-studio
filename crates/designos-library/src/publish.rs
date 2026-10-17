//! Shared libraries on disk.
//!
//! ```text
//! libraries/<library-id>/<version>.json   one `LibrarySnapshot` per published version
//! libraries/<library-id>/index.json       name, versions with description and changelog
//! trash/libraries/<library-id>[-n]/       unpublished libraries (soft delete)
//! ```
//!
//! A file publishes under its own id, so republishing adds a version to the
//! same library.

use crate::{Library, LibraryError, Result};
use designos_doc::{diff_snapshots, snapshot, Document, LibraryAssetKind, LibrarySnapshot, PublishInfo, UpdateKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One line of a version's changelog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub key: String,
    pub kind: LibraryAssetKind,
    pub name: String,
    /// "added", "changed" or "removed".
    pub change: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionEntry {
    pub version: u32,
    pub description: String,
    /// RFC 3339, UTC.
    pub published_at: String,
    pub changelog: Vec<ChangeEntry>,
}

/// `index.json` of a published library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryIndex {
    pub id: String,
    pub name: String,
    pub versions: Vec<VersionEntry>,
    /// Asset counts of the latest version: components (incl. sets), styles, variables.
    #[serde(default)]
    pub components: usize,
    #[serde(default)]
    pub styles: usize,
    #[serde(default)]
    pub variables: usize,
    /// Path of the file this library was published from, when it is saved on
    /// this device (lets "Go to main component" open it).
    #[serde(default)]
    pub source_path: Option<String>,
    /// Id of the projects-library file this library was published from, when
    /// it lives in the same library root (opens through the home store).
    #[serde(default)]
    pub source_file: Option<String>,
}

impl LibraryIndex {
    pub fn latest(&self) -> Option<&VersionEntry> {
        self.versions.iter().max_by_key(|v| v.version)
    }

    pub fn latest_version(&self) -> u32 {
        self.latest().map(|v| v.version).unwrap_or(0)
    }
}

/// Characters allowed in a library id used as a directory name.
fn safe_id(s: &str) -> String {
    let id: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    if id.is_empty() {
        "library".into()
    } else {
        id
    }
}

/// Current UTC time as RFC 3339 ("2026-01-02T03:04:05Z").
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

use crate::store::write_atomic;

fn read_or_not_found(p: &Path, what: impl FnOnce() -> String) -> Result<String> {
    std::fs::read_to_string(p).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { LibraryError::NotFound(what()) } else { e.into() })
}

impl Library {
    fn library_dir(&self, id: &str) -> PathBuf {
        self.path(format!("libraries/{}", safe_id(id)))
    }

    /// Reads `index.json` of a published library.
    pub fn library_index(&self, id: &str) -> Result<LibraryIndex> {
        let s = read_or_not_found(&self.library_dir(id).join("index.json"), || format!("library {id}"))?;
        Ok(serde_json::from_str(&s)?)
    }

    /// Publishes `doc` as a new version of the library `file_id_or_name` (the
    /// id of the file, so the same file always updates the same library; an
    /// existing [`PublishInfo`] on the document wins). Returns the info to
    /// store in the file's library state.
    pub fn publish_library(&self, file_id_or_name: &str, doc: &Document, description: &str) -> Result<PublishInfo> {
        let id = doc.library.published.as_ref().map(|p| p.library_id.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| safe_id(file_id_or_name));
        let dir = self.library_dir(&id);
        std::fs::create_dir_all(&dir)?;
        let mut index = self.library_index(&id).unwrap_or(LibraryIndex {
            id: id.clone(),
            name: doc.name.clone(),
            versions: Vec::new(),
            components: 0,
            styles: 0,
            variables: 0,
            source_path: None,
            source_file: None,
        });
        let previous = if index.versions.is_empty() { None } else { self.load_snapshot(&id, None).ok() };
        let version = index.latest_version().max(doc.library.published.as_ref().map(|p| p.version).unwrap_or(0)) + 1;
        let name = if doc.name.is_empty() { file_id_or_name.to_string() } else { doc.name.clone() };
        let snap = snapshot(doc, &id, &name, version);
        let changelog = diff_snapshots(previous.as_ref(), &snap)
            .into_iter()
            .map(|u| ChangeEntry {
                key: u.key,
                kind: u.kind,
                name: u.name,
                change: match u.change {
                    UpdateKind::Added => "added",
                    UpdateKind::Changed => "changed",
                    UpdateKind::Removed => "removed",
                }
                .into(),
            })
            .collect();
        let published_at = now_rfc3339();
        let (snapshot_json, replaced) = designos_doc::finite_json::to_string_finite(&snap)?;
        if replaced > 0 {
            eprintln!("designos-library: published library {id} version {version} with {replaced} non-finite numbers replaced by their neutral value");
        }
        write_atomic(&dir.join(format!("{version}.json")), snapshot_json.as_bytes())?;
        index.name = name.clone();
        index.versions.push(VersionEntry { version, description: description.into(), published_at: published_at.clone(), changelog });
        index.components = snap.count(LibraryAssetKind::Component) + snap.count(LibraryAssetKind::ComponentSet);
        index.styles = snap.count(LibraryAssetKind::Style);
        index.variables = snap.count(LibraryAssetKind::Variable);
        write_atomic(&dir.join("index.json"), serde_json::to_string_pretty(&index)?.as_bytes())?;
        Ok(PublishInfo { library_id: id, name, version, description: description.into(), published_at })
    }

    /// Records where the library's source file lives on this device.
    pub fn set_library_source(&self, id: &str, path: Option<&str>) -> Result<()> {
        let mut index = self.library_index(id)?;
        index.source_path = path.map(String::from);
        write_atomic(&self.library_dir(id).join("index.json"), serde_json::to_string_pretty(&index)?.as_bytes())
    }

    /// Records the projects-library file the library was published from.
    pub fn set_library_source_file(&self, id: &str, file_id: Option<&str>) -> Result<()> {
        let mut index = self.library_index(id)?;
        index.source_file = file_id.map(String::from);
        write_atomic(&self.library_dir(id).join("index.json"), serde_json::to_string_pretty(&index)?.as_bytes())
    }

    /// Published libraries, by name. Unpublished (trashed) ones are not listed.
    pub fn list_libraries(&self) -> Result<Vec<LibraryIndex>> {
        let mut out = Vec::new();
        let root = self.path("libraries");
        if !root.is_dir() {
            return Ok(out);
        }
        for e in std::fs::read_dir(root)? {
            let e = e?;
            if !e.path().is_dir() {
                continue;
            }
            if let Ok(idx) = self.library_index(&e.file_name().to_string_lossy()) {
                out.push(idx);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// Loads one version of a library, the latest with `None`.
    pub fn load_snapshot(&self, id: &str, version: Option<u32>) -> Result<LibrarySnapshot> {
        let version = match version {
            Some(v) => v,
            None => self.library_index(id)?.latest_version(),
        };
        let s = read_or_not_found(&self.library_dir(id).join(format!("{version}.json")), || format!("library {id} version {version}"))?;
        let (snapshot, replaced) = designos_doc::finite_json::from_str_repairing(&s)?;
        if replaced > 0 {
            eprintln!("designos-library: library {id} version {version} held {replaced} null numbers, read as their neutral value");
        }
        Ok(snapshot)
    }

    /// Unpublishes a library: moves it to `trash/libraries/` (restorable with
    /// [`Library::restore_library`]). Files that enabled it keep their copies.
    pub fn unpublish(&self, id: &str) -> Result<()> {
        let src = self.library_dir(id);
        if !src.is_dir() {
            return Err(LibraryError::NotFound(format!("library {id}")));
        }
        let trash = self.path("trash/libraries");
        std::fs::create_dir_all(&trash)?;
        let mut dst = trash.join(safe_id(id));
        let mut n = 1;
        while dst.exists() {
            dst = trash.join(format!("{}-{n}", safe_id(id)));
            n += 1;
        }
        std::fs::rename(src, dst)?;
        Ok(())
    }

    /// Restores the most recently unpublished copy of a library.
    pub fn restore_library(&self, id: &str) -> Result<()> {
        let dst = self.library_dir(id);
        if dst.exists() {
            return Err(LibraryError::Invalid(format!("library {id} is already published")));
        }
        let trash = self.path("trash/libraries");
        let base = safe_id(id);
        let mut best = None;
        let mut n = 0;
        loop {
            let p = if n == 0 { trash.join(&base) } else { trash.join(format!("{base}-{n}")) };
            if p.exists() {
                best = Some(p);
            } else if n > 0 {
                break;
            }
            n += 1;
        }
        let src = best.ok_or_else(|| LibraryError::NotFound(format!("library {id} in trash")))?;
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(src, dst)?;
        Ok(())
    }
}

/// Publishes `doc` (see [`Library::publish_library`]).
pub fn publish(lib: &Library, file_id_or_name: &str, doc: &Document, description: &str) -> Result<PublishInfo> {
    lib.publish_library(file_id_or_name, doc, description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("designos-publish-{name}-{}", crate::new_id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn lib_doc() -> Document {
        let mut d = Document::new("Design System");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut c = Node::new(id, NodeKind::Component(FrameProps::default()));
        c.name = "Button".into();
        c.size = Vec2::new(100.0, 40.0);
        let c = d.insert(page, c).unwrap();
        let tid = d.alloc_id();
        d.insert(c, Node::text(tid, Rect::new(0.0, 0.0, 40.0, 20.0), "Label", TextStyle::default())).unwrap();
        d.styles.insert("Brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(255, 0, 0))]));
        d
    }

    #[test]
    fn publish_list_load_and_changelog() {
        let root = tmp("a");
        let lib = Library::open(&root).unwrap();
        let mut doc = lib_doc();
        let info = publish(&lib, "file-1", &doc, "First").unwrap();
        assert_eq!((info.library_id.as_str(), info.version), ("file-1", 1));
        assert!(info.published_at.ends_with('Z') && info.published_at.len() == 20);
        doc.library.published = Some(info.clone());
        let page = doc.pages[0];
        let comp = doc.get(page).unwrap().children[0];
        let mut n = doc.get(comp).unwrap().clone();
        n.size = Vec2::new(120.0, 40.0);
        Command::Update { node: Box::new(n) }.apply(&mut doc).unwrap();
        let info2 = publish(&lib, "ignored", &doc, "Wider button").unwrap();
        assert_eq!((info2.library_id.as_str(), info2.version), ("file-1", 2));

        let all = lib.list_libraries().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].name.as_str(), all[0].latest_version(), all[0].components, all[0].styles), ("Design System", 2, 1, 1));
        let v1 = &all[0].versions[0];
        assert!(v1.changelog.iter().all(|c| c.change == "added") && v1.changelog.len() == 2);
        let v2 = &all[0].versions[1];
        assert_eq!(v2.description, "Wider button");
        assert_eq!(v2.changelog.len(), 1);
        assert_eq!((v2.changelog[0].name.as_str(), v2.changelog[0].change.as_str()), ("Button", "changed"));

        assert_eq!(lib.load_snapshot("file-1", None).unwrap().version, 2);
        assert_eq!(lib.load_snapshot("file-1", Some(1)).unwrap().version, 1);
        assert!(matches!(lib.load_snapshot("file-1", Some(9)), Err(LibraryError::NotFound(_))));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unpublish_is_soft_delete() {
        let root = tmp("b");
        let lib = Library::open(&root).unwrap();
        publish(&lib, "f", &lib_doc(), "").unwrap();
        lib.unpublish("f").unwrap();
        assert!(lib.list_libraries().unwrap().is_empty());
        assert!(root.join("trash/libraries/f/index.json").exists());
        lib.restore_library("f").unwrap();
        assert_eq!(lib.list_libraries().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Publish in one file, enable in another, edit, republish, update: instance
    /// overrides survive.
    #[test]
    fn round_trip_between_two_documents_preserves_overrides() {
        let root = tmp("c");
        let lib = Library::open(&root).unwrap();
        let mut a = lib_doc();
        let info = publish(&lib, "ds", &a, "v1").unwrap();
        a.library.published = Some(info);

        let snap = lib.load_snapshot("ds", None).unwrap();
        let mut b = Document::new("App");
        let page = b.add_page("Main");
        let key = snap.assets.iter().find(|x| x.name == "Button").unwrap().key.clone();
        let cmd = import_asset(&mut b, &snap, &key, Some(Placement { parent: page, at: Vec2::new(10.0, 10.0) }));
        cmd.apply(&mut b).unwrap();
        let inst = *b.get(page).unwrap().children.last().unwrap();
        let label = b.get(inst).unwrap().children[0];
        let mut n = b.get(label).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "Pay".into();
        }
        n.component.overrides.insert(OverrideField::Characters);
        Command::Update { node: Box::new(n) }.apply(&mut b).unwrap();
        assert!(pending_updates(&b, &snap).is_empty());

        // The library author edits the component and republishes.
        let comp = a.get(a.pages[0]).unwrap().children[0];
        let mut cn = a.get(comp).unwrap().clone();
        cn.size = Vec2::new(160.0, 48.0);
        Command::Update { node: Box::new(cn) }.apply(&mut a).unwrap();
        let info = publish(&lib, "ds", &a, "wider").unwrap();
        assert_eq!(info.version, 2);

        let latest = lib.load_snapshot("ds", None).unwrap();
        let ups = pending_updates(&b, &latest);
        assert_eq!(ups.len(), 1);
        apply_updates(&mut b, &latest, &[ups[0].key.clone()]).apply(&mut b).unwrap();
        assert_eq!(b.get(inst).unwrap().size.x, 160.0);
        let NodeKind::Text(t) = &b.get(b.get(inst).unwrap().children[0]).unwrap().kind else { panic!() };
        assert_eq!(t.content, "Pay");
        assert_eq!(b.library.enabled[0].version, 2);
        // The consumer document survives a JSON round trip with its library state.
        let back = Document::from_json(&b.to_json().unwrap()).unwrap();
        assert_eq!(back.library, b.library);
        assert!(pending_updates(&back, &latest).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// A NaN in a published component or style used to be saved as `null`, which made the whole
    /// version unreadable.
    #[test]
    fn a_published_version_with_non_finite_numbers_still_loads() {
        let root = tmp("nan");
        let lib = Library::open(&root).unwrap();
        let mut doc = lib_doc();
        let comp = doc.get(doc.pages[0]).unwrap().children[0];
        doc.get_mut(comp).unwrap().size = Vec2::new(f32::NAN, 40.0);
        doc.styles.insert("Faded".into(), Style::Fill(vec![Paint { opacity: f32::NAN, ..Paint::solid(Color::rgb(0, 0, 255)) }]));
        publish(&lib, "ds", &doc, "v1").unwrap();
        let snap = lib.load_snapshot("ds", None).unwrap();
        assert!(snap.assets.iter().any(|a| a.name == "Button"));

        // A version written by an older release holds `null` there; it loads too.
        let legacy = serde_json::to_string(&snapshot(&doc, "ds", "Design System", 1)).unwrap();
        assert!(serde_json::from_str::<LibrarySnapshot>(&legacy).is_err(), "the strict parse must refuse the legacy text");
        std::fs::write(root.join("libraries/ds/1.json"), legacy).unwrap();
        assert!(lib.load_snapshot("ds", None).unwrap().assets.iter().any(|a| a.name == "Button"));
        let _ = std::fs::remove_dir_all(root);
    }
}
