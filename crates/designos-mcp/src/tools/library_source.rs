//! Shared library root resolution and cached snapshot loading (`T7.38`, reused by `E15-1`, `E15-3`, `T7.9`).

use designos_doc::{Document, LibrarySnapshot};
use designos_library::Library;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

type CacheKey = (PathBuf, String, u32);

fn snapshot_cache() -> &'static Mutex<HashMap<CacheKey, Arc<LibrarySnapshot>>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, Arc<LibrarySnapshot>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn doc_roots() -> &'static Mutex<HashMap<u64, PathBuf>> {
    static ROOTS: OnceLock<Mutex<HashMap<u64, PathBuf>>> = OnceLock::new();
    ROOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolves the active library root directory (`$DESIGNOS_LIBRARY`, `$DESIGNOS_LIBRARY_DIR`, or platform default).
pub fn resolve_library_root() -> PathBuf {
    Library::default_root()
}

/// Clears the in-process `(root, library_id, version)` snapshot cache.
pub fn clear_snapshot_cache() {
    if let Ok(mut guard) = snapshot_cache().lock() {
        guard.clear();
    }
}

/// Loads one library snapshot at `(library_id, version)` (or latest when `version` is `0`) from `root`,
/// caching the loaded [`LibrarySnapshot`] in memory by `(root, id, resolved_version)`.
pub fn load_cached_snapshot_at(root: &Path, library_id: &str, version: Option<u32>) -> Option<Arc<LibrarySnapshot>> {
    if !root.is_dir() {
        return None;
    }
    let lib = Library::open(root).ok()?;
    let resolved_version = match version.filter(|&v| v > 0) {
        Some(v) => v,
        None => lib.library_index(library_id).ok()?.latest_version(),
    };
    if resolved_version == 0 {
        return None;
    }
    let key = (root.to_path_buf(), library_id.to_string(), resolved_version);
    if let Ok(guard) = snapshot_cache().lock() {
        if let Some(found) = guard.get(&key) {
            return Some(Arc::clone(found));
        }
    }
    let (actual_version, snap) = match lib.load_snapshot(library_id, Some(resolved_version)) {
        Ok(s) => (resolved_version, Arc::new(s)),
        Err(_) => {
            // Fall back to latest version if the pinned version file is absent.
            let latest = lib.library_index(library_id).ok()?.latest_version();
            if latest == 0 || latest == resolved_version {
                return None;
            }
            (latest, Arc::new(lib.load_snapshot(library_id, Some(latest)).ok()?))
        }
    };
    let actual_key = (root.to_path_buf(), library_id.to_string(), actual_version);
    if let Ok(mut guard) = snapshot_cache().lock() {
        guard.insert(actual_key, Arc::clone(&snap));
    }
    Some(snap)
}

/// Returns the snapshots for all libraries enabled on `doc`, resolved from [`resolve_library_root`]
/// and cached per session by `(id, version)` (`T7.38` §3.5).
pub fn enabled_snapshots(doc: &Document) -> Vec<LibrarySnapshot> {
    let doc_root = doc_roots().lock().ok().and_then(|g| g.get(&doc.runtime_id()).cloned()).filter(|p| p.is_dir());
    let root = resolve_library_root();
    doc.library
        .enabled
        .iter()
        .filter_map(|en| {
            if let Some(ref dr) = doc_root {
                if let Some(arc) = load_cached_snapshot_at(dr, &en.id, Some(en.version)) {
                    return Some((*arc).clone());
                }
            }
            if let Some(arc) = load_cached_snapshot_at(&root, &en.id, Some(en.version)) {
                return Some((*arc).clone());
            }
            let guard = snapshot_cache().lock().ok()?;
            guard.iter().find(|((_, id, ver), _)| id == &en.id && (en.version == 0 || *ver == en.version)).map(|(_, arc)| (**arc).clone())
        })
        .collect()
}

/// Like [`enabled_snapshots`], using an explicit library `root`.
pub fn enabled_snapshots_from_root(doc: &Document, root: &Path) -> Vec<LibrarySnapshot> {
    if let Ok(mut guard) = doc_roots().lock() {
        guard.insert(doc.runtime_id(), root.to_path_buf());
    }
    doc.library.enabled.iter().filter_map(|en| load_cached_snapshot_at(root, &en.id, Some(en.version)).map(|arc| (*arc).clone())).collect()
}

/// Loads the published `LibrarySnapshot` for each library enabled in `doc.library.enabled`
/// from the default `designos_library::Library::default_root()`.
pub fn load_enabled_snapshots(doc: &Document) -> Vec<LibrarySnapshot> {
    enabled_snapshots(doc)
}

/// Loads the published `LibrarySnapshot` for each library enabled in `doc.library.enabled`
/// from `root`.
pub fn load_enabled_snapshots_from(doc: &Document, root: &Path) -> Vec<LibrarySnapshot> {
    enabled_snapshots_from_root(doc, root)
}
