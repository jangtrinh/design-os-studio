//! Branch metadata and working copies inside a library file (`T10.1`, issue `#343`).
//!
//! Layout under [`Library::root`]:
//!
//! ```text
//! branches/<file-id>/<branch-id>.designos   working copy of the branch
//! versions/<file-id>~<branch-id>/           version history of the branch
//! ```
//!
//! Metadata lives in `FileEntry.branches: Vec<BranchMeta>` inside `projects/<project-id>/project.json`.
//! Creating a branch saves a named version on main labelled `Branch point: <name>` (`base_version`),
//! writes the main document unchanged as the branch working copy (preserving every `NodeId`), and
//! records the [`BranchMeta`] entry under the manifest write lock.

use crate::store::{now_ms, write_atomic, write_lock};
use crate::{new_id, Library, LibraryError};
use designos_doc::Document;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Lifecycle state of a branch (`R5.19`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    #[default]
    Active,
    Archived,
    Merged {
        into_version: u32,
    },
}

impl BranchStatus {
    /// Archived and merged branches open in read-only view mode and refuse saves until restored.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Archived | Self::Merged { .. })
    }
}

/// Metadata for a branch stored on its parent [`crate::FileEntry`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchMeta {
    pub id: String,
    pub name: String,
    pub base_version: u32,
    pub created: u64,
    pub modified: u64,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub status: BranchStatus,
    #[serde(default)]
    pub description: String,
}

