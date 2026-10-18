//! Shared libraries in the editor: publishing this file, enabling other
//! files' libraries, browsing their components in the Assets panel and
//! reviewing updates. Pure logic lives in `designos_doc::library_ops`; storage in
//! `designos_library`.
//!
//! # Using library styles and variables from other UI
//!
//! Library styles and variables are only copied into the file when first used.
//! Pickers that want to offer them (the variable binding picker, style pickers)
//! call:
//!
//! * [`App::library_variables`] / [`App::library_styles`]: everything the enabled
//!   libraries offer, with `local_id` / `local_key` set when it is already in the
//!   file (then it is also listed in `doc.variables` / `doc.styles`).
//! * [`App::bind_library_variable`] / [`App::apply_library_style`] (and
//!   [`App::use_library_style`]): copy the asset (with the variables it aliases)
//!   into the file and bind / apply it in the same undo step.
//!
//! Imported variables and styles live under `"<library id>/<key>"`, so they
//! never collide with local ones.
//!
//! The library directory defaults to [`default_root`], the same root as the
//! projects library; `App::attach_library` (and the `--library` flag) points it
//! at the projects library's root with [`LibrariesState::set_root`].

pub mod modal;
pub mod review;

use crate::app::App;
use crate::canvas::create::container_at;
use crate::comp_ops::AssetEntry;
use crate::edit::Tx;
use crate::layout;
use designos_doc::*;
use designos_library::{Library, LibraryIndex};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

/// Seconds between checks of enabled libraries for new versions.
const CHECK_EVERY: f64 = 2.0;
/// Seconds between recomputations of the publish changelog while the modal is open.
const CHANGES_EVERY: f64 = 1.0;

/// Updates waiting for one enabled library.
#[derive(Clone, Debug)]
pub struct PendingLib {
    pub library_id: String,
    pub name: String,
    /// Latest published version.
    pub version: u32,
    pub updates: Vec<Update>,
}

impl PendingLib {
    /// Updates that can be applied (everything except "missing from library").
    pub fn applicable(&self) -> impl Iterator<Item = &Update> {
        self.updates.iter().filter(|u| u.change != UpdateKind::Removed)
    }
}

/// A component (or set) offered by an enabled library.
#[derive(Clone, Debug)]
pub struct LibEntry {
    pub library_id: String,
    pub library_name: String,
    pub key: String,
    pub name: String,
    /// Node to render in the library's render document (first variant of a set).
    pub node: NodeId,
    pub variants: usize,
    pub size: Vec2,
}

/// A variable offered by an enabled library.
#[derive(Clone, Debug)]
pub struct LibraryVariable {
    pub library_id: String,
    pub library_name: String,
    /// Asset key inside the library; pass to [`App::use_library_variable`].
    pub key: String,
    pub name: String,
    pub kind: VariableKind,
    /// Where the variable may be applied (empty: everywhere).
    pub scopes: Vec<String>,
    /// Id in `doc.variables` when already used in this file.
    pub local_id: Option<String>,
}

/// A style offered by an enabled library.
#[derive(Clone, Debug)]
pub struct LibraryStyle {
    /// The style itself, for previews.
    pub style: Style,
    pub library_id: String,
    pub library_name: String,
    /// Asset key inside the library; pass to [`App::use_library_style`].
    pub key: String,
    pub name: String,
    pub text: bool,
    /// Key in `doc.styles` when already used in this file.
    pub local_key: Option<String>,
}

/// What a pending-updates result was computed from: (latest version, pinned
/// version, number of local copies, file name).
type PendingStamp = (u32, u32, usize, String);
/// Enabled libraries with their latest versions, and the entries built from them.
type EntriesCache = (Vec<(String, u32)>, Arc<Vec<LibEntry>>);

