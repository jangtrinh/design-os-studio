//! 1:1 Figma UI3 Multi-Tab Project Workspace (`US-10`) and Brand Wordmark Chrome (`DIR-12`).
//!
//! Manages multiple simultaneously open project tabs (`Design`, `Board`, `Deck`, `Spec`),
//! per-tab isolated state (`Document`, `History`, `page`, `selection`, `View`, `file_path`,
//! `lib_file`, `dev_mode`, `pinned`, `dirty`), seamless switching between the `Home` screen
//! and open project tabs without losing state, and the 32px top window tab bar.

use crate::app::{App, RailItem, View};
use crate::home::LibFile;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use crate::widgets::menu::{popup_menu, MenuItem, MenuResult};
use designos_doc::{Document, DocumentKind, History, NodeId};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::path::{Path, PathBuf};

/// Exact height of the 1:1 Figma UI3 Top Window Tab Bar (`32px`).
pub const TOP_TAB_BAR_HEIGHT: f32 = 32.0;

// Top Tab Strip palette tokens (unified with theme.rs Zinc dark palette)
const TAB_BAR_BG: Color32 = theme::BG_PRIMARY;
const TAB_BAR_BORDER: Color32 = theme::BORDER;
const TAB_ACTIVE_BG: Color32 = theme::BG_SECONDARY;
const TAB_ACTIVE_BORDER: Color32 = theme::BORDER_STRONG;
const TAB_HOVER_BG: Color32 = theme::BG_HOVER;
const BADGE_DESIGN_BLUE: Color32 = Color32::from_rgb(0x0D, 0x99, 0xFF);
const BADGE_BOARD_PURPLE: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
const BADGE_DECK_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x3D);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectTabKind {
    Design,
    Board,
    Deck,
    Spec,
}

impl ProjectTabKind {
    pub fn from_doc(doc: &Document) -> Self {
        match doc.kind {
            DocumentKind::Design => Self::Design,
            DocumentKind::Board => Self::Board,
            DocumentKind::Deck => Self::Deck,
        }
    }

    pub fn badge_color(self) -> Color32 {
        match self {
            Self::Design => BADGE_DESIGN_BLUE,
            Self::Board => BADGE_BOARD_PURPLE,
            Self::Deck | Self::Spec => BADGE_DECK_ORANGE,
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Design => Icon::Design,
            Self::Board => Icon::Sticky,
            Self::Deck => Icon::Page,
            Self::Spec => Icon::Specs,
        }
    }
}

pub struct ProjectTab {
    pub id: u64,
    pub title: String,
    pub kind: ProjectTabKind,
    pub doc: Document,
    pub hist: History,
    pub terminal: crate::terminal::TerminalState,
    pub page: NodeId,
    pub selection: Vec<NodeId>,
    pub view: View,
    pub file_path: Option<PathBuf>,
    pub lib_file: Option<LibFile>,
    pub dev_mode: bool,
    pub pinned: bool,
    pub dirty: bool,
}

impl ProjectTab {
    fn from_app(id: u64, app: &App, kind_override: Option<ProjectTabKind>) -> Self {
        let title = Self::derive_title(app);
        let kind = kind_override.unwrap_or_else(|| {
            if app.ui.rail == Some(RailItem::Specs) && app.specs.open {
                ProjectTabKind::Spec
            } else {
                ProjectTabKind::from_doc(&app.doc)
            }
        });
        Self {
            id,
            title,
            kind,
            doc: Document::default(),
            hist: History::default(),
            terminal: crate::terminal::TerminalState::for_tab(id),
            page: app.page,
            selection: app.selection.clone(),
            view: app.view,
            file_path: app.file_path.clone(),
            lib_file: app.lib_file.clone(),
            dev_mode: app.ui.dev_mode,
            pinned: false,
            dirty: app.hist.can_undo() || app.autosave.pending(),
        }
    }

    fn derive_title(app: &App) -> String {
        if let Some(lf) = &app.lib_file {
            if !lf.name.trim().is_empty() {
                return lf.name.clone();
            }
        }
        if !app.doc.name.trim().is_empty() {
            return app.doc.name.clone();
        }
        if let Some(stem) = app.file_path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()) {
            if !stem.trim().is_empty() {
                return stem.to_string();
            }
        }
        "Untitled".to_string()
    }
}

pub struct TabsState {
    pub entries: Vec<ProjectTab>,
    pub active_idx: usize,
    pub next_id: u64,
    pub instance_id: String,
    pub ctx_menu_tab: Option<(usize, Pos2)>,
    pub new_menu_pos: Option<Pos2>,
    /// True in the live native window and screenshot runs so the 32px Top Tab Bar is always shown.
    pub always_visible: bool,
    /// True when the user navigated to `Home` while keeping open project tabs parked in `entries`.
    pub parked_in_home: bool,
    /// Tracks whether `entries[active_idx]`'s live `History` currently resides in `app.hist`.
    hist_in_app: bool,
}

impl Default for TabsState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            active_idx: 0,
            next_id: 1,
            instance_id: String::new(),
            ctx_menu_tab: None,
            new_menu_pos: None,
            always_visible: false,
            parked_in_home: false,
            hist_in_app: true,
        }
    }
}

impl TabsState {
    fn ensure_instance_id(&mut self) {
        if self.instance_id.is_empty() {
            static NEXT_INSTANCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let n = NEXT_INSTANCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.instance_id = format!("p{}-w{n}", std::process::id());
        }
    }

    pub fn session_id_for_tab(&mut self, tab_id: u64) -> String {
        self.ensure_instance_id();
        crate::terminal::tab_session_id(&self.instance_id, tab_id)
    }

    /// Session id the window's terminal keeps: the active tab's, or the first tab's when no tab
    /// exists yet. A shell started before the first frame captures its session directory once, so
    /// the terminal is bound to this id before such a shell can start.
    pub fn active_session_id(&mut self) -> String {
        let tab_id = self.entries.get(self.active_idx).map_or(self.next_id, |t| t.id);
        self.session_id_for_tab(tab_id)
    }

