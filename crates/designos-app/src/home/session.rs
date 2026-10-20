//! The editor's link to the library: which library file is open, saving,
//! debounced autosave and thumbnails.

use super::{thumb, Home, Section};
use crate::app::App;
use designos_doc::{Document, DocumentKind};
use designos_library::Library;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Seconds without a document change before a library file is autosaved.
pub const AUTOSAVE_IDLE: f64 = 1.5;
/// Seconds of calm after which a revision change is compared with the saved document.
const PROBE_DELAY: f64 = 0.3;

/// The library file currently open in the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct LibFile {
    pub id: String,
    pub project_id: String,
    pub project: String,
    pub name: String,
}

struct DiskWatcherInner {
    state: std::sync::Mutex<(Option<PathBuf>, Option<std::time::SystemTime>, bool)>,
    cvar: std::sync::Condvar,
    dirty: std::sync::atomic::AtomicBool,
}

pub struct DiskWatcher {
    inner: std::sync::Arc<DiskWatcherInner>,
}

impl Drop for DiskWatcher {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.inner.state.lock() {
            guard.2 = true;
            self.inner.cvar.notify_all();
        }
    }
}

#[derive(Default)]
pub struct Autosave {
    /// Last canvas revision seen (a change bumps it).
    pub seen_rev: u64,
    /// When the document last changed, while a save is pending.
    pub changed_at: Option<f64>,
    /// The pending change was checked against `saved_hash` and really differs (view-only
    /// changes such as zooming bump the revision too, and must not show "Saving").
    pub confirmed: bool,
    /// Hash of the document as last written to (or read from) the library.
    pub saved_hash: u64,
    /// Number of autosaves performed (tests).
    pub saves: u32,
    /// Last observed on-disk modification timestamp of the active file.
    pub disk_mtime: Option<std::time::SystemTime>,
    /// Last `self.now` timestamp when the disk mtime was polled.
    pub last_mtime_poll: f64,
    /// When the last save of unsaved edits failed. Those edits stay in the editor and the next frame
    /// after `AUTOSAVE_IDLE` tries again; no timer is scheduled for it.
    pub save_failed_at: Option<f64>,
    /// Background thread watching `watched_disk_path()` off the UI thread.
    watcher: Option<DiskWatcher>,
}

impl Autosave {
    /// An edit is waiting for the idle delay before it is saved.
    pub fn pending(&self) -> bool {
        self.changed_at.is_some() && self.confirmed
    }

    fn note_known_mtime(&self, mtime: Option<std::time::SystemTime>) {
        if let Some(w) = &self.watcher {
            if let Ok(mut guard) = w.inner.state.lock() {
                guard.1 = mtime;
            }
        }
    }

    fn take_watcher_dirty(&self) -> bool {
        self.watcher.as_ref().is_some_and(|w| w.inner.dirty.swap(false, std::sync::atomic::Ordering::AcqRel))
    }

    /// Whether the watcher saw a write that no frame has looked at yet.
    fn watcher_dirty(&self) -> bool {
        self.watcher.as_ref().is_some_and(|w| w.inner.dirty.load(std::sync::atomic::Ordering::Acquire))
    }

    fn sync_watcher(&mut self, ctx: &egui::Context, path: Option<PathBuf>, mtime: Option<std::time::SystemTime>) {
        if path.is_none() {
            if let Some(w) = &self.watcher {
                if let Ok(mut guard) = w.inner.state.lock() {
                    guard.0 = None;
                    guard.1 = None;
                }
            }
            return;
        }
        if let Some(w) = &self.watcher {
            if let Ok(mut guard) = w.inner.state.lock() {
                if guard.0 != path || (mtime.is_some() && guard.1.is_none()) {
                    guard.0 = path;
                    if mtime.is_some() {
                        guard.1 = mtime;
                    }
                }
            }
            return;
        }
        let inner = std::sync::Arc::new(DiskWatcherInner {
            state: std::sync::Mutex::new((path, mtime, false)),
            cvar: std::sync::Condvar::new(),
            dirty: std::sync::atomic::AtomicBool::new(false),
        });
        let inner_bg = inner.clone();
        let ctx_bg = ctx.clone();
        let _ = std::thread::Builder::new().name("designos-disk-watch".into()).spawn(move || loop {
            let (watched_path, known_mtime) = {
                let Ok(guard) = inner_bg.state.lock() else { break };
                let Ok((guard, _)) = inner_bg.cvar.wait_timeout(guard, std::time::Duration::from_millis(500)) else { break };
                if guard.2 {
                    break;
                }
                (guard.0.clone(), guard.1)
            };
            let Some(p) = watched_path else { continue };
            let current_mtime = std::fs::metadata(&p).ok().and_then(|m| m.modified().ok());
            if current_mtime.is_some() && current_mtime != known_mtime {
                if let Ok(mut guard) = inner_bg.state.lock() {
                    if guard.0.as_ref() == Some(&p) && guard.1 == known_mtime {
                        guard.1 = current_mtime;
                        if known_mtime.is_some() {
                            inner_bg.dirty.store(true, std::sync::atomic::Ordering::Release);
                            ctx_bg.request_repaint();
                        }
                    }
                }
            }
        });
        self.watcher = Some(DiskWatcher { inner });
    }
}

#[derive(Debug)]
struct HashError;

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("hash error")
    }
}

impl std::error::Error for HashError {}

impl serde::ser::Error for HashError {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        Self
    }
}

struct HashSerializer<'a>(&'a mut std::collections::hash_map::DefaultHasher);

impl<'a> serde::Serializer for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i128(self, v: i128) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u128(self, v: u128) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        v.to_bits().hash(self.0);
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        v.to_bits().hash(self.0);
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Self::Error> {
        0u8.hash(self.0);
        Ok(())
    }
    fn serialize_some<T: ?Sized + serde::Serialize>(self, value: &T) -> Result<(), Self::Error> {
        1u8.hash(self.0);
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Self::Error> {
        name.hash(self.0);
        Ok(())
    }
    fn serialize_unit_variant(self, _name: &'static str, variant_index: u32, variant: &'static str) -> Result<(), Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(self, _name: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(self)
    }
}