#[derive(Default)]
pub struct LibrariesState {
    /// Library directory; `None` until first use, then [`default_root`].
    pub root: Option<PathBuf>,
    handle: Option<Library>,
    /// The Libraries modal.
    pub open: bool,
    /// The update review modal.
    pub review: bool,
    /// Published libraries (refreshed while the modal is open).
    pub available: Vec<LibraryIndex>,
    /// Description typed in "This file".
    pub description: String,
    /// Assets that changed since this file was last published.
    pub changes: Vec<Update>,
    changes_at: f64,
    pub search: String,
    pub scroll: f32,
    /// Latest version per enabled library (`None`: not available on this device).
    latest: BTreeMap<String, Option<u32>>,
    pending: BTreeMap<String, (PendingStamp, PendingLib)>,
    /// Review checkboxes: (library id, asset key).
    pub selected: BTreeSet<(String, String)>,
    pub review_scroll: f32,
    dismissed: BTreeSet<(String, u32)>,
    next_check: f64,
    snapshots: BTreeMap<(String, u32), Arc<LibrarySnapshot>>,
    render_docs: BTreeMap<(String, u32), Arc<Document>>,
    entries: Option<EntriesCache>,
    /// Read-only preview of a library copy whose file is not available.
    pub preview: Option<NodeId>,
    pub(super) preview_tex: Option<(NodeId, egui::TextureHandle)>,
    /// Confirmation pending for unpublishing this file's library.
    pub confirm_unpublish: bool,
}

impl LibrariesState {
    /// Points the app at a library directory (shared with the projects library).
    #[allow(dead_code)] // called by the projects library
    pub fn set_root(&mut self, root: PathBuf) {
        self.root = Some(root);
        self.handle = None;
        self.latest.clear();
        self.pending.clear();
        self.snapshots.clear();
        self.render_docs.clear();
        self.entries = None;
        self.available.clear();
        self.next_check = 0.0;
    }

    /// Every library with differences against its latest version, including
    /// assets the library dropped.
    pub fn pending_all(&self) -> Vec<PendingLib> {
        self.pending.values().map(|(_, p)| p.clone()).collect()
    }

    /// Forces the next frame to re-check enabled libraries.
    pub fn invalidate(&mut self) {
        self.next_check = 0.0;
        self.pending.clear();
        self.latest.clear();
        self.entries = None;
        self.changes_at = f64::NEG_INFINITY;
    }
}

/// The library root shared with the projects library: `$DESIGNOS_LIBRARY`
/// (`$DESIGNOS_LIBRARY_DIR` is a deprecated alias), else the per-user data directory.
pub fn default_root() -> Option<PathBuf> {
    Some(Library::default_root())
}

pub(crate) fn is_noop(c: &Command) -> bool {
    matches!(c, Command::Batch(v) if v.is_empty())
}

impl App {
    /// Handle on the library directory, opening it on first use.
    pub fn library(&mut self) -> Option<Library> {
        if self.libs.handle.is_none() {
            let root = self.libs.root.clone().or_else(default_root)?;
            match Library::open(&root) {
                Ok(l) => {
                    self.libs.root = Some(root);
                    self.libs.handle = Some(l);
                }
                Err(e) => {
                    self.show_toast(format!("Libraries unavailable: {e}"), true);
                    return None;
                }
            }
        }
        self.libs.handle.clone()
    }

    pub fn open_libraries(&mut self) {
        self.libs.open = true;
        self.libs.confirm_unpublish = false;
        self.ui.menu = None;
        self.refresh_libraries();
    }

    pub fn close_libraries(&mut self) {
        self.libs.open = false;
        self.libs.confirm_unpublish = false;
    }

    /// Reloads the list of published libraries and the publish changelog.
    pub fn refresh_libraries(&mut self) {
        if let Some(lib) = self.library() {
            self.libs.available = lib.list_libraries().unwrap_or_default();
        }
        self.libs.changes_at = f64::NEG_INFINITY;
        self.recompute_changes();
    }