    fn alloc_id(&mut self) -> u64 {
        self.ensure_instance_id();
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Whether the 32px Top Window Tab Bar should be rendered this frame.
    pub fn is_bar_visible(&self, _is_home: bool) -> bool {
        self.always_visible || self.entries.len() > 1
    }

    /// Snapshots lightweight active `app` metadata into `entries[active_idx]` (and initializes the
    /// first tab from `app` if `entries.is_empty()`). Never clones `app.doc` on per-frame ticks
    /// (`es:rust` Law 10).
    pub fn sync_from_app(&mut self, app: &App) {
        if self.entries.is_empty() {
            let id = self.alloc_id();
            let sid = self.session_id_for_tab(id);
            let mut tab = ProjectTab::from_app(id, app, None);
            tab.terminal.bind_session(&sid);
            self.entries.push(tab);
            self.active_idx = 0;
            self.hist_in_app = true;
            return;
        }
        if app.home.is_some() {
            return;
        }
        if self.active_idx >= self.entries.len() {
            self.active_idx = self.entries.len() - 1;
        }
        let tab = &mut self.entries[self.active_idx];
        tab.title = ProjectTab::derive_title(app);
        if tab.kind != ProjectTabKind::Spec {
            tab.kind = ProjectTabKind::from_doc(&app.doc);
        }
        tab.page = app.page;
        tab.selection = app.selection.clone();
        tab.view = app.view;
        tab.file_path = app.file_path.clone();
        tab.lib_file = app.lib_file.clone();
        tab.dev_mode = app.ui.dev_mode;
        tab.dirty = app.hist.can_undo() || tab.hist.can_undo() || app.autosave.pending();
    }

    /// Moves the live document out of `app` and detaches `app` from its library file in the same step.
    /// `flush_autosave` and `go_home` write `app.doc` to `app.lib_file`; a file still attached to an
    /// emptied document would be overwritten by the next of them. The parked tab keeps its own
    /// `lib_file`, and `apply_to_app` attaches the next tab's file together with its document. Callers
    /// flush the document before taking it, never after.
    fn take_live_doc(app: &mut App) -> Document {
        app.lib_file = None;
        std::mem::take(&mut app.doc)
    }

    /// Parks the active tab's state (moving `app.doc`, `app.hist`, and `app.terminal` into `entries[active_idx]`).
    fn park_active_from_app(&mut self, app: &mut App) {
        if self.entries.is_empty() {
            crate::decks::ui::flush_notes(app);
            app.stop_plugin();
            app.exit_version_view();
            app.flush_autosave();
            let id = self.alloc_id();
            let sid = self.session_id_for_tab(id);
            let mut tab = ProjectTab::from_app(id, app, None);
            tab.doc = Self::take_live_doc(app);
            tab.hist = std::mem::take(&mut app.hist);
            app.terminal.bind_session(&sid);
            tab.terminal = std::mem::take(&mut app.terminal);
            tab.dirty = tab.hist.can_undo() || app.autosave.pending();
            self.entries.push(tab);
            self.active_idx = 0;
            self.hist_in_app = false;
            return;
        }
        if app.home.is_some() {
            if self.hist_in_app && self.active_idx < self.entries.len() {
                let active_id = self.entries[self.active_idx].id;
                let sid = self.session_id_for_tab(active_id);
                app.terminal.bind_session(&sid);
                if self.entries[self.active_idx].doc.nodes.is_empty() {
                    self.entries[self.active_idx].doc = Self::take_live_doc(app);
                }
                self.entries[self.active_idx].hist = std::mem::take(&mut app.hist);
                self.entries[self.active_idx].terminal = std::mem::take(&mut app.terminal);
                self.hist_in_app = false;
            }
            return;
        }
        self.sync_from_app(app);
        if self.hist_in_app && self.active_idx < self.entries.len() {
            crate::decks::ui::flush_notes(app);
            app.stop_plugin();
            app.exit_version_view();
            app.flush_autosave();
            let active_id = self.entries[self.active_idx].id;
            let sid = self.session_id_for_tab(active_id);
            app.terminal.bind_session(&sid);
            self.entries[self.active_idx].doc = Self::take_live_doc(app);
            self.entries[self.active_idx].hist = std::mem::take(&mut app.hist);
            self.entries[self.active_idx].terminal = std::mem::take(&mut app.terminal);
            self.entries[self.active_idx].dirty = self.entries[self.active_idx].hist.can_undo() || app.autosave.pending();
            self.hist_in_app = false;
        }
    }

    /// Moves `entries[idx]` into `app.{doc, page, selection, view, file_path, lib_file, ui.dev_mode}`
    /// and sets `app.home = None`.
    pub fn apply_to_app(&mut self, idx: usize, app: &mut App) {
        if idx >= self.entries.len() {
            return;
        }
        crate::decks::ui::flush_notes(app);
        app.stop_plugin();
        app.exit_version_view();
        app.flush_autosave();

        let (mut doc, tab_page, tab_selection, tab_view, tab_file_path, tab_lib_file, tab_dev_mode, tab_kind) = {
            let tab = &mut self.entries[idx];
            let doc = if tab.doc.nodes.is_empty() && idx == self.active_idx && !app.doc.nodes.is_empty() {
                std::mem::take(&mut app.doc)
            } else {
                std::mem::take(&mut tab.doc)
            };
            (doc, tab.page, tab.selection.clone(), tab.view, tab.file_path.clone(), tab.lib_file.clone(), tab.dev_mode, tab.kind)
        };
        crate::io::ensure_page(&mut doc);
        let page = if doc.pages.contains(&tab_page) { tab_page } else { doc.pages[0] };
        app.doc = doc;
        app.page = page;
        app.selection = tab_selection;
        app.view = tab_view;
        app.file_path = tab_file_path;
        app.lib_file = tab_lib_file;
        app.ui.dev_mode = tab_dev_mode;
        app.renaming_file = false;
        app.hover = None;
        app.hover_layer = None;
        app.canvas.reset_interaction();
        app.comments.close_popovers();
        app.layers.reset();
        app.fonts.load_for(&app.doc);
        if tab_kind == ProjectTabKind::Spec {
            app.ui.rail = Some(RailItem::Specs);
            app.specs.open = true;
            app.specs.ensure_loaded();
        } else if app.specs.open && app.ui.rail == Some(RailItem::Specs) {
            app.specs.open = false;
        }
        app.home = None;
        app.ui.menu = None;
        app.ui.ctx_menu = None;
        app.sanitize();
        app.changed();
    }

    /// Switches from the current tab (or `Home`) to `entries[idx]`, preserving `History`, `TerminalState`, and view state.
    pub fn switch_to(&mut self, idx: usize, app: &mut App) {
        if self.entries.is_empty() {
            self.sync_from_app(app);
        }
        if idx >= self.entries.len() {
            return;
        }
        self.park_active_from_app(app);
        self.active_idx = idx;
        self.parked_in_home = false;
        self.apply_to_app(idx, app);
        let tab_id = self.entries[idx].id;
        let sid = self.session_id_for_tab(tab_id);
        app.hist = std::mem::take(&mut self.entries[idx].hist);
        app.terminal = std::mem::take(&mut self.entries[idx].terminal);
        app.terminal.bind_session(&sid);
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        self.entries[idx].dirty = app.hist.can_undo() || app.autosave.pending();
        self.hist_in_app = true;
    }

    /// Switches to the `Home` screen while keeping all open project tabs intact in the Top Tab Bar.
    pub fn enter_home(&mut self, app: &mut App) {
        if app.home.is_none() {
            // The closing version needs the live document and its file, which parking detaches.
            crate::decks::ui::flush_notes(app);
            app.save_close_version();
            self.park_active_from_app(app);
        }
        self.parked_in_home = true;
        if app.lib.is_some() {
            app.go_home();
        } else {
            app.home = Some(crate::home::Home::new());
            app.ui.menu = None;
            app.ui.ctx_menu = None;
        }
    }

    /// Reconciles tab state when the user opens or creates a file from the `Home` screen.
    pub fn reconcile_frame(&mut self, app: &mut App) {
        if app.home.is_some() {
            self.parked_in_home = true;
            return;
        }
        if self.entries.is_empty() {
            self.sync_from_app(app);
            if let Some(tab_id) = self.entries.get(self.active_idx).map(|t| t.id) {
                let sid = self.session_id_for_tab(tab_id);
                app.terminal.bind_session(&sid);
                app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
            }
            return;
        }
        if self.parked_in_home {
            self.parked_in_home = false;
            // Check if the file now in `app` matches an existing tab
            if let Some(lf) = &app.lib_file {
                if let Some(existing) = self.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|tlf| tlf.id == lf.id)) {
                    let tab_id = self.entries[existing].id;
                    let sid = self.session_id_for_tab(tab_id);
                    self.active_idx = existing;
                    self.entries[existing].doc = Document::default();
                    app.terminal = std::mem::take(&mut self.entries[existing].terminal);
                    app.terminal.bind_session(&sid);
                    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
                    self.hist_in_app = true;
                    self.sync_from_app(app);
                    return;
                }
            } else if let Some(path) = &app.file_path {
                if let Some(existing) = self.entries.iter().position(|t| t.file_path.as_ref() == Some(path)) {
                    let tab_id = self.entries[existing].id;
                    let sid = self.session_id_for_tab(tab_id);
                    self.active_idx = existing;
                    self.entries[existing].doc = Document::default();
                    app.terminal = std::mem::take(&mut self.entries[existing].terminal);
                    app.terminal.bind_session(&sid);
                    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
                    self.hist_in_app = true;
                    self.sync_from_app(app);
                    return;
                }
            }
            // If it differs from the previously active tab, open it as a new tab!
            let differs = self
                .entries
                .get(self.active_idx)
                .is_some_and(|cur| cur.lib_file != app.lib_file || cur.file_path != app.file_path || cur.title != ProjectTab::derive_title(app));
            if differs {
                let id = self.alloc_id();
                let sid = self.session_id_for_tab(id);
                let mut tab = ProjectTab::from_app(id, app, None);
                tab.terminal.bind_session(&sid);
                self.entries.push(tab);
                self.active_idx = self.entries.len() - 1;
                app.terminal = crate::terminal::TerminalState::for_tab(id);
                app.terminal.bind_session(&sid);
                app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
                self.hist_in_app = true;
                return;
            }
            if let Some(tab) = self.entries.get_mut(self.active_idx) {
                let tab_id = tab.id;
                tab.doc = Document::default();
                if !self.hist_in_app {
                    app.terminal = std::mem::take(&mut tab.terminal);
                }
                self.hist_in_app = true;
                let sid = self.session_id_for_tab(tab_id);
                app.terminal.bind_session(&sid);
            }
        }
        self.sync_from_app(app);
        if let Some(tab_id) = self.entries.get(self.active_idx).map(|t| t.id) {
            let sid = self.session_id_for_tab(tab_id);
            app.terminal.bind_session(&sid);
            app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        }
    }

    /// Saves the current tab, pushes a new `ProjectTab`, switches to it, and closes `Home`.
    pub fn open_new_tab(&mut self, app: &mut App, title: &str, mut doc: Document, kind: ProjectTabKind) -> usize {
        if app.home.is_none() {
            self.park_active_from_app(app);
        }
        doc.name = title.to_string();
        doc.kind = match kind {
            ProjectTabKind::Board => DocumentKind::Board,
            ProjectTabKind::Deck => DocumentKind::Deck,
            ProjectTabKind::Design | ProjectTabKind::Spec => DocumentKind::Design,
        };
        crate::io::ensure_page(&mut doc);
        let page = doc.pages[0];
        let id = self.alloc_id();
        let sid = self.session_id_for_tab(id);
        let mut terminal = crate::terminal::TerminalState::for_tab(id);
        terminal.bind_session(&sid);
        let tab = ProjectTab {
            id,
            title: title.to_string(),
            kind,
            doc,
            hist: History::default(),
            terminal,
            page,
            selection: Vec::new(),
            view: View::default(),
            file_path: None,
            lib_file: None,
            dev_mode: false,
            pinned: false,
            dirty: false,
        };
        self.entries.push(tab);
        let new_idx = self.entries.len() - 1;
        self.active_idx = new_idx;
        self.parked_in_home = false;
        self.apply_to_app(new_idx, app);
        app.hist = History::default();
        app.terminal = std::mem::take(&mut self.entries[new_idx].terminal);
        app.terminal.bind_session(&sid);
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        self.hist_in_app = true;
        app.fit_all();
        self.entries[new_idx].view = app.view;
        new_idx
    }

    /// Creates a new `"Untitled"` design tab and activates it.
    pub fn open_new_design_tab(&mut self, app: &mut App) -> usize {
        let mut doc = Document::new("Untitled");
        doc.add_page("Page 1");
        self.open_new_tab(app, "Untitled", doc, ProjectTabKind::Design)
    }

    /// Creates a new `"Untitled"` board tab and activates it.
    pub fn open_new_board_tab(&mut self, app: &mut App) -> usize {
        let doc = crate::board::create::new_document("Untitled");
        self.open_new_tab(app, "Untitled", doc, ProjectTabKind::Board)
    }

    /// Creates a new `"Untitled"` slide deck tab and activates it.
    pub fn open_new_deck_tab(&mut self, app: &mut App) -> usize {
        let mut doc = Document::new("Untitled");
        doc.kind = DocumentKind::Deck;
        doc.add_page("Slide 1");
        self.open_new_tab(app, "Untitled", doc, ProjectTabKind::Deck)
    }

    /// Focuses an existing tab with `file_path == Some(path)`, or opens `path` in a new tab.
    #[allow(dead_code)]
    pub fn open_or_focus_path(&mut self, app: &mut App, path: &Path) -> Option<usize> {
        if let Some(idx) = self.entries.iter().position(|t| t.file_path.as_deref() == Some(path)) {
            self.switch_to(idx, app);
            return Some(idx);
        }
        let (doc, warning) = match crate::io::load_path_with_warning(path) {
            Ok(pair) => pair,
            Err(e) => {
                app.show_toast(e, true);
                return None;
            }
        };
        let title = if doc.name.trim().is_empty() { path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string() } else { doc.name.clone() };
        let kind = ProjectTabKind::from_doc(&doc);
        let idx = self.open_new_tab(app, &title, doc, kind);
        if warning.is_some() {
            app.show_toast(crate::io::opened_toast_text(&title, warning.as_deref()), true);
        }
        let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        if !svg {
            let pb = path.to_path_buf();
            self.entries[idx].file_path = Some(pb.clone());
            app.file_path = Some(pb);
            app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        }
        Some(idx)
    }

    /// Focuses an existing tab with `lib_file.id == file_id`, or opens `file_id` from the library in a new tab.
    #[allow(dead_code)]
    pub fn open_or_focus_lib_file(&mut self, app: &mut App, file_id: &str) -> Option<usize> {
        if let Some(idx) = self.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|lf| lf.id == file_id)) {
            self.switch_to(idx, app);
            return Some(idx);
        }
        if app.home.is_none() {
            self.park_active_from_app(app);
        }
        app.open_library_file(file_id);
        if app.lib_file.as_ref().is_some_and(|lf| lf.id == file_id) {
            let id = self.alloc_id();
            let sid = self.session_id_for_tab(id);
            let mut tab = ProjectTab::from_app(id, app, None);
            tab.terminal.bind_session(&sid);
            self.entries.push(tab);
            let idx = self.entries.len() - 1;
            self.active_idx = idx;
            self.parked_in_home = false;
            self.hist_in_app = true;
            app.terminal = crate::terminal::TerminalState::for_tab(id);
            app.terminal.bind_session(&sid);
            app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
            Some(idx)
        } else {
            None
        }
    }

    /// Disk path of tab `idx` (its external `file_path` or its library file's disk path), reading
    /// live `app` fields when `idx` is the active unparked tab.
    pub(crate) fn disk_path_of_tab(&self, idx: usize, app: &App) -> Option<PathBuf> {
        let live = app.home.is_none() && !self.parked_in_home && idx == self.active_idx;
        if live {
            if let Some(p) = app.file_path.clone().or_else(|| app.lib_file.as_ref().and_then(|lf| app.lib_file_disk_path(lf))) {
                return Some(p);
            }
        }
        let tab = self.entries.get(idx)?;
        tab.file_path.clone().or_else(|| tab.lib_file.as_ref().and_then(|lf| app.lib_file_disk_path(lf)))
    }

    /// True when any tab in `self.entries` (other than `skip_idx`, if given) holds a file with the
    /// same [`crate::io::same_repair_key`] as `path`.
    pub(crate) fn has_open_repair_key(&self, app: &App, path: &Path, skip_idx: Option<usize>) -> bool {
        (0..self.entries.len()).filter(|i| Some(*i) != skip_idx).filter_map(|i| self.disk_path_of_tab(i, app)).any(|tp| crate::io::same_repair_key(&tp, path))
    }

    /// Drops the cached pre-repair bytes of closed tabs no remaining tab holds (by repair key).
    /// Runs after the closed file's last save, which is what consumes them to write the backup.
    fn release_closed_paths(&self, app: &App, closed: &[PathBuf]) {
        for path in closed {
            if !self.has_open_repair_key(app, path, None) {
                crate::io::drop_repaired_original(path);
            }
        }
    }

    /// Closes `entries[idx]` unless it is `pinned`. If `entries` becomes empty, opens `Home`;
    /// otherwise activates the nearest remaining tab.
    pub fn close_tab(&mut self, idx: usize, app: &mut App) -> bool {
        let closed_paths: Vec<_> = self.disk_path_of_tab(idx, app).into_iter().collect();
        let closed = self.close_tab_entry(idx, app);
        if closed {
            self.release_closed_paths(app, &closed_paths);
        }
        closed
    }

    fn close_tab_entry(&mut self, idx: usize, app: &mut App) -> bool {
        if idx >= self.entries.len() || self.entries[idx].pinned {
            return false;
        }
        let was_home = app.home.is_some();
        let old_active = self.active_idx;
        if !was_home && idx == old_active {
            app.terminal.shutdown();
        }
        let mut removed = self.entries.remove(idx);
        removed.terminal.shutdown();
        if self.entries.is_empty() {
            self.active_idx = 0;
            self.hist_in_app = false;
            self.parked_in_home = true;
            if !was_home {
                if app.lib.is_some() {
                    // Saves the file being left, so it must still be attached to the app here.
                    app.go_home();
                } else {
                    app.home = Some(crate::home::Home::new());
                }
            }
            app.file_path = None;
            app.lib_file = None;
            return true;
        }
        if was_home {
            if idx < self.active_idx {
                self.active_idx -= 1;
            } else if self.active_idx >= self.entries.len() {
                self.active_idx = self.entries.len() - 1;
            }
            return true;
        }
        if idx != old_active {
            if idx < old_active {
                self.active_idx = old_active - 1;
            }
            return true;
        }
        let next_idx = idx.min(self.entries.len() - 1);
        self.active_idx = next_idx;
        self.parked_in_home = false;
        self.apply_to_app(next_idx, app);
        let tab_id = self.entries[next_idx].id;
        let sid = self.session_id_for_tab(tab_id);
        app.hist = std::mem::take(&mut self.entries[next_idx].hist);
        app.terminal = std::mem::take(&mut self.entries[next_idx].terminal);
        app.terminal.bind_session(&sid);
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        self.hist_in_app = true;
        true
    }

    /// Closes all unpinned tabs except `keep_idx` and activates `keep_idx`.
    pub fn close_other_tabs(&mut self, keep_idx: usize, app: &mut App) {
        if keep_idx >= self.entries.len() {
            return;
        }
        if app.home.is_none() {
            self.park_active_from_app(app);
        }
        let keep_id = self.entries[keep_idx].id;
        let mut closed_paths = Vec::new();
        for idx in 0..self.entries.len() {
            if self.entries[idx].id != keep_id && !self.entries[idx].pinned {
                if let Some(p) = self.disk_path_of_tab(idx, app) {
                    closed_paths.push(p);
                }
            }
        }
        for t in &mut self.entries {
            if t.id != keep_id && !t.pinned {
                t.terminal.shutdown();
            }
        }
        self.entries.retain(|t| t.id == keep_id || t.pinned);
        if let Some(new_idx) = self.entries.iter().position(|t| t.id == keep_id) {
            let tab_id = self.entries[new_idx].id;
            let sid = self.session_id_for_tab(tab_id);
            self.active_idx = new_idx;
            self.parked_in_home = false;
            self.apply_to_app(new_idx, app);
            app.hist = std::mem::take(&mut self.entries[new_idx].hist);
            app.terminal = std::mem::take(&mut self.entries[new_idx].terminal);
            app.terminal.bind_session(&sid);
            app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
            self.hist_in_app = true;
        }
        self.release_closed_paths(app, &closed_paths);
    }

    /// Clones `entries[idx]` as `"<title> (Copy)"` in a new tab right after `idx` and activates it.
    pub fn duplicate_tab(&mut self, idx: usize, app: &mut App) -> Option<usize> {
        if idx >= self.entries.len() {
            return None;
        }
        if app.home.is_none() {
            self.park_active_from_app(app);
        }
        let src = &self.entries[idx];
        let copy_title = format!("{} (Copy)", src.title);
        let mut copy_doc = src.doc.clone();
        copy_doc.name = copy_title.clone();
        let copy_kind = src.kind;
        let copy_page = src.page;
        let copy_selection = src.selection.clone();
        let copy_view = src.view;
        let copy_dev_mode = src.dev_mode;

        let new_id = self.alloc_id();
        let sid = self.session_id_for_tab(new_id);
        let mut terminal = crate::terminal::TerminalState::for_tab(new_id);
        terminal.bind_session(&sid);
        let new_tab = ProjectTab {
            id: new_id,
            title: copy_title,
            kind: copy_kind,
            doc: copy_doc,
            hist: History::default(),
            terminal,
            page: copy_page,
            selection: copy_selection,
            view: copy_view,
            file_path: None,
            lib_file: None,
            dev_mode: copy_dev_mode,
            pinned: false,
            dirty: false,
        };
        let insert_idx = (idx + 1).min(self.entries.len());
        self.entries.insert(insert_idx, new_tab);
        self.active_idx = insert_idx;
        self.parked_in_home = false;
        self.apply_to_app(insert_idx, app);
        app.hist = History::default();
        app.terminal = std::mem::take(&mut self.entries[insert_idx].terminal);
        app.terminal.bind_session(&sid);
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
        self.hist_in_app = true;
        Some(insert_idx)
    }

    /// Toggles the `pinned` state of `entries[idx]`.
    pub fn toggle_pin(&mut self, idx: usize) {
        if let Some(tab) = self.entries.get_mut(idx) {
            tab.pinned = !tab.pinned;
        }
    }

    /// Cycles to the next open project tab (`Ctrl+Tab`).
    pub fn next_tab(&mut self, app: &mut App) {
        if self.entries.len() <= 1 {
            return;
        }
        let next = (self.active_idx + 1) % self.entries.len();
        self.switch_to(next, app);
    }

    /// Cycles to the previous open project tab (`Ctrl+Shift+Tab`).
    pub fn prev_tab(&mut self, app: &mut App) {
        if self.entries.len() <= 1 {
            return;
        }
        let prev = (self.active_idx + self.entries.len() - 1) % self.entries.len();
        self.switch_to(prev, app);
    }
}

