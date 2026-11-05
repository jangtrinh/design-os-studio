//! The Project tab's disk worker.
//!
//! Every read, parse and write of the docs folder happens on one background thread; the UI only sends [`Job`]s and
//! takes what the thread leaves in the [`Inbox`]. The inbox keeps the latest snapshot and the latest page, not a
//! queue, so a tab parked behind other tabs while an agent writes does not pile results up.

use super::ops::{self, Scanner, TreeSnapshot, MAX_PAGE_BYTES};
use designos_docs::{ContentHash, DocChange, DocsError, DocsStore, DocsWatcher, Group, Page};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// A page as the reader needs it.
#[derive(Clone, Debug)]
pub struct LoadedPage {
    pub rel: String,
    pub hash: ContentHash,
    pub page: Arc<Page>,
    /// Counts reads of this page, so the reader knows when to lay it out again.
    pub rev: u64,
    /// The file is over the size `designos-docs` reads and watches, so what shows is the last version read.
    pub oversize: bool,
}

#[derive(Clone, Debug)]
pub enum PageEvent {
    Loaded(LoadedPage),
    /// The page could not be read; `message` names the cause and the fix.
    Failed {
        rel: String,
        message: String,
        oversize: bool,
    },
    /// The file is gone (deleted or renamed by something else).
    Gone(String),
}

impl PageEvent {
    pub fn rel(&self) -> &str {
        match self {
            PageEvent::Loaded(p) => &p.rel,
            PageEvent::Failed { rel, .. } | PageEvent::Gone(rel) => rel,
        }
    }
}

#[derive(Debug)]
pub enum Job {
    /// Ensure the first page exists, then list the docs.
    Init,
    Open(String),
    Rename {
        rel: String,
        title: String,
    },
    Reorder {
        group: Group,
        order: Vec<String>,
    },
    Trash {
        rel: String,
    },
    NewPage {
        group: Group,
    },
    /// The watcher saw a change.
    Wake,
    /// Look again at the folder (the window came back to the front).
    Refresh,
    Stop,
}

/// What the worker leaves for the UI. Each field holds the latest value only, except `errors`.
#[derive(Default)]
pub struct Inbox {
    pub snapshot: Option<Arc<TreeSnapshot>>,
    pub page: Option<PageEvent>,
    /// A page the tab should select (a new page, or a renamed one).
    pub select: Option<String>,
    /// A page the tab should start renaming (a page that was just created).
    pub rename: Option<String>,
    pub renamed: Option<(String, String)>,
    /// The open page grew past the readable size: the UI keeps the version it shows and says so.
    pub oversize: Option<String>,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
}

const MAX_QUEUED_MESSAGES: usize = 8;

/// A tab nobody looks at cannot pile messages up: past the cap the oldest go, because the newest says what the
/// folder is like now.
fn keep_newest(messages: &mut Vec<String>) {
    if messages.len() > MAX_QUEUED_MESSAGES {
        messages.drain(..messages.len() - MAX_QUEUED_MESSAGES);
    }
}

pub struct DocsService {
    jobs: Sender<Job>,
    inbox: Arc<Mutex<Inbox>>,
    /// The pass number the tab last drew in (0: never yet), so the worker can tell a parked tab from a visible one.
    shown: Arc<AtomicU64>,
}

/// Passes after the last draw during which the tab still counts as on screen: the pass that switches tabs, and one
/// more, still wake the window once.
const SHOWN_GRACE_PASSES: u64 = 2;

/// Whether a worker result should wake the window: the tab is on screen (or has not been drawn yet), not parked
/// behind another tab. A parked tab keeps the latest result in its inbox and catches up when it is shown again.
fn tab_is_on_screen(shown: u64, now: u64) -> bool {
    shown == 0 || now <= shown + SHOWN_GRACE_PASSES
}