impl<'a> serde::ser::SerializeSeq for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTuple for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTupleStruct for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTupleVariant for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeMap for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(HashSerializer(self.0))
    }
    fn serialize_value<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeStruct for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeStructVariant for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(HashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) fn doc_hash(doc: &Document) -> u64 {
    if doc.nodes.len() > 5_000 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (doc.runtime_id(), doc.mutation_epoch(), doc.nodes.len(), doc.pages.len(), &doc.name).hash(&mut h);
        return h.finish();
    }
    use serde::Serialize;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let _ = (
        &doc.name,
        &doc.kind,
        &doc.nodes,
        &doc.pages,
        &doc.styles,
        &doc.style_meta,
        &doc.flows,
        &doc.variables,
        &doc.variable_collections,
        &doc.guides,
        &doc.prototype_device,
        &doc.comments,
        &doc.library,
        &doc.library_page,
    )
        .serialize(HashSerializer(&mut h));
    let _ = doc.check_designs.serialize(HashSerializer(&mut h));
    doc.images.len().hash(&mut h);
    for (k, v) in &doc.images {
        (k, v.len(), &v[..v.len().min(64)]).hash(&mut h);
    }
    h.finish()
}

impl App {
    /// Opens (or creates) the library at `root` and seeds it on first run.
    pub fn attach_library(&mut self, root: PathBuf) {
        // Shared libraries use the same root as the projects library.
        self.libs.set_root(root.clone());
        self.terminal.library_root = Some(root.clone());
        for entry in &mut self.tabs.entries {
            entry.terminal.library_root = Some(root.clone());
        }
        match Library::open(&root) {
            Ok(lib) => {
                match lib.seed_first_run(crate::demo::demo_document) {
                    // The sample file gets its thumbnail right away so the home screen is not blank.
                    Ok(Some(entry)) => {
                        if let Ok(doc) = lib.load_file(&entry.id) {
                            thumb::refresh(&lib, &entry.id, &doc);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => self.show_toast(format!("Could not create the sample project: {e}"), true),
                }
                self.lib = Some(lib);
            }
            Err(e) => self.show_toast(format!("Cannot open the library at {}: {e}", root.display()), true),
        }
    }

    /// Shows the home screen ("Back to files"), saving pending edits first.
    pub fn go_home(&mut self) {
        let Some(lib) = self.lib.clone() else {
            self.show_toast("The projects library is unavailable", true);
            return;
        };
        crate::decks::ui::flush_notes(self);
        self.save_close_version();
        self.stop_plugin();
        self.flush_autosave();
        if !self.tabs.entries.is_empty() || self.lib_file.is_some() || !self.image_jobs.jobs.is_empty() {
            let mut tabs = std::mem::take(&mut self.tabs);
            tabs.park_active_from_app(self);
            tabs.parked_in_home = true;
            self.tabs = tabs;
        }
        self.canvas.reset_interaction();
        let mut home = Home::new();
        home.refresh(&lib);
        self.home = Some(home);
        self.ui.menu = None;
        self.ui.ctx_menu = None;
    }

    /// Disk path of a library file; also the key of its cached pre-repair bytes.
    pub(crate) fn lib_file_disk_path(&self, lf: &LibFile) -> Option<PathBuf> {
        self.lib.as_ref().map(|lib| lib.path("projects").join(&lf.project_id).join("files").join(format!("{}.designos", lf.id)))
    }

    /// Disk path of the currently open document (library file or external path) for live hot-reload polling.
    fn watched_disk_path(&self) -> Option<PathBuf> {
        if let (Some(lib), Some(lf)) = (&self.lib, &self.lib_file) {
            Some(lib.path("projects").join(&lf.project_id).join("files").join(format!("{}.designos", lf.id)))
        } else {
            self.file_path.clone()
        }
    }

    /// Opens a library file in the editor.
    pub fn open_library_file(&mut self, id: &str) {
        let Some(lib) = self.lib.clone() else { return };
        let entry = match lib.file(id) {
            Ok(e) => e,
            Err(e) => {
                self.show_toast(format!("Cannot open the file: {e}"), true);
                return;
            }
        };
        let needs_thumb = lib.thumbnail_path(&entry).is_none();
        if self.home.is_some() || self.tabs.parked_in_home {
            if let Some(existing) = self.tabs.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|lf| lf.id == id) && !t.doc.nodes.is_empty()) {
                let mut tabs = std::mem::take(&mut self.tabs);
                tabs.switch_to(existing, self);
                self.tabs = tabs;
                self.autosave = Autosave { seen_rev: self.canvas.tiles.rev(), saved_hash: doc_hash(&self.doc), ..Default::default() };
                self.autosave.disk_mtime = self.watched_disk_path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
                self.history_open_file();
                let _ = lib.mark_opened(id);
                if needs_thumb && self.doc.nodes.len() <= 25_000 {
                    thumb::refresh_in_background(lib, entry.id, self.export_doc().into_owned());
                }
                return;
            }
        }
        let disk_path = lib.path("projects").join(&entry.project).join("files").join(format!("{}.designos", entry.id));
        let (mut doc, warning) = match crate::io::load_path_with_warning(&disk_path) {
            Ok(x) => x,
            Err(e) => {
                self.show_toast(format!("Cannot open the file: {e}"), true);
                return;
            }
        };
        let project = lib.project(&entry.project).map(|p| p.name).unwrap_or_default();
        doc.name = entry.name.clone();
        self.exit_version_view();
        self.flush_autosave();
        self.release_active_repaired_original(Some(&disk_path));
        self.replace_document(doc, None);
        if let Some(w) = warning.as_deref() {
            self.show_toast(crate::io::opened_toast_text(&entry.name, Some(w)), true);
        }
        self.lib_file = Some(LibFile { id: entry.id.clone(), project_id: entry.project.clone(), project, name: entry.name.clone() });
        let window_id = self.tabs.active_session_id();
        self.terminal.bind_library_file(lib.root(), &entry.id, &window_id);
        self.autosave = Autosave { seen_rev: self.canvas.tiles.rev(), saved_hash: doc_hash(&self.doc), ..Default::default() };
        self.autosave.disk_mtime = self.watched_disk_path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
        self.history_open_file();
        let _ = lib.mark_opened(id);
        self.home = None;
        self.ui.menu = None;
        if self.tabs.parked_in_home && !self.tabs.entries.is_empty() {
            let mut tabs = std::mem::take(&mut self.tabs);
            tabs.reconcile_frame(self);
            self.tabs = tabs;
        }
        if needs_thumb && self.doc.nodes.len() <= 25_000 {
            thumb::refresh_in_background(lib, entry.id, self.export_doc().into_owned());
        }
    }