    fn recompute_changes(&mut self) {
        self.libs.changes_at = self.now;
        let published = self.doc.library.published.clone();
        let prev = published.as_ref().and_then(|p| self.latest_snapshot_uncached(&p.library_id));
        let id = published.as_ref().map(|p| p.library_id.clone()).unwrap_or_default();
        let version = prev.as_ref().map(|s| s.version + 1).unwrap_or(1);
        let current = snapshot(&self.doc, &id, &self.doc.name, version);
        self.libs.changes = diff_snapshots(prev.as_ref(), &current);
    }

    fn latest_snapshot_uncached(&mut self, id: &str) -> Option<LibrarySnapshot> {
        self.library()?.load_snapshot(id, None).ok()
    }

    /// A stored snapshot, cached.
    pub fn snapshot_of(&mut self, id: &str, version: u32) -> Option<Arc<LibrarySnapshot>> {
        let key = (id.to_string(), version);
        if let Some(s) = self.libs.snapshots.get(&key) {
            return Some(s.clone());
        }
        let snap = Arc::new(self.library()?.load_snapshot(id, Some(version)).ok()?);
        self.libs.snapshots.insert(key, snap.clone());
        Some(snap)
    }

    /// Latest published version of `id`, from cache (read once, refreshed by [`App::libs_tick`]).
    fn latest_version_of(&mut self, id: &str) -> Option<u32> {
        if let Some(v) = self.libs.latest.get(id) {
            return *v;
        }
        let v = self.library().and_then(|l| l.library_index(id).ok()).map(|i| i.latest_version()).filter(|v| *v > 0);
        self.libs.latest.insert(id.to_string(), v);
        v
    }

    /// Latest snapshot of an enabled library.
    pub fn latest_snapshot(&mut self, id: &str) -> Option<Arc<LibrarySnapshot>> {
        let v = self.latest_version_of(id)?;
        self.snapshot_of(id, v)
    }

    /// Document used to render thumbnails of a library's components.
    pub fn library_render_doc(&mut self, id: &str) -> Option<Arc<Document>> {
        let v = self.latest_version_of(id)?;
        let key = (id.to_string(), v);
        if let Some(d) = self.libs.render_docs.get(&key) {
            return Some(d.clone());
        }
        let snap = self.snapshot_of(id, v)?;
        let mut d = Document::new(snap.name.clone());
        d.nodes = snap.nodes.clone();
        d.styles = snap.styles.iter().map(|(k, s)| (k.clone(), s.clone())).collect();
        d.style_meta = snap.style_meta.clone();
        d.variables = snap.variables.clone();
        d.variable_collections = snap.variable_collections.clone();
        d.images = snap.images.clone();
        let d = Arc::new(d);
        self.libs.render_docs.insert(key, d.clone());
        Some(d)
    }

    // ------------------------------------------------------------ publishing

    /// Publishes this file as a new version of its library.
    pub fn publish_file(&mut self) -> bool {
        if publishable_assets(&self.doc).is_empty() {
            self.show_toast("Nothing to publish: create components, styles or variables first", true);
            return false;
        }
        let Some(lib) = self.library() else { return false };
        let description = std::mem::take(&mut self.libs.description);
        let file_id = match &self.doc.library.published {
            Some(p) => p.library_id.clone(),
            None => {
                let id = designos_library::new_id();
                let name: String = self.doc.name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
                format!("{}-{}", name.trim_matches('-'), &id[id.len().saturating_sub(8)..])
            }
        };
        match lib.publish_library(&file_id, &self.doc, &description) {
            Ok(info) => {
                if let Some(p) = &self.file_path {
                    let _ = lib.set_library_source(&info.library_id, Some(&p.to_string_lossy()));
                }
                if let Some(f) = &self.lib_file {
                    let _ = lib.set_library_source_file(&info.library_id, Some(&f.id));
                }
                self.show_toast(format!("Published {} v{}", info.name, info.version), false);
                self.doc.library.published = Some(info);
                self.libs.invalidate();
                self.refresh_libraries();
                true
            }
            Err(e) => {
                self.libs.description = description;
                self.show_toast(format!("Publish failed: {e}"), true);
                false
            }
        }
    }