impl DocsService {
    pub fn start(ctx: egui::Context, library: designos_library::Library, project_id: String, project_name: String) -> Result<Self, String> {
        let store = DocsStore::for_project(library.root(), &project_id).map_err(|e| e.to_string())?;
        let (jobs, rx) = channel();
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let pending = Arc::new(AtomicBool::new(false));
        let wake_tx = jobs.clone();
        let wake_flag = pending.clone();
        let watcher = DocsWatcher::start(store.dir(), move || {
            if !wake_flag.swap(true, Ordering::AcqRel) {
                let _ = wake_tx.send(Job::Wake);
            }
        });
        let shown = Arc::new(AtomicU64::new(0));
        let worker = Worker {
            shown: shown.clone(),
            scanner: Scanner::new(store, Some((library.clone(), project_id.clone()))),
            library_root: library.root().to_path_buf(),
            project_id,
            project_name,
            rx,
            inbox: inbox.clone(),
            ctx,
            watcher,
            pending,
            open: None,
            open_hash: None,
            last: None,
            rev: 0,
        };
        std::thread::Builder::new()
            .name("designos-project-docs".into())
            .spawn(move || worker.run())
            .map_err(|e| format!("Cannot start the docs worker: {e}"))?;
        let service = Self { jobs, inbox, shown };
        service.send(Job::Init);
        Ok(service)
    }

    /// The tab was drawn in egui pass `pass`.
    pub fn mark_shown(&self, pass: u64) {
        self.shown.store(pass.max(1), Ordering::Relaxed);
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// Everything the worker has left since the last call.
    pub fn take(&self) -> Inbox {
        match self.inbox.lock() {
            Ok(mut inbox) => std::mem::take(&mut *inbox),
            Err(_) => Inbox::default(),
        }
    }
}

impl Drop for DocsService {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
    }
}

struct Worker {
    scanner: Scanner,
    library_root: PathBuf,
    project_id: String,
    project_name: String,
    rx: Receiver<Job>,
    inbox: Arc<Mutex<Inbox>>,
    ctx: egui::Context,
    shown: Arc<AtomicU64>,
    watcher: DocsWatcher,
    pending: Arc<AtomicBool>,
    open: Option<String>,
    open_hash: Option<ContentHash>,
    last: Option<Arc<TreeSnapshot>>,
    rev: u64,
}

impl Worker {
    fn run(mut self) {
        while let Ok(job) = self.rx.recv() {
            if matches!(job, Job::Stop) {
                break;
            }
            self.handle(job);
        }
    }

    fn post(&self, f: impl FnOnce(&mut Inbox)) {
        if let Ok(mut inbox) = self.inbox.lock() {
            f(&mut inbox);
            keep_newest(&mut inbox.errors);
            keep_newest(&mut inbox.notes);
        }
        if tab_is_on_screen(self.shown.load(Ordering::Relaxed), self.ctx.cumulative_pass_nr()) {
            self.ctx.request_repaint();
        }
    }

    fn fail(&self, message: String) {
        self.post(|i| i.errors.push(message));
    }

    fn handle(&mut self, job: Job) {
        match job {
            Job::Init => {
                match ops::ensure_first_page(self.scanner.store(), &self.project_name) {
                    Ok(Some(rel)) => self.post(|i| i.select = Some(rel)),
                    Ok(None) => {}
                    Err(e) => self.fail(e),
                }
                self.publish_snapshot();
            }
            Job::Open(rel) => {
                self.open = Some(rel);
                self.open_hash = None;
                self.read_open(true);
            }
            Job::Rename { rel, title } => match ops::rename_page(self.scanner.store(), &rel, &title) {
                Ok(done) => {
                    for w in done.warnings {
                        self.fail(w);
                    }
                    if done.links_updated > 0 {
                        let n = done.links_updated;
                        self.post(|i| i.notes.push(format!("Updated {n} link{}", if n == 1 { "" } else { "s" })));
                    }
                    if self.open.as_deref() == Some(rel.as_str()) {
                        self.open = Some(done.new_rel.clone());
                        self.open_hash = None;
                    }
                    let new_rel = done.new_rel;
                    self.post(|i| i.renamed = Some((rel, new_rel)));
                    self.publish_snapshot();
                    self.read_open(true);
                }
                Err(e) => self.fail(e),
            },
            Job::Reorder { group, order } => {
                if let Err(e) = ops::reorder_pages(self.scanner.store(), group, &order) {
                    self.fail(e);
                }
                self.publish_snapshot();
            }
            Job::Trash { rel } => match ops::trash_page(self.scanner.store(), &self.library_root, &self.project_id, &rel) {
                Ok(_) => {
                    if self.open.as_deref() == Some(rel.as_str()) {
                        self.open = None;
                        self.open_hash = None;
                    }
                    self.publish_snapshot();
                }
                Err(e) => self.fail(e),
            },
            Job::NewPage { group } => match ops::create_page(self.scanner.store(), group, "Untitled") {
                Ok(rel) => {
                    self.post(|i| {
                        i.select = Some(rel.clone());
                        i.rename = Some(rel);
                    });
                    self.publish_snapshot();
                }
                Err(e) => {
                    self.fail(e);
                    self.publish_snapshot();
                }
            },
            Job::Wake => {
                self.pending.store(false, Ordering::Release);
                let changes = self.watcher.drain();
                self.apply_changes(&changes);
            }
            Job::Refresh => {
                self.publish_snapshot();
                self.read_open(false);
            }
            Job::Stop => {}
        }
    }