#[derive(Clone, Debug)]
enum TabUiAction {
    ClickHome,
    SwitchTab(usize),
    CloseTab(usize),
    DuplicateTab(usize),
    TogglePin(usize),
    CloseOtherTabs(usize),
    NewDesignTab,
    NewBoardTab,
    NewDeckTab,
}

/// Renders the 1:1 Figma UI3 Top Window Tab Bar (`32px` height) inside `ui`.
#[allow(dead_code)]
pub fn show_top_tab_bar(ui: &mut Ui, app: &mut App) {
    egui::Panel::top("project_tabs_bar").exact_size(TOP_TAB_BAR_HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| {
        let rect = ui.max_rect();
        show_top_tab_bar_ui(ui, rect, app);
    });
}

/// Renders the 1:1 Figma UI3 Top Window Tab Bar (`32px` height) inside `bar_rect`.
pub fn show_top_tab_bar_ui(ui: &mut Ui, bar_rect: Rect, app: &mut App) {
    {
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(app);
        app.tabs = tabs;
    }

    let p = ui.painter_at(bar_rect);
    p.rect_filled(bar_rect, 0.0, TAB_BAR_BG);
    p.hline(bar_rect.x_range(), bar_rect.bottom() - 0.5, Stroke::new(1.0, TAB_BAR_BORDER));

    let mut actions: Vec<TabUiAction> = Vec::new();
    let cy = bar_rect.center().y;
    let mut x = bar_rect.left() + 8.0;

    // Brand wordmark is rendered in the top window title bar (`window-title-brand`);
    // keep a zero-size anchor so existing widget-id queries stay satisfied.
    widgets::record(ui, "tab-bar-brand", Rect::from_min_size(Pos2::new(bar_rect.left(), bar_rect.top()), Vec2::ZERO));

    // 1. Home Tab Button (`id: "tab-bar-home"`)
    {
        let is_home = app.home.is_some();
        let home_rect = Rect::from_min_size(Pos2::new(x, bar_rect.top() + 4.0), Vec2::new(72.0, 24.0));
        widgets::record(ui, "tab-bar-home", home_rect);
        let resp = ui.interact(home_rect, ui.id().with("tab-bar-home"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if is_home {
            p.rect_filled(home_rect, theme::cr(6), TAB_ACTIVE_BG);
            p.rect_stroke(home_rect, theme::cr(6), Stroke::new(1.0, TAB_ACTIVE_BORDER), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_filled(home_rect, theme::cr(6), TAB_HOVER_BG);
        }
        let fg = if is_home || resp.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY };
        paint_home_glyph(&p, Pos2::new(home_rect.left() + 14.0, cy), fg);
        p.text(Pos2::new(home_rect.left() + 26.0, cy), Align2::LEFT_CENTER, "Home", if is_home { theme::font_body_strong() } else { theme::font_body() }, fg);
        if resp.clicked() {
            actions.push(TabUiAction::ClickHome);
        }
        x = home_rect.right() + 6.0;
    }

    // Divider between Home button and open project tabs
    p.vline(x, (bar_rect.top() + 8.0)..=(bar_rect.bottom() - 8.0), Stroke::new(1.0, TAB_ACTIVE_BORDER));
    x += 6.0;

    // 3. Open Project Tabs Strip
    let is_home = app.home.is_some();
    let active_idx = app.tabs.active_idx;
    let active_dirty_live = app.hist.can_undo() || app.autosave.pending();
    let avail_for_tabs = (bar_rect.right() - x - 118.0).max(120.0);
    let tab_count = app.tabs.entries.len().max(1);
    let ideal_w = (avail_for_tabs / tab_count as f32).clamp(112.0, 240.0);

    for idx in 0..app.tabs.entries.len() {
        let tab = &app.tabs.entries[idx];
        let tab_id = tab.id;
        let is_active = !is_home && idx == active_idx;
        let is_dirty = tab.dirty || (is_active && active_dirty_live);
        let tab_w = if tab.pinned && tab.title.len() <= 8 { ideal_w.min(132.0) } else { ideal_w };
        let tab_rect = Rect::from_min_size(Pos2::new(x, bar_rect.top() + 3.0), Vec2::new(tab_w, 26.0));
        if tab_rect.right() > bar_rect.right() - 104.0 {
            break;
        }
        widgets::record(ui, &format!("project-tab-{idx}"), tab_rect);
        widgets::record(ui, &format!("project-tab-id-{tab_id}"), tab_rect);

        let close_rect = Rect::from_center_size(Pos2::new(tab_rect.right() - 13.0, tab_rect.center().y), Vec2::splat(18.0));
        widgets::record(ui, &format!("tab-close-{tab_id}"), close_rect);

        // Exclude close_rect from tab_hit_rect so clicking `×` never triggers tab switch instead of close
        let tab_hit_right = if tab.pinned { tab_rect.right() } else { (close_rect.left() - 2.0).max(tab_rect.left() + 24.0) };
        let tab_hit_rect = Rect::from_min_max(tab_rect.min, Pos2::new(tab_hit_right, tab_rect.bottom()));
        let tab_resp = ui.interact(tab_hit_rect, ui.id().with(("project-tab", tab_id)), Sense::click());
        let close_resp = if !tab.pinned { Some(ui.interact(close_rect, ui.id().with(("tab-close", tab_id)), Sense::click())) } else { None };

        let hovered = tab_resp.hovered() || close_resp.as_ref().is_some_and(|r| r.hovered());
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if is_active {
            p.rect_filled(tab_rect, theme::cr_each(theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, theme::RADIUS_SEGMENT, theme::RADIUS_SEGMENT), TAB_ACTIVE_BG);
            p.rect_stroke(
                tab_rect,
                theme::cr_each(theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, theme::RADIUS_SEGMENT, theme::RADIUS_SEGMENT),
                Stroke::new(1.0, TAB_ACTIVE_BORDER),
                StrokeKind::Inside,
            );
        } else if hovered {
            p.rect_filled(tab_rect, theme::cr(theme::RADIUS_CONTROL), TAB_HOVER_BG);
        } else {
            // Subtle right separator between inactive tabs
            p.vline(tab_rect.right() + 1.0, (tab_rect.top() + 5.0)..=(tab_rect.bottom() - 5.0), Stroke::new(1.0, TAB_BAR_BORDER));
        }

        // Colored file-type icon badge (or Figma Dev Mode #1BC47D `</>` badge when dev_mode is active)
        let badge_rect = Rect::from_center_size(Pos2::new(tab_rect.left() + 13.0, tab_rect.center().y), Vec2::splat(14.0));
        let in_dev_mode = tab.dev_mode || (is_active && app.ui.dev_mode);
        let (badge_col, badge_icon) = if in_dev_mode { (Color32::from_rgb(0x1B, 0xC4, 0x7D), Icon::Dev) } else { (tab.kind.badge_color(), tab.kind.icon()) };
        if in_dev_mode {
            p.rect_filled(badge_rect, theme::cr(3), badge_col);
            icons::paint(&p, badge_icon, badge_rect.center(), 10.0, Color32::from_rgb(0x0D, 0x1F, 0x16));
        } else {
            p.rect_filled(badge_rect, theme::cr(3), Color32::from_rgba_unmultiplied(badge_col.r(), badge_col.g(), badge_col.b(), 38));
            icons::paint(&p, badge_icon, badge_rect.center(), 11.0, badge_col);
        }

        let mut text_left = badge_rect.right() + 6.0;
        if tab.pinned {
            icons::paint(&p, Icon::Lock, Pos2::new(text_left + 5.0, tab_rect.center().y), 10.0, theme::TEXT_SECONDARY);
            text_left += 13.0;
        }

        let right_reserve = if tab.pinned { 10.0 } else { 24.0 } + if is_dirty { 10.0 } else { 0.0 };
        let max_title_w = (tab_rect.right() - right_reserve - text_left).max(24.0);
        let font = if is_active { theme::font_body_strong() } else { theme::font_body() };
        let fg = if is_active || hovered { theme::TEXT } else { theme::TEXT_SECONDARY };
        let display_title = truncate_tab_title(&p, &tab.title, &font, max_title_w);
        p.text(Pos2::new(text_left, tab_rect.center().y), Align2::LEFT_CENTER, &display_title, font, fg);

        // Unsaved/modified dot `•`
        if is_dirty {
            let dot_x = if tab.pinned { tab_rect.right() - 10.0 } else { close_rect.left() - 6.0 };
            p.circle_filled(Pos2::new(dot_x, tab_rect.center().y), 3.0, BADGE_DESIGN_BLUE);
        }

        // Close `×` button — always visible on unpinned tabs so user can close in 1 click
        if let Some(cr) = &close_resp {
            if cr.hovered() {
                p.rect_filled(close_rect, theme::cr(4), Color32::from_rgb(0xE1, 0x1D, 0x48));
            }
            let close_fg = if cr.hovered() {
                Color32::WHITE
            } else if is_active || hovered {
                theme::TEXT
            } else {
                theme::TEXT_SECONDARY
            };
            icons::paint(&p, Icon::Close, close_rect.center(), 10.0, close_fg);
        }

        let close_clicked = close_resp.as_ref().is_some_and(|r| r.clicked());
        if close_clicked {
            actions.push(TabUiAction::CloseTab(idx));
        } else if tab_resp.clicked() {
            actions.push(TabUiAction::SwitchTab(idx));
        } else if tab_resp.middle_clicked() && !tab.pinned {
            actions.push(TabUiAction::CloseTab(idx));
        }
        if tab_resp.secondary_clicked() {
            let pos = ui.ctx().input(|i| i.pointer.interact_pos()).unwrap_or_else(|| tab_rect.left_bottom());
            app.tabs.ctx_menu_tab = Some((idx, pos));
        }

        x = tab_rect.right() + 3.0;
    }

    // 4. Trailing `+` New Tab Button ("tab-bar-new") — clean 1:1 Figma UI3 end-cap
    {
        let plus_rect = Rect::from_min_size(Pos2::new(x + 2.0, bar_rect.top() + 4.0), Vec2::splat(24.0));
        widgets::record(ui, "tab-bar-new", plus_rect);
        let plus_resp = ui.interact(plus_rect, ui.id().with("tab-bar-new"), Sense::click());
        if plus_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            p.rect_filled(plus_rect, theme::cr(theme::RADIUS_CONTROL), TAB_HOVER_BG);
        }
        icons::paint(&p, Icon::Plus, plus_rect.center(), 13.0, if plus_resp.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY });
        if plus_resp.clicked() {
            actions.push(TabUiAction::NewDesignTab);
        }
        if plus_resp.secondary_clicked() {
            let pos = ui.ctx().input(|i| i.pointer.interact_pos()).unwrap_or_else(|| plus_rect.left_bottom());
            app.tabs.new_menu_pos = Some(pos);
        }
    }

    // 4c. Right-aligned compact Update check button ("top-tab-update-btn"); hidden until the updater is real (see `updater::UPDATER_UI_ENABLED`)
    if crate::home::updater::UPDATER_UI_ENABLED {
        let upd_rect = Rect::from_min_size(Pos2::new(bar_rect.right() - 68.0, bar_rect.top() + 4.0), Vec2::new(60.0, 24.0));
        widgets::record(ui, "top-tab-update-btn", upd_rect);
        let upd_resp = ui.interact(upd_rect, ui.id().with("top-tab-update-btn"), Sense::click());
        if upd_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            p.rect_filled(upd_rect, theme::cr(theme::RADIUS_CONTROL), TAB_HOVER_BG);
        }
        p.circle_filled(Pos2::new(upd_rect.left() + 8.0, upd_rect.center().y), 2.5, Color32::from_rgb(0x1B, 0xC4, 0x7D));
        p.text(
            Pos2::new(upd_rect.left() + 14.0, upd_rect.center().y),
            Align2::LEFT_CENTER,
            "Update",
            theme::font_caption(),
            if upd_resp.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        if upd_resp.on_hover_text("Check for updates & verify binary SHA-256").clicked() {
            let msg = if let Some(home) = &mut app.home {
                home.updater.check_for_updates();
                home.updater.modal_open = true;
                let sha_short: String = home.updater.current_binary_sha256.chars().take(12).collect();
                let badge = home.updater.upstream.summary_badge();
                format!("design:os v{} verified (SHA-256 {}…) · {}", env!("CARGO_PKG_VERSION"), sha_short, badge)
            } else {
                let mut probe = crate::home::updater::UpdaterState::new();
                probe.check_for_updates();
                let sha_short: String = probe.current_binary_sha256.chars().take(12).collect();
                format!("design:os v{} up to date (SHA-256 {}…) · {}", env!("CARGO_PKG_VERSION"), sha_short, probe.upstream.summary_badge())
            };
            app.show_toast(msg, false);
        }
    }

    // 4b. Right-Click Menu on `+` Button (`New Design File` / `New FigJam Board` / `New Slide Deck`)
    if let Some(pos) = app.tabs.new_menu_pos {
        let items = vec![MenuItem::new("New Design File").shortcut("Ctrl+T"), MenuItem::new("New FigJam Board"), MenuItem::new("New Slide Deck")];
        match popup_menu(ui.ctx(), egui::Id::new("project-tab-new-menu"), pos, 188.0, &items, None) {
            MenuResult::Picked(0) => {
                app.tabs.new_menu_pos = None;
                actions.push(TabUiAction::NewDesignTab);
            }
            MenuResult::Picked(1) => {
                app.tabs.new_menu_pos = None;
                actions.push(TabUiAction::NewBoardTab);
            }
            MenuResult::Picked(2) => {
                app.tabs.new_menu_pos = None;
                actions.push(TabUiAction::NewDeckTab);
            }
            MenuResult::Picked(_) | MenuResult::Closed => {
                app.tabs.new_menu_pos = None;
            }
            MenuResult::Open => {}
        }
    }

    // 5. Right-Click Tab Context Menu
    if let Some((menu_idx, pos)) = app.tabs.ctx_menu_tab {
        if menu_idx < app.tabs.entries.len() {
            let pinned = app.tabs.entries[menu_idx].pinned;
            let can_close_others = app.tabs.entries.len() > 1;
            let items = vec![
                MenuItem::new("New Design Tab").shortcut("Ctrl+T"),
                MenuItem::new("Duplicate Tab").sep(),
                MenuItem::new(if pinned { "Unpin Tab" } else { "Pin Tab" }),
                MenuItem::new("Close Tab").shortcut("Ctrl+W").disabled(pinned),
                MenuItem::new("Close Other Tabs").disabled(!can_close_others),
            ];
            match popup_menu(ui.ctx(), egui::Id::new("project-tab-ctx-menu"), pos, 184.0, &items, None) {
                MenuResult::Picked(0) => {
                    app.tabs.ctx_menu_tab = None;
                    actions.push(TabUiAction::NewDesignTab);
                }
                MenuResult::Picked(1) => {
                    app.tabs.ctx_menu_tab = None;
                    actions.push(TabUiAction::DuplicateTab(menu_idx));
                }
                MenuResult::Picked(2) => {
                    app.tabs.ctx_menu_tab = None;
                    actions.push(TabUiAction::TogglePin(menu_idx));
                }
                MenuResult::Picked(3) => {
                    app.tabs.ctx_menu_tab = None;
                    actions.push(TabUiAction::CloseTab(menu_idx));
                }
                MenuResult::Picked(4) => {
                    app.tabs.ctx_menu_tab = None;
                    actions.push(TabUiAction::CloseOtherTabs(menu_idx));
                }
                MenuResult::Picked(_) | MenuResult::Closed => {
                    app.tabs.ctx_menu_tab = None;
                }
                MenuResult::Open => {}
            }
        } else {
            app.tabs.ctx_menu_tab = None;
        }
    }

    if !actions.is_empty() {
        let mut tabs = std::mem::take(&mut app.tabs);
        for act in actions {
            match act {
                TabUiAction::ClickHome => tabs.enter_home(app),
                TabUiAction::SwitchTab(idx) => tabs.switch_to(idx, app),
                TabUiAction::CloseTab(idx) => {
                    tabs.close_tab(idx, app);
                }
                TabUiAction::DuplicateTab(idx) => {
                    tabs.duplicate_tab(idx, app);
                }
                TabUiAction::TogglePin(idx) => {
                    tabs.toggle_pin(idx);
                }
                TabUiAction::CloseOtherTabs(idx) => {
                    tabs.close_other_tabs(idx, app);
                }
                TabUiAction::NewDesignTab => {
                    tabs.open_new_design_tab(app);
                }
                TabUiAction::NewBoardTab => {
                    tabs.open_new_board_tab(app);
                }
                TabUiAction::NewDeckTab => {
                    tabs.open_new_deck_tab(app);
                }
            }
        }
        app.tabs = tabs;
    }
}

