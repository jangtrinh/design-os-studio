//! Projects, folders and files on disk, plus trash, recents, search and thumbnails.
//!
//! Everything here is plain `std::fs` and JSON; there is no database and no
//! account. A project is a directory (`projects/<id>/`) whose `project.json`
//! holds the project name, its folder tree and the index of its files. File
//! contents live next to it as `files/<file-id>.designos`.
//!
//! Nothing is hard-deleted except by [`Library::empty_trash`]: deleting a file
//! or a project moves it to `trash/<id>/`, from where it can be restored.

use crate::{new_id, Library, LibraryError, Result};
use designos_doc::{Document, DocumentKind, Vec2};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Name of the project created on first launch.
pub const SEED_PROJECT: &str = "Getting started";
const SEED_MARKER: &str = ".seeded";
const MANIFEST: &str = "project.json";
const TRASH_MANIFEST: &str = "trash.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// `None` for a folder at the top of the project.
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folders: Vec<Folder>,
    /// Milliseconds since the Unix epoch.
    pub created: u64,
    pub modified: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub project: String,
    pub folder: Option<String>,
    pub name: String,
    pub created: u64,
    pub modified: u64,
    pub opened: u64,
    #[serde(default)]
    pub starred: bool,
    /// PNG thumbnail, relative to the library root (resolve with [`Library::thumbnail_path`]).
    #[serde(default)]
    pub thumbnail: Option<PathBuf>,
    /// Branches belonging to this file (`branches/<file-id>/<branch-id>.designos`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<crate::BranchMeta>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrashKind {
    File,
    Project,
}

/// Something in the trash, restorable until the trash is emptied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    pub kind: TrashKind,
    pub name: String,
    /// Milliseconds since the Unix epoch.
    pub trashed: u64,
    /// For files: the project they were deleted from.
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub entry: Option<FileEntry>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Manifest {
    #[serde(flatten)]
    pub(crate) project: Project,
    #[serde(default)]
    pub(crate) files: Vec<FileEntry>,
}

/// Milliseconds since the epoch, strictly increasing within the process so
/// that "last opened" ordering is stable even for operations in the same ms.
pub fn now_ms() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let mut prev = LAST.load(Ordering::Relaxed);
    loop {
        let next = t.max(prev + 1);
        match LAST.compare_exchange_weak(prev, next, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(p) => prev = p,
        }
    }
}

static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Serialises manifest read-modify-write cycles within the process (the app saves
/// thumbnails on a background thread while the UI thread keeps editing the index).
pub(crate) fn write_lock() -> MutexGuard<'static, ()> {
    WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes `bytes` to `path` via a temporary file next to it, so a crash mid-write never leaves a
/// truncated file behind.
///
/// Delegates to [`designos_fonts::write_atomic`] so every crate in the workspace shares the same
/// atomic save guarantees: symlink traversal (including dangling links), temp-file names truncated
/// on a char boundary for 249–255 byte filenames, the existing permission mode applied when the
/// temp file is created, refusal of read-only, unwritable and non-regular targets, and cleanup of
/// stale temp files left by crashed saves. Because the save renames a new file over the old one,
/// the old file's ACLs, extended attributes, ownership and hard links are not carried over.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    Ok(designos_fonts::write_atomic(path, bytes)?)
}

/// Loads a document from any importable file: `.designos` / `.json` (native or
/// Figma REST), `.fig` or `.svg`.
pub fn load_external(path: &Path) -> Result<Document> {
    load_external_counted(path).map(|(doc, _)| doc)
}

/// Like [`load_external`], also returning how many `null` numbers were replaced on read.
pub fn load_external_counted(path: &Path) -> Result<(Document, usize)> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let invalid = |m: String| LibraryError::Invalid(m);
    let mut replaced_on_read = 0;
    let mut doc = match ext.as_str() {
        "fig" => designos_fig::import_fig(&std::fs::read(path)?).map_err(|e| invalid(format!("Cannot import .fig: {e}")))?,
        "svg" => {
            let text = std::fs::read_to_string(path)?;
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("svg");
            let mut doc = Document::new(name);
            let page = doc.add_page("Page 1");
            designos_svg::import_svg_named(&text, name, &mut doc, page, Vec2::ZERO).map_err(|e| invalid(format!("Cannot import SVG: {e}")))?;
            doc
        }
        "json" => {
            let text = std::fs::read_to_string(path)?;
            match designos_fig::import_rest_json(&text) {
                Ok(d) => d,
                Err(fig) => {
                    let (d, report) =
                        Document::from_json_with_report(&text).map_err(|e| invalid(format!("Not a Figma REST file ({fig}) nor a designos file ({e})")))?;
                    if !report.is_clean() {
                        eprintln!("designos-doc: repaired a damaged document on load: {report:?}");
                    }
                    replaced_on_read = report.non_finite_numbers;
                    d
                }
            }
        }
        _ => {
            let (d, report) = Document::from_json_with_report(&std::fs::read_to_string(path)?).map_err(|e| invalid(format!("Invalid designos file: {e}")))?;
            if !report.is_clean() {
                eprintln!("designos-doc: repaired a damaged document on load: {report:?}");
            }
            replaced_on_read = report.non_finite_numbers;
            d
        }
    };
    if doc.name.is_empty() {
        doc.name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string();
    }
    doc.pages.retain(|p| doc.nodes.contains_key(p));
    if doc.pages.is_empty() {
        doc.add_page("Page 1");
    }
    Ok((doc, replaced_on_read))
}

fn clean_name(name: &str, what: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(LibraryError::Invalid(format!("The {what} name cannot be empty")));
    }
    Ok(n.to_string())
}

#[derive(Debug)]
struct NonFiniteFound;

impl std::fmt::Display for NonFiniteFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("non-finite float")
    }
}

impl std::error::Error for NonFiniteFound {}

impl serde::ser::Error for NonFiniteFound {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        NonFiniteFound
    }
}