/// Typed errors returned by branch operations (`T10.1` §5).
#[derive(Debug, thiserror::Error)]
pub enum BranchError {
    #[error("Save to your library to create branches")]
    NotALibraryFile,
    #[error("branch name already taken: {0}")]
    NameTaken(String),
    #[error("branch is archived and read-only: {0}")]
    BranchArchived(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("io at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Merge(#[from] designos_doc::MergeError),
}

pub type BranchResult<T> = std::result::Result<T, BranchError>;

/// Outcome returned by [`Library::merge_branch`] (`T10.3` §3.5, §3.6).
#[derive(Debug, Clone)]
pub struct MergedBranchOutcome {
    pub branch: BranchMeta,
    pub before_version: crate::FileVersion,
    pub after_version: crate::FileVersion,
    pub report: designos_doc::MergeReport,
}

/// Outcome returned by [`Library::update_branch_from_main`] (`T10.3` §3.6).
#[derive(Debug, Clone)]
pub struct UpdatedBranchOutcome {
    pub branch: BranchMeta,
    pub before_branch_version: crate::FileVersion,
    pub new_base_version: crate::FileVersion,
    pub report: designos_doc::MergeReport,
}

impl From<BranchError> for LibraryError {
    fn from(err: BranchError) -> Self {
        match err {
            BranchError::NotFound(s) => LibraryError::NotFound(s),
            BranchError::Io { source, .. } => LibraryError::Io(source),
            BranchError::Json(e) => LibraryError::Json(e),
            other => LibraryError::Invalid(other.to_string()),
        }
    }
}

fn map_lib_err(err: LibraryError, fallback_path: &Path) -> BranchError {
    match err {
        LibraryError::NotFound(s) => BranchError::NotFound(s),
        LibraryError::Io(source) => BranchError::Io { path: fallback_path.to_path_buf(), source },
        LibraryError::Json(e) => BranchError::Json(e),
        LibraryError::Invalid(s) => BranchError::Invalid(s),
    }
}

fn write_atomic_branch(path: &Path, bytes: &[u8]) -> BranchResult<()> {
    write_atomic(path, bytes).map_err(|err| map_lib_err(err, path))
}

fn validate_library_file_id(file_id: &str) -> BranchResult<()> {
    let trimmed = file_id.trim();
    if trimmed.is_empty() || trimmed.contains(['/', '\\']) || trimmed.ends_with(".designos") || trimmed.ends_with(".fig") {
        return Err(BranchError::NotALibraryFile);
    }
    if trimmed.contains('~') {
        return Err(BranchError::Invalid("Branch of a branch is not supported".into()));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TrashedBranch {
    file_id: String,
    meta: BranchMeta,
}

impl Library {
    /// On-disk path of the working copy for `branch_id` under `file_id`:
    /// `branches/<file-id>/<branch-id>.designos`.
    pub fn branch_path(&self, file_id: &str, branch_id: &str) -> PathBuf {
        self.path("branches").join(file_id).join(format!("{branch_id}.designos"))
    }

    fn trashed_branch_dir(&self, file_id: &str, branch_id: &str) -> PathBuf {
        self.path("trash").join(format!("branch-{file_id}-{branch_id}"))
    }

    /// Creates a new branch of `file_id` from `doc`:
    /// 1. Saves a named version on main labelled `Branch point: <name>` (`base_version`).
    /// 2. Writes `doc` unchanged to `branches/<file-id>/<branch-id>.designos` so every `NodeId` matches main.
    /// 3. Appends [`BranchMeta`] to the file's manifest entry under the manifest write lock.
    pub fn create_branch(&self, file_id: &str, doc: &Document, name: &str, author: &str) -> BranchResult<BranchMeta> {
        self.create_branch_described(file_id, doc, name, author, None)
    }

    /// [`Self::create_branch`] with an optional description.
    pub fn create_branch_described(&self, file_id: &str, doc: &Document, name: &str, author: &str, description: Option<&str>) -> BranchResult<BranchMeta> {
        validate_library_file_id(file_id)?;
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Err(BranchError::Invalid("Branch name cannot be empty".into()));
        }

        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| match e {
            LibraryError::NotFound(_) => BranchError::NotALibraryFile,
            other => map_lib_err(other, self.root()),
        })?;

        if m.files[i].branches.iter().any(|b| b.name.eq_ignore_ascii_case(clean_name)) {
            return Err(BranchError::NameTaken(clean_name.to_string()));
        }

        let branch_point_label = format!("Branch point: {clean_name}");
        let ver = self
            .save_version_described(file_id, doc, Some(branch_point_label), None, false)
            .map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;

        let branch_id = new_id();
        let copy_path = self.branch_path(file_id, &branch_id);
        let (json, _) = doc.to_json_counted().map_err(|e| BranchError::Invalid(e.to_string()))?;

        if let Err(err) = write_atomic_branch(&copy_path, json.as_bytes()) {
            let _ = std::fs::remove_file(&copy_path);
            return Err(err);
        }

        let now = now_ms();
        let meta = BranchMeta {
            id: branch_id,
            name: clean_name.to_string(),
            base_version: ver.n,
            created: now,
            modified: now,
            author: author.trim().to_string(),
            status: BranchStatus::Active,
            description: description.unwrap_or("").trim().to_string(),
        };

        m.files[i].branches.push(meta.clone());
        m.files[i].modified = now;
        m.project.modified = now;

        if let Err(err) = self.save_manifest(&m) {
            let _ = std::fs::remove_file(&copy_path);
            let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
            return Err(map_lib_err(err, &manifest_path));
        }

        Ok(meta)
    }

    /// Lists all branches belonging to `file_id` in creation order.
    pub fn list_branches(&self, file_id: &str) -> BranchResult<Vec<BranchMeta>> {
        validate_library_file_id(file_id)?;
        let (m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        Ok(m.files[i].branches.clone())
    }

    /// Loads the working copy of `branch_id` under `file_id`.
    pub fn load_branch(&self, file_id: &str, branch_id: &str) -> BranchResult<Document> {
        validate_library_file_id(file_id)?;
        let (m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        if !m.files[i].branches.iter().any(|b| b.id == branch_id) {
            return Err(BranchError::NotFound(format!("branch {branch_id} of {file_id}")));
        }
        let path = self.branch_path(file_id, branch_id);
        let text = std::fs::read_to_string(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                BranchError::NotFound(format!("branch file {}", path.display()))
            } else {
                BranchError::Io { path: path.clone(), source }
            }
        })?;
        Document::from_json(&text).map_err(|e| BranchError::Invalid(e.to_string()))
    }

    /// Saves `doc` to the working copy of `branch_id` atomically and bumps its `modified` timestamp.
    /// Refuses saves when the branch is archived or merged (`BranchError::BranchArchived`).
    pub fn save_branch(&self, file_id: &str, branch_id: &str, doc: &Document) -> BranchResult<()> {
        validate_library_file_id(file_id)?;
        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        let b_idx =
            m.files[i].branches.iter().position(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;

        if m.files[i].branches[b_idx].status.is_read_only() {
            return Err(BranchError::BranchArchived(m.files[i].branches[b_idx].name.clone()));
        }

        let path = self.branch_path(file_id, branch_id);
        let (json, _) = doc.to_json_counted().map_err(|e| BranchError::Invalid(e.to_string()))?;
        write_atomic_branch(&path, json.as_bytes())?;

        let now = now_ms();
        m.files[i].branches[b_idx].modified = now;
        m.files[i].modified = now;
        m.project.modified = now;
        let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
        self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))
    }

    /// Archives `branch_id` (making it read-only until restored).
    pub fn archive_branch(&self, file_id: &str, branch_id: &str) -> BranchResult<BranchMeta> {
        validate_library_file_id(file_id)?;
        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        let b = m.files[i].branches.iter_mut().find(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
        let now = now_ms();
        b.status = BranchStatus::Archived;
        b.modified = now;
        let out = b.clone();
        m.project.modified = now;
        let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
        self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))?;
        Ok(out)
    }