    /// Creates an empty design file in `project` (or the first / a new project) and opens it.
    pub fn new_library_file(&mut self, target: Option<(String, Option<String>)>) {
        self.new_library_file_of(target, DocumentKind::Design);
    }

    /// Creates an empty board in `project` (or the first / a new project) and opens it.
    pub fn new_library_board(&mut self, target: Option<(String, Option<String>)>) {
        self.new_library_file_of(target, DocumentKind::Board);
    }

    pub fn new_library_deck(&mut self, target: Option<(String, Option<String>)>) {
        self.new_library_file_of(target, DocumentKind::Deck);
    }

    pub fn new_deck_here(&mut self) {
        let target = self.lib_file.as_ref().map(|f| (f.project_id.clone(), None));
        self.new_library_deck(target);
    }

    fn new_library_file_of(&mut self, target: Option<(String, Option<String>)>, kind: DocumentKind) {
        let Some(lib) = self.lib.clone() else {
            if kind == DocumentKind::Board {
                self.new_board_document();
            } else if kind == DocumentKind::Deck {
                self.new_deck_document();
            } else {
                self.new_document();
            }
            return;
        };
        let result = (|| {
            let (project, folder) = match target {
                Some(t) => t,
                None => match lib.list_projects().first() {
                    Some(p) => (p.id.clone(), None),
                    None => (lib.create_project("My project")?.id, None),
                },
            };
            let doc = match kind {
                DocumentKind::Board => crate::board::create::new_document("Untitled"),
                DocumentKind::Deck => crate::decks::new_document("Untitled deck"),
                DocumentKind::Design => {
                    let mut doc = Document::new("Untitled");
                    doc.add_page("Page 1");
                    doc
                }
            };
            lib.create_file(&project, folder.as_deref(), "Untitled", &doc)
        })();
        match result {
            Ok(e) => self.open_library_file(&e.id),
            Err(e) => self.show_toast(format!("Cannot create the file: {e}"), true),
        }
    }

    /// File → New file: next to the open library file when there is one.
    pub fn new_file_here(&mut self) {
        let target = self.lib_file.as_ref().map(|f| (f.project_id.clone(), None));
        self.new_library_file(target);
    }

    /// File → New board: next to the open library file when there is one.
    pub fn new_board_here(&mut self) {
        let target = self.lib_file.as_ref().map(|f| (f.project_id.clone(), None));
        self.new_library_board(target);
    }

    /// Writes `doc` to `lf` in `lib` with repair-backup preservation and non-finite number neutralization.
    pub(crate) fn save_doc_to_library(
        lib: &Library,
        lf: &LibFile,
        doc: &mut Document,
        hist: &mut designos_doc::History,
    ) -> Result<(Option<String>, bool), String> {
        let disk_path = lib.path("projects").join(&lf.project_id).join("files").join(format!("{}.designos", lf.id));
        let wrote_backup = crate::io::write_pending_repair_backup(&disk_path)?;
        let replaced = lib.save_file_counted(&lf.id, doc).map_err(|e| format!("Cannot save: {e}"))?;
        if wrote_backup {
            crate::io::clear_pending_repair_backup(&disk_path);
        }
        let neutralized = replaced > 0;
        if neutralized {
            doc.neutralize_non_finite();
            hist.neutralize_non_finite();
        }
        Ok((crate::io::non_finite_notice_once(&disk_path, replaced), neutralized))
    }

    /// Writes the open library file; also refreshes its thumbnail. There is no "Saved" toast, but a save
    /// that replaced non-finite numbers says so, because no later open of the file would.
    pub fn save_library_file(&mut self) -> Result<(), String> {
        if let Some(notice) = self.save_library_file_noticing()? {
            self.show_toast(&notice, true);
            self.pending_flush_notice = Some((self.now, notice));
        }
        Ok(())
    }

    /// [`Self::save_library_file`] without any toast: returns the once-per-file notice for the
    /// non-finite numbers this save replaced, for the caller to show.
    fn save_library_file_noticing(&mut self) -> Result<Option<String>, String> {
        crate::decks::ui::flush_notes(self);
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else { return Err("This file is not in the library".into()) };
        if self.history.viewing.is_some() {
            // an old version is on screen: the file on disk must keep the live document
            return Err("Go back to the current version before saving".into());
        }
        self.settle_pending_relayout();
        let (notice, neutralized) = Self::save_doc_to_library(&lib, &lf, &mut self.doc, &mut self.hist)?;
        if neutralized {
            self.changed();
        }
        self.autosave.saved_hash = doc_hash(&self.doc);
        self.autosave.seen_rev = self.canvas.tiles.rev();
        self.autosave.changed_at = None;
        self.autosave.save_failed_at = None;
        self.autosave.disk_mtime = self.watched_disk_path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
        self.autosave.note_known_mtime(self.autosave.disk_mtime);
        if self.doc.nodes.len() <= 25_000 {
            thumb::refresh_in_background(lib, lf.id, self.export_doc().into_owned());
        }
        Ok(notice)
    }

