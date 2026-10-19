//! Opening and saving documents.

use designos_doc::{Document, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const NATIVE_EXT: &str = "designos";
pub const LEGACY_EXT: &str = "designos";

fn repaired_originals() -> &'static Mutex<HashMap<PathBuf, Vec<u8>>> {
    static ORIGINALS: OnceLock<Mutex<HashMap<PathBuf, Vec<u8>>>> = OnceLock::new();
    ORIGINALS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn non_finite_notified() -> &'static Mutex<HashSet<PathBuf>> {
    static NOTIFIED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    NOTIFIED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn repair_key(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    if p.extension().is_none() {
        p.set_extension(NATIVE_EXT);
    }
    std::fs::canonicalize(&p).unwrap_or(p)
}

/// Formats the user-facing notice when `count` non-finite (`NaN`, `Infinity`, or `null`) numbers
/// were replaced by their neutral value (`0` or `1`).
pub fn non_finite_notice(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 number in this file was not valid and was replaced by 0 or 1".to_string()),
        n => Some(format!("{n} numbers in this file were not valid and were replaced by 0 or 1")),
    }
}

/// Returns [`non_finite_notice`] at most once per file and per run.
pub fn non_finite_notice_once(path: &Path, count: usize) -> Option<String> {
    let notice = non_finite_notice(count)?;
    let key = repair_key(path);
    if let Ok(mut set) = non_finite_notified().lock() {
        if !set.insert(key) {
            return None;
        }
    }
    Some(notice)
}

pub fn same_repair_key(a: &Path, b: &Path) -> bool {
    repair_key(a) == repair_key(b)
}

/// Drops any cached pre-repair original bytes for `path` when its tab closes without saving.
pub fn drop_repaired_original(path: &Path) {
    let key = repair_key(path);
    if let Ok(mut map) = repaired_originals().lock() {
        map.remove(&key);
    }
}

#[cfg(test)]
pub(crate) fn has_repaired_original(path: &Path) -> bool {
    let key = repair_key(path);
    repaired_originals().lock().ok().is_some_and(|map| map.contains_key(&key))
}

fn parse_native_with_warning(path: &Path, text: &str, record_once: bool) -> serde_json::Result<(Document, Option<String>)> {
    let (mut doc, report) = Document::from_json_with_report(text)?;
    if !report.is_clean() {
        eprintln!("designos-doc: repaired a damaged document on load: {report:?}");
    }
    if report.id_space_exhausted {
        doc.renumber_ids_for_headroom();
    }
    let key = repair_key(path);
    let structural_warning = match report.structural_summary() {
        Some(what) => {
            if let Ok(mut map) = repaired_originals().lock() {
                map.insert(key, text.as_bytes().to_vec());
            }
            let bak_name = backup_path(path).file_name().and_then(|n| n.to_str()).unwrap_or("backup.bak").to_string();
            Some(format!("repaired malformed document ({what}); original will be backed up to {bak_name} on save"))
        }
        None => {
            if let Ok(mut map) = repaired_originals().lock() {
                map.remove(&key);
            }
            None
        }
    };
    let nf_notice = if record_once { non_finite_notice_once(path, report.non_finite_numbers) } else { non_finite_notice(report.non_finite_numbers) };
    let warning = match (structural_warning, nf_notice) {
        (Some(s), Some(n)) => Some(format!("{s}; {n}")),
        (Some(s), None) => Some(s),
        (None, Some(n)) => Some(n),
        (None, None) => None,
    };
    Ok((doc, warning))
}

/// Loads a document from disk, dispatching on the file extension. Import warnings go to stderr;
/// use [`load_path_with_warning`] where the user should see them.
pub fn load_path(path: &Path) -> Result<Document, String> {
    let (doc, warning) = load_path_importing_pen_with_mode(path, designos_fig::import_pen_json_with_warnings, false)?;
    if let Some(w) = warning {
        eprintln!("designos: {}: {w}", path.display());
    }
    Ok(doc)
}

/// Like [`load_path`], but also returns the non-fatal import or repair warning, if any: a `.pen`
/// file whose `ref` expansion hit the clone budget or a native `.designos` file whose node graph
/// needed structural repair on load still opens, and reports what changed.
pub fn load_path_with_warning(path: &Path) -> Result<(Document, Option<String>), String> {
    load_path_importing_pen_with(path, designos_fig::import_pen_json_with_warnings)
}

/// [`load_path_with_warning`] with the `.pen` importer injected, so a test can produce a warning
/// without a document that really exhausts the import's clone budget.
fn load_path_importing_pen_with(
    path: &Path,
    import_pen: impl Fn(&str) -> Result<(Document, Option<String>), String>,
) -> Result<(Document, Option<String>), String> {
    load_path_importing_pen_with_mode(path, import_pen, true)
}