    /// Marks `branch_id` as merged into main version `into_version` (archived & read-only, restorable via [`Self::restore_branch`]).
    pub fn mark_branch_merged(&self, file_id: &str, branch_id: &str, into_version: u32) -> BranchResult<BranchMeta> {
        validate_library_file_id(file_id)?;
        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        let b = m.files[i].branches.iter_mut().find(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
        let now = now_ms();
        b.status = BranchStatus::Merged { into_version };
        b.modified = now;
        let out = b.clone();
        m.project.modified = now;
        let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
        self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))?;
        Ok(out)
    }

    /// Restores an archived, merged, or trashed branch back to `BranchStatus::Active`.
    pub fn restore_branch(&self, file_id: &str, branch_id: &str) -> BranchResult<BranchMeta> {
        validate_library_file_id(file_id)?;
        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        let now = now_ms();

        if let Some(b) = m.files[i].branches.iter_mut().find(|b| b.id == branch_id) {
            b.status = BranchStatus::Active;
            b.modified = now;
            let out = b.clone();
            m.project.modified = now;
            let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
            self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))?;
            return Ok(out);
        }

        // Check if the branch was moved to trash by `delete_branch`.
        let trash_dir = self.trashed_branch_dir(file_id, branch_id);
        let meta_path = trash_dir.join("branch.json");
        if !meta_path.exists() {
            return Err(BranchError::NotFound(format!("branch {branch_id} of {file_id}")));
        }
        let text = std::fs::read_to_string(&meta_path).map_err(|source| BranchError::Io { path: meta_path.clone(), source })?;
        let trashed: TrashedBranch = serde_json::from_str(&text)?;
        let mut meta = trashed.meta;
        if m.files[i].branches.iter().any(|b| b.name.eq_ignore_ascii_case(&meta.name)) {
            return Err(BranchError::NameTaken(meta.name));
        }
        let dst = self.branch_path(file_id, branch_id);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|source| BranchError::Io { path: parent.to_path_buf(), source })?;
        }
        let src_doc = trash_dir.join("branch.designos");
        if src_doc.exists() {
            std::fs::rename(&src_doc, &dst).map_err(|source| BranchError::Io { path: dst.clone(), source })?;
        }
        meta.status = BranchStatus::Active;
        meta.modified = now;
        m.files[i].branches.push(meta.clone());
        m.project.modified = now;
        let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
        self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))?;
        let _ = std::fs::remove_dir_all(&trash_dir);
        Ok(meta)
    }

    /// Moves a branch working copy and its metadata to `trash/branch-<file-id>-<branch-id>/`
    /// (never hard-deleted until [`Self::empty_trash`]) and removes it from `FileEntry.branches`.
    pub fn delete_branch(&self, file_id: &str, branch_id: &str) -> BranchResult<()> {
        validate_library_file_id(file_id)?;
        let _guard = write_lock();
        let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
        let b_idx =
            m.files[i].branches.iter().position(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
        let meta = m.files[i].branches[b_idx].clone();

        let trash_dir = self.trashed_branch_dir(file_id, branch_id);
        std::fs::create_dir_all(&trash_dir).map_err(|source| BranchError::Io { path: trash_dir.clone(), source })?;
        let record = TrashedBranch { file_id: file_id.to_string(), meta };
        write_atomic_branch(&trash_dir.join("branch.json"), serde_json::to_string_pretty(&record)?.as_bytes())?;

        let src = self.branch_path(file_id, branch_id);
        if src.exists() {
            let dst = trash_dir.join("branch.designos");
            std::fs::rename(&src, &dst).map_err(|source| BranchError::Io { path: dst, source })?;
        }

        m.files[i].branches.remove(b_idx);
        m.project.modified = now_ms();
        let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
        self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))
    }

    /// Merges `branch_id` into `main_doc` on `file_id` (`T10.3` §3.5, §3.6):
    /// 1. Refuses if the branch is archived or already merged (`BranchError::BranchArchived`).
    /// 2. Loads `base` (`branch.base_version`) and `branch_doc` (`load_branch`).
    /// 3. Computes `diff3(&base, main_doc, &branch_doc)` and `merge(&base, main_doc, &branch_doc, &diff, resolution)`
    ///    (validating zero unresolved items and zero dangling references before mutating anything).
    /// 4. Saves an auto version on main labelled `Before merge of <branch.name>`.
    /// 5. Executes `plan.command` on `(main_doc, history)` as a single `Command::Batch` undo step.
    /// 6. Saves the merged `main_doc` to disk (`save_file`) and saves an auto version on main
    ///    labelled `After merge of <branch.name>`.
    /// 7. Marks the branch `BranchStatus::Merged { into_version: after_version.n }`.
    pub fn merge_branch(
        &self,
        file_id: &str,
        branch_id: &str,
        main_doc: &mut Document,
        history: &mut designos_doc::History,
        resolution: &designos_doc::Resolution,
    ) -> BranchResult<MergedBranchOutcome> {
        validate_library_file_id(file_id)?;
        let branch_meta = {
            let (m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
            let b = m.files[i].branches.iter().find(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
            if b.status.is_read_only() {
                return Err(BranchError::BranchArchived(b.name.clone()));
            }
            b.clone()
        };

        let base = self.load_version(file_id, branch_meta.base_version).map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;
        let branch_doc = self.load_branch(file_id, branch_id)?;

        let diff = designos_doc::diff3(&base, main_doc, &branch_doc);
        let plan = designos_doc::merge(&base, main_doc, &branch_doc, &diff, resolution)?;

        let before_label = format!("Before merge of {}", branch_meta.name);
        let before_version =
            self.save_version_described(file_id, main_doc, Some(before_label), None, true).map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;

        history.exec(main_doc, plan.command).map_err(|e| BranchError::Invalid(e.to_string()))?;

        self.save_file(file_id, main_doc).map_err(|e| map_lib_err(e, self.root()))?;

        let after_label = format!("After merge of {}", branch_meta.name);
        let after_version =
            self.save_version_described(file_id, main_doc, Some(after_label), None, true).map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;

        let merged_meta = self.mark_branch_merged(file_id, branch_id, after_version.n)?;

        Ok(MergedBranchOutcome { branch: merged_meta, before_version, after_version, report: plan.report })
    }

    /// Updates `branch_id` with changes from `main_doc` (`T10.3` §3.6):
    /// 1. Runs the 3-way merge with `branch_doc` as "ours" and `main_doc` as "theirs".
    /// 2. Saves a `Before update from main` auto version on the branch.
    /// 3. Executes the merge batch command on `(branch_doc, branch_history)` and saves the branch working copy.
    /// 4. Saves a new `Branch point: <branch.name>` version on main and advances `branch.base_version`
    ///    so subsequent merges use the new base.
    pub fn update_branch_from_main(
        &self,
        file_id: &str,
        branch_id: &str,
        main_doc: &Document,
        branch_doc: &mut Document,
        branch_history: &mut designos_doc::History,
        resolution: &designos_doc::Resolution,
    ) -> BranchResult<UpdatedBranchOutcome> {
        validate_library_file_id(file_id)?;
        let branch_meta = {
            let (m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
            let b = m.files[i].branches.iter().find(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
            if b.status.is_read_only() {
                return Err(BranchError::BranchArchived(b.name.clone()));
            }
            b.clone()
        };

        let base = self.load_version(file_id, branch_meta.base_version).map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;

        let diff = designos_doc::diff3(&base, branch_doc, main_doc);
        let plan = designos_doc::merge(&base, branch_doc, main_doc, &diff, resolution)?;

        let before_branch_version = self
            .save_branch_version(file_id, branch_id, branch_doc, Some("Before update from main".to_string()), true)
            .map_err(|e| map_lib_err(e, &self.path("versions").join(format!("{file_id}~{branch_id}"))))?;

        branch_history.exec(branch_doc, plan.command).map_err(|e| BranchError::Invalid(e.to_string()))?;

        self.save_branch(file_id, branch_id, branch_doc)?;

        let new_base_label = format!("Branch point: {}", branch_meta.name);
        let new_base_version = self
            .save_version_described(file_id, main_doc, Some(new_base_label), None, false)
            .map_err(|e| map_lib_err(e, &self.path("versions").join(file_id)))?;

        let updated_meta = {
            let _guard = write_lock();
            let (mut m, i) = self.find_file(file_id).map_err(|e| map_lib_err(e, self.root()))?;
            let b =
                m.files[i].branches.iter_mut().find(|b| b.id == branch_id).ok_or_else(|| BranchError::NotFound(format!("branch {branch_id} of {file_id}")))?;
            let now = now_ms();
            b.base_version = new_base_version.n;
            b.modified = now;
            let out = b.clone();
            m.project.modified = now;
            let manifest_path = self.path("projects").join(&m.project.id).join("project.json");
            self.save_manifest(&m).map_err(|e| map_lib_err(e, &manifest_path))?;
            out
        };

        Ok(UpdatedBranchOutcome { branch: updated_meta, before_branch_version, new_base_version, report: plan.report })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeId, Rect};
    use std::collections::BTreeSet;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("designos_branches_{}_{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            Self(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample_doc() -> Document {
        let mut d = Document::new("Design System");
        let page = d.add_page("Page 1");
        let f_id = d.alloc_id();
        let f = d.insert(page, Node::frame(f_id, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        for idx in 0..4 {
            let r_id = d.alloc_id();
            d.insert(f, Node::rectangle(r_id, Rect::new(idx as f32 * 24.0, 0.0, 20.0, 20.0))).unwrap();
        }
        d
    }

    fn node_ids(doc: &Document) -> BTreeSet<NodeId> {
        doc.nodes.keys().copied().collect()
    }

    #[test]
    fn create_saves_branch_point_and_same_ids() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "Home", &doc).unwrap();

        let branch = lib.create_branch(&file.id, &doc, "exploration-a", "Alice").unwrap();
        assert_eq!(branch.name, "exploration-a");
        assert_eq!(branch.author, "Alice");
        assert_eq!(branch.status, BranchStatus::Active);

        let versions = lib.list_versions(&file.id);
        let bp = versions.iter().find(|v| v.n == branch.base_version).expect("branch point version must exist");
        assert_eq!(bp.label.as_deref(), Some("Branch point: exploration-a"));
        assert!(!bp.auto, "branch point must be a named version so retention never drops it");

        let branch_doc = lib.load_branch(&file.id, &branch.id).unwrap();
        assert_eq!(node_ids(&branch_doc), node_ids(&doc), "branch working copy must preserve identical NodeIds");
    }

    #[test]
    fn legacy_index_loads() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Legacy").unwrap();
        let doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "Legacy File", &doc).unwrap();

        let manifest_path = lib.path("projects").join(&proj.id).join("project.json");
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        assert!(!raw.contains("\"branches\""), "saving an entry with no branches must omit the branches key");

        // Explicitly strip any `branches` key and reload.
        let mut val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        for f in val["files"].as_array_mut().unwrap() {
            f.as_object_mut().unwrap().remove("branches");
        }
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap()).unwrap();

        let loaded = lib.file(&file.id).unwrap();
        assert!(loaded.branches.is_empty());

        // Saving an unchanged file keeps `branches` omitted.
        lib.star(&file.id, true).unwrap();
        let after = std::fs::read_to_string(&manifest_path).unwrap();
        assert!(!after.contains("\"branches\""));
    }

    #[test]
    fn archived_is_read_only() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let mut doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &doc).unwrap();
        let branch = lib.create_branch(&file.id, &doc, "v2", "Bob").unwrap();

        let archived = lib.archive_branch(&file.id, &branch.id).unwrap();
        assert_eq!(archived.status, BranchStatus::Archived);

        doc.name = "Mutated on archived branch".into();
        let err = lib.save_branch(&file.id, &branch.id, &doc).unwrap_err();
        assert!(matches!(err, BranchError::BranchArchived(_)), "expected BranchArchived, got {err:?}");

        // Restore re-enables saving.
        let restored = lib.restore_branch(&file.id, &branch.id).unwrap();
        assert_eq!(restored.status, BranchStatus::Active);
        lib.save_branch(&file.id, &branch.id, &doc).unwrap();
        assert_eq!(lib.load_branch(&file.id, &branch.id).unwrap().name, "Mutated on archived branch");
    }

    #[test]
    fn branch_versions_separate() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &doc).unwrap();
        let branch = lib.create_branch(&file.id, &doc, "feature-x", "Carol").unwrap();

        let main_count_before = lib.list_versions(&file.id).len();
        let bv = lib.save_branch_version(&file.id, &branch.id, &doc, Some("Checkpoint 1".into()), false).unwrap();
        assert_eq!(bv.n, 1);

        assert_eq!(lib.list_versions(&file.id).len(), main_count_before, "saving a branch version must not add entries to main's version index");
        let branch_versions = lib.list_branch_versions(&file.id, &branch.id);
        assert_eq!(branch_versions.len(), 1);
        assert_eq!(branch_versions[0].label.as_deref(), Some("Checkpoint 1"));
    }

    #[test]
    fn failure_leaves_manifest_unchanged() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &doc).unwrap();

        // Inject a write failure on the branch copy by placing a regular file where the directory
        // `branches/<file-id>` needs to be created.
        let branches_root = lib.path("branches");
        std::fs::create_dir_all(&branches_root).unwrap();
        let blocker = branches_root.join(&file.id);
        std::fs::write(&blocker, b"not a directory").unwrap();

        let res = lib.create_branch(&file.id, &doc, "broken", "Dave");
        assert!(res.is_err(), "expected create_branch to fail when branches/<file-id> is blocked by a file");

        let entry = lib.file(&file.id).unwrap();
        assert!(entry.branches.is_empty(), "manifest must have no BranchMeta after failed create_branch");

        // Remove the blocker and verify a retry succeeds cleanly with a new branch point.
        std::fs::remove_file(&blocker).unwrap();
        let ok_branch = lib.create_branch(&file.id, &doc, "broken", "Dave").unwrap();
        assert_eq!(lib.list_branches(&file.id).unwrap().len(), 1);
        assert!(lib.branch_path(&file.id, &ok_branch.id).is_file());
    }

    #[test]
    fn edge_cases_name_taken_trash_and_duplicate() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &doc).unwrap();

        let b1 = lib.create_branch(&file.id, &doc, "Dark-Mode", "Eve").unwrap();
        // Case-insensitive duplicate name is refused.
        let dup_err = lib.create_branch(&file.id, &doc, "dark-mode", "Eve").unwrap_err();
        assert!(matches!(dup_err, BranchError::NameTaken(_)));

        // Branch of a branch is refused.
        let bob_err = lib.create_branch(&format!("{}~{}", file.id, b1.id), &doc, "nested", "Eve").unwrap_err();
        assert!(matches!(bob_err, BranchError::Invalid(_)));

        // Plain path / non-library file returns NotALibraryFile.
        let non_lib = lib.create_branch("/tmp/outside.designos", &doc, "ext", "Eve").unwrap_err();
        assert!(matches!(non_lib, BranchError::NotALibraryFile));

        // Duplicating the file does not duplicate branches.
        let copy = lib.duplicate_file(&file.id).unwrap();
        assert!(copy.branches.is_empty());

        // Deleting a branch moves it to trash (not hard-deleted) and restore_branch brings it back.
        lib.delete_branch(&file.id, &b1.id).unwrap();
        assert!(lib.list_branches(&file.id).unwrap().is_empty());
        assert!(!lib.branch_path(&file.id, &b1.id).exists());
        let restored = lib.restore_branch(&file.id, &b1.id).unwrap();
        assert_eq!(restored.id, b1.id);
        assert!(lib.branch_path(&file.id, &b1.id).exists());

        // Trashing and restoring the main file moves its branches with it.
        lib.trash_file(&file.id).unwrap();
        assert!(!lib.branch_path(&file.id, &b1.id).exists());
        lib.restore_file(&file.id).unwrap();
        assert!(lib.branch_path(&file.id, &b1.id).exists());
        assert_eq!(lib.list_branches(&file.id).unwrap().len(), 1);
    }

    #[test]
    fn versions_before_after_and_one_undo() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let mut main_doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &main_doc).unwrap();
        let branch = lib.create_branch(&file.id, &main_doc, "header-redesign", "Alice").unwrap();

        // Main adds a layer; branch adds a colliding layer with the same raw NodeId.
        let page = main_doc.pages[0];
        let mut branch_doc = lib.load_branch(&file.id, &branch.id).unwrap();

        let m_id = main_doc.alloc_id();
        let mut m_rect = Node::rectangle(m_id, Rect::new(200.0, 20.0, 50.0, 50.0));
        m_rect.name = "Main Card".into();
        main_doc.insert(page, m_rect).unwrap();
        lib.save_file(&file.id, &main_doc).unwrap();

        let b_id = branch_doc.alloc_id();
        assert_eq!(m_id, b_id, "both sides allocate the same raw NodeId after branch point");
        let mut b_rect = Node::rectangle(b_id, Rect::new(300.0, 40.0, 80.0, 80.0));
        b_rect.name = "Branch Hero".into();
        branch_doc.insert(page, b_rect).unwrap();
        lib.save_branch(&file.id, &branch.id, &branch_doc).unwrap();

        let pre_merge_snapshot = main_doc.clone();
        let mut history = designos_doc::History::default();
        let resolution = designos_doc::Resolution::default();

        let outcome = lib.merge_branch(&file.id, &branch.id, &mut main_doc, &mut history, &resolution).unwrap();

        assert_eq!(outcome.before_version.label.as_deref(), Some("Before merge of header-redesign"));
        assert!(outcome.before_version.auto);
        assert_eq!(outcome.after_version.label.as_deref(), Some("After merge of header-redesign"));
        assert!(outcome.after_version.auto);
        assert!(outcome.after_version.n > outcome.before_version.n);

        // Verify both versions exist in main's version index and match pre/post merge snapshots.
        let before_loaded = lib.load_version(&file.id, outcome.before_version.n).unwrap();
        assert_eq!(before_loaded.nodes.len(), pre_merge_snapshot.nodes.len());
        let after_loaded = lib.load_version(&file.id, outcome.after_version.n).unwrap();
        assert_eq!(after_loaded.nodes.len(), main_doc.nodes.len());
        assert_eq!(main_doc.nodes.len(), pre_merge_snapshot.nodes.len() + 1);
        assert!(designos_doc::validate_merged_references(&main_doc).is_empty());

        // Single undo reverts the entire merge batch in one step.
        assert!(history.can_undo());
        assert!(history.undo(&mut main_doc).unwrap());
        assert!(!history.can_undo(), "entire merge must be a single undo step");
        assert_eq!(main_doc.nodes, pre_merge_snapshot.nodes);
    }

    #[test]
    fn branch_archived_after_merge() {
        let tmp = Tmp::new();
        let lib = Library::open(&tmp.0).unwrap();
        let proj = lib.create_project("Product").unwrap();
        let mut main_doc = sample_doc();
        let file = lib.create_file(&proj.id, None, "App", &main_doc).unwrap();
        let branch = lib.create_branch(&file.id, &main_doc, "token-pass", "Bob").unwrap();

        let page = main_doc.pages[0];
        // First exercise update_branch_from_main when main advances.
        let m_id = main_doc.alloc_id();
        main_doc.insert(page, Node::rectangle(m_id, Rect::new(10.0, 120.0, 40.0, 40.0))).unwrap();
        lib.save_file(&file.id, &main_doc).unwrap();

        let mut branch_doc = lib.load_branch(&file.id, &branch.id).unwrap();
        let mut branch_history = designos_doc::History::default();
        let upd =
            lib.update_branch_from_main(&file.id, &branch.id, &main_doc, &mut branch_doc, &mut branch_history, &designos_doc::Resolution::default()).unwrap();
        assert_eq!(upd.before_branch_version.label.as_deref(), Some("Before update from main"));
        assert_eq!(upd.branch.base_version, upd.new_base_version.n);
        assert!(branch_doc.nodes.contains_key(&m_id));

        // Now add a layer on the branch and merge into main.
        let b_id = branch_doc.alloc_id();
        branch_doc.insert(page, Node::rectangle(b_id, Rect::new(80.0, 120.0, 40.0, 40.0))).unwrap();
        lib.save_branch(&file.id, &branch.id, &branch_doc).unwrap();

        let mut history = designos_doc::History::default();
        let outcome = lib.merge_branch(&file.id, &branch.id, &mut main_doc, &mut history, &designos_doc::Resolution::default()).unwrap();

        assert_eq!(outcome.branch.status, BranchStatus::Merged { into_version: outcome.after_version.n });
        assert!(outcome.branch.status.is_read_only());

        // Saving or re-merging an archived/merged branch is refused with BranchArchived.
        let save_err = lib.save_branch(&file.id, &branch.id, &branch_doc).unwrap_err();
        assert!(matches!(save_err, BranchError::BranchArchived(_)));

        let remerge_err = lib.merge_branch(&file.id, &branch.id, &mut main_doc, &mut history, &designos_doc::Resolution::default()).unwrap_err();
        assert!(matches!(remerge_err, BranchError::BranchArchived(_)));

        // Restoring the merged branch re-activates it.
        let restored = lib.restore_branch(&file.id, &branch.id).unwrap();
        assert_eq!(restored.status, BranchStatus::Active);
    }
}