fn paint_home_glyph(p: &egui::Painter, center: Pos2, color: Color32) {
    icons::paint(p, Icon::Home, center, 14.0, color);
}

fn truncate_tab_title(p: &egui::Painter, text: &str, font: &egui::FontId, max_w: f32) -> String {
    if widgets::text_width(p, text, font) <= max_w {
        return text.to_string();
    }
    let mut s = String::new();
    for ch in text.chars() {
        let cand = format!("{s}{ch}\u{2026}");
        if widgets::text_width(p, &cand, font) > max_w {
            break;
        }
        s.push(ch);
    }
    if s.is_empty() {
        "\u{2026}".to_string()
    } else {
        format!("{s}\u{2026}")
    }
}

#[allow(dead_code)]
impl App {
    pub fn open_home(&mut self) {
        let mut tabs = std::mem::take(&mut self.tabs);
        tabs.enter_home(self);
        self.tabs = tabs;
    }

    pub fn switch_tab(&mut self, idx: usize) {
        let mut tabs = std::mem::take(&mut self.tabs);
        tabs.switch_to(idx, self);
        self.tabs = tabs;
    }

    pub fn open_new_tab(&mut self, title: &str, doc: Document, kind: ProjectTabKind) -> usize {
        let mut tabs = std::mem::take(&mut self.tabs);
        let idx = tabs.open_new_tab(self, title, doc, kind);
        self.tabs = tabs;
        idx
    }

