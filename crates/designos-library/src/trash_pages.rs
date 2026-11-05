//! Docs pages in the trash. Deleting a page moves its file into `trash/<id>/page.md` and writes a `trash.json` item
//! of kind `Page`, so Home lists it, counts it in "Empty trash" and can put it back.

use super::{now_ms, write_atomic, write_lock, PageTrash, TrashItem, TrashKind, TRASH_MANIFEST};
use crate::{new_id, Library, LibraryError, Result};
use designos_docs::{DocsError, DocsStore, Group};

fn docs_err(e: DocsError) -> LibraryError {
    LibraryError::Invalid(e.to_string())
}

/// `spec/a.md` is taken: `spec/a-2.md`, `spec/a-3.md` ... The same rule as a new page in the Project tab. Names are
/// compared without case, as a case-insensitive file system would.
fn free_path(existing: &[String], rel: &str) -> String {
    let taken = |p: &str| existing.iter().any(|e| e.eq_ignore_ascii_case(p));
    if !taken(rel) {
        return rel.to_string();
    }
    let (dir, file) = rel.rsplit_once('/').map(|(d, f)| (format!("{d}/"), f)).unwrap_or((String::new(), rel));
    let stem = file.strip_suffix(".md").unwrap_or(file);
    (2..).map(|n| format!("{dir}{stem}-{n}.md")).find(|p| !taken(p)).unwrap_or_else(|| rel.to_string())
}

impl Library {
    /// Moves the docs page `rel` of `project` to the trash and takes it out of the manifest. `title` is what the
    /// trash list shows. The file moves only if it still has the content that was just read, under the docs lock;
    /// when the manifest cannot be updated the page is put back, so the folder and the manifest agree.
    pub fn trash_page(&self, project: &str, rel: &str, title: &str) -> Result<TrashItem> {
        let _guard = write_lock();
        let manifest = self.load_manifest(project)?;
        let store = DocsStore::for_project(self.root(), project).map_err(docs_err)?;
        let read = store.read_page(rel).map_err(docs_err)?;
        let listed = store.read_manifest().ok();
        let group = listed.as_ref().and_then(|m| Group::ALL.into_iter().find(|g| m.value.group(*g).pages.iter().any(|p| p == rel)));
        let id = new_id();
        let dir = self.trash_dir(&id);
        std::fs::create_dir_all(&dir)?;
        let item = TrashItem {
            id: id.clone(),
            kind: TrashKind::Page,
            name: title.to_string(),
            trashed: now_ms(),
            project: Some(project.to_string()),
            project_name: Some(manifest.project.name.clone()),
            entry: None,
            page: Some(PageTrash { path: rel.to_string(), group: group.map(|g| g.as_str().to_string()) }),
        };
        let cleanup = |dir: &std::path::Path| {
            let _ = std::fs::remove_dir_all(dir);
        };
        if let Err(e) = write_atomic(&dir.join(TRASH_MANIFEST), serde_json::to_string_pretty(&item)?.as_bytes()) {
            cleanup(&dir);
            return Err(e);
        }
        if let Err(e) = store.move_page_out(rel, &dir.join("page.md"), read.hash) {
            cleanup(&dir);
            return Err(LibraryError::Invalid(format!("{e}; nothing was deleted")));
        }
        if listed.is_some() {
            let name = rel.to_string();
            let updated = store.update_manifest(|m| {
                for g in Group::ALL {
                    m.group_mut(g).pages.retain(|p| *p != name);
                }
            });
            if let Err(e) = updated {
                // Put the page back so the manifest and the folder agree.
                return Err(match store.adopt_file(&dir.join("page.md"), rel) {
                    Ok(()) => {
                        cleanup(&dir);
                        LibraryError::Invalid(format!("{e}; the page was not deleted"))
                    }
                    Err(back) => LibraryError::Invalid(format!("{e}; the page is in {} (could not move it back: {back})", dir.display())),
                });
            }
        }
        Ok(item)
    }

    /// Puts a trashed page back into its project's docs folder (the next free name when its name is used now) and
    /// lists it in the group it came from, or in Discovery when that group is gone. Returns the path it has now.
    pub fn restore_page(&self, id: &str) -> Result<String> {
        let _guard = write_lock();
        let dir = self.trash_dir(id);
        let item: TrashItem =
            serde_json::from_str(&std::fs::read_to_string(dir.join(TRASH_MANIFEST)).map_err(|_| LibraryError::NotFound(format!("trashed page {id}")))?)?;
        let (Some(project), Some(page)) = (item.project.clone(), item.page.clone()) else {
            return Err(LibraryError::Invalid("Not a trashed page".into()));
        };
        self.load_manifest(&project).map_err(|_| {
            LibraryError::Invalid(format!("Its project \"{}\" is not available; restore the project first", item.project_name.clone().unwrap_or_default()))
        })?;
        let store = DocsStore::for_project(self.root(), &project).map_err(docs_err)?;
        let existing = store.list_page_files().map_err(docs_err)?;
        let rel = free_path(&existing, &page.path);
        store.adopt_file(&dir.join("page.md"), &rel).map_err(docs_err)?;
        let _ = std::fs::remove_dir_all(&dir);
        // Screens and States hold pinned frames, not pages; anything else unknown goes to Discovery.
        let group = page.group.as_deref().and_then(Group::parse).filter(|g| !matches!(g, Group::Screens | Group::States)).unwrap_or(Group::Discovery);
        let listed = rel.clone();
        store
            .update_manifest(|m| {
                if !Group::ALL.into_iter().any(|g| m.group(g).pages.contains(&listed)) {
                    m.group_mut(group).pages.push(listed.clone());
                }
            })
            .map_err(|e| LibraryError::Invalid(format!("The page is back as {rel} but could not be listed: {e}")))?;
        Ok(rel)
    }
}
