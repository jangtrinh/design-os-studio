//! Disk operations behind the Project tab, written against `designos-docs` and run off the UI thread.
//!
//! Everything here is plain functions over a [`DocsStore`], so the tab's behaviour (rename writes the file and the
//! manifest, delete moves the page to Studio's trash, reorder persists) is testable without a window. A function
//! returns an error string that names the next action; the tab shows it as a toast.

use designos_docs::{BlockKind, ContentHash, DocsError, DocsStore, Expect, Group, Manifest, Page};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::SystemTime;

/// The description of the Overview page a new project starts with.
pub const OVERVIEW_DESCRIPTION: &str = "What this project is and where to start.";
/// `designos-docs` refuses to read a page over this size; the tab names the limit instead of a bare error.
pub const MAX_PAGE_BYTES: u64 = 8 * 1024 * 1024;
const UNTITLED: &str = "Untitled";

/// What the tree needs to know about one page without keeping its blocks.
#[derive(Clone, Debug, PartialEq)]
pub struct PageMeta {
    pub rel: String,
    pub title: String,
    pub description: Option<String>,
    /// Text of every level 1 to 3 heading, for search.
    pub headings: Vec<String>,
    /// Front matter values that could not be used, as one line each (field, problem, fix).
    pub problems: Vec<String>,
    /// Why the page could not be read (not UTF-8, over the limit, unreadable); the row still shows.
    pub error: Option<String>,
    /// The `group` key of the page's front matter.
    pub front_group: Option<Group>,
}

/// One design file of the project, for the "N files" line and the names on embed cards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeSnapshot {
    pub manifest: Manifest,
    /// Set when `docs-manifest.json` exists but cannot be used; the tree then shows every page unlisted and
    /// nothing writes the manifest until it is fixed.
    pub manifest_error: Option<String>,
    pub pages: BTreeMap<String, PageMeta>,
    pub files: Vec<FileInfo>,
}

impl TreeSnapshot {
    pub fn empty() -> Self {
        Self { manifest: Manifest::new(), manifest_error: None, pages: BTreeMap::new(), files: Vec::new() }
    }

    /// Pages of `group` in tree order: the manifest's order first (a listed page whose file is missing stays, so
    /// nothing disappears silently), then pages on disk that no group lists, in the group their front matter names
    /// (Discovery when it names none).
    pub fn group_pages(&self, group: Group) -> Vec<String> {
        let mut out: Vec<String> = self.manifest.group(group).pages.clone();
        for (rel, meta) in &self.pages {
            if self.is_listed(rel) {
                continue;
            }
            if meta.front_group.unwrap_or(Group::Discovery) == group {
                out.push(rel.clone());
            }
        }
        out
    }

    fn is_listed(&self, rel: &str) -> bool {
        self.manifest.groups.iter().any(|g| g.pages.iter().any(|p| p == rel))
    }

    pub fn title_of(&self, rel: &str) -> String {
        self.pages.get(rel).map(|m| m.title.clone()).unwrap_or_else(|| title_from_path(rel))
    }
}

/// `spec/payment-flow.md` -> `Payment flow`.
pub fn title_from_path(rel: &str) -> String {
    let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md");
    let words = stem.replace(['-', '_'], " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => UNTITLED.to_string(),
    }
}

/// A file name for `title`: lower-case letters and digits joined by `-` (non-ASCII letters stay).
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.extend(c.to_lowercase());
        } else {
            dash = true;
        }
    }
    if out.is_empty() {
        "untitled".into()
    } else {
        out.chars().take(80).collect()
    }
}

fn meta_of(rel: &str, page: &Page) -> PageMeta {
    let headings = page.headings();
    let front = page.front_matter();
    let title = front
        .and_then(|f| f.title())
        .map(str::to_string)
        .or_else(|| headings.iter().find(|h| h.level == 1).map(|h| h.text.clone()))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| title_from_path(rel));
    PageMeta {
        rel: rel.to_string(),
        title,
        description: front.and_then(|f| f.description()).map(str::to_string),
        headings: headings.into_iter().map(|h| h.text).collect(),
        problems: front.map(|f| f.problems().iter().map(|p| format!("{}: {}; fix: {}", p.field, p.problem, p.fix)).collect()).unwrap_or_default(),
        error: None,
        front_group: front.and_then(|f| f.group()),
    }
}