    pub fn open_new_design_tab(&mut self) -> usize {
        let mut tabs = std::mem::take(&mut self.tabs);
        let idx = tabs.open_new_design_tab(self);
        self.tabs = tabs;
        idx
    }

    pub fn open_new_board_tab(&mut self) -> usize {
        let mut tabs = std::mem::take(&mut self.tabs);
        let idx = tabs.open_new_board_tab(self);
        self.tabs = tabs;
        idx
    }

    pub fn close_tab(&mut self, idx: usize) -> bool {
        let mut tabs = std::mem::take(&mut self.tabs);
        let ok = tabs.close_tab(idx, self);
        self.tabs = tabs;
        ok
    }

    pub fn duplicate_tab(&mut self, idx: usize) -> Option<usize> {
        let mut tabs = std::mem::take(&mut self.tabs);
        let res = tabs.duplicate_tab(idx, self);
        self.tabs = tabs;
        res
    }

    pub fn next_tab(&mut self) {
        let mut tabs = std::mem::take(&mut self.tabs);
        tabs.next_tab(self);
        self.tabs = tabs;
    }

    pub fn prev_tab(&mut self) {
        let mut tabs = std::mem::take(&mut self.tabs);
        tabs.prev_tab(self);
        self.tabs = tabs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as DRect, Vec2 as DVec2};