    /// Ctrl+S on a library file.
    pub fn save_library_file_with_toast(&mut self) {
        match self.save_library_file_noticing() {
            Ok(notice) => {
                let project = self.lib_file.as_ref().map(|f| f.project.clone()).unwrap_or_default();
                match notice {
                    Some(w) => self.show_toast(format!("Saved to {project} ({w})"), true),
                    None => self.show_toast(format!("Saved to {project}"), false),
                }
            }
            Err(e) => self.show_toast(e, true),
        }
    }

    /// Saves pending edits right away (before leaving the editor or the app).
    pub fn flush_autosave(&mut self) {
        if self.history.viewing.is_some() {
            return;
        }
        self.settle_pending_relayout();
        if self.lib_file.is_some() && doc_hash(&self.doc) != self.autosave.saved_hash {
            if let Err(e) = self.save_library_file() {
                self.show_toast(&e, true);
                self.pending_flush_notice = Some((self.now, e));
            }
        }
        self.autosave.changed_at = None;
    }

    /// Called every frame: debounced autosave of the open library file.
    pub fn autosave_tick(&mut self, ctx: &egui::Context) {
        if self.home.is_some() || self.history.viewing.is_some() {
            self.autosave.changed_at = None;
            self.autosave.sync_watcher(ctx, None, None);
            return;
        }
        if self.lib_file.is_some() {
            let rev = self.canvas.tiles.rev();
            if rev != self.autosave.seen_rev {
                self.autosave.seen_rev = rev;
                self.autosave.changed_at = Some(self.now);
                self.autosave.confirmed = false;
            }
        } else {
            self.autosave.changed_at = None;
        }
        let watched_path = self.watched_disk_path();
        if self.autosave.disk_mtime.is_none() {
            if let Some(ref p) = watched_path {
                self.autosave.disk_mtime = std::fs::metadata(p).ok().and_then(|m| m.modified().ok());
                if self.autosave.saved_hash == 0 {
                    self.autosave.saved_hash = doc_hash(&self.doc);
                }
            }
        }
        self.autosave.sync_watcher(ctx, watched_path.clone(), self.autosave.disk_mtime);
        let busy = ctx.input(|i| i.pointer.any_down()) || self.canvas.text.is_some();
        self.reload_external_write(ctx, watched_path, busy);
        self.save_when_settled(ctx, busy);
        if !busy && self.autosave.changed_at.is_none() && self.autosave.watcher_dirty() {
            // A write was held back while an edit was under way and that edit is over: look at it on the next frame.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Loads the file again when it changed on disk (a write by an agent or another tool).
    /// Nothing is replaced while an edit is under way or unsaved edits exist; the write the watcher
    /// reported stays flagged for the frame after the edit is over.
    fn reload_external_write(&mut self, ctx: &egui::Context, watched_path: Option<PathBuf>, busy: bool) {
        if busy || self.autosave.changed_at.is_some() {
            return;
        }
        let watcher_woke = self.autosave.take_watcher_dirty();
        if !watcher_woke && self.now - self.autosave.last_mtime_poll < 0.5 {
            return;
        }
        self.autosave.last_mtime_poll = self.now;
        let Some(path) = watched_path else { return };
        let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        if mtime.is_none() || mtime == self.autosave.disk_mtime {
            return;
        }
        if self.lib_file.is_some() && doc_hash(&self.doc) != self.autosave.saved_hash {
            // Edits no save has written yet, as after a failed save: they stay. The modification time is not
            // taken over, so a later frame looks at the file again once those edits are saved or undone.
            return;
        }
        self.autosave.disk_mtime = mtime;
        self.autosave.note_known_mtime(mtime);
        // The same load as an open, so a damaged file reports what was repaired or replaced the same way.
        let Ok((mut new_doc, warning)) = crate::io::load_path_with_warning(&path) else { return };
        if let Some(lf) = &self.lib_file {
            new_doc.name = lf.name.clone();
        }
        let new_hash = doc_hash(&new_doc);
        if new_hash == self.autosave.saved_hash {
            if let Some(w) = warning {
                self.show_toast(format!("Synced external agent changes ({w})"), true);
                ctx.request_repaint();
            }
            return;
        }
        let old_page = self.page;
        let old_sel = self.selection.clone();
        self.doc = new_doc;
        if !self.doc.pages.contains(&old_page) {
            if let Some(&p) = self.doc.pages.first() {
                self.page = p;
            }
        }
        self.selection = old_sel.into_iter().filter(|id| self.doc.get(*id).is_some()).collect();
        self.changed();
        self.autosave.saved_hash = new_hash;
        self.autosave.seen_rev = self.canvas.tiles.rev();
        self.autosave.changed_at = None;
        match warning {
            Some(w) => self.show_toast(format!("Synced external agent changes ({w})"), true),
            None => self.show_toast("Synced external agent changes", false),
        }
        ctx.request_repaint();
    }

    /// Debounced autosave: probes a changed revision once it settled, then saves after `AUTOSAVE_IDLE`.
    fn save_when_settled(&mut self, ctx: &egui::Context, busy: bool) {
        let Some(t) = self.autosave.changed_at else {
            if let Some(failed_at) = self.autosave.save_failed_at {
                // The last save failed and the edits are still here. Retry on a frame that comes anyway: a timer
                // that keeps firing while the disk stays unwritable is not worth the wakeups.
                if !busy && self.now - failed_at >= AUTOSAVE_IDLE {
                    self.save_unsaved_edits();
                }
            }
            return;
        };
        if busy {
            self.autosave.changed_at = Some(self.now);
            self.autosave.confirmed = false;
        } else if !self.autosave.confirmed && self.now - t >= PROBE_DELAY {
            // Cheap-ish check once the revision settled: did the document really change?
            if doc_hash(&self.doc) == self.autosave.saved_hash {
                self.autosave.changed_at = None;
                return;
            }
            self.autosave.confirmed = true;
        } else if self.autosave.confirmed && self.now - t >= AUTOSAVE_IDLE {
            self.autosave.changed_at = None;
            self.save_unsaved_edits();
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    /// Writes the open library file if the document differs from what was last saved.
    fn save_unsaved_edits(&mut self) {
        if doc_hash(&self.doc) == self.autosave.saved_hash {
            self.autosave.save_failed_at = None;
            return;
        }
        match self.save_library_file() {
            Ok(()) => self.autosave.saves += 1,
            Err(e) => {
                self.autosave.save_failed_at = Some(self.now);
                self.show_toast(e, true);
            }
        }
    }

    /// Renames the open library file (header double-click).
    pub fn rename_library_file(&mut self, name: &str) {
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else { return };
        let name = name.trim();
        if name.is_empty() || name == lf.name {
            return;
        }
        match lib.rename_file(&lf.id, name) {
            Ok(()) => {
                if let Some(f) = self.lib_file.as_mut() {
                    f.name = name.to_string();
                }
                self.doc.name = name.to_string();
            }
            Err(e) => self.show_toast(format!("Cannot rename: {e}"), true),
        }
    }

    /// Stores the open (external or unsaved) document as a new library file and keeps editing it there.
    pub fn save_to_library_as(&mut self, project: &str, folder: Option<&str>, name: &str) {
        crate::decks::ui::flush_notes(self);
        let Some(lib) = self.lib.clone() else { return };
        self.settle_pending_relayout();
        let mut doc = self.doc.clone();
        doc.name = name.trim().to_string();
        match lib.create_file_counted(project, folder, name, &doc) {
            Ok((e, replaced)) => {
                if replaced > 0 {
                    self.doc.neutralize_non_finite();
                    self.hist.neutralize_non_finite();
                    self.changed();
                }
                self.doc.name = e.name.clone();
                let project_name = lib.project(project).map(|p| p.name).unwrap_or_default();
                let new_lf = LibFile { id: e.id.clone(), project_id: project.to_string(), project: project_name.clone(), name: e.name };
                let new_disk_path = self.lib_file_disk_path(&new_lf);
                self.release_active_repaired_original(new_disk_path.as_deref());
                self.lib_file = Some(new_lf);
                if self.terminal.tabs.is_empty() {
                    let window_id = self.tabs.active_session_id();
                    self.terminal.bind_library_file(lib.root(), &e.id, &window_id);
                }
                self.autosave = Autosave { seen_rev: self.canvas.tiles.rev(), saved_hash: doc_hash(&self.doc), ..Default::default() };
                let disk_path = self.watched_disk_path();
                self.autosave.disk_mtime = disk_path.as_ref().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
                self.history_open_file();
                thumb::refresh_in_background(lib, e.id, self.export_doc().into_owned());
                let notice = disk_path.as_deref().and_then(|p| crate::io::non_finite_notice_once(p, replaced));
                match notice {
                    Some(w) => self.show_toast(format!("Saved to {project_name} ({w})"), true),
                    None => self.show_toast(format!("Saved to {project_name}"), false),
                }
            }
            Err(e) => self.show_toast(format!("Cannot save to the library: {e}"), true),
        }
    }

    /// Imports files into a project; returns how many succeeded.
    pub fn import_into_library(&mut self, paths: &[PathBuf], project: &str, folder: Option<&str>) -> usize {
        let Some(lib) = self.lib.clone() else { return 0 };
        let mut ok = 0;
        let mut last_err = None;
        // One entry per imported file that had numbers replaced, so the toast can name every one of them.
        let mut notices: Vec<String> = Vec::new();
        for p in paths {
            let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            match lib.import_file_with_doc_counted(p, project, folder) {
                Ok((e, doc, replaced)) => {
                    ok += 1;
                    if let Some(w) = crate::io::non_finite_notice_once(p, replaced) {
                        notices.push(format!("{file_name}: {w}"));
                    }
                    thumb::refresh_in_background(lib.clone(), e.id, doc);
                }
                Err(e) => last_err = Some(format!("{file_name}: {e}")),
            }
        }
        let notice_suffix = if notices.is_empty() { String::new() } else { format!(" ({})", notices.join("; ")) };
        let has_notice = !notices.is_empty();
        match (ok, last_err) {
            (0, Some(e)) => self.show_toast(format!("Import failed. {e}"), true),
            (n, Some(e)) => self.show_toast(format!("Imported {n}, but {e}{notice_suffix}"), true),
            (n, None) if n > 0 => self.show_toast(format!("Imported {n} file{}{notice_suffix}", if n == 1 { "" } else { "s" }), has_notice),
            _ => {}
        }
        if let Some(h) = self.home.as_mut() {
            h.dirty = true;
        }
        ok
    }

    /// The section a "New design file" button creates into.
    pub fn home_target(&self) -> Option<(String, Option<String>)> {
        match &self.home.as_ref()?.section {
            Section::Project(p, f) => Some((p.clone(), f.clone())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_editor_with_open_file_requests_no_periodic_repaint() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_idle_repaint_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut doc = Document::new("Test");
        doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.attach_library(dir.clone());
        app.new_file_here();
        app.toast = None;
        app.now = 10.0;

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });
        let repaint_delay = out.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(repaint_delay, std::time::Duration::MAX, "idle editor with an unchanged open library file must not schedule a 500ms wakeup timer");
    }

    #[test]
    fn external_disk_write_wakes_idle_editor_and_hot_reloads_document() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_idle_hot_reload_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut doc = Document::new("Test");
        doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.attach_library(dir.clone());
        app.new_file_here();
        app.toast = None;
        app.now = 10.0;

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });
        assert_eq!(out.viewport_output[&egui::ViewportId::ROOT].repaint_delay, Duration::MAX);

        let wakeups = Arc::new(AtomicUsize::new(0));
        let wakeups_cb = wakeups.clone();
        ctx.set_request_repaint_callback(move |info| {
            if info.delay == Duration::ZERO {
                wakeups_cb.fetch_add(1, Ordering::SeqCst);
            }
        });

        // Wait 650ms with no disk write: must produce 0 wakeups.
        std::thread::sleep(Duration::from_millis(650));
        assert_eq!(wakeups.load(Ordering::SeqCst), 0, "untouched file must not wake the UI thread");

        // Now simulate an external MCP/CLI write to the watched file on disk.
        let path = app.watched_disk_path().expect("watched disk path");
        let mut ext_doc = app.doc.clone();
        let page = ext_doc.pages[0];
        let nid = ext_doc.alloc_id();
        ext_doc.insert(page, designos_doc::Node::frame(nid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
        // Ensure mtime advances even on coarse-grained filesystems.
        std::thread::sleep(Duration::from_millis(20));
        designos_library::write_atomic(&path, ext_doc.to_json().unwrap().as_bytes()).unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while wakeups.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(wakeups.load(Ordering::SeqCst) > 0, "external disk write must wake the idle editor via request_repaint");

        // Run one frame on wakeup: the editor hot-reloads the external document even if <0.5s of app.now elapsed.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert!(app.doc.get(nid).is_some(), "woken editor must hot-reload the external node");
        assert_eq!(app.toast.as_ref().map(|t| t.text.as_str()), Some("Synced external agent changes"));
    }

    /// A library file open in an idle editor whose disk watcher has settled on the file's modification time.
    struct IdleEditor {
        app: App,
        ctx: egui::Context,
        dir: PathBuf,
        /// Every repaint delay requested since the last `take_requests`.
        requests: std::sync::Arc<std::sync::Mutex<Vec<std::time::Duration>>>,
    }

    impl IdleEditor {
        fn open(tag: &str) -> Self {
            let mut dir = std::env::temp_dir();
            dir.push(format!("designos_{tag}_{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();

            let mut doc = Document::new("Test");
            doc.add_page("Page 1");
            let mut app = App::new(doc);
            app.attach_library(dir.clone());
            app.new_file_here();
            app.toast = None;
            app.now = 10.0;

            let ctx = egui::Context::default();
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
            // Baseline frame: starts the watcher on the freshly opened file.
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.autosave_tick(ui.ctx());
            });
            let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let requests_cb = requests.clone();
            ctx.set_request_repaint_callback(move |info| requests_cb.lock().unwrap().push(info.delay));
            Self { app, ctx, dir, requests }
        }

        /// One frame of `autosave_tick` with the given input events.
        fn tick(&mut self, events: Vec<egui::Event>) {
            let input = egui::RawInput { events, ..Default::default() };
            let _ = self.ctx.run_ui(input, |ui| {
                self.app.autosave_tick(ui.ctx());
            });
        }

        fn take_requests(&self) -> Vec<std::time::Duration> {
            std::mem::take(&mut *self.requests.lock().unwrap())
        }

        /// A delayed (not immediate, not unbounded) repaint request among `requests`.
        fn follow_up(requests: &[std::time::Duration]) -> Option<std::time::Duration> {
            requests.iter().copied().find(|d| *d > std::time::Duration::ZERO && *d < std::time::Duration::MAX)
        }

        /// A local edit as a canvas gesture makes it: the document changes and the revision moves on.
        fn edit_locally(&mut self) -> designos_doc::NodeId {
            let page = self.app.doc.pages[0];
            let id = self.app.doc.alloc_id();
            self.app.doc.insert(page, designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
            self.app.changed();
            crate::canvas::tiles::take_rev(&mut self.app);
            id
        }

        /// An agent adding a node to the file on disk. Waits until the watcher woke the editor.
        fn write_external_node(&mut self) -> designos_doc::NodeId {
            self.write_external_node_as(|json| json)
        }

        /// [`Self::write_external_node`], with `rewrite` applied to the JSON text the agent writes.
        fn write_external_node_as(&mut self, rewrite: impl FnOnce(String) -> String) -> designos_doc::NodeId {
            use std::time::{Duration, Instant};
            let path = self.app.watched_disk_path().expect("watched disk path");
            let mut ext_doc = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let page = ext_doc.pages[0];
            // Past every id the editor hands out, so the two sides never share a node id.
            for _ in 0..8 {
                ext_doc.alloc_id();
            }
            let id = ext_doc.alloc_id();
            ext_doc.insert(page, designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
            // Frames until egui has no repaint left to hand out, so the watcher's request is the next one reported.
            for _ in 0..4 {
                let _ = self.ctx.run_ui(egui::RawInput::default(), |_| {});
            }
            self.take_requests();
            std::thread::sleep(Duration::from_millis(20));
            designos_library::write_atomic(&path, rewrite(ext_doc.to_json().unwrap()).as_bytes()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.requests.lock().unwrap().is_empty() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
            }
            assert!(!self.requests.lock().unwrap().is_empty(), "external disk write must wake the editor");
            // The wake-up request belongs to the frame about to run, not to the ones after it.
            for _ in 0..4 {
                let _ = self.ctx.run_ui(egui::RawInput::default(), |_| {});
            }
            self.take_requests();
            id
        }

        /// Frames until the edit made by `edit_locally` is confirmed as a real change and waits for its save.
        fn settle_edit(&mut self) {
            for t in [10.0, 10.4] {
                self.app.now = t;
                self.tick(vec![]);
            }
            assert!(self.app.autosave.pending(), "the edit should be waiting for its save");
        }

        fn synced_toast(&self) -> bool {
            self.app.toast.as_ref().map(|t| t.text.as_str()) == Some("Synced external agent changes")
        }
    }

    impl Drop for IdleEditor {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn pointer_event(pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: egui::pos2(20.0, 20.0), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn external_write_does_not_replace_an_edit_that_has_not_settled() {
        let mut ed = IdleEditor::open("unsettled_edit");
        let local_id = ed.edit_locally();
        let ext_id = ed.write_external_node();

        ed.app.now = 10.1;
        ed.tick(vec![]);

        assert!(ed.app.doc.get(local_id).is_some(), "an edit the debounce has not looked at yet must survive an external write");
        assert!(ed.app.doc.get(ext_id).is_none());
        assert!(!ed.synced_toast());
    }

    #[test]
    fn external_write_during_a_drag_waits_for_the_gesture_to_end() {
        let mut ed = IdleEditor::open("drag_edit");
        ed.tick(vec![egui::Event::PointerMoved(egui::pos2(20.0, 20.0)), pointer_event(true)]);
        let first = ed.edit_locally();
        ed.app.now = 10.05;
        ed.tick(vec![]);
        let ext_id = ed.write_external_node();
        let mut moved = vec![first];
        for step in 0..3 {
            moved.push(ed.edit_locally());
            ed.app.now = 10.1 + step as f64 * 0.05;
            ed.tick(vec![]);
            for id in &moved {
                assert!(ed.app.doc.get(*id).is_some(), "drag step {step} lost an edit to the external write");
            }
        }
        assert!(ed.app.doc.get(ext_id).is_none());

        // Pointer up and quiet: the edits are saved, never replaced.
        ed.tick(vec![pointer_event(false)]);
        for (i, t) in [10.4, 10.8, 12.5].into_iter().enumerate() {
            ed.app.now = t;
            ed.tick(vec![]);
            assert!(ed.app.doc.get(first).is_some(), "settling step {i} lost the dragged edit");
        }
        assert_eq!(ed.app.autosave.saves, 1, "the dragged edits are saved once the gesture settled");
    }

    #[test]
    fn external_write_waits_for_a_revision_that_turns_out_to_change_nothing() {
        let mut ed = IdleEditor::open("no_op_revision");
        // A revision that moves on without touching the document, as a selection change does.
        ed.app.changed();
        crate::canvas::tiles::take_rev(&mut ed.app);
        let ext_id = ed.write_external_node();

        ed.app.now = 10.1;
        ed.tick(vec![]);
        assert!(ed.app.doc.get(ext_id).is_none(), "the file is not read while a revision is being checked");

        // The check finds nothing to save; the write that was held back is then picked up on the next frame.
        ed.app.now = 10.5;
        ed.take_requests();
        ed.tick(vec![]);
        let follow_up = IdleEditor::follow_up(&ed.take_requests()).expect("the held-back write must get a frame once the edit is settled");
        ed.app.now += follow_up.as_secs_f64();
        ed.tick(vec![]);
        assert!(ed.app.doc.get(ext_id).is_some());
        assert!(ed.synced_toast());
    }

    #[test]
    fn failed_autosave_keeps_the_edit_and_its_error_while_the_file_changed_on_disk() {
        let mut ed = IdleEditor::open("failed_autosave");
        let local_id = ed.edit_locally();
        ed.settle_edit();
        let ext_id = ed.write_external_node();

        // The save that is due now cannot write: the file turned read-only.
        let path = ed.app.watched_disk_path().unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();

        ed.app.now = 12.0;
        ed.tick(vec![]);
        assert_eq!(ed.app.toast.as_ref().map(|t| t.error), Some(true), "the save must have failed for this test to mean anything");
        let follow_up = IdleEditor::follow_up(&ed.take_requests()).expect("the write that arrived meanwhile must still get a frame");

        ed.app.now += follow_up.as_secs_f64();
        ed.tick(vec![]);
        assert!(ed.app.doc.get(local_id).is_some(), "an edit that could not be saved must not be replaced by the file on disk");
        assert!(ed.app.doc.get(ext_id).is_none());
        assert_eq!(ed.app.toast.as_ref().map(|t| t.error), Some(true), "the save error stays on screen");
        assert!(IdleEditor::follow_up(&ed.take_requests()).is_none(), "holding the write back must not start a timer");
    }

    #[cfg(unix)]
    #[test]
    fn failed_autosave_is_retried_by_the_next_frame_after_the_retry_delay() {
        use std::os::unix::fs::PermissionsExt;

        let mut ed = IdleEditor::open("failed_autosave_retry");
        let local_id = ed.edit_locally();
        ed.settle_edit();

        let path = ed.app.watched_disk_path().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        ed.app.now = 12.0;
        ed.tick(vec![]);
        assert_eq!(ed.app.toast.as_ref().map(|t| t.error), Some(true));
        assert_eq!(ed.app.autosave.saves, 0);

        // The disk recovers. Nothing wakes the editor on its own: the retry rides on the next frame.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        ed.take_requests();
        ed.app.now = 12.5;
        ed.tick(vec![]);
        assert_eq!(ed.app.autosave.saves, 0, "too soon after the failure to try again");
        assert!(IdleEditor::follow_up(&ed.take_requests()).is_none(), "waiting for the retry must not start a timer");
        ed.app.now = 14.0;
        ed.tick(vec![]);
        assert_eq!(ed.app.autosave.saves, 1);
        let lib = ed.app.lib.clone().unwrap();
        let id = ed.app.lib_file.as_ref().unwrap().id.clone();
        assert!(lib.load_file(&id).unwrap().get(local_id).is_some(), "the retried save wrote the edit to disk");
    }

    const ONE_INVALID_NUMBER: &str = "1 number in this file was not valid and was replaced by 0 or 1";

    fn json_texts_built_by(f: impl FnOnce()) -> usize {
        let before = designos_doc::finite_json::serialize_calls_on_this_thread();
        f();
        designos_doc::finite_json::serialize_calls_on_this_thread() - before
    }

    #[test]
    fn a_library_save_builds_one_json_text_whether_or_not_numbers_were_replaced() {
        let mut ed = IdleEditor::open("one_text_per_save");
        let id = ed.edit_locally();
        let built = json_texts_built_by(|| ed.app.save_library_file_with_toast());
        assert_eq!(built, 1, "a clean save serializes the document once");

        ed.app.doc.get_mut(id).unwrap().size.x = f32::NAN;
        let built = json_texts_built_by(|| ed.app.save_library_file_with_toast());
        assert_eq!(built, 1, "a save that replaces a number serializes the document once too");
        let toast = ed.app.toast.as_ref().expect("a save shows a toast");
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "{}", toast.text);
    }

    #[test]
    fn leaving_the_editor_saves_invalid_numbers_and_says_so() {
        let mut ed = IdleEditor::open("go_home_notice");
        let id = ed.edit_locally();
        ed.app.doc.get_mut(id).unwrap().size.x = f32::NAN;
        ed.app.toast = None;
        let file_id = ed.app.lib_file.as_ref().unwrap().id.clone();

        ed.app.go_home();

        let lib = ed.app.lib.clone().unwrap();
        assert_eq!(lib.load_file(&file_id).unwrap().get(id).unwrap().size.x, 0.0, "the file holds the neutral value");
        let toast = ed.app.toast.as_ref().expect("leaving the editor must say that a number was replaced");
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "{}", toast.text);
    }

    #[test]
    fn autosave_says_so_when_it_replaced_an_invalid_number() {
        let mut ed = IdleEditor::open("autosave_notice");
        let id = ed.edit_locally();
        ed.app.doc.get_mut(id).unwrap().size.x = f32::NAN;
        ed.settle_edit();
        ed.app.toast = None;
        ed.app.now = 12.0;
        let built = json_texts_built_by(|| ed.tick(vec![]));

        assert_eq!(ed.app.autosave.saves, 1, "the autosave must have run for this test to mean anything");
        assert_eq!(built, 1, "an autosave serializes the document once");
        let toast = ed.app.toast.as_ref().expect("an autosave that replaced a number must say so");
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "{}", toast.text);
    }

    #[test]
    fn a_synced_library_file_with_invalid_numbers_says_so() {
        let mut ed = IdleEditor::open("sync_notice_library");
        let ext_id = ed.write_external_node_as(|json| {
            assert!(json.contains("\"opacity\":1.0"), "the fixture needs an opacity to damage");
            json.replacen("\"opacity\":1.0", "\"opacity\":null", 1)
        });
        ed.app.now = 10.1;
        ed.tick(vec![]);

        assert!(ed.app.doc.get(ext_id).is_some(), "the external write must have been loaded for this test to mean anything");
        let toast = ed.app.toast.as_ref().expect("a sync shows a toast");
        assert!(toast.text.starts_with("Synced external agent changes"), "{}", toast.text);
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "the sync must carry the notice: {}", toast.text);
        assert!(ed.app.doc.get(ext_id).unwrap().opacity.is_finite());
    }

    #[test]
    fn a_synced_external_file_with_invalid_numbers_says_so() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_sync_notice_external_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agent.designos");
        let mut doc = Document::new("Agent file");
        doc.add_page("Page 1");
        std::fs::write(&path, doc.to_json().unwrap()).unwrap();

        let mut app = App::new(Document::new("Init"));
        app.open_path(path.clone());
        assert!(app.lib_file.is_none() && app.file_path.is_some());
        app.now = 10.0;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.autosave_tick(ui.ctx()));

        let page = doc.pages[0];
        let nid = doc.alloc_id();
        doc.insert(page, designos_doc::Node::frame(nid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
        let damaged = doc.to_json().unwrap().replacen("\"opacity\":1.0", "\"opacity\":null", 1);
        assert!(damaged.contains("null"));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, damaged).unwrap();
        app.toast = None;
        app.now = 11.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.autosave_tick(ui.ctx()));
        let _ = std::fs::remove_dir_all(&dir);

        assert!(app.doc.get(nid).is_some(), "the external write must have been loaded for this test to mean anything");
        let toast = app.toast.as_ref().expect("a sync shows a toast");
        assert!(toast.text.starts_with("Synced external agent changes"), "{}", toast.text);
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "the sync must carry the notice: {}", toast.text);
    }