    /// Unpublishes this file's library (moved to the trash; files that use it keep their copies).
    pub fn unpublish_file(&mut self) {
        let Some(p) = self.doc.library.published.clone() else { return };
        let Some(lib) = self.library() else { return };
        match lib.unpublish(&p.library_id) {
            Ok(()) => {
                self.doc.library.published = None;
                self.show_toast(format!("Unpublished {}", p.name), false);
                self.libs.invalidate();
                self.refresh_libraries();
            }
            Err(e) => self.show_toast(format!("Unpublish failed: {e}"), true),
        }
    }

    // ------------------------------------------------------------- enabling

    pub fn is_library_enabled(&self, id: &str) -> bool {
        self.doc.library.enabled.iter().any(|l| l.id == id)
    }

    /// Enables a library (pinning its latest version) or disables it. Local
    /// copies and their instances stay either way.
    pub fn set_library_enabled(&mut self, id: &str, on: bool) {
        let cmd = if on {
            let Some(v) = self.library().and_then(|l| l.library_index(id).ok()).map(|i| i.latest_version()) else { return };
            let Some(snap) = self.snapshot_of(id, v) else {
                self.show_toast("Library could not be loaded", true);
                return;
            };
            enable_library(&self.doc, &snap)
        } else {
            disable_library(&self.doc, id)
        };
        self.run_library_cmd(cmd);
        self.libs.invalidate();
    }