    fn make_app_with_doc(name: &str) -> App {
        let mut doc = Document::new(name);
        doc.add_page("Page 1");
        let mut app = App::new(doc);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        app
    }

    #[test]
    fn multi_tab_state_isolation_preserves_doc_selection_zoom_page_and_history() {
        let mut app = make_app_with_doc("Tab 1");
        let p1_b = app.doc.add_page("Page 2");
        app.page = p1_b;
        let r1_id = app.doc.alloc_id();
        let r1 = app.doc.insert(p1_b, Node::rectangle(r1_id, DRect::new(10.0, 20.0, 120.0, 80.0))).unwrap();
        // Record an undoable edit in Tab 1
        app.rename(r1, "HeroRectTab1");
        app.selection = vec![r1];
        app.view = View { offset: DVec2::new(42.0, 84.0), zoom: 2.25 };
        assert!(app.hist.can_undo(), "Tab 1 has undo history");

        // Open Tab 2 and Tab 3
        let mut doc2 = Document::new("Tab 2");
        let p2 = doc2.add_page("Board Page");
        let r2_id = doc2.alloc_id();
        let r2 = doc2.insert(p2, Node::rectangle(r2_id, DRect::new(0.0, 0.0, 60.0, 60.0))).unwrap();
        let idx2 = app.open_new_tab("Tab 2", doc2, ProjectTabKind::Board);
        assert_eq!(idx2, 1);
        assert_eq!(app.doc.name, "Tab 2");
        assert!(!app.hist.can_undo(), "Tab 2 starts with clean history");
        app.rename(r2, "BoardNodeTab2");
        app.selection = vec![r2];
        app.view = View { offset: DVec2::new(-120.0, 30.0), zoom: 0.75 };

        let doc3 = Document::new("Tab 3");
        let idx3 = app.open_new_tab("Tab 3", doc3, ProjectTabKind::Design);
        assert_eq!(idx3, 2);
        assert_eq!(app.tabs.entries.len(), 3);
        assert_eq!(app.doc.name, "Tab 3");
        assert!(app.selection.is_empty());

        // Switch back to Tab 1 and verify 100% state isolation
        app.switch_tab(0);
        assert_eq!(app.doc.name, "Tab 1");
        assert_eq!(app.doc.get(r1).unwrap().name, "HeroRectTab1");
        assert_eq!(app.page, p1_b);
        assert_eq!(app.selection, vec![r1]);
        assert!((app.view.zoom - 2.25).abs() < 1e-5);
        assert_eq!(app.view.offset, DVec2::new(42.0, 84.0));
        assert!(app.hist.can_undo(), "Tab 1 undo stack restored");

        // Switch to Tab 2 and verify its edits and view were preserved
        app.switch_tab(1);
        assert_eq!(app.doc.name, "Tab 2");
        assert_eq!(app.doc.get(r2).unwrap().name, "BoardNodeTab2");
        assert_eq!(app.selection, vec![r2]);
        assert!((app.view.zoom - 0.75).abs() < 1e-5);
        assert!(app.hist.can_undo(), "Tab 2 undo stack restored");
    }

    #[test]
    fn switching_to_home_and_back_preserves_open_tabs() {
        let mut app = make_app_with_doc("Design Alpha");
        app.open_new_board_tab();
        assert_eq!(app.tabs.entries.len(), 2);
        assert!(app.home.is_none());

        // Enter Home without closing any open project tabs
        app.open_home();
        assert!(app.home.is_some());
        assert_eq!(app.tabs.entries.len(), 2, "both project tabs remain open while on Home");

        // Switch from Home back into Tab 0
        app.switch_tab(0);
        assert!(app.home.is_none(), "switching to an open tab exits Home");
        assert_eq!(app.doc.name, "Design Alpha");
        assert_eq!(app.tabs.entries.len(), 2);
    }

    #[test]
    fn duplicate_pin_unpin_and_close_tab_lifecycle() {
        let mut app = make_app_with_doc("Main System");
        let dup_idx = app.duplicate_tab(0).expect("duplicated tab");
        assert_eq!(dup_idx, 1);
        assert_eq!(app.tabs.entries.len(), 2);
        assert_eq!(app.doc.name, "Main System (Copy)");

        // Pin Tab 0 and verify it cannot be closed while pinned
        app.tabs.toggle_pin(0);
        assert!(app.tabs.entries[0].pinned);
        assert!(!app.close_tab(0), "pinned tab refuses close");
        assert_eq!(app.tabs.entries.len(), 2);

        // Close Tab 1 (unpinned) -> activates Tab 0
        assert!(app.close_tab(1));
        assert_eq!(app.tabs.entries.len(), 1);
        assert_eq!(app.tabs.active_idx, 0);
        assert_eq!(app.doc.name, "Main System");

        // Unpin Tab 0 and close it -> opens Home when last tab closes
        app.tabs.toggle_pin(0);
        assert!(!app.tabs.entries[0].pinned);
        assert!(app.close_tab(0));
        assert!(app.tabs.entries.is_empty());
        assert!(app.home.is_some(), "closing the last tab returns to Home");
    }

    #[test]
    fn closing_background_tab_and_per_tab_terminal_isolation() {
        let mut app = make_app_with_doc("File One");
        app.terminal.input_line = "claude --file-one".to_string();

        let doc2 = Document::new("File Two");
        let idx2 = app.open_new_tab("File Two", doc2, ProjectTabKind::Design);
        assert_eq!(idx2, 1);
        assert_eq!(app.terminal.input_line, "", "New file tab gets its own clean TerminalState");
        app.terminal.input_line = "codex --file-two".to_string();

        // Switch back to File One and verify its terminal state was isolated & preserved
        app.switch_tab(0);
        assert_eq!(app.doc.name, "File One");
        assert_eq!(app.terminal.input_line, "claude --file-one");

        // Switch to File Two and verify its terminal state was isolated & preserved
        app.switch_tab(1);
        assert_eq!(app.doc.name, "File Two");
        assert_eq!(app.terminal.input_line, "codex --file-two");

        // Close background tab 0 (File One) while editing active tab 1 (File Two)
        assert!(app.close_tab(0));
        assert_eq!(app.tabs.entries.len(), 1);
        assert_eq!(app.tabs.active_idx, 0);
        assert_eq!(app.doc.name, "File Two");
        assert_eq!(app.terminal.input_line, "codex --file-two");
    }