fn load_path_importing_pen_with_mode(
    path: &Path,
    import_pen: impl Fn(&str) -> Result<(Document, Option<String>), String>,
    record_once: bool,
) -> Result<(Document, Option<String>), String> {
    let mut warning = None;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut doc = match ext.as_str() {
        "fig" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let (d, report) = designos_fig::import_fig_with_report(&bytes).map_err(|e| format!("Cannot import .fig: {e}"))?;
            if report.instances_left_unexpanded > 0 {
                warning = Some(format!("{} instances were left unexpanded to keep the import fast.", report.instances_left_unexpanded));
            }
            d
        }
        "pen" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let (mut d, w) = import_pen(&text).map_err(|e| format!("Cannot import .pen: {e}"))?;
            warning = w;
            if d.name.is_empty() || d.name == "Pencil Document" {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    d.name = stem.to_string();
                }
            }
            d
        }
        "json" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            match designos_fig::import_rest_json_with_report(&text) {
                Ok((mut d, report)) => {
                    // A number that overflowed `f32` on import is infinity here. Neutralize only the
                    // affected entries in place without re-parsing the whole document or running
                    // `migrate_text`.
                    let replaced = d.neutralize_non_finite();
                    if replaced > 0 {
                        warning = if record_once { non_finite_notice_once(path, replaced) } else { non_finite_notice(replaced) };
                    }
                    if report.instances_left_unexpanded > 0 {
                        let budget_msg = format!("{} instances were left unexpanded to keep the import fast.", report.instances_left_unexpanded);
                        warning = Some(match warning {
                            Some(prev) => format!("{prev} {budget_msg}"),
                            None => budget_msg,
                        });
                    }
                    d
                }
                Err(fig_err) => match import_pen(&text) {
                    Ok((d, w)) => {
                        warning = w;
                        d
                    }
                    Err(_) => {
                        let (d, w) = parse_native_with_warning(path, &text, record_once)
                            .map_err(|e| format!("Not a Figma REST file ({fig_err}) nor a designos file ({e})"))?;
                        warning = w;
                        d
                    }
                },
            }
        }
        "svg" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("svg");
            let mut doc = Document::new(name);
            let page = doc.add_page("Page 1");
            designos_svg::import_svg_named(&text, name, &mut doc, page, Vec2::ZERO).map_err(|e| format!("Cannot import SVG: {e}"))?;
            doc
        }
        _ => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            let (d, w) = parse_native_with_warning(path, &text, record_once).map_err(|e| format!("Invalid designos file: {e}"))?;
            warning = w;
            d
        }
    };
    // The editor can keep working on a file whose ids leave no room for new ones: it gets fresh
    // ids, where the MCP server refuses such a file.
    if doc.validate_and_repair().id_space_exhausted {
        doc.renumber_ids_for_headroom();
    }
    if doc.name.is_empty() {
        doc.name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string();
    }
    ensure_page(&mut doc);
    Ok((doc, warning))
}

/// Toast text after a successful open; carries the import warning so it reaches the user.
pub fn opened_toast_text(name: &str, warning: Option<&str>) -> String {
    match warning {
        Some(w) => format!("Opened {name} with a warning: {w}"),
        None => format!("Opened {name}"),
    }
}

pub fn ensure_page(doc: &mut Document) {
    doc.pages.retain(|p| doc.nodes.contains_key(p));
    if doc.pages.is_empty() {
        doc.add_page("Page 1");
    }
}

/// Path of the `<file>.bak` (or `<file>.bak.1`, `<file>.bak.2`, … when `<file>.bak` already exists)
/// backup written before saving a document that was repaired on load.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut base = path.as_os_str().to_os_string();
    base.push(".bak");
    let first = PathBuf::from(&base);
    if first.symlink_metadata().is_err() {
        return first;
    }
    for n in 1u64.. {
        let mut numbered = base.clone();
        numbered.push(format!(".{n}"));
        let candidate = PathBuf::from(numbered);
        if candidate.symlink_metadata().is_err() {
            return candidate;
        }
    }
    unreachable!("u64 backup index exhausted")
}

/// If `path` was structurally repaired when loaded, writes its pre-repair bytes to `<path>.bak`.
pub fn write_pending_repair_backup(path: &Path) -> Result<bool, String> {
    let mut p = path.to_path_buf();
    if p.extension().is_none() {
        p.set_extension(NATIVE_EXT);
    }
    let key = repair_key(&p);
    let pending_backup = repaired_originals().lock().ok().and_then(|map| map.get(&key).cloned());
    if let Some(original_bytes) = &pending_backup {
        let bak = backup_path(&p);
        designos_library::write_atomic(&bak, original_bytes).map_err(|e| format!("Cannot write {}: {e}", bak.display()))?;
        return Ok(true);
    }
    Ok(false)
}

/// Drops the pending pre-repair backup bytes for `path` after a save has succeeded.
pub fn clear_pending_repair_backup(path: &Path) {
    let mut p = path.to_path_buf();
    if p.extension().is_none() {
        p.set_extension(NATIVE_EXT);
    }
    let key = repair_key(&p);
    if let Ok(mut map) = repaired_originals().lock() {
        map.remove(&key);
    }
}

/// Writes the native format (or `.fig` binary / `.pen` JSON when the extension matches); returns the final path.
pub fn save_path(doc: &Document, path: &Path) -> Result<PathBuf, String> {
    let (p, replaced) = save_path_counted(doc, path)?;
    if replaced > 0 {
        eprintln!("designos: {}: wrote {replaced} non-finite numbers as their neutral value", p.display());
    }
    Ok(p)
}