/// The title and description a page shows, and the block the title came from when it was the first level 1 heading.
pub struct PageHeader {
    pub title: String,
    pub description: Option<String>,
    pub consumed_h1: Option<usize>,
}

pub fn meta_title_and_description(rel: &str, page: &Page) -> PageHeader {
    let meta = meta_of(rel, page);
    let has_title = page.front_matter().and_then(|f| f.title()).is_some_and(|t| !t.trim().is_empty());
    let consumed_h1 = if has_title { None } else { page.headings().iter().find(|h| h.level == 1 && !h.text.trim().is_empty()).map(|h| h.block_index) };
    PageHeader { title: meta.title, description: meta.description, consumed_h1 }
}

fn error_meta(rel: &str, error: &DocsError) -> PageMeta {
    PageMeta {
        rel: rel.to_string(),
        title: title_from_path(rel),
        description: None,
        headings: Vec::new(),
        problems: Vec::new(),
        error: Some(error.to_string()),
        front_group: None,
    }
}

struct Cached {
    stat: (Option<SystemTime>, u64),
    hash: ContentHash,
    meta: PageMeta,
}

/// Builds [`TreeSnapshot`]s, re-reading only the pages whose file changed since the last one.
pub struct Scanner {
    store: DocsStore,
    library: Option<(designos_library::Library, String)>,
    cache: HashMap<String, Cached>,
}

impl Scanner {
    pub fn new(store: DocsStore, library: Option<(designos_library::Library, String)>) -> Self {
        Self { store, library, cache: HashMap::new() }
    }

    pub fn store(&self) -> &DocsStore {
        &self.store
    }

    /// Forgets what was read for `rel` unless the file still has `hash` (the watcher reports Studio's own writes too).
    pub fn invalidate(&mut self, rel: &str, hash: Option<ContentHash>) {
        if self.cache.get(rel).is_some_and(|c| Some(c.hash) == hash) {
            return;
        }
        self.cache.remove(rel);
    }

    pub fn snapshot(&mut self) -> Result<TreeSnapshot, String> {
        let files = self.store.list_page_files().map_err(|e| e.to_string())?;
        let mut pages = BTreeMap::new();
        for rel in files {
            let stat = std::fs::metadata(self.store.dir().join(&rel)).ok().map(|m| (m.modified().ok(), m.len()));
            if let (Some(c), Some(stat)) = (self.cache.get(&rel), stat) {
                if c.stat == stat {
                    pages.insert(rel.clone(), c.meta.clone());
                    continue;
                }
            }
            match self.store.read_page(&rel) {
                Ok(v) => {
                    let meta = meta_of(&rel, &v.value);
                    if let Some(stat) = stat {
                        self.cache.insert(rel.clone(), Cached { stat, hash: v.hash, meta: meta.clone() });
                    }
                    pages.insert(rel, meta);
                }
                Err(e) => {
                    self.cache.remove(&rel);
                    pages.insert(rel.clone(), error_meta(&rel, &e));
                }
            }
        }
        self.cache.retain(|rel, _| pages.contains_key(rel));
        let (manifest, manifest_error) = match self.store.read_manifest() {
            Ok(v) => (v.value, None),
            Err(DocsError::NotFound { .. }) => (Manifest::new(), None),
            Err(e) => (Manifest::new(), Some(e.to_string())),
        };
        let files = match &self.library {
            Some((lib, project)) => lib.files_in(project, None).unwrap_or_default().into_iter().map(|f| FileInfo { id: f.id, name: f.name }).collect(),
            None => Vec::new(),
        };
        Ok(TreeSnapshot { manifest, manifest_error, pages, files })
    }
}