    #[test]
    fn closing_repaired_tab_without_saving_drops_repaired_original_bytes() {
        let mut damaged1 = Document::new("DamagedOne");
        let p1 = damaged1.add_page("P1");
        damaged1.nodes.get_mut(&p1).unwrap().children.push(designos_doc::NodeId(9999));

        let mut damaged2 = Document::new("DamagedTwo");
        let p2 = damaged2.add_page("P2");
        damaged2.nodes.get_mut(&p2).unwrap().children.push(designos_doc::NodeId(8888));

        let mut path1 = std::env::temp_dir();
        path1.push(format!("designos_tab_close_repaired_1_{}.designos", std::process::id()));
        let mut path2 = std::env::temp_dir();
        path2.push(format!("designos_tab_close_repaired_2_{}.designos", std::process::id()));

        std::fs::write(&path1, damaged1.to_json().unwrap()).unwrap();
        std::fs::write(&path2, damaged2.to_json().unwrap()).unwrap();

        let mut app = make_app_with_doc("Initial");
        app.open_path(path1.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        let idx2 = tabs.open_or_focus_path(&mut app, &path2).expect("opened second repaired tab");
        app.tabs = tabs;
        assert_eq!(idx2, 1);

        assert!(crate::io::has_repaired_original(&path1), "tab 0 holds repaired original bytes");
        assert!(crate::io::has_repaired_original(&path2), "tab 1 holds repaired original bytes");

        // Closing background tab 0 without saving drops its repaired original bytes immediately
        assert!(app.close_tab(0));
        assert!(!crate::io::has_repaired_original(&path1), "closed tab 0 must drop its repaired original bytes");
        assert!(crate::io::has_repaired_original(&path2), "remaining tab 1 keeps its repaired original bytes");

        // Closing the active last tab without saving drops its repaired original bytes too
        assert!(app.close_tab(0));
        assert!(!crate::io::has_repaired_original(&path2), "closed active tab must drop its repaired original bytes");

        // Re-open both repaired files and verify close_other_tabs and new_document also drop repaired bytes
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.open_or_focus_path(&mut app, &path1).expect("re-opened first repaired tab");
        tabs.open_or_focus_path(&mut app, &path2).expect("re-opened second repaired tab");
        assert!(crate::io::has_repaired_original(&path1));
        assert!(crate::io::has_repaired_original(&path2));
        tabs.close_other_tabs(1, &mut app);
        app.tabs = tabs;
        assert!(!crate::io::has_repaired_original(&path1), "close_other_tabs drops closed tab's repaired bytes");
        assert!(crate::io::has_repaired_original(&path2), "close_other_tabs preserves kept tab's repaired bytes");

        app.new_document();
        assert!(!crate::io::has_repaired_original(&path2), "replacing active document without saving drops its repaired bytes");

        std::fs::remove_file(&path1).ok();
        std::fs::remove_file(&path2).ok();
    }

    /// A library with one design file whose page holds a rectangle; returns the library, the file id and the rectangle.
    fn library_with_one_file(tag: &str) -> (designos_library::Library, std::path::PathBuf, String, NodeId) {
        let dir = std::env::temp_dir().join(format!("designos_tab_close_lib_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let lib = designos_library::Library::open(&dir).unwrap();
        let project = lib.create_project("Project").unwrap();
        let mut doc = Document::new("Doc");
        let page = doc.add_page("Page 1");
        let rect_id = doc.alloc_id();
        let rect = doc.insert(page, Node::rectangle(rect_id, DRect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let entry = lib.create_file(&project.id, None, "Doc", &doc).unwrap();
        (lib, dir, entry.id, rect)
    }

    /// Renames the rectangle and checks that the edit is not on disk yet (no autosave frame has run).
    fn edit_unsaved(app: &mut App, lib: &designos_library::Library, file_id: &str, rect: NodeId) {
        app.rename(rect, "Edited");
        assert_ne!(crate::home::doc_hash(&app.doc), app.autosave.saved_hash, "the edit is unsaved");
        assert!(!lib.load_file(file_id).unwrap().nodes.values().any(|n| n.name == "Edited"), "the edit is not on disk yet");
    }

    fn disk_has_edit(lib: &designos_library::Library, file_id: &str) -> bool {
        lib.load_file(file_id).unwrap().nodes.values().any(|n| n.name == "Edited")
    }

    #[test]
    fn closing_the_last_library_tab_saves_the_edit_and_a_closing_version() {
        let (lib, dir, file_id, rect) = library_with_one_file("last");
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert_eq!(app.tabs.entries.len(), 1);
        edit_unsaved(&mut app, &lib, &file_id, rect);
        assert!(lib.list_versions(&file_id).is_empty());

        assert!(app.close_tab(0));

        assert!(app.home.is_some(), "closing the last tab returns to Home");
        assert!(app.lib_file.is_none() && app.file_path.is_none(), "no closed file stays attached to the app");
        assert!(disk_has_edit(&lib, &file_id), "the edit made before the close is saved to the library file");
        let versions = lib.list_versions(&file_id);
        assert_eq!(versions.len(), 1, "the close stores an automatic version");
        assert!(versions[0].auto);
        assert!(lib.load_version(&file_id, versions[0].n).unwrap().nodes.values().any(|n| n.name == "Edited"));

        // Reopening from the library shows the edit.
        app.open_library_file(&file_id);
        assert!(app.doc.nodes.values().any(|n| n.name == "Edited"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two library files, each with one rectangle; returns the library, its directory and both files (id, rectangle).
    fn library_with_two_files(tag: &str) -> (designos_library::Library, std::path::PathBuf, (String, NodeId), (String, NodeId)) {
        let (lib, dir, id_a, rect_a) = library_with_one_file(tag);
        let project = lib.list_projects().remove(0);
        let mut doc = Document::new("Other");
        let page = doc.add_page("Page 1");
        let rect_id = doc.alloc_id();
        let rect_b = doc.insert(page, Node::rectangle(rect_id, DRect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let entry = lib.create_file(&project.id, None, "Other", &doc).unwrap();
        (lib, dir, (id_a, rect_a), (entry.id, rect_b))
    }

    /// One `reconcile_frame`, as the shell runs every frame.
    fn reconcile(app: &mut App) {
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(app);
        app.tabs = tabs;
    }

    /// The production route to two open library tabs, file B active and unedited: Home, open A from Home,
    /// the tab-bar Home click, open B from Home. Opening goes through `App::open_library_file` (what the
    /// Home screen calls) and the tab entries are created by `reconcile_frame`, never by `open_or_focus_lib_file`.
    fn two_library_tabs_opened_from_home(tag: &str) -> (App, designos_library::Library, std::path::PathBuf, (String, NodeId), (String, NodeId)) {
        let (lib, dir, a, b) = library_with_two_files(tag);
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        app.go_home();
        reconcile(&mut app);
        app.open_library_file(&a.0);
        reconcile(&mut app);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.enter_home(&mut app);
        app.tabs = tabs;
        reconcile(&mut app);
        app.open_library_file(&b.0);
        reconcile(&mut app);
        assert_eq!(app.tabs.entries.len(), 2, "both files are open in tabs");
        assert_eq!(app.tabs.active_idx, 1);
        assert_eq!(app.lib_file.as_ref().map(|f| f.id.as_str()), Some(b.0.as_str()));
        (app, lib, dir, a, b)
    }

    /// The file keeps its content and none of its stored versions is empty.
    fn assert_library_file_and_versions_not_emptied(lib: &designos_library::Library, file_id: &str, what: &str) {
        assert!(lib.load_file(file_id).unwrap().nodes.len() >= 2, "{what}: the file on disk was emptied");
        for v in lib.list_versions(file_id) {
            assert!(lib.load_version(file_id, v.n).unwrap().nodes.len() >= 2, "{what}: stored version {} is empty", v.n);
        }
    }

    /// Edits the active tab (file B) without letting autosave run, leaves it with `leave`, and checks the disk.
    fn leaving_the_edited_tab_saves_it(name: &str, leave: fn(&mut TabsState, &mut App)) {
        let (mut app, lib, dir, _a, b) = two_library_tabs_opened_from_home(&format!("park_{name}"));
        edit_unsaved(&mut app, &lib, &b.0, b.1);

        let mut tabs = std::mem::take(&mut app.tabs);
        leave(&mut tabs, &mut app);
        app.tabs = tabs;

        assert!(disk_has_edit(&lib, &b.0), "{name}: the edit made before leaving the tab is on disk");
        assert_library_file_and_versions_not_emptied(&lib, &b.0, name);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_tabs_keeps_the_unsaved_edit_of_the_tab_left() {
        leaving_the_edited_tab_saves_it("switch_to", |t, a| t.switch_to(0, a));
    }

    #[test]
    fn clicking_home_keeps_the_unsaved_edit_of_the_tab_left() {
        leaving_the_edited_tab_saves_it("enter_home", |t, a| t.enter_home(a));
    }

    #[test]
    fn closing_other_tabs_keeps_the_unsaved_edit_of_the_tab_left() {
        leaving_the_edited_tab_saves_it("close_other_tabs", |t, a| t.close_other_tabs(0, a));
    }

    #[test]
    fn clicking_home_stores_the_closing_version_with_the_edit() {
        let (mut app, lib, dir, _a, b) = two_library_tabs_opened_from_home("park_home_version");
        edit_unsaved(&mut app, &lib, &b.0, b.1);

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.enter_home(&mut app);
        app.tabs = tabs;

        let versions = lib.list_versions(&b.0);
        assert_eq!(versions.len(), 1, "leaving the file stores one automatic version");
        assert!(versions[0].auto);
        assert!(lib.load_version(&b.0, versions[0].n).unwrap().nodes.values().any(|n| n.name == "Edited"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_tabs_from_home_reached_by_back_to_files_does_not_empty_the_file() {
        let (mut app, lib, dir, _a, b) = two_library_tabs_opened_from_home("park_back_to_files");
        edit_unsaved(&mut app, &lib, &b.0, b.1);
        app.go_home();
        assert!(disk_has_edit(&lib, &b.0), "Back to files saves the edit");

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;

        assert!(disk_has_edit(&lib, &b.0));
        assert_library_file_and_versions_not_emptied(&lib, &b.0, "switch_to after Back to files");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clicking_home_on_a_library_tab_without_edits_does_not_empty_its_file() {
        let (lib, dir, a, _b) = library_with_two_files("park_clean_home");
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        app.go_home();
        reconcile(&mut app);
        app.open_library_file(&a.0);
        reconcile(&mut app);
        assert_eq!(app.tabs.entries.len(), 1);

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.enter_home(&mut app);
        app.tabs = tabs;

        assert_library_file_and_versions_not_emptied(&lib, &a.0, "enter_home without an edit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_the_last_tab_of_a_repaired_library_file_saves_the_edit_and_keeps_the_backup() {
        let (lib, dir, file_id, _rect) = library_with_one_file("repaired_last");
        let entry = lib.file(&file_id).unwrap();
        let disk_path = lib.path("projects").join(&entry.project).join("files").join(format!("{file_id}.designos"));
        let mut damaged = Document::new("Damaged");
        let page = damaged.add_page("P");
        damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(9999));
        let damaged_bytes = damaged.to_json().unwrap().into_bytes();
        std::fs::write(&disk_path, &damaged_bytes).unwrap();

        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert!(crate::io::has_repaired_original(&disk_path), "the repaired library file holds its original bytes");
        let page = app.page;
        app.rename(page, "Edited");
        let backup = crate::io::backup_path(&disk_path);

        assert!(app.close_tab(0));

        assert!(disk_has_edit(&lib, &file_id), "the edit is saved when the last tab closes");
        assert_eq!(std::fs::read(&backup).unwrap(), damaged_bytes, "the pre-repair bytes are backed up by that save");
        assert!(!crate::io::has_repaired_original(&disk_path), "a successful save consumes the pending backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two damaged library files open in two tabs: tab 0 (active) holds the second file, tab 1 the first.
    /// Built by pushing a tab entry instead of parking a document. Returns the app, the library root and
    /// the disk paths that key each file's cached pre-repair bytes.
    fn app_with_two_repaired_library_tabs(tag: &str) -> (App, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let (lib, dir, file_id, _rect) = library_with_one_file(tag);
        let project = lib.file(&file_id).unwrap().project;
        let other_id = lib.create_file(&project, None, "Other", &Document::new("Other")).unwrap().id;
        let disk_path_of = |id: &str| lib.path("projects").join(&project).join("files").join(format!("{id}.designos"));
        let (path_a, path_b) = (disk_path_of(&file_id), disk_path_of(&other_id));
        for path in [&path_a, &path_b] {
            let mut damaged = Document::new("Damaged");
            let page = damaged.add_page("P");
            damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(9999));
            std::fs::write(path, damaged.to_json().unwrap()).unwrap();
        }
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib);
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        let id = tabs.alloc_id();
        tabs.entries.push(ProjectTab::from_app(id, &app, None));
        app.tabs = tabs;
        app.open_library_file(&other_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert_eq!(app.tabs.entries.len(), 2);
        assert!(crate::io::has_repaired_original(&path_a) && crate::io::has_repaired_original(&path_b));
        (app, dir, path_a, path_b)
    }

    #[test]
    fn closing_a_repaired_library_tab_without_saving_drops_its_repaired_original_bytes() {
        let (mut app, dir, path_a, path_b) = app_with_two_repaired_library_tabs("repaired_unsaved");

        // A background tab, then the active one, closed without an edit: nothing is saved, so nothing may linger.
        assert!(app.close_tab(1));
        assert!(!crate::io::has_repaired_original(&path_a), "the closed background tab drops its repaired bytes");
        assert!(crate::io::has_repaired_original(&path_b), "the open tab keeps its repaired bytes");
        assert!(app.close_tab(0));
        assert!(!crate::io::has_repaired_original(&path_b), "the closed active tab drops its repaired bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_other_tabs_drops_the_repaired_original_bytes_of_closed_library_tabs() {
        let (mut app, dir, path_a, path_b) = app_with_two_repaired_library_tabs("repaired_others");
        // With Home on screen no document is parked, which keeps this to the eviction.
        app.home = Some(crate::home::Home::new());

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.close_other_tabs(0, &mut app);
        app.tabs = tabs;

        assert_eq!(app.tabs.entries.len(), 1);
        assert!(!crate::io::has_repaired_original(&path_a), "the closed tab drops its repaired bytes");
        assert!(crate::io::has_repaired_original(&path_b), "the kept tab keeps its repaired bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_the_active_library_tab_with_other_tabs_open_saves_its_edit() {
        let (lib, dir, file_id, rect) = library_with_one_file("active");
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        tabs.open_or_focus_lib_file(&mut app, &file_id).expect("library file opened in a second tab");
        app.tabs = tabs;
        assert_eq!(app.tabs.entries.len(), 2);
        assert_eq!(app.tabs.active_idx, 1);
        edit_unsaved(&mut app, &lib, &file_id, rect);

        assert!(app.close_tab(1));

        assert_eq!(app.tabs.entries.len(), 1);
        assert!(disk_has_edit(&lib, &file_id), "closing the active tab saves its edit before the next tab replaces it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moving_a_repaired_file_into_the_library_drops_its_repaired_original_bytes() {
        let (lib, dir, _file_id, _rect) = library_with_one_file("moved");
        let project = lib.list_projects()[0].id.clone();
        let mut damaged = Document::new("Damaged");
        let page = damaged.add_page("P");
        damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(9999));
        let path = std::env::temp_dir().join(format!("designos_tab_moved_repaired_{}.designos", std::process::id()));
        std::fs::write(&path, damaged.to_json().unwrap()).unwrap();
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib);
        app.open_path(path.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert!(crate::io::has_repaired_original(&path), "the opened file holds repaired original bytes");

        app.save_to_library_as(&project, None, "Copy");

        assert!(app.file_path.is_none() && app.lib_file.is_some());
        assert!(!crate::io::has_repaired_original(&path), "the document no longer lives at the old path, so its repaired bytes are dropped");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_studio_instances_have_distinct_agent_session_directories() {
        let mut app_a = App::new(Document::new("Window A"));
        let mut tabs_a = std::mem::take(&mut app_a.tabs);
        tabs_a.reconcile_frame(&mut app_a);
        app_a.tabs = tabs_a;

        let mut app_b = App::new(Document::new("Window B"));
        let mut tabs_b = std::mem::take(&mut app_b.tabs);
        tabs_b.reconcile_frame(&mut app_b);
        app_b.tabs = tabs_b;

        assert_eq!(app_a.tabs.entries[0].id, 1);
        assert_eq!(app_b.tabs.entries[0].id, 1);
        assert_ne!(app_a.terminal.session_dir, app_b.terminal.session_dir, "two Studio instances must not share the same agent session directory for tab 1");
        let pid_tag = format!("p{}-", std::process::id());
        assert!(
            app_a.terminal.session_id.starts_with(&pid_tag) && app_b.terminal.session_id.starts_with(&pid_tag),
            "session ids should include the process id: {} vs {}",
            app_a.terminal.session_id,
            app_b.terminal.session_id
        );
        let _ = std::fs::remove_dir_all(&app_a.terminal.session_dir);
        let _ = std::fs::remove_dir_all(&app_b.terminal.session_dir);
    }

    /// The terminal is bound to `active_session_id` before a shell can start in it (`Shell::build`);
    /// that must be the id the first reconcile binds, or the shell keeps writing to a directory
    /// Studio no longer watches.
    #[test]
    fn the_session_bound_before_a_shell_starts_is_the_one_the_first_reconcile_keeps() {
        let mut app = App::new(Document::new("Preset"));
        let bound = app.tabs.active_session_id();
        app.terminal.bind_session(&bound);
        app.terminal.ensure_default_tab(None);
        assert_eq!(app.terminal.tabs.len(), 1, "precondition: the shell started");
        let started_with = app.terminal.session_env();

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;

        assert_eq!(app.terminal.session_id, started_with.session_id, "the first frame moved Studio off the session the shell was started with");
        assert_eq!(app.terminal.session_dir, started_with.session_dir);
        assert_eq!(started_with.session_id, bound);
        assert_eq!(bound, app.tabs.session_id_for_tab(app.tabs.entries[0].id));
        app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&app.terminal.session_dir);
    }

    #[test]
    fn active_session_id_follows_the_active_tab() {
        let mut app = make_app_with_doc("Tab 1");
        let first = app.tabs.entries[0].id;
        assert_eq!(app.tabs.active_session_id(), app.tabs.session_id_for_tab(first));
        app.open_new_tab("Tab 2", Document::new("Tab 2"), ProjectTabKind::Design);
        let second = app.tabs.entries[app.tabs.active_idx].id;
        assert_ne!(first, second);
        assert_eq!(app.tabs.active_session_id(), app.tabs.session_id_for_tab(second));
    }

    #[test]
    fn replacing_a_repaired_library_file_in_place_drops_its_repaired_original_bytes() {
        let (lib, dir, file_id, _rect) = library_with_one_file("replace_in_place");
        let project = lib.file(&file_id).unwrap().project;
        let disk_path = lib.path("projects").join(&project).join("files").join(format!("{file_id}.designos"));
        let mut damaged = Document::new("Damaged");
        let page = damaged.add_page("P");
        damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(9999));
        std::fs::write(&disk_path, damaged.to_json().unwrap()).unwrap();

        let ext_path = dir.join("external_other.designos");
        let mut ext_doc = Document::new("External Other");
        ext_doc.add_page("P");
        std::fs::write(&ext_path, ext_doc.to_json().unwrap()).unwrap();

        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib);

        // 1. Open damaged library file without editing, then replace in-place via `new_file_here`.
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert!(crate::io::has_repaired_original(&disk_path), "opening damaged library file caches pre-repair bytes");
        app.new_file_here();
        assert!(!crate::io::has_repaired_original(&disk_path), "new_file_here must drop the replaced library file's cached pre-repair bytes");

        // 2. Re-open damaged library file without editing, then replace in-place via `open_path`.
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert!(crate::io::has_repaired_original(&disk_path), "re-opening damaged library file caches pre-repair bytes");
        app.open_path(ext_path);
        assert!(!crate::io::has_repaired_original(&disk_path), "open_path must drop the replaced library file's cached pre-repair bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn library_tab_and_external_tab_on_same_disk_path_share_still_open_check() {
        let (lib, dir, file_id, _rect) = library_with_one_file("shared_repair_key");
        let project = lib.file(&file_id).unwrap().project;
        let disk_path = lib.path("projects").join(&project).join("files").join(format!("{file_id}.designos"));
        let mut damaged = Document::new("Damaged");
        let page = damaged.add_page("P");
        damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(9999));
        let damaged_bytes = damaged.to_json().unwrap().into_bytes();
        let backup = crate::io::backup_path(&disk_path);

        // Case A: close the library tab (tab 0), then save from the external tab (tab 1).
        std::fs::write(&disk_path, &damaged_bytes).unwrap();
        let _ = std::fs::remove_file(&backup);
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib.clone());
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        tabs.open_new_design_tab(&mut app);
        app.tabs = tabs;
        app.open_path(disk_path.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert_eq!(app.tabs.entries.len(), 2);
        assert!(crate::io::has_repaired_original(&disk_path));

        assert!(app.close_tab(0), "closing library tab 0 while external tab 1 holds the same file");
        assert!(crate::io::has_repaired_original(&disk_path), "external tab still holds the file, so pre-repair bytes must stay cached");
        let page = app.page;
        app.rename(page, "Edited from external tab");
        app.save(false);
        assert_eq!(std::fs::read(&backup).unwrap(), damaged_bytes, "saving the remaining external tab writes the pre-repair .bak");

        // Case B: close the external tab (tab 1), then save from the library tab (tab 0).
        std::fs::write(&disk_path, &damaged_bytes).unwrap();
        let _ = std::fs::remove_file(&backup);
        let mut app = App::new(Document::new("Scratch"));
        app.lib = Some(lib);
        app.open_library_file(&file_id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        tabs.open_new_design_tab(&mut app);
        app.tabs = tabs;
        app.open_path(disk_path.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        assert_eq!(app.tabs.entries.len(), 2);
        assert!(crate::io::has_repaired_original(&disk_path));

        assert!(app.close_tab(1), "closing external tab 1 while library tab 0 holds the same file");
        assert!(crate::io::has_repaired_original(&disk_path), "library tab still holds the file, so pre-repair bytes must stay cached");
        let page = app.page;
        app.rename(page, "Edited from library tab");
        app.save(false);
        assert_eq!(std::fs::read(&backup).unwrap(), damaged_bytes, "saving the remaining library tab writes the pre-repair .bak");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