/// [`save_path`], also returning how many non-finite numbers the native format wrote as their neutral
/// value, counted by the one serialization the save performs (`.fig` and `.pen` exports replace none).
pub fn save_path_counted(doc: &Document, path: &Path) -> Result<(PathBuf, usize), String> {
    let mut p = path.to_path_buf();
    if p.extension().is_none() {
        p.set_extension(NATIVE_EXT);
    }
    let mut replaced = 0;
    let payload = if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("fig")) {
        designos_fig::export_fig(doc).map_err(|e| format!("Cannot export .fig: {e}"))?
    } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pen")) {
        designos_fig::export_pen_json(doc).map_err(|e| format!("Cannot export .pen: {e}"))?.into_bytes()
    } else {
        let (json, n) = doc.to_json_counted().map_err(|e| format!("Cannot serialize: {e}"))?;
        replaced = n;
        json.into_bytes()
    };

    let wrote_backup = write_pending_repair_backup(&p)?;
    designos_library::write_atomic(&p, &payload).map_err(|e| format!("Cannot write {}: {e}", p.display()))?;
    if wrote_backup {
        clear_pending_repair_backup(&p);
    }
    Ok((p, replaced))
}

pub fn pick_open() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Design files", &[NATIVE_EXT, LEGACY_EXT, "fig", "pen", "json", "svg"])
        .add_filter("design:os studio", &[NATIVE_EXT, LEGACY_EXT])
        .add_filter("Figma file", &["fig"])
        .add_filter("Pencil file", &["pen"])
        .add_filter("SVG", &["svg"])
        .add_filter("JSON", &["json"])
        .pick_file()
}

pub fn pick_save(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("design:os studio", &[NATIVE_EXT, LEGACY_EXT])
        .add_filter("Figma file", &["fig"])
        .add_filter("Pencil file", &["pen"])
        .set_file_name(format!("{default_name}.{NATIVE_EXT}"))
        .save_file()
}

pub fn pick_export(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name(format!("{default_name}.png")).save_file()
}

