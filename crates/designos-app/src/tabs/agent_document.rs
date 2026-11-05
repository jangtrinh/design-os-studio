//! An agent opens another document (`designos_open_file`, `designos_new_document`) in the window it works in.

use super::{ProjectTab, ProjectTabKind, TabsState};
use crate::app::{App, View};
use crate::home::DiskState;
use designos_doc::Document;

impl TabsState {
    /// Shows `agent_doc` in the shown tab's own entry, and moves the document that was shown there to a new entry
    /// just before it, with everything that belongs to it: its edits and history, its file, its unsaved flag, its
    /// view, its image jobs, and a terminal of its own.
    ///
    /// The shown entry keeps its id and its terminal because the agent is working in it. The agent's CLI writes its
    /// edits into the session folder that this id names (`session_id_for_tab`), and a terminal bound to another id
    /// would read a folder the CLI does not write to. So the terminal stays in `app.terminal`, is never re-bound, and
    /// what the agent writes next lands on the document it opened, never on the user's. The agent's document has no
    /// file yet: Save asks where it goes.
    pub(crate) fn show_agent_document_beside_the_users(&mut self, app: &mut App, agent_doc: Document) {
        // The entry of the shown tab, refreshed from `app`, is what the user's new entry is built from.
        self.sync_from_app(app);
        let at = self.active_idx;
        let users_id = self.alloc_id();
        let mut users_terminal = crate::terminal::TerminalState::for_tab(users_id);
        if let Some(lib) = &app.lib {
            users_terminal.library_root = Some(lib.root().to_path_buf());
            users_terminal.skills.set_library_root(lib.root().to_path_buf());
        }
        users_terminal.bind_session(&self.session_id_for_tab(users_id));
        let mut agent_doc = agent_doc;
        crate::io::ensure_page(&mut agent_doc);
        let agent_watermark = agent_doc.peek_next_id();
        let page = if agent_doc.pages.contains(&app.page) { app.page } else { agent_doc.pages[0] };
        let users_doc = Self::take_live_doc(app);
        let users_watermark = app.load_next_id;
        let shown = &mut self.entries[at];
        let users = ProjectTab {
            id: users_id,
            title: shown.title.clone(),
            kind: shown.kind,
            doc: users_doc,
            load_next_id: users_watermark,
            hist: std::mem::take(&mut app.hist),
            terminal: users_terminal,
            image_jobs: app.image_jobs.take_for_park(),
            page: shown.page,
            selection: std::mem::take(&mut shown.selection),
            view: shown.view,
            file_path: shown.file_path.take(),
            lib_file: shown.lib_file.take(),
            dev_mode: shown.dev_mode,
            pinned: std::mem::take(&mut shown.pinned),
            dirty: shown.dirty,
            disk: shown.disk,
        };
        shown.kind = ProjectTabKind::from_doc(&agent_doc);
        shown.doc = agent_doc;
        shown.load_next_id = agent_watermark;
        shown.page = page;
        shown.view = View::default();
        // The agent's document is in no file.
        shown.dirty = true;
        shown.disk = DiskState::default();
        self.entries.insert(at, users);
        self.active_idx = at + 1;
        self.ctx_menu_tab = None;
        self.apply_to_app(self.active_idx, app);
        self.sync_from_app(app);
    }

    /// Binds the terminal on screen to the session folder of the open file (`App::bind_terminal_to_open_file`), unless
    /// another tab of this window already holds that folder. The user's document, kept in a tab of its own by
    /// [`Self::show_agent_document_beside_the_users`], is the case: the agent that was working on its file stays in the
    /// agent's tab with the folder's lock, so asking for the lock again could only fail, and every painted frame would
    /// try. The tab stays on its own scratch folder, and binds once the holder lets go of the file. A lock held by another
    /// window (another process) is not this: `bind_open_file` tells the user so.
    pub(super) fn bind_shown_terminal_to_open_file(&self, app: &mut App, window_id: &str) {
        if !self.another_tab_holds_the_open_files_session_folder(app) {
            app.bind_terminal_to_open_file(window_id);
        }
    }

    fn another_tab_holds_the_open_files_session_folder(&self, app: &App) -> bool {
        let Some(lib) = &app.lib else { return false };
        let lib_file_id = app.lib_file.as_ref().map(|lf| lf.id.as_str());
        let disk_path = app.file_path.as_deref();
        if lib_file_id.is_none() && disk_path.is_none() {
            return false;
        }
        let holds_it = |t: &crate::terminal::TerminalState| {
            t.lock_owner.is_some() && t.open_file_binding.as_ref().is_some_and(|b| b.same_file(lib.root(), lib_file_id, disk_path))
        };
        self.entries.iter().enumerate().any(|(i, tab)| i != self.active_idx && holds_it(&tab.terminal))
    }
}