    fn apply_changes(&mut self, changes: &[DocChange]) {
        if changes.is_empty() {
            return;
        }
        for c in changes {
            self.scanner.invalidate(&c.rel, c.hash);
        }
        self.publish_snapshot();
        if let Some(open) = self.open.clone() {
            match changes.iter().find(|c| c.rel == open) {
                Some(c) if c.hash.is_none() => {
                    self.open_hash = None;
                    self.post(|i| i.page = Some(PageEvent::Gone(open)));
                }
                Some(c) if c.hash != self.open_hash => self.read_open(true),
                _ => {}
            }
        }
    }

    fn publish_snapshot(&mut self) {
        match self.scanner.snapshot() {
            Ok(snapshot) => {
                if self.last.as_deref() != Some(&snapshot) {
                    let shared = Arc::new(snapshot);
                    self.last = Some(shared.clone());
                    self.post(|i| i.snapshot = Some(shared));
                }
            }
            Err(e) => self.fail(e),
        }
    }

    /// Reads the open page and posts it when it differs from what the UI has. `force` posts even when the hash
    /// matches (the page was just opened).
    fn read_open(&mut self, force: bool) {
        let Some(rel) = self.open.clone() else { return };
        let oversize = std::fs::metadata(self.scanner.store().dir().join(&rel)).map(|m| m.len() > MAX_PAGE_BYTES).unwrap_or(false);
        match self.scanner.store().read_page(&rel) {
            Ok(read) => {
                if !force && self.open_hash == Some(read.hash) {
                    return;
                }
                self.open_hash = Some(read.hash);
                self.rev += 1;
                let loaded = LoadedPage { rel, hash: read.hash, page: Arc::new(read.value), rev: self.rev, oversize };
                self.post(|i| i.page = Some(PageEvent::Loaded(loaded)));
            }
            Err(DocsError::NotFound { .. }) => {
                self.open_hash = None;
                self.post(|i| i.page = Some(PageEvent::Gone(rel)));
            }
            Err(e) => {
                // A page that grew past the limit keeps the version the reader already shows; the note says why it
                // no longer follows the file. A page that was never readable shows the error.
                if oversize && self.open_hash.is_some() {
                    self.post(|i| i.oversize = Some(rel));
                } else {
                    self.post(|i| i.page = Some(PageEvent::Failed { rel, message: e.to_string(), oversize }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_over_the_cap_drop_the_oldest_not_the_newest() {
        let mut v: Vec<String> = (1..=12).map(|n| format!("m{n}")).collect();
        keep_newest(&mut v);
        assert_eq!(v.len(), MAX_QUEUED_MESSAGES);
        assert_eq!((v.first().map(String::as_str), v.last().map(String::as_str)), (Some("m5"), Some("m12")));
        let mut few = vec!["a".to_string()];
        keep_newest(&mut few);
        assert_eq!(few, vec!["a"]);
    }

    #[test]
    fn a_parked_tab_is_told_apart_from_a_visible_one_by_the_pass_it_was_last_drawn_in() {
        assert!(tab_is_on_screen(0, 50), "never drawn yet: the first load wakes the window");
        assert!(tab_is_on_screen(100, 100) && tab_is_on_screen(100, 102));
        assert!(!tab_is_on_screen(100, 103), "three passes without a draw: parked");
    }
}