/// Like [`save_path`], also returning the once-per-file notice when non-finite numbers were replaced.
/// The app saves through [`save_path_counted`] so it can repair the open document; tests use this wrapper.
#[cfg(test)]
pub fn save_path_with_notice(doc: &Document, path: &Path) -> Result<(PathBuf, Option<String>), String> {
    let (p, replaced) = save_path_counted(doc, path)?;
    let notice = non_finite_notice_once(&p, replaced);
    Ok((p, notice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect};

    fn tmp(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("designos_test_{}_{name}", std::process::id()));
        p
    }

    /// Chained `ref`s doubling at every level: 2^24 clones if unbudgeted, far past the import budget.
    fn pen_past_the_clone_budget() -> String {
        let mut children = vec![r##"{"id":"c24","type":"frame","reusable":true,"children":[]}"##.to_string()];
        for i in (0..24).rev() {
            let next = i + 1;
            children.push(format!(
                r##"{{"id":"c{i}","type":"frame","reusable":true,"children":[{{"id":"a{i}","type":"ref","ref":"c{next}"}},{{"id":"b{i}","type":"ref","ref":"c{next}"}}]}}"##
            ));
        }
        format!(r##"{{"version":"1.0","children":[{}]}}"##, children.join(","))
    }

    #[test]
    fn an_import_warning_is_returned_for_pen_files_and_for_pen_text_in_a_json_file() {
        let warn = |_: &str| Ok((Document::new("Pencil Document"), Some("3 instance(s) left unexpanded".to_string())));
        for name in ["warned.pen", "warned.json"] {
            let p = tmp(name);
            std::fs::write(&p, r#"{"version":"1.0","children":[]}"#).unwrap();
            let (doc, warning) = load_path_importing_pen_with(&p, warn).unwrap();
            std::fs::remove_file(&p).ok();
            assert_eq!(warning.as_deref(), Some("3 instance(s) left unexpanded"), "{name}");
            assert!(!doc.pages.is_empty(), "{name}: the document still opens");
            let toast = opened_toast_text(&doc.name, warning.as_deref());
            assert!(toast.starts_with("Opened ") && toast.ends_with("3 instance(s) left unexpanded"), "{toast}");
        }
    }

    /// End to end through the real importer; builds about 2M nodes (~16 s in a debug build), so it runs on demand.
    #[test]
    #[ignore = "allocates the full 2M-node clone budget; run with --ignored"]
    fn pen_clone_budget_warning_reaches_the_caller_and_the_toast_text() {
        let p = tmp("budget.pen");
        std::fs::write(&p, pen_past_the_clone_budget()).unwrap();
        let (doc, warning) = load_path_with_warning(&p).unwrap();
        std::fs::remove_file(&p).ok();
        let warning = warning.expect("an exhausted clone budget must come back as a warning, not only on stderr");
        assert!(warning.contains("expand past"), "{warning}");
        assert!(!doc.pages.is_empty(), "the document still opens");
        let toast = opened_toast_text(&doc.name, Some(&warning));
        assert!(toast.contains(&doc.name) && toast.contains(&warning), "{toast}");
    }

    #[test]
    fn documents_without_a_warning_open_with_the_plain_toast() {
        let p = tmp("plain.pen");
        std::fs::write(&p, r##"{"version":"1.0","children":[{"id":"c","type":"frame","reusable":true},{"id":"i","type":"ref","ref":"c"}]}"##).unwrap();
        let (_, warning) = load_path_with_warning(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert!(warning.is_none());
        assert_eq!(opened_toast_text("Doc", None), "Opened Doc");
    }

    #[test]
    fn file_with_a_node_id_at_u64_max_opens_and_new_ids_stay_distinct() {
        let mut d = Document::new("Maxed");
        let page = d.add_page("P");
        d.insert(page, Node::rectangle(designos_doc::NodeId(u64::MAX), Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
        let p = tmp("maxid.designos");
        std::fs::write(&p, d.to_json().unwrap()).unwrap();
        let mut back = load_path(&p).unwrap();
        std::fs::remove_file(&p).ok();

        assert!(back.nodes.keys().all(|id| id.0 < u64::MAX / 2), "ids were compacted on open");
        let page = back.pages[0];
        let before = back.nodes.len();
        let ids = [back.alloc_id(), back.alloc_id(), back.alloc_id()];
        for id in ids {
            back.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
        }
        assert_eq!(back.nodes.len(), before + 3, "three pasted nodes stay three nodes");
        let children = &back.get(page).unwrap().children;
        assert_eq!(children.iter().collect::<std::collections::BTreeSet<_>>().len(), children.len());
        assert!(back.validate_and_repair().is_clean());
    }

    #[test]
    fn native_roundtrip() {
        let mut d = Document::new("Saved");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(1.0, 2.0, 3.0, 4.0))).unwrap();
        let p = save_path(&d, &tmp("a")).unwrap();
        assert_eq!(p.extension().unwrap(), NATIVE_EXT);
        let back = load_path(&p).unwrap();
        assert_eq!(back.name, "Saved");
        assert_eq!(back.nodes.len(), 2);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn json_falls_back_to_native() {
        let mut d = Document::new("J");
        d.add_page("P");
        let p = tmp("b.json");
        std::fs::write(&p, d.to_json().unwrap()).unwrap();
        let back = load_path(&p).unwrap();
        assert_eq!(back.name, "J");
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn errors_are_readable() {
        let p = tmp("c.designos");
        std::fs::write(&p, "not json").unwrap();
        assert!(load_path(&p).unwrap_err().contains("Invalid designos file"));
        std::fs::remove_file(p).ok();
        assert!(load_path(Path::new("/definitely/missing.fig")).is_err());
    }

    #[test]
    fn svg_loads_as_a_document_named_after_the_file() {
        let p = tmp("icon.svg");
        std::fs::write(&p, r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#f00"/></svg>"##).unwrap();
        let d = load_path(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert!(d.name.ends_with("icon"));
        assert_eq!(d.pages.len(), 1);
        let page = d.get(d.pages[0]).unwrap();
        assert_eq!(page.name, "Page 1");
        assert_eq!(page.children.len(), 1);
        assert_eq!(d.get(page.children[0]).unwrap().name, d.name);
    }

    #[test]
    fn empty_doc_gets_a_page() {
        let mut d = Document::new("E");
        ensure_page(&mut d);
        assert_eq!(d.pages.len(), 1);
    }

    #[test]
    fn fig_roundtrip() {
        let mut d = Document::new("FigDoc");
        let page = d.add_page("Page 1");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(10.0, 20.0, 120.0, 80.0))).unwrap();
        let p = save_path(&d, &tmp("roundtrip.fig")).unwrap();
        assert_eq!(p.extension().unwrap(), "fig");
        let raw = std::fs::read(&p).unwrap();
        assert!(raw.starts_with(b"PK") || raw.starts_with(b"fig-kiwi"));
        let back = load_path(&p).unwrap();
        assert!(!back.pages.is_empty());
        assert!(back.nodes.len() >= 2);
        std::fs::remove_file(p).ok();
    }

    #[cfg(unix)]
    #[test]
    fn failed_save_leaves_original_intact() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("readonly_dir");
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = Document::new("Old");
        d.add_page("P");
        for name in ["keep.designos", "keep.fig", "keep.pen"] {
            let target = dir.join(name);
            std::fs::write(&target, b"original bytes").unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
            let res = save_path(&d, &target);
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            if std::fs::File::create(dir.join("probe")).is_ok() {
                std::fs::remove_file(dir.join("probe")).ok();
            } else {
                panic!("cannot restore dir permissions");
            }
            assert!(res.is_err(), "{name}: save into a read-only dir must fail");
            assert_eq!(std::fs::read(&target).unwrap(), b"original bytes", "{name}: original must be intact");
            let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
            assert!(leftovers.iter().all(|n| !n.to_string_lossy().ends_with(".tmp")), "{name}: tmp file left behind: {leftovers:?}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cyclic_file_loads_into_a_consistent_tree() {
        let mut d = Document::new("Cyc");
        let page = d.add_page("P");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
        d.nodes.get_mut(&a).unwrap().children.push(page);
        d.nodes.get_mut(&page).unwrap().parent = Some(a);
        let p = tmp("cyc.designos");
        std::fs::write(&p, d.to_json().unwrap()).unwrap();
        let back = load_path(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(back.page_of(a), Some(page));
        let _ = back.world_transform(a);
    }

    #[test]
    fn malformed_native_open_reports_what_was_repaired_and_backs_up_original_bytes_on_save() {
        let mut d = Document::new("Damaged");
        let page = d.add_page("P");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        // Inject a dangling child id and a self/page cycle so load-time repair has to strip them.
        d.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(999));
        d.nodes.get_mut(&a).unwrap().children.push(page);
        d.nodes.get_mut(&page).unwrap().parent = Some(a);

        let p = tmp("damaged_backup.designos");
        let bak = backup_path(&p);
        std::fs::remove_file(&bak).ok();
        let original_bytes = d.to_json().unwrap().into_bytes();
        std::fs::write(&p, &original_bytes).unwrap();

        let (mut doc, warning) = load_path_with_warning(&p).unwrap();
        let warning = warning.expect("malformed native open must return a repair warning");
        assert!(warning.contains("1 dangling ids"), "warning should list dangling ids: {warning}");
        assert!(warning.contains("duplicate or cyclic child claims"), "warning should list cyclic claims: {warning}");
        let toast = opened_toast_text(&doc.name, Some(&warning));
        assert!(toast.starts_with("Opened Damaged with a warning:"), "{toast}");
        assert!(!bak.exists(), "backup is only written when saving");

        // First save writes <file>.bak with the untouched pre-repair bytes before rewriting <file>.
        save_path(&doc, &p).unwrap();
        assert_eq!(std::fs::read(&bak).unwrap(), original_bytes, "original pre-repair bytes must be recoverable from <file>.bak after saving");
        let saved_repaired = std::fs::read(&p).unwrap();
        assert_ne!(saved_repaired, original_bytes, "saved file holds the repaired document");

        // A second save updates <file> without overwriting <file>.bak with the already-repaired bytes.
        doc.name = "Damaged Edited".to_string();
        save_path(&doc, &p).unwrap();
        assert_eq!(std::fs::read(&bak).unwrap(), original_bytes, "second save must preserve the original pre-repair backup");

        std::fs::remove_file(&p).ok();
        std::fs::remove_file(&bak).ok();
    }

    #[test]
    fn clean_native_open_has_no_warning_and_writes_no_backup_on_save() {
        let mut d = Document::new("Clean");
        let page = d.add_page("P");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

        let p = tmp("clean_no_backup.designos");
        let bak = backup_path(&p);
        std::fs::remove_file(&bak).ok();
        std::fs::write(&p, d.to_json().unwrap()).unwrap();

        let (doc, warning) = load_path_with_warning(&p).unwrap();
        assert!(warning.is_none(), "clean file must not warn: {warning:?}");
        save_path(&doc, &p).unwrap();
        assert!(!bak.exists(), "clean file must not write a .bak file on save");

        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn non_finite_numbers_notice_shown_once_per_file_on_open_import_and_save() {
        let mut poisoned = Document::new("Poisoned");
        let page = poisoned.add_page("Page 1");
        let id = poisoned.alloc_id();
        poisoned.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        {
            let n = poisoned.get_mut(id).unwrap();
            n.size.x = f32::NAN;
            n.opacity = f32::NAN;
            n.transform.tx = f32::INFINITY;
        }

        // 1. Open: a legacy file containing 3 `null` numbers shows the notice on first open, and not on second open.
        let legacy_json = serde_json::to_string(&poisoned).unwrap();
        let open_file = tmp("legacy_null_numbers.designos");
        std::fs::write(&open_file, &legacy_json).unwrap();

        let mut app = crate::app::App::new(Document::new("Init"));
        app.open_path(open_file.clone());
        let toast1 = app.toast.as_ref().expect("first open must show a toast");
        assert!(toast1.error, "non-finite replacement toast should use warning styling");
        assert!(toast1.text.contains("3 numbers in this file were not valid and were replaced by 0 or 1"), "unexpected open toast: {}", toast1.text);
        assert!(!backup_path(&open_file).exists(), "non-finite numbers alone must not trigger a .bak backup");

        // Second open of the same file in the same run must not repeat the notice.
        app.open_path(open_file.clone());
        let toast2 = app.toast.as_ref().expect("second open must still show opened toast");
        assert!(!toast2.error, "second open of the same file must not repeat warning: {}", toast2.text);
        assert_eq!(toast2.text, "Opened Poisoned");
        std::fs::remove_file(&open_file).ok();

        // 2. Import into library & library open/save:
        let lib_dir = tmp("non_finite_lib");
        let _ = std::fs::remove_dir_all(&lib_dir);
        app.attach_library(lib_dir.clone());
        let lib = app.lib.clone().expect("library attached");
        let proj = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("P").unwrap());

        let overflow_rest = r#"{
          "name": "Overflow", "version": "1", "schemaVersion": 0,
          "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
            {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
              {"id": "1:1", "name": "Huge", "type": "RECTANGLE", "size": {"x": 1e39, "y": 20},
               "relativeTransform": [[1, 0, -1e39], [0, 1, 4]], "strokeWeight": 1e39, "cornerRadius": 1e39}]}]}
        }"#;
        let import_src = tmp("overflow_import.json");
        std::fs::write(&import_src, overflow_rest).unwrap();

        app.toast = None;
        assert_eq!(app.import_into_library(std::slice::from_ref(&import_src), &proj.id, None), 1);
        let import_toast = app.toast.as_ref().expect("import must show a toast");
        assert!(import_toast.error, "import with replaced numbers must surface warning styling");
        assert!(
            import_toast.text.contains("numbers in this file were not valid and were replaced by 0 or 1"),
            "unexpected import toast: {}",
            import_toast.text
        );

        // Importing the same source file again in the same run does not repeat the notice for that source path.
        app.toast = None;
        assert_eq!(app.import_into_library(std::slice::from_ref(&import_src), &proj.id, None), 1);
        let import_toast2 = app.toast.as_ref().expect("second import must show a toast");
        assert!(!import_toast2.error, "second import of same file in same run must not repeat warning: {}", import_toast2.text);
        std::fs::remove_file(&import_src).ok();

        // 3. Save: saving a library file (or external path) that acquired a NaN in memory warns once per file.
        let clean_entry = lib.create_file(&proj.id, None, "Editable", &Document::new("Editable")).unwrap();
        app.open_library_file(&clean_entry.id);
        let page = app.page;
        let rid = app.doc.alloc_id();
        app.doc.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        app.doc.get_mut(rid).unwrap().size.x = f32::NAN;

        app.toast = None;
        app.save_library_file_with_toast();
        let save_toast = app.toast.as_ref().expect("save must show a toast");
        assert!(save_toast.error, "save that replaced a NaN must warn");
        assert!(save_toast.text.contains("1 number in this file was not valid and was replaced by 0 or 1"), "unexpected save toast: {}", save_toast.text);

        // Saving the same file a second time in the same run (even with another NaN) must not repeat the notice.
        app.doc.get_mut(rid).unwrap().size.x = f32::NAN;
        app.toast = None;
        app.save_library_file_with_toast();
        let save_toast2 = app.toast.as_ref().expect("second save must show a toast");
        assert!(!save_toast2.error, "second save of the same file must not repeat the notice: {}", save_toast2.text);

        let _ = std::fs::remove_dir_all(&lib_dir);
    }

    #[test]
    fn saving_repaired_document_never_overwrites_existing_bak_files() {
        let mut d = Document::new("Damaged");
        let page = d.add_page("P");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(999));

        let p = tmp("damaged_existing_bak.designos");
        let bak0 = PathBuf::from(format!("{}.bak", p.display()));
        let bak1 = PathBuf::from(format!("{}.bak.1", p.display()));
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(&bak0).ok();
        std::fs::remove_file(&bak1).ok();

        let pre_existing_bak = b"earlier-user-backup-bytes";
        std::fs::write(&bak0, pre_existing_bak).unwrap();
        let damaged_bytes = d.to_json().unwrap().into_bytes();
        std::fs::write(&p, &damaged_bytes).unwrap();

        let (doc, warning) = load_path_with_warning(&p).unwrap();
        let warning = warning.expect("malformed native open must return a repair warning");
        assert!(warning.contains("damaged_existing_bak.designos.bak.1"), "warning must name the numbered backup slot when .bak already exists: {warning}");

        save_path(&doc, &p).unwrap();
        assert_eq!(std::fs::read(&bak0).unwrap(), pre_existing_bak, "existing .bak must never be overwritten");
        assert_eq!(std::fs::read(&bak1).unwrap(), damaged_bytes, "new backup must be written to .bak.1");

        std::fs::remove_file(&p).ok();
        std::fs::remove_file(&bak0).ok();
        std::fs::remove_file(&bak1).ok();
    }

    /// A Figma REST file with one rectangle; `stroke` and `radius` are raw JSON numbers, so `1e39`
    /// overflows to infinity when the importer reads it as an `f32`.
    fn rest_json_with(stroke: &str, radius: &str) -> String {
        format!(
            r#"{{"name": "Shapes", "version": "1", "schemaVersion": 0,
              "document": {{"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
                {{"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
                  {{"id": "1:1", "name": "Box", "type": "RECTANGLE", "size": {{"x": 10, "y": 20}},
                   "relativeTransform": [[1, 0, 0], [0, 1, 4]], "strokeWeight": {stroke}, "cornerRadius": {radius}}}]}}]}}}}"#
        )
    }

    fn json_texts_built_by(f: impl FnOnce()) -> usize {
        let before = designos_doc::finite_json::serialize_calls_on_this_thread();
        f();
        designos_doc::finite_json::serialize_calls_on_this_thread() - before
    }

    #[test]
    fn opening_a_finite_rest_json_builds_no_native_json_text() {
        let p = tmp("finite_rest_open.json");
        std::fs::write(&p, rest_json_with("2", "3")).unwrap();
        let mut warning = Some("unset".to_string());
        let built = json_texts_built_by(|| warning = load_path_with_warning(&p).unwrap().1);
        std::fs::remove_file(&p).ok();
        assert_eq!(warning, None, "nothing was replaced, so nothing to tell");
        assert_eq!(built, 0, "a clean REST open must not serialize the document just to count");
    }

    #[test]
    fn opening_a_rest_json_with_overflowing_numbers_replaces_them_and_tells_the_user_once() {
        let p = tmp("overflow_rest_open.json");
        std::fs::write(&p, rest_json_with("1e39", "1e39")).unwrap();
        let (doc, warning) = load_path_with_warning(&p).unwrap();
        let again = load_path_with_warning(&p).unwrap().1;
        std::fs::remove_file(&p).ok();
        assert!(warning.as_deref().is_some_and(|w| w.contains("replaced by 0 or 1")), "{warning:?}");
        assert_eq!(doc.count_non_finite().unwrap(), 0, "the opened document holds only finite numbers");
        assert_eq!(again, None, "the second open in the same run stays quiet");
    }

    #[test]
    fn saving_to_a_path_builds_one_json_text_and_reports_the_replaced_numbers() {
        let mut doc = Document::new("Nan");
        let page = doc.add_page("P");
        let id = doc.alloc_id();
        doc.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        doc.get_mut(id).unwrap().size.x = f32::NAN;
        let p = tmp("one_text_per_save.designos");
        let mut notice = None;
        let built = json_texts_built_by(|| notice = save_path_with_notice(&doc, &p).unwrap().1);
        std::fs::remove_file(&p).ok();
        assert!(notice.as_deref().is_some_and(|w| w.starts_with("1 number in this file")), "{notice:?}");
        assert_eq!(built, 1, "counting the replaced numbers and writing the file must share one serialization");
    }

    #[test]
    fn importing_several_files_names_each_one_that_had_numbers_replaced() {
        let lib_dir = tmp("import_many_lib");
        let _ = std::fs::remove_dir_all(&lib_dir);
        let mut app = crate::app::App::new(Document::new("Init"));
        app.attach_library(lib_dir.clone());
        let lib = app.lib.clone().expect("library attached");
        let proj = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("P").unwrap());

        let files = [
            (tmp("import_stroke_overflow.json"), rest_json_with("1e39", "3")),
            (tmp("import_radius_overflow.json"), rest_json_with("2", "1e39")),
            (tmp("import_all_finite.json"), rest_json_with("2", "3")),
        ];
        for (path, text) in &files {
            std::fs::write(path, text).unwrap();
        }
        let paths: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(app.import_into_library(&paths, &proj.id, None), 3);

        let toast = app.toast.as_ref().expect("import must show a toast");
        for (path, _) in &files {
            std::fs::remove_file(path).ok();
        }
        let _ = std::fs::remove_dir_all(&lib_dir);
        assert!(toast.error, "{}", toast.text);
        for named in ["import_stroke_overflow.json", "import_radius_overflow.json"] {
            assert!(toast.text.contains(named), "the toast must name {named}: {}", toast.text);
        }
        assert!(!toast.text.contains("import_all_finite.json"), "a clean file is not mentioned: {}", toast.text);
        assert_eq!(toast.text.matches("replaced by 0 or 1").count(), 2, "one notice per affected file: {}", toast.text);
    }

    #[test]
    fn counted_save_neutralizes_open_document_and_saved_hash_matches_disk() {
        let lib_dir = tmp("counted_save_neutral_lib");
        let _ = std::fs::remove_dir_all(&lib_dir);
        let mut app = crate::app::App::new(Document::new("Init"));
        app.attach_library(lib_dir.clone());
        let lib = app.lib.clone().expect("library attached");
        let proj = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("P").unwrap());
        let entry = lib.create_file(&proj.id, None, "PoisonedSave", &Document::new("PoisonedSave")).unwrap();
        app.open_library_file(&entry.id);

        let page = app.page;
        let rid = app.doc.alloc_id();
        app.doc.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        app.doc.get_mut(rid).unwrap().size.x = f32::NAN;
        app.doc.get_mut(rid).unwrap().opacity = f32::NAN;

        app.save_library_file_with_toast();
        let n = app.doc.get(rid).unwrap();
        assert_eq!((n.size.x, n.opacity), (0.0, 1.0), "open document must hold the neutral values written to disk");
        assert_eq!(app.export_doc().get(rid).unwrap().size.x, 0.0);

        // External save (`App::save`) and `save_to_library_as` must also neutralize the open document.
        let ext_path = tmp("counted_ext_save.designos");
        app.lib_file = None;
        app.file_path = Some(ext_path.clone());
        app.doc.get_mut(rid).unwrap().size.y = f32::INFINITY;
        app.save(false);
        assert_eq!(app.doc.get(rid).unwrap().size.y, 0.0, "App::save must neutralize non-finite numbers in self.doc");
        std::fs::remove_file(&ext_path).ok();

        app.doc.get_mut(rid).unwrap().transform.tx = f32::NEG_INFINITY;
        app.save_to_library_as(&proj.id, None, "SavedAsCopy");
        assert_eq!(app.doc.get(rid).unwrap().transform.tx, 0.0, "save_to_library_as must neutralize non-finite numbers in self.doc");

        let _ = std::fs::remove_dir_all(&lib_dir);
    }

    #[test]
    fn hot_reload_surfaces_non_finite_warning_even_when_repaired_hash_matches_saved_hash() {
        let ext_path = tmp("hot_reload_null_opacity.designos");
        let mut doc = Document::new("HotReloadDoc");
        let page = doc.add_page("Page 1");
        let rid = doc.alloc_id();
        doc.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 40.0, 30.0))).unwrap();
        std::fs::write(&ext_path, doc.to_json().unwrap()).unwrap();

        let mut app = crate::app::App::new(Document::new("Init"));
        app.open_path(ext_path.clone());
        app.toast = None;

        // First frame seeds `autosave.disk_mtime` and `saved_hash`.
        let ctx = egui::Context::default();
        app.now = 1.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });

        // External tool rewrites `opacity: 1.0` to `opacity: null`, which repairs back to `1.0` (same doc_hash!).
        let mut poisoned = app.doc.clone();
        poisoned.get_mut(rid).unwrap().opacity = f32::NAN;
        let legacy_null = serde_json::to_string(&poisoned).unwrap();
        assert!(legacy_null.contains("\"opacity\":null"));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&ext_path, &legacy_null).unwrap();

        app.now = 2.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });

        std::fs::remove_file(&ext_path).ok();
        let toast = app.toast.as_ref().expect("hot reload must show the non-finite warning even when repaired doc_hash matches");
        assert!(toast.error, "hot reload warning toast must be error-styled: {}", toast.text);
        assert!(toast.text.contains("1 number in this file was not valid and was replaced by 0 or 1"), "unexpected hot reload toast: {}", toast.text);
    }

    #[test]
    fn switching_documents_preserves_flush_autosave_non_finite_notice() {
        let lib_dir = tmp("switch_preserves_notice_lib");
        let _ = std::fs::remove_dir_all(&lib_dir);
        let mut app = crate::app::App::new(Document::new("Init"));
        app.attach_library(lib_dir.clone());
        let lib = app.lib.clone().expect("library attached");
        let proj = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("P").unwrap());
        let entry = lib.create_file(&proj.id, None, "FirstLibFile", &Document::new("FirstLibFile")).unwrap();
        app.open_library_file(&entry.id);

        let page = app.page;
        let rid = app.doc.alloc_id();
        app.doc.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        app.doc.get_mut(rid).unwrap().size.x = f32::NAN;

        let other_path = tmp("other_clean_switch.designos");
        let mut other_doc = Document::new("Other");
        other_doc.add_page("Page 1");
        std::fs::write(&other_path, other_doc.to_json().unwrap()).unwrap();

        app.toast = None;
        app.open_path(other_path.clone());
        std::fs::remove_file(&other_path).ok();
        let _ = std::fs::remove_dir_all(&lib_dir);

        let toast = app.toast.as_ref().expect("open_path must show a toast");
        assert!(toast.error, "flush notice from previous library file must keep error styling: {}", toast.text);
        assert!(toast.text.contains("Opened Other"), "toast must still mention Opened Other: {}", toast.text);
        assert!(
            toast.text.contains("1 number in this file was not valid and was replaced by 0 or 1"),
            "toast must retain the flushed non-finite notice: {}",
            toast.text
        );
    }

    #[test]
    fn opening_rest_json_with_overflow_neutralizes_in_place_without_running_migrate_text() {
        let rest_with_bullets = |stroke: &str| {
            format!(
                r#"{{"name": "RestBullets", "version": "1", "schemaVersion": 0,
                  "document": {{"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
                    {{"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
                      {{"id": "1:1", "name": "Bullets", "type": "TEXT", "characters": "\u2022 one\n\u2022 two",
                       "size": {{"x": 100, "y": 40}}, "relativeTransform": [[1, 0, 0], [0, 1, 0]], "strokeWeight": {stroke}}}]}}]}}}}"#
            )
        };
        let p_clean = tmp("rest_bullets_clean.json");
        let p_overflow = tmp("rest_bullets_overflow.json");
        std::fs::write(&p_clean, rest_with_bullets("2")).unwrap();
        std::fs::write(&p_overflow, rest_with_bullets("1e39")).unwrap();

        let (doc_clean, warn_clean) = load_path_with_warning(&p_clean).unwrap();
        let (doc_overflow, warn_overflow) = load_path_with_warning(&p_overflow).unwrap();
        std::fs::remove_file(&p_clean).ok();
        std::fs::remove_file(&p_overflow).ok();

        assert!(warn_clean.is_none());
        assert!(warn_overflow.is_some());
        let text_clean = doc_clean
            .nodes
            .values()
            .find_map(|n| match &n.kind {
                designos_doc::NodeKind::Text(t) => Some(t),
                _ => None,
            })
            .unwrap();
        let text_overflow = doc_overflow
            .nodes
            .values()
            .find_map(|n| match &n.kind {
                designos_doc::NodeKind::Text(t) => Some(t),
                _ => None,
            })
            .unwrap();
        assert_eq!(text_overflow.content, text_clean.content, "overflow neutralization must not strip literal bullets via migrate_text");
        assert_eq!(text_overflow.lists, text_clean.lists);
    }

    #[test]
    fn opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast() {
        let mut children = Vec::with_capacity(1421);
        children.push(
            r#"{"id": "1:0", "name": "C0", "type": "COMPONENT", "size": {"x": 10, "y": 10}, "relativeTransform": [[1, 0, 0], [0, 1, 0]], "children": [{"id": "2:0", "name": "R0", "type": "RECTANGLE", "size": {"x": 4, "y": 4}, "relativeTransform": [[1, 0, 0], [0, 1, 0]]}]}"#
                .to_string(),
        );
        for k in 1..=1420 {
            let prev = k - 1;
            children.push(format!(
                r#"{{"id": "1:{k}", "name": "C{k}", "type": "COMPONENT", "size": {{"x": 10, "y": 10}}, "relativeTransform": [[1, 0, 0], [0, 1, 0]], "children": [{{"id": "3:{k}", "name": "I{k}", "type": "INSTANCE", "componentId": "1:{prev}", "size": {{"x": 10, "y": 10}}, "relativeTransform": [[1, 0, 0], [0, 1, 0]], "children": []}}, {{"id": "2:{k}", "name": "R{k}", "type": "RECTANGLE", "size": {{"x": 4, "y": 4}}, "relativeTransform": [[1, 0, 0], [0, 1, 0]]}}]}}"#
            ));
        }
        let json = format!(
            r#"{{"name": "BudgetKit", "version": "1", "schemaVersion": 0, "document": {{"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [{{"id": "0:1", "name": "Page 1", "type": "CANVAS", "children": [{}]}}]}}}}"#,
            children.join(",")
        );
        let p = tmp("budget_kit_warning.json");
        std::fs::write(&p, json).unwrap();
        let mut app = crate::app::App::new(Document::new("Init"));
        app.open_path(p.clone());
        std::fs::remove_file(&p).ok();

        let toast = app.toast.as_ref().expect("open_path must show a toast");
        assert_eq!(toast.text, "Opened BudgetKit with a warning: 5 instances were left unexpanded to keep the import fast.");
    }
}