/// First open of a project without docs: writes `overview.md` (title = project name) and the manifest. Returns the
/// page's path when it created one. A project that already has pages or a manifest is left alone.
pub fn ensure_first_page(store: &DocsStore, project_name: &str) -> Result<Option<String>, String> {
    if !store.list_page_files().map_err(|e| e.to_string())?.is_empty() {
        return Ok(None);
    }
    match store.read_manifest() {
        Ok(_) => return Ok(None),
        Err(DocsError::NotFound { .. }) => {}
        Err(e) => return Err(e.to_string()),
    }
    let rel = "overview.md";
    let mut page = Page::parse("");
    page.set_title(Some(if project_name.trim().is_empty() { "About this project" } else { project_name.trim() }));
    page.set_description(Some(OVERVIEW_DESCRIPTION));
    match store.write_page(rel, &page, Expect::Absent) {
        Ok(_) => {}
        // Another window or an agent created it between the listing and now: theirs stands.
        Err(DocsError::Conflict { .. }) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    }
    let mut manifest = Manifest::new();
    manifest.group_mut(Group::Overview).pages.push(rel.to_string());
    match store.write_manifest(&manifest, Expect::Absent) {
        Ok(_) | Err(DocsError::Conflict { .. }) => Ok(Some(rel.to_string())),
        Err(e) => Err(e.to_string()),
    }
}

/// Runs `edit` on the manifest as it is on disk now and writes it back (again from a fresh read when another
/// writer got in between), under the docs lock's hash check.
fn update_manifest(store: &DocsStore, edit: impl FnMut(&mut Manifest)) -> Result<(), String> {
    store.update_manifest(edit).map_err(|e| match e {
        DocsError::Conflict { .. } => "docs-manifest.json keeps changing under Studio; fix: close the other program writing it, then retry".to_string(),
        e => format!("{e}; nothing was changed"),
    })
}

/// Moves `order`'s pages to the front of `group` in that order (the order the tree shows, so pages no manifest
/// listed yet are adopted into it). It is applied to the manifest as it is now, never written as a stale full list:
/// a page another writer added meanwhile stays, after the ordered ones, and a page deleted meanwhile does not come back.
pub fn reorder_pages(store: &DocsStore, group: Group, order: &[String]) -> Result<(), String> {
    let existing = store.list_page_files().map_err(|e| e.to_string())?;
    update_manifest(store, |m| {
        let ordered: Vec<String> = order.iter().filter(|p| existing.contains(p)).cloned().collect();
        let kept: Vec<String> = m.group(group).pages.iter().filter(|p| !ordered.contains(p)).cloned().collect();
        for g in Group::ALL {
            if g != group {
                m.group_mut(g).pages.retain(|p| !ordered.contains(p));
            }
        }
        m.group_mut(group).pages = ordered.into_iter().chain(kept).collect();
    })
}

/// Where a new page of `group` goes. Screens and States hold pinned frames, not pages.
fn dir_of_group(group: Group) -> Option<&'static str> {
    match group {
        Group::Overview => Some(""),
        Group::Spec => Some("spec/"),
        Group::Discovery => Some("discovery/"),
        Group::Screens | Group::States => None,
    }
}

fn unique_rel(existing: &[String], dir: &str, slug: &str) -> String {
    let taken = |rel: &str| existing.iter().any(|e| e.eq_ignore_ascii_case(rel));
    let first = format!("{dir}{slug}.md");
    if !taken(&first) {
        return first;
    }
    (2..).map(|n| format!("{dir}{slug}-{n}.md")).find(|rel| !taken(rel)).unwrap_or(first)
}

/// Creates an "Untitled" page at the end of `group` and returns its path.
pub fn create_page(store: &DocsStore, group: Group, title: &str) -> Result<String, String> {
    let dir = dir_of_group(group).ok_or_else(|| "Screens and States hold pinned frames; add a page under Overview, Spec or Discovery".to_string())?;
    let existing = store.list_page_files().map_err(|e| e.to_string())?;
    let rel = unique_rel(&existing, dir, &slugify(title));
    let mut page = Page::parse("");
    page.set_title(Some(title));
    store.write_page(&rel, &page, Expect::Absent).map_err(|e| e.to_string())?;
    let added = rel.clone();
    if let Err(e) = update_manifest(store, move |m| m.group_mut(group).pages.push(added.clone())) {
        // The page is on disk but not in the manifest; the tree still shows it (unlisted), so report and keep it.
        return Err(format!("The page was created but not listed: {e}"));
    }
    Ok(rel)
}

#[derive(Debug)]
pub struct Renamed {
    pub new_rel: String,
    pub links_updated: usize,
    pub warnings: Vec<String>,
}