    #[test]
    fn opening_a_library_file_with_null_numbers_says_so_once() {
        let mut ed = IdleEditor::open("open_null_numbers");
        let lib = ed.app.lib.clone().unwrap();
        let project = lib.list_projects().remove(0);
        let mut poisoned = Document::new("Legacy");
        let page = poisoned.add_page("Page 1");
        let id = poisoned.alloc_id();
        poisoned.insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        poisoned.get_mut(id).unwrap().size.x = f32::NAN;
        let entry = lib.create_file(&project.id, None, "Legacy", &Document::new("Legacy")).unwrap();
        // What a version before the fix left on disk: `null` where the NaN was.
        let on_disk = lib.path("projects").join(&project.id).join("files").join(format!("{}.designos", entry.id));
        std::fs::write(&on_disk, serde_json::to_string(&poisoned).unwrap()).unwrap();

        ed.app.toast = None;
        ed.app.open_library_file(&entry.id);
        let toast = ed.app.toast.as_ref().expect("opening a damaged file must say so");
        assert!(toast.error && toast.text.contains(ONE_INVALID_NUMBER), "{}", toast.text);
        assert!(ed.app.doc.get(id).unwrap().size.x.is_finite());

        ed.app.toast = None;
        ed.app.open_library_file(&entry.id);
        assert!(ed.app.toast.as_ref().is_none_or(|t| !t.error), "the second open in the same run stays quiet");
    }
}