    fn run_library_cmd(&mut self, cmd: Command) -> bool {
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, cmd).is_err() {
            return false;
        }
        self.run_tx(tx)
    }

    // --------------------------------------------------------------- updates

    /// Checks enabled libraries for new versions (cheap; every couple of
    /// seconds) and keeps the publish changelog fresh while the modal is open.
    pub fn libs_tick(&mut self) {
        if self.libs.open && self.now - self.libs.changes_at >= CHANGES_EVERY {
            self.recompute_changes();
        }
        if self.now < self.libs.next_check {
            return;
        }
        self.libs.next_check = self.now + CHECK_EVERY;
        let enabled = self.doc.library.enabled.clone();
        if enabled.is_empty() {
            self.libs.pending.clear();
            return;
        }
        let Some(lib) = self.library() else { return };
        self.libs.pending.retain(|id, _| enabled.iter().any(|r| &r.id == id));
        for r in enabled.iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            let latest = lib.library_index(&r.id).ok().map(|i| i.latest_version()).filter(|v| *v > 0);
            self.libs.latest.insert(r.id.clone(), latest);
            let Some(latest) = latest.filter(|l| *l > r.version) else {
                self.libs.pending.remove(&r.id);
                continue;
            };
            let stamp = (latest, r.version, self.doc.library.external.len(), self.doc.name.clone());
            if self.libs.pending.get(&r.id).is_some_and(|(s, _)| *s == stamp) {
                continue;
            }
            let Some(snap) = self.snapshot_of(&r.id, latest) else { continue };
            let updates = pending_updates(&self.doc, &snap);
            self.libs.pending.insert(r.id.clone(), (stamp, PendingLib { library_id: r.id.clone(), name: snap.name.clone(), version: latest, updates }));
        }
    }

    /// Libraries with updates waiting (changed or new assets).
    pub fn pending_libraries(&self) -> Vec<PendingLib> {
        let mut v: Vec<PendingLib> = self.libs.pending.values().map(|(_, p)| p.clone()).filter(|p| p.applicable().next().is_some()).collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        v
    }

    /// Whether the "Library updates available" banner should show.
    pub fn update_banner_visible(&self) -> bool {
        !self.libs.review && self.pending_libraries().iter().any(|p| !self.libs.dismissed.contains(&(p.library_id.clone(), p.version)))
    }

    pub fn dismiss_update_banner(&mut self) {
        let keys: Vec<(String, u32)> = self.pending_libraries().into_iter().map(|p| (p.library_id, p.version)).collect();
        self.libs.dismissed.extend(keys);
    }

    /// Opens the update review for all enabled libraries (including assets they dropped).
    pub fn open_updates_review(&mut self) {
        // Recompute against the latest versions so missing assets are listed too.
        let enabled = self.doc.library.enabled.clone();
        for r in enabled.iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            let Some(latest) = self.latest_version_of(&r.id) else { continue };
            let Some(snap) = self.snapshot_of(&r.id, latest) else { continue };
            let updates = pending_updates(&self.doc, &snap);
            let stamp = (latest, r.version, self.doc.library.external.len(), self.doc.name.clone());
            self.libs.pending.insert(r.id.clone(), (stamp, PendingLib { library_id: r.id.clone(), name: snap.name.clone(), version: latest, updates }));
        }
        self.libs.selected.clear();
        self.libs.review_scroll = 0.0;
        self.libs.review = true;
    }

    pub fn close_updates_review(&mut self) {
        self.libs.review = false;
    }

    /// Applies the selected updates of one library as one undo step.
    pub fn apply_library_updates(&mut self, library_id: &str, keys: &[String]) -> usize {
        let Some(snap) = self.latest_snapshot(library_id) else {
            self.show_toast("Library is not available", true);
            return 0;
        };
        let cmd = apply_updates(&mut self.doc, &snap, keys);
        if is_noop(&cmd) {
            return 0;
        }
        let mut tx = Tx::new();
        if let Err(e) = tx.cmd(&mut self.doc, cmd) {
            self.show_toast(format!("Update failed: {e}"), true);
            return 0;
        }
        // Instances that changed size may sit in auto layout.
        let mut parents = BTreeSet::new();
        for e in self.doc.library.external.values().filter(|e| e.library_id == library_id) {
            let Some(root) = e.node else { continue };
            for c in components_of(&self.doc, root) {
                for i in instances_of(&self.doc, c) {
                    if let Some(p) = self.doc.get(i).and_then(|n| n.parent).filter(|p| !is_library_node(&self.doc, *p)) {
                        parents.insert(p);
                    }
                }
            }
        }
        for p in parents {
            if self.doc.get(p).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some()) {
                layout::relayout_around(&mut self.doc, &mut tx.ed, p);
            }
        }
        self.run_tx(tx);
        self.libs.invalidate();
        keys.len()
    }

    /// Applies every selected update (or all of them) across libraries.
    pub fn apply_review(&mut self, all: bool) {
        let mut applied = 0;
        for p in self.pending_libraries() {
            let keys: Vec<String> =
                p.applicable().filter(|u| all || self.libs.selected.contains(&(p.library_id.clone(), u.key.clone()))).map(|u| u.key.clone()).collect();
            if !keys.is_empty() {
                applied += self.apply_library_updates(&p.library_id, &keys);
            }
        }
        self.libs.review = false;
        if applied > 0 {
            self.show_toast(if applied == 1 { "Updated 1 library asset".to_string() } else { format!("Updated {applied} library assets") }, false);
        }
    }

    // ------------------------------------------------------- assets panel

    /// Components of enabled libraries, for the Assets panel.
    pub fn library_entries(&mut self) -> Arc<Vec<LibEntry>> {
        let stamp: Vec<(String, u32)> = self
            .doc
            .library
            .enabled
            .clone()
            .into_iter()
            .filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX))
            .map(|r| (r.id.clone(), self.latest_version_of(&r.id).unwrap_or(0)))
            .collect();
        if let Some((s, e)) = &self.libs.entries {
            if *s == stamp {
                return e.clone();
            }
        }
        let mut out = Vec::new();
        for (id, v) in stamp.iter().filter(|(_, v)| *v > 0) {
            let Some(snap) = self.snapshot_of(id, *v) else { continue };
            let mut rows: Vec<LibEntry> = snap
                .assets
                .iter()
                .filter(|a| matches!(a.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet))
                .filter_map(|a| {
                    let root = a.node?;
                    let n = snap.nodes.get(&root)?;
                    let (node, variants) = match n.kind {
                        NodeKind::ComponentSet(_) => (
                            n.children.iter().copied().find(|c| snap.nodes.get(c).is_some_and(|k| matches!(k.kind, NodeKind::Component(_))))?,
                            n.children.len(),
                        ),
                        _ => (root, 0),
                    };
                    Some(LibEntry {
                        library_id: id.clone(),
                        library_name: snap.name.clone(),
                        key: a.key.clone(),
                        name: a.name.clone(),
                        node,
                        variants,
                        size: snap.nodes[&node].size,
                    })
                })
                .collect();
            rows.sort_by_key(|r| r.name.to_lowercase());
            out.extend(rows);
        }
        let out = Arc::new(out);
        self.libs.entries = Some((stamp, out.clone()));
        out
    }

    /// Imported components of libraries that are not available (placeholders
    /// for other files, unpublished libraries), listed under the library's name.
    pub fn unavailable_library_components(&mut self) -> Vec<AssetEntry> {
        let mut out = Vec::new();
        let copies = library_component_copies(&self.doc);
        for (lib_id, node, name) in copies {
            let available = !lib_id.starts_with(REMOTE_LIBRARY_PREFIX) && self.is_library_enabled(&lib_id) && self.latest_version_of(&lib_id).is_some();
            if available {
                continue;
            }
            let page = self.library_display_name(&lib_id);
            let variants = match self.doc.get(node).map(|n| &n.kind) {
                Some(NodeKind::ComponentSet(_)) => self.doc.get(node).map(|n| n.children.len()).unwrap_or(0),
                _ => 0,
            };
            let id = if variants > 0 { components_of(&self.doc, node).first().copied().unwrap_or(node) } else { node };
            out.push(AssetEntry { page, container: String::new(), name, id, variants });
        }
        out.sort_by(|a, b| (a.page.to_lowercase(), a.name.to_lowercase()).cmp(&(b.page.to_lowercase(), b.name.to_lowercase())));
        out
    }

    /// Name shown for a library: its pinned name, else a tidy form of its id.
    pub fn library_display_name(&self, id: &str) -> String {
        if let Some(r) = self.doc.library.enabled.iter().find(|l| l.id == id) {
            return r.name.clone();
        }
        match id.strip_prefix(REMOTE_LIBRARY_PREFIX) {
            Some(rest) => format!("From another file ({rest})"),
            None => id.to_string(),
        }
    }

    /// Drops an instance of a library component at page point `at`, importing
    /// the component (and what it uses) into this file first. One undo step.
    pub fn drop_library_asset(&mut self, library_id: &str, key: &str, at: Vec2) -> Option<NodeId> {
        let snap = self.latest_snapshot(library_id)?;
        let asset = snap.asset(key)?;
        let root = snap.nodes.get(&asset.node?)?;
        let size = match root.kind {
            NodeKind::ComponentSet(_) => root.children.first().and_then(|c| snap.nodes.get(c)).map(|c| c.size).unwrap_or(root.size),
            _ => root.size,
        };
        let target = Vec2::new(at.x - size.x / 2.0, at.y - size.y / 2.0);
        let parent = container_at(&self.doc, self.page, at, &[]);
        let local = self.doc.world_transform(parent).invert()?.apply(target);
        // `import_asset` copies the component's images into the file before it returns the command (outside
        // undo), so a drop that is refused afterwards has to take them out again.
        let images_before: BTreeSet<String> = self.doc.images.keys().cloned().collect();
        let cmd = import_asset(&mut self.doc, &snap, key, Some(Placement { parent, at: local }));
        if is_noop(&cmd) {
            return None;
        }
        // Judge the drop as the insert it is, before anything is applied.
        if let Some(root) = inserted_under(&cmd, parent) {
            if !self.slot_insert_allowed(parent, &[root]) {
                self.doc.images.retain(|k, _| images_before.contains(k));
                return None;
            }
        }
        let mut tx = Tx::new();
        if let Err(e) = tx.cmd(&mut self.doc, cmd) {
            self.doc.images.retain(|k, _| images_before.contains(k));
            self.show_toast(format!("Edit failed: {e}"), true);
            return None;
        }
        App::mark_slot_content_override(&mut self.doc, &mut tx.ed, parent);
        layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        if !self.run_tx(tx) {
            return None;
        }
        let id = self.doc.get(parent).and_then(|p| p.children.last().copied());
        if let Some(id) = id {
            self.selection = vec![id];
        }
        self.libs.invalidate();
        id
    }

    // ------------------------------------------- styles and variables API

    /// Variables offered by enabled libraries.
    pub fn library_variables(&mut self) -> Vec<LibraryVariable> {
        let mut out = Vec::new();
        for r in self.doc.library.enabled.clone().into_iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            let Some(snap) = self.latest_snapshot(&r.id) else { continue };
            for a in snap.assets.iter().filter(|a| a.kind == LibraryAssetKind::Variable) {
                let Some(v) = snap.variables.get(&a.key["var:".len()..]) else { continue };
                let local_id = self.doc.library.external.get(&local_key(&r.id, &a.key)).and_then(|e| e.local_key.clone());
                out.push(LibraryVariable {
                    library_id: r.id.clone(),
                    library_name: snap.name.clone(),
                    key: a.key.clone(),
                    name: a.name.clone(),
                    kind: v.kind,
                    scopes: v.scopes.clone(),
                    local_id,
                });
            }
        }
        out
    }

    /// Styles offered by enabled libraries.
    pub fn library_styles(&mut self) -> Vec<LibraryStyle> {
        let mut out = Vec::new();
        for r in self.doc.library.enabled.clone().into_iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            let Some(snap) = self.latest_snapshot(&r.id) else { continue };
            for a in snap.assets.iter().filter(|a| a.kind == LibraryAssetKind::Style) {
                let raw = &a.key["style:".len()..];
                let Some(style) = snap.styles.get(raw) else { continue };
                let local_key = self.doc.library.external.get(&local_key(&r.id, &a.key)).and_then(|e| e.local_key.clone());
                out.push(LibraryStyle {
                    style: style.clone(),
                    library_id: r.id.clone(),
                    library_name: snap.name.clone(),
                    key: a.key.clone(),
                    name: a.name.clone(),
                    text: matches!(style, Style::Text(_)),
                    local_key,
                });
            }
        }
        out
    }

    /// Copies a library style into the file; returns its key in `doc.styles`.
    pub fn use_library_style(&mut self, library_id: &str, key: &str) -> Option<String> {
        self.use_library_asset(library_id, key)
    }

    fn use_library_asset(&mut self, library_id: &str, key: &str) -> Option<String> {
        let mut tx = Tx::new();
        let local = self.import_asset_in(&mut tx, library_id, key);
        self.run_tx(tx);
        self.libs.invalidate();
        local
    }

    /// Copies a library asset into the file as part of `tx` (so the caller can
    /// bind or apply it in the same undo step); returns its local id / key.
    pub fn import_asset_in(&mut self, tx: &mut Tx, library_id: &str, key: &str) -> Option<String> {
        let ekey = local_key(library_id, key);
        if let Some(l) = self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone()) {
            return Some(l);
        }
        let snap = self.latest_snapshot(library_id)?;
        let cmd = import_asset(&mut self.doc, &snap, key, None);
        if !is_noop(&cmd) && tx.cmd(&mut self.doc, cmd).is_err() {
            return None;
        }
        self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone())
    }

    /// Binds `keys` of `nodes` to a library variable, copying it into the file
    /// first. The copy and the binding are one undo step.
    pub fn bind_library_variable(&mut self, nodes: &[NodeId], keys: &[&str], library_id: &str, key: &str) {
        let mut tx = Tx::new();
        match self.import_asset_in(&mut tx, library_id, key) {
            Some(local) => {
                self.bind_variable_in(&mut tx, nodes, keys, &local);
                self.run_tx(tx);
            }
            None => {
                tx.cancel(&mut self.doc);
                self.show_toast("Library variable could not be loaded", true);
            }
        }
        self.libs.invalidate();
    }

    /// Applies a library style to `sel`, copying it into the file first, in
    /// `tx` (which may already hold the caller's edits) as one undo step.
    pub fn apply_library_style(&mut self, mut tx: Tx, sel: &[NodeId], library_id: &str, key: &str) {
        match self.import_asset_in(&mut tx, library_id, key) {
            Some(local) => {
                self.apply_style_in(&mut tx, sel, &local);
                self.run_tx(tx);
            }
            None => {
                tx.cancel(&mut self.doc);
                self.show_toast("Library style could not be loaded", true);
            }
        }
        self.libs.invalidate();
    }

    // ---------------------------------------------------- go to main

    /// "Go to main component" for a component copy on the library page: opens
    /// the library's file when it is saved on this device, else previews the
    /// local copy read-only.
    pub fn go_to_library_main(&mut self, comp: NodeId) {
        let Some((ekey, ext)) = library_asset_of_component(&self.doc, comp).map(|(k, e)| (k.clone(), e.clone())) else {
            self.libs.preview = Some(comp);
            return;
        };
        let name = self.doc.get(ext.node.unwrap_or(comp)).map(|n| n.name.clone()).unwrap_or_default();
        let index = (!ext.library_id.starts_with(REMOTE_LIBRARY_PREFIX)).then(|| self.library().and_then(|l| l.library_index(&ext.library_id).ok())).flatten();
        // Files of the projects library open through the home store; others by path.
        let home_file = index.as_ref().and_then(|i| i.source_file.clone()).filter(|f| self.lib.as_ref().is_some_and(|l| l.file(f).is_ok()));
        let source = index.and_then(|i| i.source_path).map(PathBuf::from).filter(|p| p.exists());
        let _ = ekey;
        match home_file.map(Ok).or(source.map(Err)) {
            Some(opened) => {
                match opened {
                    Ok(file_id) => self.open_library_file(&file_id),
                    Err(path) => self.open_path(path),
                }
                let found = self.doc.pages.iter().find_map(|p| {
                    let mut hit = None;
                    self.doc.walk(*p, |n, _| {
                        if hit.is_none() && n.name == name && matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
                            hit = Some(n.id);
                        }
                    });
                    hit
                });
                if let Some(id) = found {
                    self.go_to_main_component(id);
                }
            }
            None => self.libs.preview = Some(comp),
        }
    }
}