/// Zero-allocation `Serializer` that short-circuits as soon as any `f32` or `f64` is not finite.
struct NonFiniteProbe;

impl serde::Serializer for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, _: bool) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i8(self, _: i8) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i16(self, _: i16) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i32(self, _: i32) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i64(self, _: i64) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_i128(self, _: i128) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u8(self, _: u8) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u16(self, _: u16) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u32(self, _: u32) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u64(self, _: u64) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_u128(self, _: u128) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> std::result::Result<(), Self::Error> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(NonFiniteFound)
        }
    }
    fn serialize_f64(self, v: f64) -> std::result::Result<(), Self::Error> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(NonFiniteFound)
        }
    }
    fn serialize_char(self, _: char) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_str(self, _: &str) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_bytes(self, _: &[u8]) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_none(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_unit(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _: &'static str, _: u32, _: &'static str, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn serialize_seq(self, _: Option<usize>) -> std::result::Result<Self::SerializeSeq, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple(self, _: usize) -> std::result::Result<Self::SerializeTuple, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> std::result::Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(Self)
    }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> std::result::Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(Self)
    }
    fn serialize_map(self, _: Option<usize>) -> std::result::Result<Self::SerializeMap, Self::Error> {
        Ok(Self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> std::result::Result<Self::SerializeStruct, Self::Error> {
        Ok(Self)
    }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> std::result::Result<Self::SerializeStructVariant, Self::Error> {
        Ok(Self)
    }
}

impl serde::ser::SerializeSeq for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTuple for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleStruct for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleVariant for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeMap for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> std::result::Result<(), Self::Error> {
        key.serialize(Self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStruct for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStructVariant for NonFiniteProbe {
    type Ok = ();
    type Error = NonFiniteFound;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> std::result::Result<(), Self::Error> {
        value.serialize(Self)
    }
    fn end(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

fn has_non_finite<T: Serialize + ?Sized>(value: &T) -> bool {
    value.serialize(NonFiniteProbe).is_err()
}

fn sanitize_entry<T: Serialize + for<'de> Deserialize<'de>>(value: &mut T) -> Result<()> {
    if has_non_finite(value) {
        let (json, _) = designos_doc::finite_json::to_string_finite(value)?;
        *value = serde_json::from_str(&json)?;
    }
    Ok(())
}

/// Replaces non-finite numbers in `doc` with their neutral values in place by round-tripping only
/// the individual entries that carry a non-finite float, without re-parsing unaffected nodes or
/// image blobs.
fn sanitize_document_in_memory(doc: &mut Document) -> Result<()> {
    for node in doc.nodes.values_mut() {
        sanitize_entry(node)?;
    }
    for style in doc.styles.values_mut() {
        sanitize_entry(style)?;
    }
    for meta in doc.style_meta.values_mut() {
        sanitize_entry(meta)?;
    }
    for flow in &mut doc.flows {
        sanitize_entry(flow)?;
    }
    for var in doc.variables.values_mut() {
        sanitize_entry(var)?;
    }
    for col in doc.variable_collections.values_mut() {
        sanitize_entry(col)?;
    }
    for guides in doc.guides.values_mut() {
        sanitize_entry(guides)?;
    }
    sanitize_entry(&mut doc.prototype_device)?;
    for comment in &mut doc.comments {
        sanitize_entry(comment)?;
    }
    sanitize_entry(&mut doc.library)?;
    doc.validate_and_repair();
    doc.migrate_text();
    Ok(())
}

impl Library {
    /// `$DESIGNOS_LIBRARY` if set (`$DESIGNOS_LIBRARY_DIR` is accepted as a
    /// deprecated alias), otherwise `<data dir>/designos/library`.
    pub fn default_root() -> PathBuf {
        for var in ["DESIGNOS_LIBRARY", "DESIGNOS_LIBRARY_DIR"] {
            if let Some(p) = std::env::var_os(var).filter(|p| !p.is_empty()) {
                return PathBuf::from(p);
            }
        }
        dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("designos").join("library")
    }

    fn project_dir(&self, id: &str) -> PathBuf {
        self.path("projects").join(id)
    }
    fn file_path(&self, project: &str, file: &str) -> PathBuf {
        self.project_dir(project).join("files").join(format!("{file}.designos"))
    }
    fn thumb_rel(project: &str, file: &str) -> PathBuf {
        PathBuf::from("projects").join(project).join("thumbs").join(format!("{file}.png"))
    }
    fn trash_dir(&self, id: &str) -> PathBuf {
        self.path("trash").join(id)
    }

    fn load_manifest(&self, id: &str) -> Result<Manifest> {
        let p = self.project_dir(id).join(MANIFEST);
        let text = std::fs::read_to_string(&p).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LibraryError::NotFound(format!("project {id}"))
            } else {
                e.into()
            }
        })?;
        Ok(serde_json::from_str(&text)?)
    }

    pub(crate) fn save_manifest(&self, m: &Manifest) -> Result<()> {
        let p = self.project_dir(&m.project.id).join(MANIFEST);
        write_atomic(&p, serde_json::to_string_pretty(m)?.as_bytes())
    }

    /// Finds the manifest holding `file` and the file's index in it.
    pub(crate) fn find_file(&self, file: &str) -> Result<(Manifest, usize)> {
        for p in self.list_projects() {
            if let Ok(m) = self.load_manifest(&p.id) {
                if let Some(i) = m.files.iter().position(|f| f.id == file) {
                    return Ok((m, i));
                }
            }
        }
        Err(LibraryError::NotFound(format!("file {file}")))
    }

    // ------------------------------------------------------------ projects

    /// All projects, oldest first.
    pub fn list_projects(&self) -> Vec<Project> {
        let Ok(rd) = std::fs::read_dir(self.path("projects")) else { return vec![] };
        let mut out: Vec<Project> = rd
            .flatten()
            .filter_map(|e| {
                let id = e.file_name().to_string_lossy().into_owned();
                self.load_manifest(&id).ok().map(|m| m.project)
            })
            .collect();
        out.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.name.cmp(&b.name)));
        out
    }

    pub fn project(&self, id: &str) -> Result<Project> {
        Ok(self.load_manifest(id)?.project)
    }

    pub fn create_project(&self, name: &str) -> Result<Project> {
        let name = clean_name(name, "project")?;
        let now = now_ms();
        let project = Project { id: new_id(), name, folders: vec![], created: now, modified: now };
        self.save_manifest(&Manifest { project: project.clone(), files: vec![] })?;
        Ok(project)
    }

    pub fn rename_project(&self, id: &str, name: &str) -> Result<()> {
        let name = clean_name(name, "project")?;
        let mut m = self.load_manifest(id)?;
        m.project.name = name;
        m.project.modified = now_ms();
        self.save_manifest(&m)
    }

    /// Moves the whole project to the trash (it can be restored).
    pub fn delete_project(&self, id: &str) -> Result<()> {
        let m = self.load_manifest(id)?;
        let dir = self.trash_dir(id);
        std::fs::create_dir_all(&dir)?;
        let item = TrashItem {
            id: id.to_string(),
            kind: TrashKind::Project,
            name: m.project.name.clone(),
            trashed: now_ms(),
            project: None,
            project_name: None,
            entry: None,
        };
        write_atomic(&dir.join(TRASH_MANIFEST), serde_json::to_string_pretty(&item)?.as_bytes())?;
        let dest = dir.join("project");
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::rename(self.project_dir(id), dest)?;
        Ok(())
    }

    // ------------------------------------------------------------ folders

    pub fn create_folder(&self, project: &str, parent: Option<&str>, name: &str) -> Result<Folder> {
        let name = clean_name(name, "folder")?;
        let mut m = self.load_manifest(project)?;
        if let Some(p) = parent {
            if !m.project.folders.iter().any(|f| f.id == p) {
                return Err(LibraryError::NotFound(format!("folder {p}")));
            }
        }
        let folder = Folder { id: new_id(), name, parent: parent.map(str::to_string) };
        m.project.folders.push(folder.clone());
        m.project.modified = now_ms();
        self.save_manifest(&m)?;
        Ok(folder)
    }

    pub fn rename_folder(&self, project: &str, folder: &str, name: &str) -> Result<()> {
        let name = clean_name(name, "folder")?;
        let mut m = self.load_manifest(project)?;
        let f = m.project.folders.iter_mut().find(|f| f.id == folder).ok_or_else(|| LibraryError::NotFound(format!("folder {folder}")))?;
        f.name = name;
        self.save_manifest(&m)
    }

    /// Re-parents a folder (`None` = top level). A folder cannot move into itself or its own subtree.
    pub fn move_folder(&self, project: &str, folder: &str, new_parent: Option<&str>) -> Result<()> {
        let mut m = self.load_manifest(project)?;
        if !m.project.folders.iter().any(|f| f.id == folder) {
            return Err(LibraryError::NotFound(format!("folder {folder}")));
        }
        let mut cur = new_parent.map(str::to_string);
        while let Some(c) = cur {
            if c == folder {
                return Err(LibraryError::Invalid("A folder cannot be moved into itself".into()));
            }
            cur = m.project.folders.iter().find(|f| f.id == c).and_then(|f| f.parent.clone());
        }
        if let Some(p) = new_parent {
            if !m.project.folders.iter().any(|f| f.id == p) {
                return Err(LibraryError::NotFound(format!("folder {p}")));
            }
        }
        if let Some(f) = m.project.folders.iter_mut().find(|f| f.id == folder) {
            f.parent = new_parent.map(str::to_string);
        }
        self.save_manifest(&m)
    }

    // ------------------------------------------------------------ files

    /// Every file in every project.
    pub fn all_files(&self) -> Vec<FileEntry> {
        self.list_projects().iter().filter_map(|p| self.load_manifest(&p.id).ok()).flat_map(|m| m.files).collect()
    }

    /// Files of one project; with `folder` only that folder's direct children (`Some(None)` = top level).
    pub fn files_in(&self, project: &str, folder: Option<Option<&str>>) -> Result<Vec<FileEntry>> {
        let m = self.load_manifest(project)?;
        Ok(m.files.into_iter().filter(|f| folder.is_none_or(|want| f.folder.as_deref() == want)).collect())
    }

    pub fn file(&self, id: &str) -> Result<FileEntry> {
        let (m, i) = self.find_file(id)?;
        Ok(m.files[i].clone())
    }

    pub fn create_file(&self, project: &str, folder: Option<&str>, name: &str, doc: &Document) -> Result<FileEntry> {
        self.create_file_counted(project, folder, name, doc).map(|(entry, _)| entry)
    }

    /// [`Self::create_file`], also returning how many non-finite numbers (NaN, infinity) were
    /// written as their neutral value, so the stored file always opens.
    pub fn create_file_counted(&self, project: &str, folder: Option<&str>, name: &str, doc: &Document) -> Result<(FileEntry, usize)> {
        let _guard = write_lock();
        let name = clean_name(name, "file")?;
        let mut m = self.load_manifest(project)?;
        if let Some(f) = folder {
            if !m.project.folders.iter().any(|x| x.id == f) {
                return Err(LibraryError::NotFound(format!("folder {f}")));
            }
        }
        let now = now_ms();
        let entry = FileEntry {
            id: new_id(),
            project: project.to_string(),
            folder: folder.map(str::to_string),
            name,
            created: now,
            modified: now,
            opened: now,
            starred: false,
            thumbnail: None,
            branches: vec![],
        };
        let (json, replaced) = doc.to_json_counted()?;
        let file_path = self.file_path(project, &entry.id);
        write_atomic(&file_path, json.as_bytes())?;
        m.files.push(entry.clone());
        m.project.modified = now;
        if let Err(err) = self.save_manifest(&m) {
            self.rollback_unlisted_file(project, &entry.id, &file_path);
            return Err(err);
        }
        Ok((entry, replaced))
    }

    /// Removes `file_path` after a failed manifest save only when the on-disk manifest does not
    /// already list `file_id` (so a post-rename `sync_dir` failure never deletes a listed file).
    fn rollback_unlisted_file(&self, project: &str, file_id: &str, file_path: &Path) {
        let listed_on_disk = self.load_manifest(project).is_ok_and(|disk| disk.files.iter().any(|f| f.id == file_id));
        if !listed_on_disk {
            let _ = std::fs::remove_file(file_path);
        }
    }

    /// Overwrites the file's contents atomically and bumps its modified time.
    pub fn save_file(&self, id: &str, doc: &Document) -> Result<()> {
        let replaced = self.save_file_counted(id, doc)?;
        if replaced > 0 {
            eprintln!("designos-library: saved file {id} with {replaced} non-finite numbers replaced by their neutral value");
        }
        Ok(())
    }

    /// [`Self::save_file`], also returning how many non-finite numbers (NaN, infinity) were written
    /// as their neutral value, counted by the one serialization the save performs.
    pub fn save_file_counted(&self, id: &str, doc: &Document) -> Result<usize> {
        let _guard = write_lock();
        let (mut m, i) = self.find_file(id)?;
        let (json, replaced) = doc.to_json_counted()?;
        write_atomic(&self.file_path(&m.project.id, id), json.as_bytes())?;
        let now = now_ms();
        m.files[i].modified = now;
        m.project.modified = now;
        self.save_manifest(&m)?;
        Ok(replaced)
    }

    /// Whether the file is a design file or a board, read from the document itself (the manifest
    /// has no field for it). Parses only the small header before `"nodes"` when possible so
    /// multi-hundred-MB files are inspected in microseconds.
    pub fn file_kind(&self, id: &str) -> Result<DocumentKind> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            kind: DocumentKind,
        }
        let (m, _) = self.find_file(id)?;
        let path = self.file_path(&m.project.id, id);
        let mut f = std::fs::File::open(&path)?;
        let mut head = Vec::with_capacity(8192);
        {
            use std::io::Read;
            f.by_ref().take(8192).read_to_end(&mut head)?;
        }
        if let Ok(s) = std::str::from_utf8(&head) {
            if let Some(pos) = s.find("\"nodes\"") {
                let prefix = s[..pos].trim_end();
                if let Some(stripped) = prefix.strip_suffix(',') {
                    let candidate = format!("{stripped}}}");
                    if let Ok(p) = serde_json::from_str::<Probe>(&candidate) {
                        return Ok(p.kind);
                    }
                }
            }
        }
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice::<Probe>(&bytes)?.kind)
    }

    pub fn load_file(&self, id: &str) -> Result<Document> {
        let (m, _) = self.find_file(id)?;
        let text = std::fs::read_to_string(self.file_path(&m.project.id, id))?;
        Ok(Document::from_json(&text)?)
    }

    fn update_file(&self, id: &str, f: impl FnOnce(&mut FileEntry)) -> Result<()> {
        let _guard = write_lock();
        let (mut m, i) = self.find_file(id)?;
        f(&mut m.files[i]);
        self.save_manifest(&m)
    }

    pub fn rename_file(&self, id: &str, name: &str) -> Result<()> {
        let name = clean_name(name, "file")?;
        self.update_file(id, |e| e.name = name)
    }

    /// Records that the file was just opened (drives "Recents").
    pub fn mark_opened(&self, id: &str) -> Result<()> {
        self.update_file(id, |e| e.opened = now_ms())
    }

    pub fn star(&self, id: &str, starred: bool) -> Result<()> {
        self.update_file(id, |e| e.starred = starred)
    }

    pub fn duplicate_file(&self, id: &str) -> Result<FileEntry> {
        let _guard = write_lock();
        let (mut m, i) = self.find_file(id)?;
        let src = m.files[i].clone();
        let now = now_ms();
        let mut new = FileEntry {
            id: new_id(),
            name: format!("{} (copy)", src.name),
            created: now,
            modified: now,
            opened: now,
            starred: false,
            thumbnail: None,
            branches: vec![],
            ..src.clone()
        };
        std::fs::copy(self.file_path(&src.project, &src.id), self.file_path(&src.project, &new.id))?;
        if let Some(t) = &src.thumbnail {
            let rel = Self::thumb_rel(&src.project, &new.id);
            if std::fs::copy(self.path(t), self.path(&rel)).is_ok() {
                new.thumbnail = Some(rel);
            }
        }
        m.files.push(new.clone());
        m.project.modified = now;
        self.save_manifest(&m)?;
        Ok(new)
    }

    /// Moves a file to another project and/or folder.
    pub fn move_file(&self, id: &str, project: &str, folder: Option<&str>) -> Result<()> {
        let _guard = write_lock();
        let (mut src, i) = self.find_file(id)?;
        if src.project.id == project {
            if let Some(f) = folder {
                if !src.project.folders.iter().any(|x| x.id == f) {
                    return Err(LibraryError::NotFound(format!("folder {f}")));
                }
            }
            src.files[i].folder = folder.map(str::to_string);
            return self.save_manifest(&src);
        }
        let mut dst = self.load_manifest(project)?;
        if let Some(f) = folder {
            if !dst.project.folders.iter().any(|x| x.id == f) {
                return Err(LibraryError::NotFound(format!("folder {f}")));
            }
        }
        let mut entry = src.files.remove(i);
        let from = entry.project.clone();
        if let Some(parent) = self.file_path(project, id).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(self.file_path(&from, id), self.file_path(project, id))?;
        if let Some(old) = entry.thumbnail.take() {
            let rel = Self::thumb_rel(project, id);
            if let Some(parent) = self.path(&rel).parent() {
                std::fs::create_dir_all(parent)?;
            }
            if std::fs::rename(self.path(old), self.path(&rel)).is_ok() {
                entry.thumbnail = Some(rel);
            }
        }
        entry.project = project.to_string();
        entry.folder = folder.map(str::to_string);
        dst.files.push(entry);
        let now = now_ms();
        dst.project.modified = now;
        src.project.modified = now;
        self.save_manifest(&dst)?;
        self.save_manifest(&src)
    }

    /// Stores a PNG thumbnail for the file.
    pub fn set_thumbnail(&self, id: &str, png: &[u8]) -> Result<()> {
        let _guard = write_lock();
        let (mut m, i) = self.find_file(id)?;
        let rel = Self::thumb_rel(&m.project.id, id);
        write_atomic(&self.path(&rel), png)?;
        m.files[i].thumbnail = Some(rel);
        self.save_manifest(&m)
    }

    /// Absolute path of an entry's thumbnail, if it has one on disk.
    pub fn thumbnail_path(&self, entry: &FileEntry) -> Option<PathBuf> {
        entry.thumbnail.as_ref().map(|r| self.path(r)).filter(|p| p.exists())
    }

    /// Copies a `.designos` file, or converts a `.fig` / `.json` / `.svg`, into a project.
    pub fn import_file(&self, path: &Path, project: &str, folder: Option<&str>) -> Result<FileEntry> {
        self.import_file_with_doc(path, project, folder).map(|(e, _)| e)
    }

    /// Same as [`Self::import_file`], also returning the parsed [`Document`] so callers
    /// (such as background thumbnail rendering) do not re-read and re-parse the saved file.
    pub fn import_file_with_doc(&self, path: &Path, project: &str, folder: Option<&str>) -> Result<(FileEntry, Document)> {
        self.import_file_with_doc_counted(path, project, folder).map(|(e, d, _)| (e, d))
    }

    /// Like [`Self::import_file_with_doc`], also returning how many non-finite numbers were
    /// replaced by their neutral value on read or write.
    pub fn import_file_with_doc_counted(&self, path: &Path, project: &str, folder: Option<&str>) -> Result<(FileEntry, Document, usize)> {
        let (mut doc, replaced_on_read) = load_external_counted(path)?;
        let stem = path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty());
        let name = if !doc.name.is_empty() { doc.name.clone() } else { stem.unwrap_or("Imported").to_string() };
        let (entry, replaced_on_write) = self.create_file_counted(project, folder, &name, &doc)?;
        let total_replaced = replaced_on_read + replaced_on_write;
        if replaced_on_write == 0 {
            return Ok((entry, doc, total_replaced));
        }
        // The import produced NaN or infinity and the stored file holds neutral values instead.
        // Sanitise only the affected items in memory so the editor and the file agree without
        // re-reading or re-parsing the whole stored file.
        eprintln!("designos-library: imported \"{name}\" with {replaced_on_write} non-finite numbers replaced by their neutral value");
        sanitize_document_in_memory(&mut doc)?;
        Ok((entry, doc, total_replaced))
    }

    // ------------------------------------------------------------ recents / search

    /// Most recently opened files first.
    pub fn recents(&self, limit: usize) -> Vec<FileEntry> {
        let mut v = self.all_files();
        v.sort_by(|a, b| b.opened.cmp(&a.opened).then_with(|| b.modified.cmp(&a.modified)));
        v.truncate(limit);
        v
    }

    pub fn starred(&self) -> Vec<FileEntry> {
        let mut v: Vec<FileEntry> = self.all_files().into_iter().filter(|f| f.starred).collect();
        v.sort_by_key(|f| std::cmp::Reverse(f.opened));
        v
    }

    /// Case-insensitive substring match over file names, most recently modified first.
    pub fn search(&self, query: &str) -> Vec<FileEntry> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let mut v: Vec<FileEntry> = self.all_files().into_iter().filter(|f| f.name.to_lowercase().contains(&q)).collect();
        v.sort_by_key(|f| std::cmp::Reverse(f.modified));
        v
    }

    // ------------------------------------------------------------ trash

    /// Moves a file to the trash.
    pub fn trash_file(&self, id: &str) -> Result<()> {
        let _guard = write_lock();
        let (mut m, i) = self.find_file(id)?;
        let entry = m.files[i].clone();
        let dir = self.trash_dir(id);
        std::fs::create_dir_all(&dir)?;
        let item = TrashItem {
            id: id.to_string(),
            kind: TrashKind::File,
            name: entry.name.clone(),
            trashed: now_ms(),
            project: Some(m.project.id.clone()),
            project_name: Some(m.project.name.clone()),
            entry: Some(entry.clone()),
        };
        write_atomic(&dir.join(TRASH_MANIFEST), serde_json::to_string_pretty(&item)?.as_bytes())?;
        std::fs::rename(self.file_path(&m.project.id, id), dir.join("file.designos"))?;
        if let Some(t) = &entry.thumbnail {
            let _ = std::fs::rename(self.path(t), dir.join("thumb.png"));
        }
        let branches_dir = self.path("branches").join(id);
        if branches_dir.exists() {
            let _ = std::fs::remove_dir_all(dir.join("branches"));
            let _ = std::fs::rename(branches_dir, dir.join("branches"));
        }
        m.files.remove(i);
        m.project.modified = now_ms();
        self.save_manifest(&m)
    }

    /// Everything in the trash, most recently trashed first.
    pub fn list_trash(&self) -> Vec<TrashItem> {
        let Ok(rd) = std::fs::read_dir(self.path("trash")) else { return vec![] };
        let mut v: Vec<TrashItem> =
            rd.flatten().filter_map(|e| std::fs::read_to_string(e.path().join(TRASH_MANIFEST)).ok()).filter_map(|t| serde_json::from_str(&t).ok()).collect();
        v.sort_by_key(|t| std::cmp::Reverse(t.trashed));
        v
    }

    /// Restores a trashed file into its original project (and folder, if it still exists).
    pub fn restore_file(&self, id: &str) -> Result<FileEntry> {
        let _guard = write_lock();
        let dir = self.trash_dir(id);
        let item: TrashItem =
            serde_json::from_str(&std::fs::read_to_string(dir.join(TRASH_MANIFEST)).map_err(|_| LibraryError::NotFound(format!("trashed file {id}")))?)?;
        let (Some(project), Some(mut entry)) = (item.project.clone(), item.entry.clone()) else {
            return Err(LibraryError::Invalid("Not a trashed file".into()));
        };
        let mut m = self.load_manifest(&project).map_err(|_| {
            LibraryError::Invalid(format!("Its project \"{}\" is not available; restore the project first", item.project_name.unwrap_or_default()))
        })?;
        if entry.folder.as_ref().is_some_and(|f| !m.project.folders.iter().any(|x| &x.id == f)) {
            entry.folder = None;
        }
        if let Some(parent) = self.file_path(&project, id).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(dir.join("file.designos"), self.file_path(&project, id))?;
        entry.thumbnail = None;
        if dir.join("thumb.png").exists() {
            let rel = Self::thumb_rel(&project, id);
            if let Some(parent) = self.path(&rel).parent() {
                std::fs::create_dir_all(parent)?;
            }
            if std::fs::rename(dir.join("thumb.png"), self.path(&rel)).is_ok() {
                entry.thumbnail = Some(rel);
            }
        }
        if dir.join("branches").exists() {
            let branches_root = self.path("branches");
            std::fs::create_dir_all(&branches_root)?;
            let dst_branches = branches_root.join(id);
            let _ = std::fs::remove_dir_all(&dst_branches);
            std::fs::rename(dir.join("branches"), dst_branches)?;
        }
        m.files.push(entry.clone());
        m.project.modified = now_ms();
        self.save_manifest(&m)?;
        let _ = std::fs::remove_dir_all(dir);
        Ok(entry)
    }

    pub fn restore_project(&self, id: &str) -> Result<Project> {
        let dir = self.trash_dir(id);
        if !dir.join("project").exists() {
            return Err(LibraryError::NotFound(format!("trashed project {id}")));
        }
        if self.project_dir(id).exists() {
            return Err(LibraryError::Invalid("A project with this id already exists".into()));
        }
        std::fs::create_dir_all(self.path("projects"))?;
        std::fs::rename(dir.join("project"), self.project_dir(id))?;
        let _ = std::fs::remove_dir_all(dir);
        self.project(id)
    }

    /// Restores whatever the trash item `id` is.
    pub fn restore(&self, id: &str) -> Result<()> {
        match self.list_trash().into_iter().find(|t| t.id == id) {
            Some(t) if t.kind == TrashKind::Project => self.restore_project(id).map(|_| ()),
            Some(_) => self.restore_file(id).map(|_| ()),
            None => Err(LibraryError::NotFound(format!("trash item {id}"))),
        }
    }

    /// Permanently deletes everything in the trash. This is the only hard delete in the library.
    pub fn empty_trash(&self) -> Result<()> {
        let dir = self.path("trash");
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------ first run

    /// On the very first launch creates the "Getting started" project holding `demo()`.
    /// Returns the created file; later launches (even after the project was
    /// deleted) do nothing.
    pub fn seed_first_run(&self, demo: impl FnOnce() -> Document) -> Result<Option<FileEntry>> {
        let marker = self.path(SEED_MARKER);
        if marker.exists() || !self.list_projects().is_empty() {
            return Ok(None);
        }
        let project = self.create_project(SEED_PROJECT)?;
        let mut doc = demo();
        doc.name = "Welcome".into();
        let entry = self.create_file(&project.id, None, "Welcome", &doc)?;
        write_atomic(&marker, b"1")?;
        Ok(Some(entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect};

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> (Self, Library) {
            let p = std::env::temp_dir().join(format!("designos_lib_{}_{}", std::process::id(), new_id()));
            let lib = Library::open(&p).unwrap();
            (Tmp(p), lib)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn doc(name: &str, rects: usize) -> Document {
        let mut d = Document::new(name);
        let page = d.add_page("P");
        for i in 0..rects {
            let id = d.alloc_id();
            d.insert(page, Node::rectangle(id, Rect::new(i as f32, 0.0, 10.0, 10.0))).unwrap();
        }
        d
    }

    #[test]
    fn project_crud_and_trash() {
        let (_t, lib) = Tmp::new();
        let a = lib.create_project("  Alpha ").unwrap();
        assert_eq!(a.name, "Alpha");
        let b = lib.create_project("Beta").unwrap();
        assert_eq!(lib.list_projects().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alpha", "Beta"]);
        lib.rename_project(&a.id, "Alpha 2").unwrap();
        assert_eq!(lib.project(&a.id).unwrap().name, "Alpha 2");
        assert!(lib.create_project("   ").is_err());
        let f = lib.create_file(&b.id, None, "Keep", &doc("Keep", 1)).unwrap();
        lib.delete_project(&b.id).unwrap();
        assert_eq!(lib.list_projects().len(), 1);
        assert!(lib.load_file(&f.id).is_err());
        let trash = lib.list_trash();
        assert_eq!(trash.len(), 1);
        assert_eq!((trash[0].kind.clone(), trash[0].name.as_str()), (TrashKind::Project, "Beta"));
        lib.restore(&b.id).unwrap();
        assert_eq!(lib.list_projects().len(), 2);
        assert_eq!(lib.load_file(&f.id).unwrap().nodes.len(), 2);
        assert!(lib.list_trash().is_empty());
    }

    #[test]
    fn folders_nest_rename_and_refuse_cycles() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let a = lib.create_folder(&p.id, None, "A").unwrap();
        let b = lib.create_folder(&p.id, Some(&a.id), "B").unwrap();
        let c = lib.create_folder(&p.id, Some(&b.id), "C").unwrap();
        lib.rename_folder(&p.id, &a.id, "Alpha").unwrap();
        let folders = lib.project(&p.id).unwrap().folders;
        assert_eq!(folders.iter().find(|f| f.id == a.id).unwrap().name, "Alpha");
        assert_eq!(folders.iter().find(|f| f.id == c.id).unwrap().parent.as_deref(), Some(b.id.as_str()));
        assert!(lib.move_folder(&p.id, &a.id, Some(&c.id)).is_err(), "into own subtree");
        assert!(lib.move_folder(&p.id, &a.id, Some(&a.id)).is_err(), "into itself");
        lib.move_folder(&p.id, &c.id, None).unwrap();
        assert_eq!(lib.project(&p.id).unwrap().folders.iter().find(|f| f.id == c.id).unwrap().parent, None);
        assert!(lib.create_folder(&p.id, Some("nope"), "X").is_err());
    }

    #[test]
    fn file_crud_roundtrip() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let fo = lib.create_folder(&p.id, None, "F").unwrap();
        let e = lib.create_file(&p.id, Some(&fo.id), "Home", &doc("Home", 2)).unwrap();
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 3);
        lib.save_file(&e.id, &doc("Home", 4)).unwrap();
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 5);
        assert!(lib.file(&e.id).unwrap().modified > e.modified);
        lib.rename_file(&e.id, "Landing").unwrap();
        assert_eq!(lib.file(&e.id).unwrap().name, "Landing");
        assert_eq!(lib.files_in(&p.id, Some(Some(&fo.id))).unwrap().len(), 1);
        assert_eq!(lib.files_in(&p.id, Some(None)).unwrap().len(), 0);
        let d = lib.duplicate_file(&e.id).unwrap();
        assert_ne!(d.id, e.id);
        assert_eq!(d.name, "Landing (copy)");
        assert_eq!(lib.load_file(&d.id).unwrap().nodes.len(), 5);
        // move to another project drops the folder and moves the data
        let q = lib.create_project("Q").unwrap();
        lib.move_file(&d.id, &q.id, None).unwrap();
        assert_eq!(lib.file(&d.id).unwrap().project, q.id);
        assert_eq!(lib.files_in(&q.id, None).unwrap().len(), 1);
        assert_eq!(lib.files_in(&p.id, None).unwrap().len(), 1);
        assert_eq!(lib.load_file(&d.id).unwrap().nodes.len(), 5);
        assert!(lib.move_file(&d.id, &q.id, Some("missing")).is_err());
        assert!(lib.load_file("missing").is_err());
    }

    #[test]
    fn trash_restore_and_empty() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let fo = lib.create_folder(&p.id, None, "F").unwrap();
        let e = lib.create_file(&p.id, Some(&fo.id), "Gone", &doc("Gone", 1)).unwrap();
        lib.set_thumbnail(&e.id, b"png").unwrap();
        lib.trash_file(&e.id).unwrap();
        assert!(lib.all_files().is_empty());
        assert!(lib.search("gone").is_empty());
        let t = lib.list_trash();
        assert_eq!((t.len(), t[0].kind.clone(), t[0].name.as_str()), (1, TrashKind::File, "Gone"));
        assert_eq!(t[0].project_name.as_deref(), Some("P"));
        let back = lib.restore_file(&e.id).unwrap();
        assert_eq!(back.folder.as_deref(), Some(fo.id.as_str()));
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 2);
        assert!(lib.thumbnail_path(&lib.file(&e.id).unwrap()).is_some(), "thumbnail travels with the file");
        assert!(lib.list_trash().is_empty());
        // trash again, then empty: the only hard delete
        lib.trash_file(&e.id).unwrap();
        assert!(lib.trash_dir(&e.id).join("file.designos").exists(), "trashing never deletes");
        lib.empty_trash().unwrap();
        assert!(lib.list_trash().is_empty());
        assert!(lib.restore_file(&e.id).is_err());
        assert!(lib.all_files().is_empty());
        lib.empty_trash().unwrap();
    }

    #[test]
    fn restoring_a_file_of_a_trashed_project_asks_for_the_project_first() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let e = lib.create_file(&p.id, None, "F", &doc("F", 0)).unwrap();
        lib.trash_file(&e.id).unwrap();
        lib.delete_project(&p.id).unwrap();
        assert!(lib.restore_file(&e.id).is_err());
        lib.restore_project(&p.id).unwrap();
        lib.restore_file(&e.id).unwrap();
        assert_eq!(lib.all_files().len(), 1);
    }

    #[test]
    fn atomic_save_leaves_no_temp_file_and_survives_a_stale_one() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let e = lib.create_file(&p.id, None, "A", &doc("A", 1)).unwrap();
        let path = lib.file_path(&p.id, &e.id);
        // a leftover temp file from a crashed write must not corrupt the real one
        std::fs::write(path.with_extension("designos.tmp"), "garbage").unwrap();
        lib.save_file(&e.id, &doc("A", 3)).unwrap();
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 4);
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec![format!("{}.designos", e.id)]);
    }

    #[test]
    fn search_and_recents_order() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let a = lib.create_file(&p.id, None, "Login screen", &doc("a", 0)).unwrap();
        let b = lib.create_file(&p.id, None, "Dashboard", &doc("b", 0)).unwrap();
        let c = lib.create_file(&p.id, None, "Login flow", &doc("c", 0)).unwrap();
        assert_eq!(lib.recents(10).iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), [&c.id, &b.id, &a.id]);
        lib.mark_opened(&a.id).unwrap();
        assert_eq!(lib.recents(2).iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), [&a.id, &c.id]);
        lib.save_file(&b.id, &doc("b", 1)).unwrap();
        let hits = lib.search("LOGIN");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|f| f.name.contains("Login")));
        assert!(lib.search("").is_empty() && lib.search("zzz").is_empty());
        lib.star(&b.id, true).unwrap();
        assert_eq!(lib.starred().iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), [&b.id]);
        lib.star(&b.id, false).unwrap();
        assert!(lib.starred().is_empty());
    }

    #[test]
    fn thumbnails_are_stored_and_follow_moves() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let q = lib.create_project("Q").unwrap();
        let e = lib.create_file(&p.id, None, "A", &doc("A", 0)).unwrap();
        assert!(lib.thumbnail_path(&e).is_none());
        lib.set_thumbnail(&e.id, &[1, 2, 3]).unwrap();
        let e2 = lib.file(&e.id).unwrap();
        assert_eq!(std::fs::read(lib.thumbnail_path(&e2).unwrap()).unwrap(), vec![1, 2, 3]);
        let dup = lib.duplicate_file(&e.id).unwrap();
        assert!(lib.thumbnail_path(&dup).is_some());
        lib.move_file(&e.id, &q.id, None).unwrap();
        let moved = lib.file(&e.id).unwrap();
        assert_eq!(std::fs::read(lib.thumbnail_path(&moved).unwrap()).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn import_native_json_and_svg() {
        let (t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let src = t.0.join("incoming.designos");
        std::fs::write(&src, doc("From disk", 2).to_json().unwrap()).unwrap();
        let e = lib.import_file(&src, &p.id, None).unwrap();
        assert_eq!(e.name, "From disk");
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 3);
        let svg = t.0.join("icon.svg");
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#f00"/></svg>"##).unwrap();
        let s = lib.import_file(&svg, &p.id, None).unwrap();
        assert_eq!(s.name, "icon");
        assert_eq!(lib.load_file(&s.id).unwrap().pages.len(), 1);
        let bad = t.0.join("bad.designos");
        std::fs::write(&bad, "nope").unwrap();
        assert!(lib.import_file(&bad, &p.id, None).is_err());
        assert_eq!(lib.all_files().len(), 2, "failed imports leave nothing behind");
    }

    #[test]
    fn first_run_seeds_once() {
        let (_t, lib) = Tmp::new();
        let e = lib.seed_first_run(|| doc("demo", 1)).unwrap().unwrap();
        let projects = lib.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, SEED_PROJECT);
        assert_eq!(lib.load_file(&e.id).unwrap().nodes.len(), 2);
        assert!(lib.seed_first_run(|| doc("demo", 1)).unwrap().is_none());
        lib.delete_project(&projects[0].id).unwrap();
        assert!(lib.seed_first_run(|| doc("demo", 1)).unwrap().is_none(), "a deleted seed does not come back");
    }

    #[test]
    fn default_root_honours_the_env_override() {
        // Only this test touches the variable.
        std::env::set_var("DESIGNOS_LIBRARY", "/tmp/somewhere-else");
        assert_eq!(Library::default_root(), PathBuf::from("/tmp/somewhere-else"));
        // The old name still works, but the new one wins.
        std::env::set_var("DESIGNOS_LIBRARY_DIR", "/tmp/old-name");
        assert_eq!(Library::default_root(), PathBuf::from("/tmp/somewhere-else"));
        std::env::remove_var("DESIGNOS_LIBRARY");
        assert_eq!(Library::default_root(), PathBuf::from("/tmp/old-name"));
        std::env::remove_var("DESIGNOS_LIBRARY_DIR");
        assert!(Library::default_root().ends_with("designos/library") || Library::default_root().ends_with("designos\\library"));
    }

    #[test]
    fn times_are_strictly_increasing() {
        let a = now_ms();
        let b = now_ms();
        assert!(b > a);
    }

    #[test]
    fn sanitize_in_memory_touches_only_affected_entries() {
        let mut d = Document::new("mixed");
        let page = d.add_page("P");
        let clean_id = d.alloc_id();
        let mut clean = Node::rectangle(clean_id, Rect::new(1.0, 2.0, 30.0, 40.0));
        clean.name = "clean-rectangle-on-heap".to_string();
        let clean_id = d.insert(page, clean).unwrap();

        let bad_id = d.alloc_id();
        let mut bad = Node::rectangle(bad_id, Rect::new(5.0, 6.0, 10.0, 20.0));
        bad.size.x = f32::NAN;
        bad.opacity = f32::NAN;
        let bad_id = d.insert(page, bad).unwrap();

        let clean_name_ptr = d.get(clean_id).unwrap().name.as_ptr();
        let (stored_json, replaced) = d.to_json_counted().unwrap();
        assert_eq!(replaced, 2);

        sanitize_document_in_memory(&mut d).unwrap();

        assert_eq!(d.get(clean_id).unwrap().name.as_ptr(), clean_name_ptr, "unaffected nodes are not re-serialized and re-parsed");
        let sanitized_bad = d.get(bad_id).unwrap();
        assert_eq!((sanitized_bad.size.x, sanitized_bad.opacity), (0.0, 1.0));
        assert_eq!(d.to_json().unwrap(), Document::from_json(&stored_json).unwrap().to_json().unwrap());
    }

    #[test]
    fn rollback_unlisted_file_keeps_stored_file_when_manifest_already_lists_it() {
        let (_t, lib) = Tmp::new();
        let p = lib.create_project("P").unwrap();
        let listed = lib.create_file(&p.id, None, "Listed", &doc("Listed", 1)).unwrap();
        let listed_path = lib.file_path(&p.id, &listed.id);
        assert!(listed_path.exists());

        // Simulates a post-rename `sync_dir` error: the on-disk manifest already contains `listed.id`.
        lib.rollback_unlisted_file(&p.id, &listed.id, &listed_path);
        assert!(listed_path.exists(), "must not unlink a file that the on-disk manifest already lists");

        // When the on-disk manifest does not contain the id, the orphan file is unlinked.
        let orphan_path = lib.file_path(&p.id, "unlisted-id");
        std::fs::write(&orphan_path, "{}").unwrap();
        lib.rollback_unlisted_file(&p.id, "unlisted-id", &orphan_path);
        assert!(!orphan_path.exists(), "must unlink an orphan file not listed in the on-disk manifest");
    }
}