/// Renames a page: new title in its front matter, new file name from the title (same folder), the manifest entry
/// follows, and links to it in other pages are retargeted. Writes the new file first, so a failure never loses the page.
pub fn rename_page(store: &DocsStore, rel: &str, new_title: &str) -> Result<Renamed, String> {
    let title = new_title.trim();
    if title.is_empty() {
        return Err("A page needs a name; type one, or press Esc to keep the old name".into());
    }
    let read = store.read_page(rel).map_err(|e| e.to_string())?;
    let mut page = read.value;
    page.set_title(Some(title));
    let dir = rel.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
    let existing = store.list_page_files().map_err(|e| e.to_string())?;
    let others: Vec<String> = existing.iter().filter(|e| e.as_str() != rel).cloned().collect();
    let new_rel = unique_rel(&others, &dir, &slugify(title));
    let mut warnings = Vec::new();
    if new_rel == rel {
        store.write_page(rel, &page, Expect::Hash(read.hash)).map_err(|e| e.to_string())?;
        return Ok(Renamed { new_rel, links_updated: 0, warnings });
    }
    let (from, to) = (rel.to_string(), new_rel.clone());
    if new_rel.eq_ignore_ascii_case(rel) {
        // Only the case differs: on a case-insensitive file system the new name is this very file. Write the new
        // title in place, then rename the file itself (through a temporary name) under the docs lock.
        let written = store.write_page(rel, &page, Expect::Hash(read.hash)).map_err(|e| e.to_string())?;
        store.rename_page(rel, &new_rel, written).map_err(|e| e.to_string())?;
    } else {
        store.write_page(&new_rel, &page, Expect::Absent).map_err(|e| e.to_string())?;
    }
    let moved = update_manifest(store, |m| {
        for g in Group::ALL {
            for p in m.group_mut(g).pages.iter_mut() {
                if *p == from {
                    *p = to.clone();
                }
            }
        }
    });
    if let Err(e) = moved {
        if new_rel.eq_ignore_ascii_case(rel) {
            return Err(format!("{e}; the file is now {new_rel} but the manifest still lists {rel}"));
        }
        // The manifest still names the old file, which is untouched: drop the copy so nothing is listed twice.
        if let Ok(copy) = store.read_page(&new_rel) {
            let _ = store.remove_page(&new_rel, copy.hash);
        }
        return Err(e);
    }
    if !new_rel.eq_ignore_ascii_case(rel) {
        // The old file goes only if nobody changed it since it was read; the check and the delete are one step under the lock.
        match store.remove_page(rel, read.hash) {
            Ok(()) => {}
            Err(DocsError::Conflict { .. }) => warnings.push(format!("{rel} changed while it was renamed, so the old file was kept next to {new_rel}")),
            Err(e) => warnings.push(format!("The old file {rel} could not be removed: {e}")),
        }
    }
    let (links_updated, mut link_warnings) = super::links::retarget_all(store, rel, &new_rel);
    warnings.append(&mut link_warnings);
    Ok(Renamed { new_rel, links_updated, warnings })
}

/// Moves a page to Studio's trash (a restorable trash item, see `Library::trash_page`) and takes it out of the manifest.
pub fn trash_page(store: &DocsStore, library_root: &Path, project_id: &str, rel: &str) -> Result<(), String> {
    let read = store.read_page(rel).map_err(|e| e.to_string())?;
    let title = meta_of(rel, &read.value).title;
    let library = designos_library::Library::open(library_root).map_err(|e| format!("Cannot open the library: {e}; nothing was deleted"))?;
    library.trash_page(project_id, rel, &title).map(|_| ()).map_err(|e| e.to_string())
}

/// True when the block list is only HTML comments (an `<!-- -->` separator the page writer inserts between lists).
pub fn is_comment_only(text: &str) -> bool {
    let mut rest = text.trim();
    if rest.is_empty() {
        return false;
    }
    while let Some(after) = rest.strip_prefix("<!--") {
        match after.find("-->") {
            Some(end) => rest = after[end + 3..].trim_start(),
            None => return false,
        }
    }
    rest.is_empty()
}

/// Whether `kind` shows nothing in the reader.
pub fn renders_nothing(kind: &BlockKind) -> bool {
    matches!(kind, BlockKind::Raw(text) if is_comment_only(text))
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