/// Per-frame entry point: update checks, banner, and the modals.
pub fn show(app: &mut App, ctx: &egui::Context) {
    app.libs_tick();
    review::show_banner(app, ctx);
    modal::show(app, ctx);
    review::show(app, ctx);
    modal::show_preview(app, ctx);
}

/// Inspector row of a library instance: the library's name, or "From another
/// file" for components imported from a file that is not available.
pub fn instance_library_row(ui: &mut egui::Ui, row: egui::Rect, info: &LibraryInstanceInfo) {
    let p = ui.painter().clone();
    crate::icons::paint(&p, crate::icons::Icon::Book, egui::pos2(row.left() + 8.0, row.center().y), 14.0, crate::theme::ICON_SECONDARY);
    let text = if info.remote { "From another file".to_string() } else { info.library_name.clone() };
    let text = crate::widgets::elide(&p, &text, &crate::theme::font_body(), row.width() - 24.0);
    p.text(egui::pos2(row.left() + 22.0, row.center().y), egui::Align2::LEFT_CENTER, text, crate::theme::font_body(), crate::theme::TEXT_SECONDARY);
    crate::widgets::record(ui, "instance-library", row);
}

/// Root of the subtree `cmd` inserts under `parent` (the dropped instance), looking inside batches.
fn inserted_under(cmd: &Command, parent: NodeId) -> Option<&Node> {
    match cmd {
        Command::Insert { parent: p, nodes, .. } if *p == parent => nodes.first(),
        Command::Batch(cmds) => cmds.iter().rev().find_map(|c| inserted_under(c, parent)),
        _ => None,
    }
}
