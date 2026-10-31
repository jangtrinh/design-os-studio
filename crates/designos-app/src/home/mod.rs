//! The projects library UI: the 1:1 Figma UI3 Home screen (file browser) shown at launch and
//! from File -> "Back to files", the dialogs it shares with the editor, and the
//! glue that opens library files in the editor.
//!
//! The data lives in `designos-library`; this module draws the Figma UI3 Home interface (`S04`)
//! and turns user interactions into [`Act`]s that [`apply`] runs against the library.

pub mod dialog;
mod leave;
mod session;
#[cfg(test)]
mod spec_bundle_hidden_tests;
pub mod thumb;
pub mod time;
mod unsaved;
#[path = "../updater.rs"]
pub mod updater;

pub use dialog::{Dialog, Intent};
#[cfg(test)]
pub use leave::Answer;
#[cfg(test)]
pub use leave::LeaveFrom;
pub use leave::{Leave, LeaveFlow};
#[cfg(test)]
pub(crate) use main_view::{fit_breadcrumb, Crumb};
pub(crate) use session::library_file_disk_path;
pub(crate) use session::load_starter_document;
pub(crate) use session::LibrarySaveError;
pub(crate) use session::{doc_hash, node_fingerprint};
pub use session::{Autosave, DiskState, LibFile};
pub use unsaved::UnsavedDot;

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_STARTER_BYTES_OVERRIDE: std::cell::Cell<Option<&'static [u8]>> = const { std::cell::Cell::new(None) };
}

use crate::app::App;
use crate::theme;
use designos_library::{FileEntry, Library, Project, TrashItem};
use egui::{Pos2, Rect, Ui, Vec2};
use std::collections::HashSet;

pub const GITHUB_URL: &str = "https://github.com/jangtrinh/design-os-studio";

/// Navigation sections in the 1:1 Figma UI3 Home sidebar (`US-04` / `S04`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Recents,
    Drafts,
    AllProjects,
    Templates,
    Resources,
    Starred,
    /// A project, optionally narrowed to one folder.
    Project(String, Option<String>),
    Trash,
}

/// Alias for callers/tests referencing `HomeSection`.
#[allow(dead_code)]
pub type HomeSection = Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// Alias for callers/tests referencing `HomeViewMode`.
#[allow(dead_code)]
pub type HomeViewMode = ViewMode;

/// File-type filter chip in the Figma UI3 Home sub-header (`Filter: All files v`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FileFilter {
    #[default]
    AllFiles,
    DesignFiles,
    Boards,
    SpecBundles,
}

impl FileFilter {
    pub const ALL: [FileFilter; 4] = [FileFilter::AllFiles, FileFilter::DesignFiles, FileFilter::Boards, FileFilter::SpecBundles];

    pub fn label(self) -> &'static str {
        match self {
            FileFilter::AllFiles => "All files",
            FileFilter::DesignFiles => "Design files",
            FileFilter::Boards => "FigJam / Boards",
            FileFilter::SpecBundles => "Spec bundles",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    LastOpened,
    LastModified,
    Name,
    DateCreated,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::LastOpened, Sort::LastModified, Sort::Name, Sort::DateCreated];

    pub fn label(self) -> &'static str {
        match self {
            Sort::LastOpened => "Last viewed",
            Sort::LastModified => "Last modified",
            Sort::Name => "Alphabetical",
            Sort::DateCreated => "Date created",
        }
    }
}

/// What a context menu was opened on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    File(String),
    Project(String),
    Folder(String, String),
    TrashItem(String),
}

/// Where a dragged file can be dropped.
#[derive(Clone, Debug, PartialEq)]
pub enum DropTo {
    Project(String),
    Folder(String, String),
}

/// Alias for callers/tests referencing `RecentEntry`.
#[allow(dead_code)]
pub type RecentEntry = FileEntry;

/// Everything the home screen shows, read from disk when something changes.
#[derive(Default)]
pub struct Snapshot {
    pub projects: Vec<Project>,
    pub files: Vec<FileEntry>,
    pub trash: Vec<TrashItem>,
}

impl Snapshot {
    pub fn project_name(&self, id: &str) -> &str {
        self.projects.iter().find(|p| p.id == id).map(|p| p.name.as_str()).unwrap_or("")
    }
}

pub struct Home {
    pub section: Section,
    pub view: ViewMode,
    pub filter: FileFilter,
    pub sort: Sort,
    pub search: String,
    pub snap: Snapshot,
    /// The snapshot is stale; reload it before drawing.
    pub dirty: bool,
    /// Projects whose folders are collapsed in the sidebar.
    pub collapsed: HashSet<String>,
    /// Whether the `Starred` accordion in the left sidebar is expanded.
    pub starred_expanded: bool,
    pub selected: Option<String>,
    pub menu: Option<(Target, Pos2)>,
    pub sort_menu: Option<Rect>,
    pub filter_menu: Option<Rect>,
    /// File being dragged: (id, name).
    pub drag: Option<(String, String)>,
    /// Drop zones drawn this frame.
    pub drop_rects: Vec<(Rect, DropTo)>,
    pub scroll: f32,
    pub side_scroll: f32,
    pub thumbs: thumb::Textures,
    /// Design file or board, per file.
    pub kinds: thumb::Kinds,
    /// Optional direct document kind filter (`Design` or `Board`).
    pub kind_filter: Option<designos_doc::DocumentKind>,
    /// Whether alphabetical (`A–Z`) sorting is explicitly active.
    pub sort_alpha: bool,
    /// Dual-Layer Auto-Update & Upstream Figma Watchdog state (`US-09`).
    pub updater: updater::UpdaterState,
    /// Test-only (`#1126` §3 rules 6–7): fixed wall-clock milliseconds for deterministic Home views.
    #[cfg(test)]
    pub fixed_now: Option<u64>,
}

/// Alias for callers/tests referencing `HomeState`.
#[allow(dead_code)]
pub type HomeState = Home;

impl Default for Home {
    fn default() -> Self {
        Self::new()
    }
}

impl Home {
    pub fn new() -> Self {
        Self {
            section: Section::Recents,
            view: ViewMode::Grid,
            filter: FileFilter::AllFiles,
            sort: Sort::LastOpened,
            search: String::new(),
            snap: Snapshot::default(),
            dirty: true,
            collapsed: HashSet::new(),
            starred_expanded: true,
            selected: None,
            menu: None,
            sort_menu: None,
            filter_menu: None,
            drag: None,
            drop_rects: vec![],
            scroll: 0.0,
            side_scroll: 0.0,
            thumbs: Default::default(),
            kinds: Default::default(),
            kind_filter: None,
            sort_alpha: false,
            updater: updater::UpdaterState::new(),
            #[cfg(test)]
            fixed_now: None,
        }
    }

    pub fn now_ms(&self) -> u64 {
        #[cfg(test)]
        if let Some(t) = self.fixed_now {
            return t;
        }
        time::wall_ms()
    }

    pub fn refresh(&mut self, lib: &Library) {
        let projects = lib.list_projects();
        let files = projects.iter().flat_map(|p| lib.files_in(&p.id, None).unwrap_or_default()).collect();
        self.snap = Snapshot { projects, files, trash: lib.list_trash() };
        self.dirty = false;
        // A section pointing at something that no longer exists falls back to Recents.
        if let Section::Project(p, f) = &self.section {
            let gone = match self.snap.projects.iter().find(|x| &x.id == p) {
                None => true,
                Some(pr) => f.as_ref().is_some_and(|f| !pr.folders.iter().any(|x| &x.id == f)),
            };
            if gone {
                self.section = Section::Recents;
            }
        }
    }

    pub fn searching(&self) -> bool {
        !self.search.trim().is_empty()
    }

    /// Files of the current section (or search), filtered by `self.filter` / `self.kind_filter` and sorted by `self.sort`.
    pub fn files(&self) -> Vec<FileEntry> {
        self.visible_files()
    }

    /// Returns the filtered and sorted file entries visible in the current Home view.
    pub fn visible_files(&self) -> Vec<FileEntry> {
        let q = self.search.trim().to_lowercase();
        let mut v: Vec<FileEntry> = if !q.is_empty() {
            self.snap.files.iter().filter(|f| f.name.to_lowercase().contains(&q)).cloned().collect()
        } else {
            match &self.section {
                Section::Recents => self.snap.files.clone(),
                Section::AllProjects => self.snap.files.iter().filter(|f| f.folder.is_some()).cloned().collect(),
                Section::Templates => self
                    .snap
                    .files
                    .iter()
                    .filter(|f| {
                        let low = f.name.to_ascii_lowercase();
                        low.contains("template") || low.contains("spec") || low.contains("kit") || low.contains("starter") || low.contains("bundle")
                    })
                    .cloned()
                    .collect(),
                Section::Resources => self
                    .snap
                    .files
                    .iter()
                    .filter(|f| {
                        let low = f.name.to_ascii_lowercase();
                        low.contains("token")
                            || low.contains("system")
                            || low.contains("library")
                            || low.contains("component")
                            || low.contains("resource")
                            || low.contains("icon")
                            || low.contains("ui")
                    })
                    .cloned()
                    .collect(),
                Section::Drafts => self.snap.files.iter().filter(|f| f.folder.is_none()).cloned().collect(),
                Section::Starred => self.snap.files.iter().filter(|f| f.starred).cloned().collect(),
                Section::Project(p, folder) => self.snap.files.iter().filter(|f| &f.project == p && &f.folder == folder).cloned().collect(),
                Section::Trash => vec![],
            }
        };

        // Apply FileFilter
        match self.filter {
            FileFilter::AllFiles => {}
            FileFilter::DesignFiles => {
                v.retain(|f| {
                    let low = f.name.to_ascii_lowercase();
                    !low.contains("board") && !low.contains("spec")
                });
            }
            FileFilter::Boards => {
                v.retain(|f| {
                    let low = f.name.to_ascii_lowercase();
                    low.contains("board") || low.contains("retro") || low.contains("brainstorm") || low.contains("figjam")
                });
            }
            FileFilter::SpecBundles => {
                v.retain(|f| {
                    let low = f.name.to_ascii_lowercase();
                    low.contains("spec") || low.contains("v2") || low.contains("template")
                });
            }
        }

        // Apply optional DocumentKind filter
        if let Some(kind) = self.kind_filter {
            match kind {
                designos_doc::DocumentKind::Board => {
                    v.retain(|f| {
                        let low = f.name.to_ascii_lowercase();
                        low.contains("board") || low.contains("retro") || low.contains("brainstorm") || low.contains("figjam")
                    });
                }
                designos_doc::DocumentKind::Design => {
                    v.retain(|f| {
                        let low = f.name.to_ascii_lowercase();
                        !low.contains("board") && !low.contains("spec")
                    });
                }
                designos_doc::DocumentKind::Deck => {
                    v.retain(|f| {
                        let low = f.name.to_ascii_lowercase();
                        low.contains("deck") || low.contains("slide") || low.contains("pitch")
                    });
                }
            }
        }

        if self.sort_alpha {
            v.sort_by_key(|f| f.name.to_lowercase());
        } else {
            match self.sort {
                Sort::LastOpened => v.sort_by_key(|f| std::cmp::Reverse(f.opened)),
                Sort::LastModified => v.sort_by_key(|f| std::cmp::Reverse(f.modified)),
                Sort::Name => v.sort_by_key(|f| f.name.to_lowercase()),
                Sort::DateCreated => v.sort_by_key(|f| f.modified),
            }
        }
        v
    }

    /// Sub-folders shown as tiles in a project view or `All folders` view.
    pub fn folders(&self) -> Vec<(String, String)> {
        if self.searching() {
            return vec![];
        }
        match &self.section {
            Section::Project(p, parent) => {
                let Some(pr) = self.snap.projects.iter().find(|x| &x.id == p) else {
                    return vec![];
                };
                let mut v: Vec<(String, String)> = pr.folders.iter().filter(|f| &f.parent == parent).map(|f| (f.id.clone(), f.name.clone())).collect();
                v.sort_by_key(|(_, n)| n.to_lowercase());
                v
            }
            Section::AllProjects => {
                let mut v: Vec<(String, String)> = self.snap.projects.iter().flat_map(|pr| pr.folders.iter().map(|f| (f.id.clone(), f.name.clone()))).collect();
                v.sort_by_key(|(_, n)| n.to_lowercase());
                v
            }
            _ => vec![],
        }
    }

    pub fn title(&self) -> String {
        if self.searching() {
            return format!("Results for \u{201c}{}\u{201d}", self.search.trim());
        }
        match &self.section {
            Section::Recents => "Recents".into(),
            Section::Drafts => "Drafts".into(),
            Section::AllProjects => "All folders".into(),
            Section::Templates => "Templates & Specs".into(),
            Section::Resources => "Resources & Libraries".into(),
            Section::Starred => "Starred".into(),
            Section::Trash => "Trash".into(),
            Section::Project(p, _) => self.snap.project_name(p).to_string(),
        }
    }

    pub fn file(&self, id: &str) -> Option<&FileEntry> {
        self.snap.files.iter().find(|f| f.id == id)
    }
}

/// Something the user asked for on the home screen.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Go(Section),
    Open(String),
    OpenWindow(String),
    NewFile,
    NewBoard,
    NewDeck,
    OpenStarterSample,
    OpenSpecs,
    OpenLibrariesModal,
    OpenVariables,
    OpenPlugins,
    OpenAgents,
    Import,
    NewProject,
    RenameProject(String),
    DeleteProject(String),
    NewFolder(String, Option<String>),
    RenameFolder(String, String),
    FolderToTop(String, String),
    RenameFile(String),
    Duplicate(String),
    MoveTo(String),
    Star(String, bool),
    Reveal(String),
    ExportFig(String),
    Trash(String),
    RemoveMissing(String),
    Restore(String),
    EmptyTrash,
    Drop(String, DropTo),
    SetView(ViewMode),
    SetFilter(FileFilter),
    SetSort(Sort),
}

/// Draws the home screen filling `ui`, then runs what the user did.
pub fn show(app: &mut App, ui: &mut Ui) {
    let Some(mut h) = app.home.take() else { return };
    let Some(lib) = app.lib.clone() else { return };
    if h.dirty {
        h.refresh(&lib);
    }
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, theme::BG);
    let side = Rect::from_min_max(full.min, Pos2::new(full.left() + theme::HOME_SIDEBAR_W, full.bottom()));
    let main = Rect::from_min_max(Pos2::new(side.right(), full.top()), full.max);
    h.drop_rects.clear();
    let mut acts = Vec::new();
    sidebar::show(&mut h, ui, side, &mut acts);
    main_view::show(&lib, &mut h, ui, main, &mut acts);
    menus(&mut h, ui.ctx(), &mut acts);
    updater::show_update_modal(ui.ctx(), &mut h.updater);
    drag_ghost(&mut h, ui, &mut acts);
    // Escape clears the search.
    if h.searching()
        && app.dialog.is_none()
        && ui.input(|i| i.key_pressed(egui::Key::Escape))
        && h.menu.is_none()
        && h.sort_menu.is_none()
        && h.filter_menu.is_none()
    {
        h.search.clear();
    }
    app.home = Some(h);
    let mut imported_in_frame = false;
    for a in acts {
        if matches!(a, Act::Import) {
            if imported_in_frame {
                continue;
            }
            imported_in_frame = true;
        }
        apply(app, a);
    }
}

fn menus(h: &mut Home, ctx: &egui::Context, acts: &mut Vec<Act>) {
    use crate::widgets::menu::{popup_menu_with_row_h, MenuItem, MenuResult};
    if let Some(anchor) = h.filter_menu {
        let items: Vec<MenuItem> = FileFilter::ALL.iter().map(|f| MenuItem::new(f.label()).checked(*f == h.filter)).collect();
        match popup_menu_with_row_h(
            ctx,
            egui::Id::new("home-filter-menu"),
            anchor.left_bottom() + Vec2::new(0.0, 4.0),
            168.0,
            &items,
            Some(anchor),
            theme::CONTROL_SM,
        ) {
            MenuResult::Picked(i) => {
                h.filter_menu = None;
                acts.push(Act::SetFilter(FileFilter::ALL[i]));
            }
            MenuResult::Closed => h.filter_menu = None,
            MenuResult::Open => {}
        }
    }
    if let Some(anchor) = h.sort_menu {
        let items: Vec<MenuItem> = Sort::ALL.iter().map(|s| MenuItem::new(s.label()).checked(*s == h.sort)).collect();
        match popup_menu_with_row_h(
            ctx,
            egui::Id::new("home-sort-menu"),
            anchor.left_bottom() + Vec2::new(0.0, 4.0),
            168.0,
            &items,
            Some(anchor),
            theme::CONTROL_SM,
        ) {
            MenuResult::Picked(i) => {
                h.sort_menu = None;
                acts.push(Act::SetSort(Sort::ALL[i]));
            }
            MenuResult::Closed => h.sort_menu = None,
            MenuResult::Open => {}
        }
    }
    let Some((target, pos)) = h.menu.clone() else { return };
    let entries = context_entries(h, &target);
    let items: Vec<MenuItem> = entries.iter().map(|e| e.0.clone()).collect();
    match popup_menu_with_row_h(ctx, egui::Id::new("home-context-menu"), pos, theme::MENU_WIDTH, &items, None, theme::CONTROL_SM) {
        MenuResult::Picked(i) => {
            h.menu = None;
            acts.push(entries[i].1.clone());
        }
        MenuResult::Closed => h.menu = None,
        MenuResult::Open => {}
    }
}

/// Menu rows for a right-click target, with the action each one runs.
/// Note: For `Target::File(id)`, rows `0..=7` and their 3 separators (`1`, `4`, `6`) are preserved
/// at exact indices so existing coordinate-based right-click tests (`home.rs` line 175) hit `"Move to trash"`.
pub fn context_entries(h: &Home, target: &Target) -> Vec<(crate::widgets::menu::MenuItem, Act)> {
    use crate::widgets::menu::MenuItem;
    match target {
        Target::File(id) => {
            let starred = h.file(id).is_some_and(|f| f.starred);
            vec![
                (MenuItem::new("Open"), Act::Open(id.clone())),
                (MenuItem::new("Open in new window").sep(), Act::OpenWindow(id.clone())),
                (MenuItem::new("Rename"), Act::RenameFile(id.clone())),
                (MenuItem::new("Duplicate"), Act::Duplicate(id.clone())),
                (MenuItem::new("Move to\u{2026}").sep(), Act::MoveTo(id.clone())),
                (MenuItem::new(if starred { "Remove star" } else { "Star" }), Act::Star(id.clone(), !starred)),
                (MenuItem::new("Show in folder").sep(), Act::Reveal(id.clone())),
                (MenuItem::new("Move to trash").sep(), Act::Trash(id.clone())),
                (MenuItem::new("Export .fig"), Act::ExportFig(id.clone())),
            ]
        }
        Target::Project(id) => vec![
            (MenuItem::new("New folder"), Act::NewFolder(id.clone(), None)),
            (MenuItem::new("Rename").sep(), Act::RenameProject(id.clone())),
            (MenuItem::new("Move to trash"), Act::DeleteProject(id.clone())),
        ],
        Target::Folder(p, f) => {
            let nested = h.snap.projects.iter().find(|x| &x.id == p).and_then(|pr| pr.folders.iter().find(|x| &x.id == f)).is_some_and(|x| x.parent.is_some());
            let mut v = vec![
                (MenuItem::new("New subfolder"), Act::NewFolder(p.clone(), Some(f.clone()))),
                (MenuItem::new("Rename"), Act::RenameFolder(p.clone(), f.clone())),
            ];
            if nested {
                v.push((MenuItem::new("Move to top level"), Act::FolderToTop(p.clone(), f.clone())));
            }
            v
        }
        Target::TrashItem(id) => vec![(MenuItem::new("Restore"), Act::Restore(id.clone())), (MenuItem::new("Empty trash\u{2026}"), Act::EmptyTrash)],
    }
}

/// Shows the dragged file under the pointer and drops it on release.
fn drag_ghost(h: &mut Home, ui: &Ui, acts: &mut Vec<Act>) {
    let Some((id, name)) = h.drag.clone() else { return };
    let (ptr, released) = ui.input(|i| (i.pointer.hover_pos().or(i.pointer.latest_pos()), i.pointer.primary_released()));
    let Some(ptr) = ptr else {
        h.drag = None;
        return;
    };
    let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("home-drag")));
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
    let w = crate::widgets::text_width(&p, &name, &font).min(220.0) + theme::SPACE_GROUP * 2.0;
    let r = Rect::from_min_size(ptr + Vec2::new(12.0, 8.0), Vec2::new(w, 28.0));
    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_BRAND);
    let shown = crate::widgets::elide(&p, &name, &font, 200.0);
    p.text(r.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER, shown, font, theme::TEXT_ON_BRAND);
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    if released {
        if let Some((_, to)) = h.drop_rects.iter().rev().find(|(r, _)| r.contains(ptr)) {
            acts.push(Act::Drop(id, to.clone()));
        }
        h.drag = None;
    }
}

/// Import: asks for design files and brings them into the current project (or the first one, or a new one).
pub fn import_files_dialog(app: &mut App, dialogs: &dyn crate::io::FileDialogs) {
    let Some(_guard) = app.try_begin_dialog() else { return };
    let request = crate::io::FileRequest::open()
        .filter("Design files", &["designos", "fig", "json", "svg"])
        .filter("design:os studio", &["designos"])
        .filter("Figma file", &["fig"])
        .filter("SVG", &["svg"])
        .filter("JSON", &["json"]);
    let outcome = dialogs.pick_many(&request);
    let Some(paths) = app.picked_paths(outcome, true) else { return };
    let Some(lib) = app.lib.clone() else { return };
    let target = match app.home_target() {
        Some(t) => Some(t),
        None => app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))),
    };
    let target = match target {
        Some(t) => Some(t),
        None => match lib.create_project("My project") {
            Ok(p) => Some((p.id, None)),
            Err(e) => {
                app.show_toast(e.to_string(), true);
                None
            }
        },
    };
    if let Some((p, f)) = target {
        app.import_into_library(&paths, &p, f.as_deref());
    }
}

/// Runs one action against the library, reporting failures as toasts.
pub fn apply(app: &mut App, act: Act) {
    let Some(lib) = app.lib.clone() else { return };
    fn ok<T>(app: &mut App, r: designos_library::Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                app.show_toast(e.to_string(), true);
                None
            }
        }
    }
    let mut changed = true;
    match act {
        Act::Go(section) => {
            if let Some(h) = app.home.as_mut() {
                h.section = section;
                h.search.clear();
                h.selected = None;
                h.scroll = 0.0;
            }
            changed = false;
        }
        Act::SetView(v) => {
            if let Some(h) = app.home.as_mut() {
                h.view = v;
            }
            changed = false;
        }
        Act::SetFilter(f) => {
            if let Some(h) = app.home.as_mut() {
                h.filter = f;
                h.scroll = 0.0;
            }
            changed = false;
        }
        Act::SetSort(s) => {
            if let Some(h) = app.home.as_mut() {
                h.sort = s;
            }
            changed = false;
        }
        Act::Open(id) => app.open_library_file(&id),
        Act::OpenWindow(id) => {
            let exe = std::env::current_exe();
            match exe.and_then(|e| std::process::Command::new(e).arg("--library").arg(lib.root()).arg("--open-id").arg(&id).spawn()) {
                Ok(_) => app.show_toast("Opened in a new window", false),
                Err(e) => app.show_toast(format!("Cannot open a new window: {e}"), true),
            }
        }
        Act::NewFile => {
            let target = match app.home_target() {
                Some(t) => Some(t),
                None => app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))),
            };
            app.new_library_file(target);
        }
        Act::NewBoard => {
            let target = match app.home_target() {
                Some(t) => Some(t),
                None => app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))),
            };
            app.new_library_board(target);
        }
        Act::NewDeck => {
            let target = match app.home_target() {
                Some(t) => Some(t),
                None => app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))),
            };
            app.new_library_deck(target);
        }
        Act::OpenStarterSample => {
            app.open_starter_sample();
        }
        Act::OpenSpecs => {
            app.home = None;
            app.ui.rail = Some(crate::app::RailItem::Specs);
            app.specs.open = true;
            app.specs.ensure_loaded();
            changed = false;
        }
        Act::OpenLibrariesModal => {
            app.home = None;
            app.ui.rail = Some(crate::app::RailItem::Assets);
            app.ui.left_collapsed = false;
            app.libs.open = true;
            changed = false;
        }
        Act::OpenVariables => {
            app.home = None;
            app.ui.rail = Some(crate::app::RailItem::Variables);
            app.ui.left_collapsed = false;
            app.vars.open = true;
            app.vars.full = true;
            changed = false;
        }
        Act::OpenPlugins => {
            app.home = None;
            app.ui.rail = Some(crate::app::RailItem::Plugins);
            app.open_plugins();
            changed = false;
        }
        Act::OpenAgents => {
            app.home = None;
            app.ui.rail = Some(crate::app::RailItem::Agents);
            app.ui.left_collapsed = false;
            app.terminal.open_for_agent_use(app.file_path.as_deref());
            changed = false;
        }
        Act::Import => import_files_dialog(app, &crate::io::SystemDialogs),
        Act::NewProject => {
            app.dialog = Some(Dialog::prompt(Intent::NewProject, "New project", "", "Create"));
        }
        Act::RenameProject(id) => {
            let name = lib.project(&id).map(|p| p.name).unwrap_or_default();
            app.dialog = Some(Dialog::prompt(Intent::RenameProject(id), "Rename project", &name, "Rename"));
            changed = false;
        }
        Act::DeleteProject(id) => {
            let name = lib.project(&id).map(|p| p.name).unwrap_or_default();
            if ok(app, lib.delete_project(&id)).is_some() {
                if app.lib_file.as_ref().is_some_and(|f| f.project_id == id) {
                    app.lib_file = None;
                }
                app.show_toast(format!("Moved {name} to the trash"), false);
            }
        }
        Act::NewFolder(p, parent) => {
            app.dialog = Some(Dialog::prompt(Intent::NewFolder(p, parent), "New folder", "", "Create"));
            changed = false;
        }
        Act::RenameFolder(p, f) => {
            let name = lib.project(&p).ok().and_then(|pr| pr.folders.into_iter().find(|x| x.id == f)).map(|x| x.name).unwrap_or_default();
            app.dialog = Some(Dialog::prompt(Intent::RenameFolder(p, f), "Rename folder", &name, "Rename"));
            changed = false;
        }
        Act::FolderToTop(p, f) => {
            ok(app, lib.move_folder(&p, &f, None));
        }
        Act::RenameFile(id) => {
            let name = lib.file(&id).map(|f| f.name).unwrap_or_default();
            app.dialog = Some(Dialog::prompt(Intent::RenameFile(id), "Rename file", &name, "Rename"));
            changed = false;
        }
        Act::Duplicate(id) => {
            if ok(app, lib.duplicate_file(&id)).is_some() {
                app.show_toast("Duplicated", false);
            }
        }
        Act::MoveTo(id) => {
            let mut d = Dialog::pick(Intent::MoveFile(id.clone()), "Move to\u{2026}", "Move", &lib);
            if let Ok(f) = lib.file(&id) {
                d.pick_where(&f.project, f.folder.as_deref());
            }
            app.dialog = Some(d);
            changed = false;
        }
        Act::Star(id, on) => {
            ok(app, lib.star(&id, on));
        }
        Act::Reveal(id) => {
            changed = false;
            if let Ok(f) = lib.file(&id) {
                let dir = lib.path("projects").join(&f.project).join("files");
                if let Err(e) = reveal(&dir) {
                    app.show_toast(e, true);
                }
            }
        }
        Act::ExportFig(id) => {
            changed = false;
            let result = lib.load_file(&id).map_err(|e| e.to_string()).and_then(|doc| crate::fig_export::export_new(&doc, &lib.path("exports"), &doc.name));
            app.show_fig_export_result(result);
        }
        Act::Trash(id) => {
            let name = lib.file(&id).map(|f| f.name).unwrap_or_default();
            if ok(app, lib.trash_file(&id)).is_some() {
                if app.lib_file.as_ref().is_some_and(|f| f.id == id) {
                    app.lib_file = None;
                }
                app.show_toast(format!("Moved {name} to the trash"), false);
            }
        }
        Act::RemoveMissing(id) => {
            let name = lib.file(&id).map(|f| f.name).unwrap_or_default();
            if ok(app, lib.remove_missing_file(&id)).is_some() {
                if app.lib_file.as_ref().is_some_and(|f| f.id == id) {
                    app.lib_file = None;
                }
                app.show_toast(format!("Removed {name}"), false);
            }
        }
        Act::Restore(id) => {
            if ok(app, lib.restore(&id)).is_some() {
                app.show_toast("Restored", false);
            }
        }
        Act::EmptyTrash => {
            let n = app.home.as_ref().map(|h| h.snap.trash.len()).unwrap_or(0);
            let body = if n == 1 { "1 item will be permanently deleted." } else { "These items will be permanently deleted." };
            let mut d = Dialog::confirm(Intent::EmptyTrash, "Empty the trash?", &[body, "This cannot be undone."], "Empty trash");
            d.danger = true;
            app.dialog = Some(d);
            changed = false;
        }
        Act::Drop(id, to) => {
            let (project, folder) = match to {
                DropTo::Project(p) => (p, None),
                DropTo::Folder(p, f) => (p, Some(f)),
            };
            if ok(app, lib.move_file(&id, &project, folder.as_deref())).is_some() {
                if let Some(lf) = app.lib_file.as_mut().filter(|f| f.id == id) {
                    lf.project_id = project.clone();
                    lf.project = lib.project(&project).map(|p| p.name).unwrap_or_default();
                }
                app.show_toast("Moved", false);
            }
        }
    }
    if changed {
        if let Some(h) = app.home.as_mut() {
            h.dirty = true;
        }
    }
}

/// Opens a folder in the system file manager.
pub fn reveal(dir: &std::path::Path) -> Result<(), String> {
    #[cfg(feature = "open-url")]
    {
        open::that(dir).map_err(|e| format!("Cannot open {}: {e}", dir.display()))
    }
    #[cfg(not(feature = "open-url"))]
    {
        Err(format!("The file is in {}", dir.display()))
    }
}

/// Opens a web link in the browser.
pub fn open_link(url: &str) -> Result<(), String> {
    #[cfg(feature = "open-url")]
    {
        open::that(url).map_err(|e| format!("Could not open link: {e}"))
    }
    #[cfg(not(feature = "open-url"))]
    {
        Err(format!("Open {url} in your browser"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline Left Sidebar (`264px`, `theme::BG`, hairline `theme::BORDER`)
// ─────────────────────────────────────────────────────────────────────────────
mod sidebar {
    use super::{Act, DropTo, Home, Section, Target};
    use crate::icons::{self, Icon};
    use crate::theme;
    use crate::widgets;
    use crate::widgets::kit::{self, State, Surface, TileKind, Tone};
    use egui::{Align2, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

    const ROW_H: f32 = theme::CONTROL_SM;
    const INDENT: f32 = 16.0;
    /// Space above and below the Trash row inside its band under the divider.
    const TRASH_PAD: f32 = 9.0;

    pub fn show(h: &mut Home, ui: &mut Ui, rect: Rect, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, theme::BG);
        widgets::vline(&p, rect.right() - theme::BORDER_WIDTH, rect.top(), rect.bottom(), theme::BORDER);
        let x0 = rect.left();
        let w = rect.width() - theme::BORDER_WIDTH;
        let mut y = rect.top();

        // 1. Top Workspace Switcher Header Row (`V1.1`: Workspace Avatar + "Personal Workspace" + Version Pill)
        let ws_rect = Rect::from_min_size(Pos2::new(x0 + 8.0, y + 10.0), Vec2::new(w - 44.0, theme::CONTROL_SM));
        let ws_resp = ui.interact(ws_rect, Id::new("home-workspace-header"), Sense::click());
        widgets::record(ui, "home-workspace-header", ws_rect);
        if ws_resp.hovered() {
            p.rect_filled(ws_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        let avatar_r = Rect::from_center_size(Pos2::new(x0 + 22.0, ws_rect.center().y), Vec2::splat(20.0));
        p.rect_filled(avatar_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
        p.rect_stroke(avatar_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_STRONG), StrokeKind::Inside);
        icons::paint(&p, Icon::Folder, avatar_r.center(), 12.0, theme::ICON_BRAND_PRIMARY);
        let ws_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
        p.text(Pos2::new(x0 + 38.0, ws_rect.center().y), Align2::LEFT_CENTER, "Personal Workspace", ws_font, theme::TEXT);
        let ver = format!("v{}", env!("CARGO_PKG_VERSION"));
        let ver_font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
        let ver_w = (widgets::text_width(&p, &ver, &ver_font) + theme::SPACE_FACT * 2.0).max(theme::BADGE_SM);
        let ver_r = Rect::from_center_size(Pos2::new(ws_rect.right() - ver_w * 0.5 - theme::SPACE_FACT, ws_rect.center().y), Vec2::new(ver_w, theme::BADGE_SM));
        kit::count_badge(&p, ver_r, &ver, Tone::Neutral);
        if ws_resp.clicked() {
            acts.push(Act::Go(Section::Recents));
        }

        // Right notification/activity icon button
        let bell_r = Rect::from_center_size(Pos2::new(x0 + w - 22.0, ws_rect.center().y), Vec2::splat(24.0));
        let _ = widgets::icon_button(ui, bell_r, "home-notifications", Icon::Eye, false, "Workspace activity");
        widgets::record(ui, "home-notifications", bell_r);
        y += theme::CONTROL_SM + 16.0;

        // Search field at the small control height, framed with the kit's search look; the text edit is drawn inside it.
        let sr = Rect::from_min_size(Pos2::new(x0 + 12.0, y), Vec2::new(w - 24.0, theme::CONTROL_SM));
        let search_focused = ui.memory(|m| m.has_focus(Id::new("home-search")));
        let search = kit::search_look(if search_focused { State::Focused } else { State::Default });
        kit::paint_frame(&p, sr, theme::RADIUS_CONTROL, &search);
        icons::paint(&p, Icon::Search, Pos2::new(sr.left() + 14.0, sr.center().y), 14.0, search.icon);
        let tr = Rect::from_min_max(Pos2::new(sr.left() + 28.0, sr.top()), sr.max - Vec2::new(8.0, 0.0));
        let tid = Id::new("home-search");
        let mut text = h.search.clone();
        let te = egui::TextEdit::singleline(&mut text)
            .id(tid)
            .font(theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular))
            .text_color(theme::TEXT)
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center)
            .hint_text(
                egui::RichText::new("Search").font(theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular)).color(theme::TEXT_PLACEHOLDER),
            )
            .min_size(tr.size())
            .desired_width(tr.width());
        ui.put(tr, te);
        widgets::record(ui, "home-search", sr);
        if text != h.search {
            h.search = text;
            h.selected = None;
            h.scroll = 0.0;
        }
        y += theme::CONTROL_SM + 10.0;

        // 3. Primary Navigation (`Recents`, `Templates & Specs`)
        for (icon, label, section, key) in
            [(Icon::Clock, "Recents", Section::Recents, "recents"), (Icon::Specs, "Templates & Specs", Section::Templates, "templates")]
        {
            let r = Rect::from_min_size(Pos2::new(x0 + 8.0, y), Vec2::new(w - 16.0, ROW_H));
            let selected = h.section == section && !h.searching();
            if row(ui, r, Id::new(("home-nav", key)), icon, label, 0.0, selected, false, key).clicked() {
                acts.push(Act::Go(section));
            }
            y += ROW_H + 2.0;
        }
        y += 6.0;
        widgets::hline(&p, x0 + 12.0, x0 + w - 12.0, y, theme::BORDER);
        y += 8.0;

        // 4. Workspace Section Header (`FILES & PROJECTS`) + `+` New Project button
        let ws_badge_c = Pos2::new(x0 + 20.0, y + 12.0);
        let design = kit::tile_look(TileKind::Design);
        p.circle_filled(ws_badge_c, 7.0, design.fill);
        p.text(ws_badge_c, Align2::CENTER_CENTER, "P", theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold), design.icon);
        p.text(
            Pos2::new(x0 + 32.0, y + 12.0),
            Align2::LEFT_CENTER,
            "FILES & PROJECTS",
            theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold),
            theme::TEXT_TERTIARY,
        );
        let plus = Rect::from_min_size(Pos2::new(x0 + w - 12.0 - 24.0, y), Vec2::splat(24.0));
        if widgets::icon_button(ui, plus, "home-new-project", Icon::Plus, false, "New project").clicked() {
            acts.push(Act::NewProject);
        }
        widgets::record(ui, "home-new-project", plus);
        y += 26.0;

        // Workspace items: `Drafts`, `All folders`, `Resources & Libraries`
        for (icon, label, section, key) in [
            (Icon::File, "Drafts", Section::Drafts, "drafts"),
            (Icon::GridView, "All folders", Section::AllProjects, "all-projects"),
            (Icon::Assets, "Resources & Libraries", Section::Resources, "resources"),
        ] {
            let r = Rect::from_min_size(Pos2::new(x0 + 8.0, y), Vec2::new(w - 16.0, ROW_H));
            let selected = h.section == section && !h.searching();
            if row(ui, r, Id::new(("home-ws", key)), icon, label, 0.0, selected, false, key).clicked() {
                acts.push(Act::Go(section));
            }
            y += ROW_H + 1.0;
        }
        y += 4.0;

        // Reserve bottom space for `Starred` expandable section + Auto-Update Watchdog card + `Trash` row
        let starred_files: Vec<_> = h.snap.files.iter().filter(|f| f.starred).cloned().collect();
        let starred_rows_n = if h.starred_expanded { starred_files.len().min(3) } else { 0 };
        let bottom_reserve = 52.0 + 48.0 + ROW_H + 16.0 + starred_rows_n as f32 * 26.0;

        // 5. Project Tree (`Getting started`, `Marketing`, and nested folders)
        let list = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x0 + w, (rect.bottom() - bottom_reserve).max(y + 64.0)));
        let rows = tree_rows(h);
        let content_h = rows.len() as f32 * ROW_H + 8.0;
        crate::panels::wheel_scroll(ui, list, &mut h.side_scroll, content_h);
        let scroll = h.side_scroll;
        let ptr = ui.input(|i| i.pointer.hover_pos());
        let dragging = h.drag.is_some();
        let mut to_toggle = None;
        let mut secondary: Option<(Target, Pos2)> = None;
        let mut dropzones: Vec<(Rect, DropTo)> = vec![];
        crate::panels::clipped(ui, list, |ui| {
            if rows.is_empty() {
                ui.painter().text(
                    Pos2::new(x0 + 20.0, list.top() + 16.0),
                    Align2::LEFT_CENTER,
                    "No projects yet",
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                );
            }
            for (i, tr) in rows.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + 8.0, list.top() + i as f32 * ROW_H - scroll), Vec2::new(w - 16.0, ROW_H));
                if r.bottom() < list.top() || r.top() > list.bottom() {
                    continue;
                }
                let selected = !h.searching() && h.section == tr.section;
                let drop_hover = dragging && ptr.is_some_and(|p| r.contains(p) && list.contains(p));
                let icon = if tr.folder { Icon::Folder } else { Icon::Frame };
                let resp = row(
                    ui,
                    r,
                    Id::new(("home-tree", &tr.key)),
                    icon,
                    &tr.label,
                    tr.depth as f32 * INDENT,
                    selected,
                    drop_hover,
                    &format!("project-{}", tr.label),
                );
                if let Some(open) = tr.expanded {
                    let c = Pos2::new(r.left() + 10.0 + tr.depth as f32 * INDENT, r.center().y);
                    icons::paint(ui.painter(), if open { Icon::ChevronDown } else { Icon::ChevronRight }, c, 12.0, theme::ICON_QUATERNARY);
                }
                let on_chevron = tr.expanded.is_some() && ptr.is_some_and(|p| p.x < r.left() + 22.0 + tr.depth as f32 * INDENT);
                if resp.clicked() {
                    if on_chevron {
                        to_toggle = Some(tr.project.clone());
                    } else {
                        acts.push(Act::Go(tr.section.clone()));
                    }
                }
                if resp.secondary_clicked() {
                    let at = ui.input(|i| i.pointer.interact_pos()).unwrap_or(r.left_bottom());
                    let target = match &tr.section {
                        Section::Project(p, Some(f)) => Target::Folder(p.clone(), f.clone()),
                        _ => Target::Project(tr.project.clone()),
                    };
                    secondary = Some((target, at));
                }
                let to = match &tr.section {
                    Section::Project(p, Some(f)) => DropTo::Folder(p.clone(), f.clone()),
                    _ => DropTo::Project(tr.project.clone()),
                };
                dropzones.push((r.intersect(list), to));
            }
        });
        h.drop_rects.extend(dropzones);
        crate::panels::scrollbar(ui, list, scroll, content_h);
        if let Some(p) = to_toggle {
            if !h.collapsed.remove(&p) {
                h.collapsed.insert(p);
            }
        }
        if secondary.is_some() {
            h.menu = secondary;
        }

        // 6. Hairline Divider + `Starred` Expandable Section (`figma_ui3_home.png`)
        let mut sy = list.bottom() + 4.0;
        widgets::hline(&p, x0 + 12.0, x0 + w - 12.0, sy, theme::BORDER);
        sy += 6.0;
        let star_r = Rect::from_min_size(Pos2::new(x0 + 8.0, sy), Vec2::new(w - 16.0, ROW_H));
        let star_selected = h.section == Section::Starred && !h.searching();
        let star_resp = row(ui, star_r, Id::new(("home-nav", "starred")), Icon::Star, "Starred", 0.0, star_selected, false, "starred");
        icons::paint(
            &p,
            if h.starred_expanded { Icon::ChevronDown } else { Icon::ChevronRight },
            Pos2::new(star_r.right() - 12.0, star_r.center().y),
            11.0,
            theme::ICON_QUATERNARY,
        );
        if star_resp.clicked() {
            let on_right_chevron = ptr.is_some_and(|q| q.x > star_r.right() - 24.0);
            if on_right_chevron {
                h.starred_expanded = !h.starred_expanded;
            } else {
                acts.push(Act::Go(Section::Starred));
            }
        }
        sy += ROW_H + 2.0;

        if h.starred_expanded {
            for sf in starred_files.iter().take(3) {
                let sr_row = Rect::from_min_size(Pos2::new(x0 + 16.0, sy), Vec2::new(w - 24.0, 24.0));
                let sresp = ui.interact(sr_row, Id::new(("home-starred-item", &sf.id)), Sense::click());
                if sresp.hovered() {
                    p.rect_filled(sr_row, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                icons::paint(&p, Icon::File, Pos2::new(sr_row.left() + 12.0, sr_row.center().y), 13.0, kit::tile_look(TileKind::Design).icon);
                let stxt = widgets::elide(&p, &sf.name, &theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular), sr_row.width() - 28.0);
                p.text(
                    Pos2::new(sr_row.left() + 24.0, sr_row.center().y),
                    Align2::LEFT_CENTER,
                    stxt,
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                    theme::TEXT_SECONDARY,
                );
                if sresp.clicked() {
                    acts.push(Act::Open(sf.id.clone()));
                }
                sy += 25.0;
            }
        }

        // 7. Auto-Update & Upstream Figma Watchdog Status Card (`US-09` / `DIR-11`)
        let trash_h = ROW_H + TRASH_PAD * 2.0;
        let updater_r = Rect::from_min_size(Pos2::new(x0 + 8.0, rect.bottom() - trash_h - 46.0), Vec2::new(w - 16.0, 40.0));
        super::updater::show_sidebar_card_in_rect(ui, updater_r, &mut h.updater);

        // 8. Bottom `Trash` Row (`V3.3` / `D85`: separate count pill badge instead of parenthetical string)
        widgets::hline(&p, x0, x0 + w, rect.bottom() - trash_h, theme::BORDER);
        let tr = Rect::from_min_size(Pos2::new(x0 + 8.0, rect.bottom() - TRASH_PAD - ROW_H), Vec2::new(w - 16.0, ROW_H));
        let n = h.snap.trash.len();
        let trash_resp = row(ui, tr, Id::new("home-trash"), Icon::Trash, "Trash", 0.0, h.section == Section::Trash && !h.searching(), false, "trash");
        if n > 0 {
            let label = n.to_string();
            let badge_font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
            let bw = (widgets::text_width(&p, &label, &badge_font) + theme::SPACE_FACT * 2.0).max(theme::BADGE_SM);
            let badge_r = Rect::from_center_size(Pos2::new(tr.right() - bw * 0.5 - theme::SPACE_FACT, tr.center().y), Vec2::new(bw, theme::BADGE_SM));
            kit::count_badge(&p, badge_r, &label, Tone::Neutral);
        }
        if trash_resp.clicked() {
            acts.push(Act::Go(Section::Trash));
        }
    }

    struct TreeRow {
        key: String,
        project: String,
        label: String,
        depth: usize,
        folder: bool,
        /// `Some(open)` when the row can be expanded.
        expanded: Option<bool>,
        section: Section,
    }

    /// Projects and (unless collapsed) their folder trees, flattened.
    fn tree_rows(h: &Home) -> Vec<TreeRow> {
        fn walk(out: &mut Vec<TreeRow>, p: &designos_library::Project, parent: Option<&str>, depth: usize) {
            let mut kids: Vec<_> = p.folders.iter().filter(|f| f.parent.as_deref() == parent).collect();
            kids.sort_by_key(|f| f.name.to_lowercase());
            for f in kids {
                out.push(TreeRow {
                    key: format!("{}/{}", p.id, f.id),
                    project: p.id.clone(),
                    label: f.name.clone(),
                    depth,
                    folder: true,
                    expanded: None,
                    section: Section::Project(p.id.clone(), Some(f.id.clone())),
                });
                walk(out, p, Some(&f.id), depth + 1);
            }
        }
        let mut out = vec![];
        for p in &h.snap.projects {
            let open = !h.collapsed.contains(&p.id);
            out.push(TreeRow {
                key: p.id.clone(),
                project: p.id.clone(),
                label: p.name.clone(),
                depth: 0,
                folder: false,
                expanded: (!p.folders.is_empty()).then_some(open),
                section: Section::Project(p.id.clone(), None),
            });
            if open {
                walk(&mut out, p, None, 1);
            }
        }
        out
    }

    /// One sidebar row: icon, label, hover, selected fill, and a drop-target stroke.
    #[allow(clippy::too_many_arguments)]
    fn row(ui: &mut Ui, r: Rect, id: Id, icon: Icon, label: &str, indent: f32, selected: bool, drop_hover: bool, record: &str) -> Response {
        let resp = ui.interact(r, id, Sense::click());
        let p = ui.painter();
        let state = if drop_hover || selected {
            State::Selected
        } else if resp.hovered() {
            State::Hover
        } else {
            State::Default
        };
        let look = kit::row_look(state, Surface::Primary);
        kit::paint_frame(p, r, theme::RADIUS_CONTROL, &look);
        if drop_hover {
            p.rect_stroke(r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
        }
        let ix = r.left() + 12.0 + indent;
        icons::paint(p, icon, Pos2::new(ix + 14.0, r.center().y), 15.0, look.icon);
        let font = if selected {
            theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold)
        } else {
            theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular)
        };
        let shown = widgets::elide(p, label, &font, r.right() - (ix + 30.0) - 8.0);
        p.text(Pos2::new(ix + 30.0, r.center().y), Align2::LEFT_CENTER, shown, font, look.text);
        widgets::record(ui, &format!("home-{record}"), r);
        resp
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline Main Area (Top Header, Creation Strip, Filter/Sort Bar, Grid/List Cards)
// ─────────────────────────────────────────────────────────────────────────────
mod main_view {
    use super::time::{edited, relative};
    use super::{Act, DropTo, Home, Section, Target, ViewMode};
    use crate::icons::{self, Icon};
    use crate::theme;
    use crate::widgets;
    use crate::widgets::kit::{self, Hierarchy, State, Surface, TileKind};
    use designos_library::{FileEntry, Library};
    use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};
    use std::borrow::Cow;

    /// Whether Home offers the spec bundle: the `+ Spec bundle` header button and the spec bundle
    /// template card. Both push `Act::OpenSpecs`, which only switches the rail to the spec reader,
    /// and that reader is switched off (`shell.rs`, `specs_reader_open`). With it off they led to a
    /// blank canvas and no Recents entry, so Home hides them until spec files exist. Turn this on
    /// together with the reader, never alone.
    const SPEC_BUNDLE_ENTRY_ENABLED: bool = false;

    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Primary,
        Secondary,
        SecondaryDestructive,
    }

    fn hierarchy(kind: Kind) -> Hierarchy {
        match kind {
            Kind::Primary => Hierarchy::Primary,
            Kind::Secondary => Hierarchy::Secondary,
            Kind::SecondaryDestructive => Hierarchy::SecondaryDestructive,
        }
    }

    /// Height of the text block under a card's thumbnail: padding, the title line, the meta line, padding.
    const CARD_META_H: f32 = theme::SPACE_GROUP + theme::LINE_TEXT_SM + theme::LINE_TEXT_XS + theme::SPACE_GROUP;

    fn file_card_size() -> (f32, f32, f32, f32) {
        (theme::HOME_CARD_W, theme::HOME_THUMB_H + CARD_META_H, theme::HOME_CARD_GAP, 24.0)
    }

    pub fn show(lib: &Library, h: &mut Home, ui: &mut Ui, rect: Rect, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, theme::BG);
        let pad = theme::HOME_PAD;
        let trash = h.section == Section::Trash && !h.searching();

        // 1. Top Header & Creation Strip (`+ Design file`, `+ FigJam board`, `+ Spec bundle` while `SPEC_BUNDLE_ENTRY_ENABLED`, `Import`, context `+ New folder`)
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), theme::HOME_HEADER_H));
        let mut right = rect.right() - pad;
        let folder_target: Option<(&str, Option<&str>)> = if trash || h.searching() {
            None
        } else {
            match &h.section {
                Section::Project(pid, folder) => Some((pid.as_str(), folder.as_deref())),
                Section::AllProjects => h.snap.projects.first().map(|pr| (pr.id.as_str(), None)),
                _ => None,
            }
        };
        let gap = 8.0;
        let mut span = 0.0;
        let mut add = |label: &str| {
            if span > 0.0 {
                span += gap;
            }
            span += button_width(&p, label);
        };
        if trash {
            if !h.snap.trash.is_empty() {
                add("Empty trash");
            }
        } else {
            add("+ Design file");
            add("+ FigJam board");
            if SPEC_BUNDLE_ENTRY_ENABLED {
                add("+ Spec bundle");
            }
            add("Import");
            if folder_target.is_some() {
                add("+ New folder");
            }
        }
        let leftmost = right - span;
        breadcrumb(h, ui, &p, Pos2::new(rect.left() + pad, head.center().y), leftmost - theme::SPACE_GROUP, acts);
        if trash {
            if !h.snap.trash.is_empty() {
                let r = button(ui, &p, &mut right, head.center().y, "Empty trash", Kind::SecondaryDestructive, "home-empty-trash");
                if r.clicked() {
                    acts.push(Act::EmptyTrash);
                }
            }
        } else {
            let r = button(ui, &p, &mut right, head.center().y, "+ Design file", Kind::Primary, "home-new-file");
            if r.clicked() {
                acts.push(Act::NewFile);
            }
            right -= gap;
            let r = button(ui, &p, &mut right, head.center().y, "+ FigJam board", Kind::Secondary, "home-new-board");
            if r.clicked() {
                acts.push(Act::NewBoard);
            }
            right -= gap;
            if SPEC_BUNDLE_ENTRY_ENABLED {
                let r = button(ui, &p, &mut right, head.center().y, "+ Spec bundle", Kind::Secondary, "home-new-spec");
                if r.clicked() {
                    acts.push(Act::OpenSpecs);
                }
                right -= gap;
            }
            let r = button(ui, &p, &mut right, head.center().y, "Import", Kind::Secondary, "home-import");
            if r.clicked() && !r.double_clicked() {
                acts.push(Act::Import);
            }
            if let Some((pid, parent_folder)) = folder_target {
                right -= gap;
                let r = button(ui, &p, &mut right, head.center().y, "+ New folder", Kind::Secondary, "home-new-folder");
                if r.clicked() {
                    acts.push(Act::NewFolder(pid.to_string(), parent_folder.map(str::to_string)));
                }
            }
        }
        let mut top = head.bottom();

        // 2. Sub-Header Filter & Sort Bar (`Filter: All files v`, `Sort: Last viewed v`, Grid/List toggle)
        if !trash {
            let y = top + 4.0;
            let filter_lbl = format!("Filter: {}", h.filter.label());
            let filter_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
            let fw = widgets::text_width(&p, &filter_lbl, &filter_font) + theme::SPACE_GROUP * 3.0;
            let fr = Rect::from_min_size(Pos2::new(rect.left() + pad, y), Vec2::new(fw, theme::CONTROL_SM));
            let fresp = ui.interact(fr, Id::new("home-filter"), Sense::click());
            let filter_hot = h.filter_menu.is_some() || fresp.hovered();
            paint_secondary(ui, fr, &filter_lbl, filter_hot, true, fresp.id);
            widgets::record(ui, "home-filter", fr);
            if fresp.clicked() {
                h.filter_menu = if h.filter_menu.is_some() { None } else { Some(fr) };
                h.sort_menu = None;
            }

            let sort_lbl = format!("Sort: {}", h.sort.label());
            let sw = widgets::text_width(&p, &sort_lbl, &filter_font) + theme::SPACE_GROUP * 3.0;
            let sr = Rect::from_min_size(Pos2::new(fr.right() + 8.0, y), Vec2::new(sw, theme::CONTROL_SM));
            let sresp = ui.interact(sr, Id::new("home-sort"), Sense::click());
            let sort_hot = h.sort_menu.is_some() || sresp.hovered();
            paint_secondary(ui, sr, &sort_lbl, sort_hot, true, sresp.id);
            widgets::record(ui, "home-sort", sr);
            if sresp.clicked() {
                h.sort_menu = if h.sort_menu.is_some() { None } else { Some(sr) };
                h.filter_menu = None;
            }

            let mut x = rect.right() - pad;
            for (mode, icon, key, tip) in
                [(ViewMode::List, Icon::ListView, "home-view-list", "List view"), (ViewMode::Grid, Icon::GridView, "home-view-grid", "Grid view")]
            {
                let br = Rect::from_min_size(Pos2::new(x - theme::CONTROL_SM, y), Vec2::splat(theme::CONTROL_SM));
                x -= theme::CONTROL_SM + theme::SPACE_FACT;
                if kit::button_icon(ui, br, key, icon, Hierarchy::Icon, kit::Size::Sm, true, h.view == mode).on_hover_text(tip).clicked() {
                    acts.push(Act::SetView(mode));
                }
                widgets::record(ui, key, br);
            }
            top += theme::CONTROL_SM + 10.0;
        }
        widgets::hline(&p, rect.left(), rect.right(), top, theme::BORDER);
        top += 1.0;

        let content = Rect::from_min_max(Pos2::new(rect.left(), top), rect.max);
        if trash {
            trash_view(h, ui, content, acts);
        } else if !h.searching() && h.section == Section::Templates {
            templates_view(lib, h, ui, content, acts);
        } else if !h.searching() && h.section == Section::AllProjects {
            all_projects_view(lib, h, ui, content, acts);
        } else if !h.searching() && h.section == Section::Resources {
            resources_view(lib, h, ui, content, acts);
        } else {
            files_view(lib, h, ui, content, acts);
        }
    }

    const CRUMB_SEP: f32 = theme::SPACE_SECTION;

    /// One crumb on screen: the path segment it stands for (`None` for the leading "…") and the text drawn.
    pub(crate) struct Crumb<'a> {
        pub(crate) segment: Option<usize>,
        pub(crate) text: Cow<'a, str>,
    }

    /// Drops leading segments behind "…" until the path fits in `avail`, then shortens the last one. `width_of`
    /// measures text in the crumb font, so the shortened text and its "…" are measured, not estimated.
    pub(crate) fn fit_breadcrumb<'a>(labels: &[&'a str], sep: f32, avail: f32, mut width_of: impl FnMut(&str) -> f32) -> Vec<Crumb<'a>> {
        let Some(last) = labels.len().checked_sub(1) else {
            return Vec::new();
        };
        let ellipsis_w = width_of("…");
        // Width of `labels[start..]` with a separator between neighbours.
        let mut tail = labels.iter().map(|&label| width_of(label)).sum::<f32>() + sep * last as f32;
        let mut start = 0;
        let lead_w = |start: usize| if start > 0 { ellipsis_w + sep } else { 0.0 };
        while start < last && lead_w(start) + tail > avail {
            tail -= width_of(labels[start]) + sep;
            start += 1;
        }
        let mut out = Vec::with_capacity(last - start + 2);
        if start > 0 {
            out.push(Crumb { segment: None, text: Cow::Borrowed("…") });
        }
        if lead_w(start) + tail <= avail {
            out.extend((start..=last).map(|segment| Crumb { segment: Some(segment), text: Cow::Borrowed(labels[segment]) }));
        } else {
            let (short, _) = widgets::elide_with(&mut width_of, labels[last], (avail - lead_w(start)).max(0.0));
            // A second "…" after the leading one would only repeat it.
            if start == 0 || short != "…" {
                out.push(Crumb { segment: Some(last), text: Cow::Owned(short) });
            }
        }
        out
    }

    /// Title, or clickable "Project / Folder / ..." path inside a project.
    fn breadcrumb(h: &Home, ui: &mut Ui, p: &egui::Painter, at: Pos2, max_x: f32, acts: &mut Vec<Act>) {
        let font = theme::font_text(theme::FONT_SIZE_TEXT_XL, theme::TextWeight::Semibold);
        let title: String;
        let mut labels: Vec<&str> = Vec::new();
        // Folders below the project, outermost first: (name, id). Segment `k + 1` of the path is `folders[k]`.
        let mut folders: Vec<(&str, &str)> = Vec::new();
        let mut project_id = None;
        match &h.section {
            Section::Project(pid, folder) if !h.searching() => {
                project_id = Some(pid.as_str());
                labels.push(h.snap.project_name(pid));
                if let Some(pr) = h.snap.projects.iter().find(|x| &x.id == pid) {
                    let mut cur = folder.as_deref();
                    while let Some(fid) = cur {
                        let Some(f) = pr.folders.iter().find(|x| x.id == fid) else { break };
                        folders.push((f.name.as_str(), f.id.as_str()));
                        cur = f.parent.as_deref();
                    }
                    folders.reverse();
                    labels.extend(folders.iter().map(|&(name, _)| name));
                }
            }
            _ => {
                title = h.title();
                labels.push(&title);
            }
        }
        let shown = fit_breadcrumb(&labels, CRUMB_SEP, (max_x - at.x).max(0.0), |text| widgets::text_width(p, text, &font));
        let mut x = at.x;
        let last = shown.len().saturating_sub(1);
        for (i, crumb) in shown.iter().enumerate() {
            let w = widgets::text_width(p, &crumb.text, &font);
            // The hit area never reaches into the next crumb, whose text starts `CRUMB_SEP` after this one ends.
            let hit_w = w.max(theme::CONTROL_SM).min(w + CRUMB_SEP - 1.0);
            let r = Rect::from_min_size(Pos2::new(x, at.y - theme::CONTROL_SM * 0.5), Vec2::new(hit_w, theme::CONTROL_SM));
            let color = if i == last { theme::TEXT } else { theme::TEXT_TERTIARY };
            if let (Some(segment), Some(pid), true) = (crumb.segment, project_id, i < last) {
                let resp = ui.interact(r, Id::new(("home-crumb", i)), Sense::click());
                if resp.hovered() {
                    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                if resp.clicked() {
                    let folder = segment.checked_sub(1).map(|k| folders[k].1.to_string());
                    acts.push(Act::Go(Section::Project(pid.to_string(), folder)));
                }
            }
            p.text(Pos2::new(x, at.y), Align2::LEFT_CENTER, &*crumb.text, font.clone(), color);
            widgets::record_with(
                ui,
                || format!("home-crumb-{i}"),
                Rect::from_min_size(Pos2::new(x, at.y - theme::LINE_TEXT_XL * 0.5), Vec2::new(w, theme::LINE_TEXT_XL)),
            );
            x += w;
            if i < last {
                p.text(Pos2::new(x + 8.0, at.y), Align2::LEFT_CENTER, "/", font.clone(), theme::TEXT_PLACEHOLDER);
                x += CRUMB_SEP;
            }
        }
    }

    fn button_width(p: &egui::Painter, label: &str) -> f32 {
        widgets::text_width(p, label, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold)) + theme::SPACE_GROUP * 2.0
    }

    /// The kit's focus band around `r` while the keyboard focus sits on `id`.
    fn paint_focus_ring(ui: &Ui, r: Rect, id: Id) {
        if kit::keyboard_focus(ui, id) {
            kit::focus_ring(ui.painter(), r, f32::from(theme::RADIUS_CONTROL));
        }
    }

    /// A secondary button painted over a rect whose click and keyboard focus belong to `id`.
    fn paint_secondary(ui: &Ui, r: Rect, label: &str, hot: bool, chevron: bool, id: Id) {
        let p = ui.painter();
        let look = kit::button_look(Hierarchy::Secondary, if hot { State::Hover } else { State::Default });
        kit::paint_frame(p, r, theme::RADIUS_CONTROL, &look);
        paint_focus_ring(ui, r, id);
        let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        if chevron {
            p.text(Pos2::new(r.left() + theme::SPACE_GROUP, r.center().y), Align2::LEFT_CENTER, label, font, look.text);
            icons::paint(p, Icon::ChevronDown, Pos2::new(r.right() - theme::SPACE_GROUP, r.center().y), theme::CHEVRON_SMALL, look.icon);
        } else {
            p.text(r.center(), Align2::CENTER_CENTER, label, font, look.text);
        }
    }

    /// Right-aligned button; moves `right` to its left edge.
    fn button(ui: &mut Ui, p: &egui::Painter, right: &mut f32, cy: f32, label: &str, kind: Kind, record: &str) -> Response {
        let w = button_width(p, label);
        let h = theme::CONTROL_SM;
        let r = Rect::from_min_size(Pos2::new(*right - w, cy - h * 0.5), Vec2::new(w, h));
        *right = r.left();
        let resp = kit::button(ui, r, record, label, hierarchy(kind), kit::Size::Sm, true);
        widgets::record(ui, record, r);
        let _ = p;
        resp
    }

    fn section_heading(p: &egui::Painter, x: f32, y: f32, title: &str, subtitle: Option<&str>) -> f32 {
        p.text(Pos2::new(x, y + 10.0), Align2::LEFT_CENTER, title, theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), theme::TEXT);
        if let Some(sub) = subtitle {
            let tw = widgets::text_width(p, title, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold));
            p.text(
                Pos2::new(x + tw + 10.0, y + 10.0),
                Align2::LEFT_CENTER,
                sub,
                theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                theme::TEXT_SECONDARY,
            );
        }
        28.0
    }

    /// Dedicated view for `Section::Templates` (`"Templates & Specs"`):
    /// Blank creation cards (`Blank design`, `Blank board`, `Blank deck`), `DESIGN-OS Studio Starter`, and user's matching template/spec files below.
    fn templates_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let now = h.now_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(240.0);
        /// (title, badge, tile, icon, description, button label, action, record key)
        type TemplateCard<'a> = (&'a str, &'a str, TileKind, Icon, &'a str, &'a str, Act, &'a str);
        let mut cards: Vec<TemplateCard> = vec![
            ("Blank design", "Design File", TileKind::Design, Icon::Pen, "Blank vector & auto-layout canvas", "New file", Act::NewFile, "home-template-design"),
            (
                "Blank board",
                "FigJam Board",
                TileKind::Board,
                Icon::Sticky,
                "Infinite whiteboard, stickies & flows",
                "New board",
                Act::NewBoard,
                "home-template-board",
            ),
            (
                "Blank deck",
                "Slide Deck",
                TileKind::Deck,
                Icon::Play,
                "16:9 presentation slides with speaker notes & transitions",
                "New deck",
                Act::NewDeck,
                "home-template-deck",
            ),
            (
                "DESIGN-OS Studio Starter",
                "Starter Sample",
                TileKind::Spec,
                Icon::Component,
                "Original starter workspace with sample frames",
                "Open starter sample",
                Act::OpenStarterSample,
                "home-template-starter",
            ),
        ];
        if SPEC_BUNDLE_ENTRY_ENABLED {
            cards.push((
                "PRODUCT:OS Spec Bundle v2",
                "Spec Driven",
                TileKind::Spec,
                Icon::Specs,
                "Interactive PRD, User Stories, Architecture & Gates G1\u{2013}G5",
                "Open Spec Hub",
                Act::OpenSpecs,
                "home-template-specs",
            ));
        }

        let (tcw, tch, tgap_x, tgap_y) = (240.0_f32, 188.0 + theme::SPACE_GROUP + theme::SPACE_FACT, 16.0_f32, 16.0_f32);
        let tcols = (((inner_w + tgap_x) / (tcw + tgap_x)).floor() as usize).max(1);
        let template_rows = cards.len().div_ceil(tcols) as f32 * (tch + tgap_y);

        let (cw, ch, gap_x, gap_y) = file_card_size();
        let cols = (((inner_w + gap_x) / (cw + gap_x)).floor() as usize).max(1);
        let file_rows = if files.is_empty() {
            0.0
        } else if list {
            28.0 + files.len() as f32 * theme::HOME_LIST_ROW_H + 28.0
        } else {
            28.0 + files.len().div_ceil(cols) as f32 * (ch + gap_y)
        };

        let content_h = pad + 28.0 + template_rows + 12.0 + file_rows + pad;
        crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
        let scroll = h.scroll;
        let dragging = h.drag.is_some();
        let mut budget = 4u32;
        let selected = h.selected.clone();
        let mut new_selected = None;
        let mut menu = None;
        let mut drag = None;
        let ctx = ui.ctx().clone();

        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let x0 = content.left() + pad;
            let mut y = content.top() + pad - scroll;

            y += section_heading(&p, x0, y, "Starter Templates & UI Kits", Some("Click any kit to create an editable file in your workspace"));

            for (i, (title, badge, kind, icon, desc, btn_lbl, act, record)) in cards.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + (i % tcols) as f32 * (tcw + tgap_x), y + (i / tcols) as f32 * (tch + tgap_y)), Vec2::new(tcw, tch));
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let resp = ui.interact(r, Id::new(("home-tpl-card", *record)), Sense::click());
                let hot = resp.hovered();
                let tile = kit::tile_look(*kind);
                let card_r = theme::cr(theme::RADIUS_XL);
                p.rect_filled(r, card_r, theme::BG_SECONDARY);
                p.rect_stroke(r, card_r, Stroke::new(if hot { 1.5 } else { 1.0 }, if hot { theme::BORDER_STRONG } else { theme::BORDER }), StrokeKind::Inside);

                let banner = Rect::from_min_size(r.min + Vec2::new(1.0, 1.0), Vec2::new(r.width() - 2.0, 72.0));
                let top_r = theme::concentric(theme::RADIUS_XL as f32, 1.0) as u8;
                p.rect_filled(banner, CornerRadius { nw: top_r, ne: top_r, sw: 0, se: 0 }, tile.fill);
                let icon_box = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 14.0), Vec2::splat(28.0));
                p.rect_filled(icon_box, theme::cr(theme::RADIUS_CONTROL), tile.fill);
                p.rect_stroke(icon_box, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
                icons::paint(&p, *icon, icon_box.center(), 15.0, tile.icon);

                let bw = widgets::text_width(&p, badge, &theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold)) + 14.0;
                let pill_r = Rect::from_min_size(Pos2::new(r.right() - bw - 12.0, r.top() + 16.0), Vec2::new(bw, 20.0));
                p.rect_filled(pill_r, theme::cr_pill(pill_r.height()), tile.fill);
                p.rect_stroke(pill_r, theme::cr_pill(pill_r.height()), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
                p.text(pill_r.center(), Align2::CENTER_CENTER, *badge, theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold), tile.icon);

                let bar_y = banner.bottom() - 14.0;
                p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 12.0, bar_y), Vec2::new(64.0, 4.0)), theme::cr(theme::RADIUS_MARK), tile.icon);
                p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 82.0, bar_y), Vec2::new(40.0, 4.0)), theme::cr(theme::RADIUS_MARK), tile.icon);

                // Title & description
                p.text(
                    Pos2::new(r.left() + 12.0, banner.bottom() + 16.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, title, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), r.width() - 24.0),
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                let galley = p.layout(
                    (*desc).to_string(),
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                    r.width() - 24.0,
                );
                p.galley(Pos2::new(r.left() + 12.0, banner.bottom() + 28.0), galley, theme::TEXT_TERTIARY);

                let btn_r = Rect::from_min_size(
                    Pos2::new(r.left() + theme::SPACE_GROUP, r.bottom() - theme::SPACE_GROUP - theme::CONTROL_SM),
                    Vec2::new(r.width() - theme::SPACE_GROUP * 2.0, theme::CONTROL_SM),
                );
                paint_secondary(ui, btn_r, btn_lbl, hot, false, resp.id);

                widgets::record(ui, record, r);
                if resp.clicked() {
                    acts.push(act.clone());
                }
            }
            y += template_rows + 12.0;

            if !files.is_empty() {
                y += section_heading(&p, x0, y, "Your Template & Spec Files", None);
                render_file_items(
                    lib,
                    h,
                    ui,
                    &p,
                    content,
                    &files,
                    x0,
                    y,
                    inner_w,
                    cols,
                    list,
                    now,
                    dragging,
                    &selected,
                    &ctx,
                    &mut budget,
                    &mut new_selected,
                    &mut menu,
                    &mut drag,
                    acts,
                );
            }
        });
        crate::panels::scrollbar(ui, content, scroll, content_h);
        if let Some(s) = new_selected {
            h.selected = Some(s);
        }
        if menu.is_some() {
            h.menu = menu;
        }
        if drag.is_some() {
            h.drag = drag;
        }
    }

    /// Dedicated view for `Section::AllProjects` (`"All folders"`):
    /// Section 1: Projects grid with 2x2 thumbnail previews + `+ Folder` mini button + `+ New project` card.
    /// Section 2: Folders across all projects with `{project_name} / {folder_name}` + `+ New folder` tile.
    /// Section 3: Files in Folders.
    fn all_projects_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let now = h.now_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(260.0);

        let (pcw, pch, pgap_x, pgap_y) = (260.0_f32, 168.0_f32, 16.0_f32, 16.0_f32);
        let pcols = (((inner_w + pgap_x) / (pcw + pgap_x)).floor() as usize).max(1);
        let proj_total = h.snap.projects.len() + 1; // +1 for `+ New project` card
        let proj_rows_h = proj_total.div_ceil(pcols) as f32 * (pch + pgap_y);

        let all_folders: Vec<(String, String, String, String)> =
            h.snap.projects.iter().flat_map(|pr| pr.folders.iter().map(move |fl| (pr.id.clone(), pr.name.clone(), fl.id.clone(), fl.name.clone()))).collect();
        let (cw, ch, gap_x, gap_y) = file_card_size();
        let cols = (((inner_w + gap_x) / (cw + gap_x)).floor() as usize).max(1);
        let folder_total = all_folders.len() + 1; // +1 for `+ New folder` tile
        let folder_rows_h = folder_total.div_ceil(cols) as f32 * (theme::HOME_FOLDER_H + 12.0);

        let file_rows_h = if files.is_empty() {
            0.0
        } else if list {
            28.0 + files.len() as f32 * theme::HOME_LIST_ROW_H + 28.0
        } else {
            28.0 + files.len().div_ceil(cols) as f32 * (ch + gap_y)
        };

        let content_h = pad + 28.0 + proj_rows_h + 12.0 + 28.0 + folder_rows_h + 12.0 + file_rows_h + pad;
        crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
        let scroll = h.scroll;
        let ptr = ui.input(|i| i.pointer.hover_pos());
        let dragging = h.drag.is_some();
        let mut budget = 6u32;
        let selected = h.selected.clone();
        let mut new_selected = None;
        let mut menu = None;
        let mut drag = None;
        let mut zones: Vec<(Rect, DropTo)> = vec![];
        let ctx = ui.ctx().clone();
        let projects = h.snap.projects.clone();
        let all_snap_files = h.snap.files.clone();

        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let x0 = content.left() + pad;
            let mut y = content.top() + pad - scroll;

            // Section 1: Projects
            y += section_heading(&p, x0, y, "Projects", Some(&format!("{} workspace project{}", projects.len(), if projects.len() == 1 { "" } else { "s" })));

            for (i, pr) in projects.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + (i % pcols) as f32 * (pcw + pgap_x), y + (i / pcols) as f32 * (pch + pgap_y)), Vec2::new(pcw, pch));
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let plus_r = Rect::from_min_size(Pos2::new(r.right() - 74.0, r.top() + 8.0), Vec2::new(66.0, theme::TARGET_MIN));
                let plus_resp = ui.interact(plus_r, Id::new(("home-proj-new-folder", &pr.id)), Sense::click());
                let card_resp = ui.interact(r, Id::new(("home-proj-card", &pr.id)), Sense::click());
                let over = dragging && ptr.is_some_and(|q| r.contains(q));
                let hot = card_resp.hovered() || plus_resp.hovered();

                p.rect_filled(r, theme::cr(theme::RADIUS_XL), if over { theme::BG_SELECTED_HOVER } else { theme::BG_SECONDARY });
                p.rect_stroke(
                    r,
                    theme::cr(theme::RADIUS_XL),
                    Stroke::new(if hot { 1.5 } else { 1.0 }, if hot { theme::BORDER_STRONG } else { theme::BORDER }),
                    StrokeKind::Inside,
                );

                // Top 2x2 preview grid (96px tall)
                let preview = Rect::from_min_size(r.min + Vec2::new(8.0, 8.0), Vec2::new(r.width() - 16.0, 92.0));
                p.rect_filled(preview, theme::cr(theme::RADIUS_CONTROL), theme::HOME_THUMB_BG);
                let proj_files: Vec<&FileEntry> = all_snap_files.iter().filter(|f| f.project == pr.id).collect();
                let cell_w = (preview.width() - 12.0) / 2.0;
                let cell_h = (preview.height() - 12.0) / 2.0;
                for slot in 0..4 {
                    let cx = preview.left() + 4.0 + (slot % 2) as f32 * (cell_w + 4.0);
                    let cy = preview.top() + 4.0 + (slot / 2) as f32 * (cell_h + 4.0);
                    let cell = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(cell_w, cell_h));
                    if let Some(fe) = proj_files.get(slot) {
                        let tex = h.thumbs.get(&ctx, lib, fe, &mut budget);
                        let board = h.kinds.get(lib, fe, &mut budget) == designos_doc::DocumentKind::Board;
                        thumb_area(&p, cell, tex, board, 4);
                    } else {
                        p.rect_filled(cell, CornerRadius::same(4), theme::BG_TERTIARY);
                    }
                }

                // Top-right `+ Folder` mini button
                paint_secondary(ui, plus_r, "+ Folder", plus_resp.hovered(), false, plus_resp.id);
                widgets::record(ui, &format!("home-project-new-folder-{}", pr.name), plus_r);

                // Bottom metadata
                let icon_c = Pos2::new(r.left() + 18.0, preview.bottom() + 24.0);
                p.rect_filled(Rect::from_center_size(icon_c, Vec2::splat(22.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
                icons::paint(&p, Icon::Frame, icon_c, 13.0, theme::ICON_QUATERNARY);

                p.text(
                    Pos2::new(r.left() + 36.0, preview.bottom() + 16.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, &pr.name, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), r.width() - 48.0),
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                let sub = format!(
                    "{} file{} \u{b7} {} folder{}",
                    proj_files.len(),
                    if proj_files.len() == 1 { "" } else { "s" },
                    pr.folders.len(),
                    if pr.folders.len() == 1 { "" } else { "s" },
                );
                p.text(
                    Pos2::new(r.left() + 36.0, preview.bottom() + 33.0),
                    Align2::LEFT_CENTER,
                    sub,
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                );

                widgets::record(ui, &format!("home-project-card-{}", pr.name), r);
                if plus_resp.clicked() {
                    acts.push(Act::NewFolder(pr.id.clone(), None));
                } else if card_resp.clicked() {
                    acts.push(Act::Go(Section::Project(pr.id.clone(), None)));
                }
                if card_resp.secondary_clicked() {
                    menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::Project(pr.id.clone()), at));
                }
                zones.push((r.intersect(content), DropTo::Project(pr.id.clone())));
            }

            // `+ New project` card
            let np_idx = projects.len();
            let np_r = Rect::from_min_size(
                Pos2::new(x0 + (np_idx % pcols) as f32 * (pcw + pgap_x), y + (np_idx / pcols) as f32 * (pch + pgap_y)),
                Vec2::new(pcw, pch),
            );
            if np_r.bottom() >= content.top() && np_r.top() <= content.bottom() {
                let np_resp = ui.interact(np_r, Id::new("home-card-new-project"), Sense::click());
                let hot = np_resp.hovered();
                p.rect_filled(np_r, theme::cr(theme::RADIUS_XL), theme::BG_SECONDARY);
                p.rect_stroke(np_r, theme::cr(theme::RADIUS_XL), Stroke::new(1.0, if hot { theme::BORDER_STRONG } else { theme::BORDER }), StrokeKind::Inside);
                let c = np_r.center();
                p.circle_filled(Pos2::new(c.x, c.y - 14.0), 18.0, theme::BG_TERTIARY);
                icons::paint(&p, Icon::Plus, Pos2::new(c.x, c.y - 14.0), 16.0, theme::ICON_QUATERNARY);
                p.text(
                    Pos2::new(c.x, c.y + 14.0),
                    Align2::CENTER_CENTER,
                    "+ New project",
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                p.text(
                    Pos2::new(c.x, c.y + 31.0),
                    Align2::CENTER_CENTER,
                    "Organize files and nested folders",
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                );
                widgets::record(ui, "home-card-new-project", np_r);
                if np_resp.clicked() {
                    acts.push(Act::NewProject);
                }
            }
            y += proj_rows_h + 12.0;

            // Section 2: Folders
            y += section_heading(&p, x0, y, "Folders", Some("Subfolders across your projects"));
            for (i, (pid, pname, fid, fname)) in all_folders.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (theme::HOME_FOLDER_H + 12.0)),
                    Vec2::new(cw, theme::HOME_FOLDER_H),
                );
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let resp = ui.interact(r, Id::new(("home-all-folder", fid)), Sense::click());
                let over = dragging && ptr.is_some_and(|q| r.contains(q));
                let fill = if over {
                    theme::BG_SELECTED_HOVER
                } else if resp.hovered() {
                    theme::BORDER_STRONG
                } else {
                    theme::BG_SECONDARY
                };
                p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), fill);
                icons::paint(&p, Icon::Folder, Pos2::new(r.left() + 22.0, r.center().y), 18.0, theme::ICON_QUATERNARY);
                let label = format!("{pname} / {fname}");
                p.text(
                    Pos2::new(r.left() + 42.0, r.center().y),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, &label, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), r.width() - 54.0),
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                widgets::record(ui, &format!("home-folder-{fname}"), r);
                if resp.clicked() || resp.double_clicked() {
                    acts.push(Act::Go(Section::Project(pid.clone(), Some(fid.clone()))));
                }
                if resp.secondary_clicked() {
                    menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::Folder(pid.clone(), fid.clone()), at));
                }
                zones.push((r.intersect(content), DropTo::Folder(pid.clone(), fid.clone())));
            }

            // `+ New folder` tile
            let nf_idx = all_folders.len();
            let nf_r = Rect::from_min_size(
                Pos2::new(x0 + (nf_idx % cols) as f32 * (cw + gap_x), y + (nf_idx / cols) as f32 * (theme::HOME_FOLDER_H + 12.0)),
                Vec2::new(cw, theme::HOME_FOLDER_H),
            );
            if nf_r.bottom() >= content.top() && nf_r.top() <= content.bottom() {
                let nf_resp = ui.interact(nf_r, Id::new("home-tile-new-folder"), Sense::click());
                let hot = nf_resp.hovered();
                p.rect_filled(nf_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
                p.rect_stroke(
                    nf_r,
                    theme::cr(theme::RADIUS_CONTROL),
                    Stroke::new(1.0, if hot { theme::BORDER_STRONG } else { theme::BORDER }),
                    StrokeKind::Inside,
                );
                icons::paint(&p, Icon::Plus, Pos2::new(nf_r.left() + 22.0, nf_r.center().y), 16.0, theme::ICON_QUATERNARY);
                p.text(
                    Pos2::new(nf_r.left() + 42.0, nf_r.center().y),
                    Align2::LEFT_CENTER,
                    "+ New folder",
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                widgets::record(ui, "home-tile-new-folder", nf_r);
                if nf_resp.clicked() {
                    if let Some(first_p) = projects.first() {
                        acts.push(Act::NewFolder(first_p.id.clone(), None));
                    }
                }
            }
            y += folder_rows_h + 12.0;

            // Section 3: Files in Folders
            if !files.is_empty() {
                y += section_heading(&p, x0, y, "Files in Folders", None);
                render_file_items(
                    lib,
                    h,
                    ui,
                    &p,
                    content,
                    &files,
                    x0,
                    y,
                    inner_w,
                    cols,
                    list,
                    now,
                    dragging,
                    &selected,
                    &ctx,
                    &mut budget,
                    &mut new_selected,
                    &mut menu,
                    &mut drag,
                    acts,
                );
            }
        });
        crate::panels::scrollbar(ui, content, scroll, content_h);
        h.drop_rects.extend(zones);
        if let Some(s) = new_selected {
            h.selected = Some(s);
        }
        if menu.is_some() {
            h.menu = menu;
        }
        if drag.is_some() {
            h.drag = drag;
        }
    }

    /// Dedicated view for `Section::Resources` (`"Resources & Libraries"`):
    /// Section 1: 4 interactive Workspace Libraries, Tokens & Extensions cards.
    /// Section 2: Published Libraries & Resource Files.
    fn resources_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let published_libs = lib.list_libraries().unwrap_or_default();
        let now = h.now_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(260.0);

        let (rcw, rch, rgap_x, rgap_y) = (260.0_f32, 182.0_f32, 16.0_f32, 16.0_f32);
        let rcols = (((inner_w + rgap_x) / (rcw + rgap_x)).floor() as usize).max(1);
        let hub_rows_h = 4usize.div_ceil(rcols) as f32 * (rch + rgap_y);

        let (cw, ch, gap_x, gap_y) = file_card_size();
        let cols = (((inner_w + gap_x) / (cw + gap_x)).floor() as usize).max(1);
        let pub_rows_h = if published_libs.is_empty() { 0.0 } else { 28.0 + published_libs.len() as f32 * 56.0 + 12.0 };
        let file_rows_h = if files.is_empty() {
            0.0
        } else if list {
            28.0 + files.len() as f32 * theme::HOME_LIST_ROW_H + 28.0
        } else {
            28.0 + files.len().div_ceil(cols) as f32 * (ch + gap_y)
        };

        let content_h = pad + 28.0 + hub_rows_h + 12.0 + pub_rows_h + file_rows_h + pad;
        crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
        let scroll = h.scroll;
        let dragging = h.drag.is_some();
        let mut budget = 4u32;
        let selected = h.selected.clone();
        let mut new_selected = None;
        let mut menu = None;
        let mut drag = None;
        let ctx = ui.ctx().clone();

        let lib_sub =
            format!("{} published librar{} \u{b7} Shared components & styles", published_libs.len(), if published_libs.len() == 1 { "y" } else { "ies" });
        /// (button label, action, key)
        type HubButton<'a> = (&'a str, Act, &'a str);
        /// (title, icon, accent, description, primary button, optional secondary button)
        type HubCard<'a> = (&'a str, Icon, TileKind, &'a str, HubButton<'a>, Option<HubButton<'a>>);
        let hub_cards: [HubCard; 4] = [
            (
                "Shared Component Libraries",
                Icon::Assets,
                TileKind::Design,
                &lib_sub,
                ("Manage Libraries", Act::OpenLibrariesModal, "home-resource-libraries"),
                None,
            ),
            (
                "Design Tokens & Variables",
                Icon::Variables,
                TileKind::Deck,
                "Color, Number, String & Boolean collections \u{b7} Light/Dark modes",
                ("Open Variables", Act::OpenVariables, "home-resource-variables"),
                None,
            ),
            (
                "Plugins & Automation",
                Icon::Component,
                TileKind::Board,
                "Installed studio plugins, XPath query & 17-rule Design Linter",
                ("Open Plugins", Act::OpenPlugins, "home-resource-plugins"),
                None,
            ),
            (
                "Agent CLI & MCP Server",
                Icon::Terminal,
                TileKind::Spec,
                "Per-file Claude Code, Codex, Grok, Antigravity & Gemini CLI + the MCP server",
                ("Open Agents", Act::OpenAgents, "home-resource-agents"),
                None,
            ),
        ];

        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let x0 = content.left() + pad;
            let mut y = content.top() + pad - scroll;

            y += section_heading(
                &p,
                x0,
                y,
                "Workspace Libraries, Tokens & Extensions",
                Some("Manage shared design systems, variable modes, plugins and AI runtimes"),
            );

            for (i, (title, icon, kind, desc, (btn1_lbl, btn1_act, btn1_key), btn2_opt)) in hub_cards.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + (i % rcols) as f32 * (rcw + rgap_x), y + (i / rcols) as f32 * (rch + rgap_y)), Vec2::new(rcw, rch));
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let card_r = theme::cr(theme::RADIUS_XL);
                p.rect_filled(r, card_r, theme::BG_SECONDARY);
                p.rect_stroke(r, card_r, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);

                let tile = kit::tile_look(*kind);
                let icon_box = Rect::from_min_size(Pos2::new(r.left() + 14.0, r.top() + 14.0), Vec2::splat(32.0));
                p.rect_filled(icon_box, theme::cr(theme::RADIUS_CONTROL), tile.fill);
                icons::paint(&p, *icon, icon_box.center(), 17.0, tile.icon);

                p.text(
                    Pos2::new(r.left() + 14.0, r.top() + 62.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, title, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), r.width() - 28.0),
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                let galley = p.layout(
                    (*desc).to_string(),
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                    r.width() - 28.0,
                );
                p.galley(Pos2::new(r.left() + 14.0, r.top() + 76.0), galley, theme::TEXT_TERTIARY);

                let btn_top = r.bottom() - theme::SPACE_GROUP - theme::CONTROL_SM;
                if let Some((btn2_lbl, btn2_act, btn2_key)) = btn2_opt {
                    let bw = (r.width() - 34.0) / 2.0;
                    let b1 = Rect::from_min_size(Pos2::new(r.left() + 14.0, btn_top), Vec2::new(bw, theme::CONTROL_SM));
                    let b2 = Rect::from_min_size(Pos2::new(b1.right() + 6.0, btn_top), Vec2::new(bw, theme::CONTROL_SM));
                    let r1 = ui.interact(b1, Id::new(("home-res-b1", *btn1_key)), Sense::click());
                    let r2 = ui.interact(b2, Id::new(("home-res-b2", *btn2_key)), Sense::click());
                    paint_secondary(ui, b1, btn1_lbl, r1.hovered(), false, r1.id);
                    widgets::record(ui, btn1_key, b1);
                    widgets::labelled(ui, r1.id, egui::accesskit::Role::Button, btn1_lbl);
                    if r1.clicked() {
                        acts.push(btn1_act.clone());
                    }
                    paint_secondary(ui, b2, btn2_lbl, r2.hovered(), false, r2.id);
                    widgets::record(ui, btn2_key, b2);
                    widgets::labelled(ui, r2.id, egui::accesskit::Role::Button, btn2_lbl);
                    if r2.clicked() {
                        acts.push(btn2_act.clone());
                    }
                } else {
                    let b1 = Rect::from_min_size(Pos2::new(r.left() + 14.0, btn_top), Vec2::new(r.width() - 28.0, theme::CONTROL_SM));
                    let r1 = ui.interact(b1, Id::new(("home-res-b1", *btn1_key)), Sense::click());
                    paint_secondary(ui, b1, btn1_lbl, r1.hovered(), false, r1.id);
                    widgets::record(ui, btn1_key, b1);
                    widgets::labelled(ui, r1.id, egui::accesskit::Role::Button, btn1_lbl);
                    if r1.clicked() {
                        acts.push(btn1_act.clone());
                    }
                }
            }
            y += hub_rows_h + 12.0;

            if !published_libs.is_empty() {
                y += section_heading(&p, x0, y, "Published Libraries", None);
                for lib_idx in &published_libs {
                    let row_r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, 48.0));
                    y += 56.0;
                    if row_r.bottom() < content.top() || row_r.top() > content.bottom() {
                        continue;
                    }
                    let resp = ui.interact(row_r, Id::new(("home-pub-lib", &lib_idx.id)), Sense::click());
                    p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BORDER_STRONG } else { theme::BG_SECONDARY });
                    icons::paint(&p, Icon::Book, Pos2::new(row_r.left() + 22.0, row_r.center().y), 18.0, theme::ICON_QUATERNARY);
                    p.text(
                        Pos2::new(row_r.left() + 44.0, row_r.center().y - 8.0),
                        Align2::LEFT_CENTER,
                        &lib_idx.name,
                        theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                        theme::TEXT,
                    );
                    let meta = format!(
                        "v{} \u{b7} {} components \u{b7} {} styles \u{b7} {} variables",
                        lib_idx.latest_version(),
                        lib_idx.components,
                        lib_idx.styles,
                        lib_idx.variables,
                    );
                    p.text(
                        Pos2::new(row_r.left() + 44.0, row_r.center().y + 9.0),
                        Align2::LEFT_CENTER,
                        meta,
                        theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                        theme::TEXT_TERTIARY,
                    );
                    widgets::record(ui, &format!("home-published-library-{}", lib_idx.name), row_r);
                    if resp.clicked() {
                        if let Some(fid) = &lib_idx.source_file {
                            acts.push(Act::Open(fid.clone()));
                        } else {
                            acts.push(Act::OpenLibrariesModal);
                        }
                    }
                }
                y += 12.0;
            }

            if !files.is_empty() {
                y += section_heading(&p, x0, y, "Published Libraries & Resource Files", None);
                render_file_items(
                    lib,
                    h,
                    ui,
                    &p,
                    content,
                    &files,
                    x0,
                    y,
                    inner_w,
                    cols,
                    list,
                    now,
                    dragging,
                    &selected,
                    &ctx,
                    &mut budget,
                    &mut new_selected,
                    &mut menu,
                    &mut drag,
                    acts,
                );
            }
        });
        crate::panels::scrollbar(ui, content, scroll, content_h);
        if let Some(s) = new_selected {
            h.selected = Some(s);
        }
        if menu.is_some() {
            h.menu = menu;
        }
        if drag.is_some() {
            h.drag = drag;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_file_items(
        lib: &Library,
        h: &mut Home,
        ui: &mut Ui,
        p: &egui::Painter,
        content: Rect,
        files: &[FileEntry],
        x0: f32,
        mut y: f32,
        inner_w: f32,
        cols: usize,
        list: bool,
        now: u64,
        dragging: bool,
        selected: &Option<String>,
        ctx: &egui::Context,
        budget: &mut u32,
        new_selected: &mut Option<String>,
        menu: &mut Option<(Target, Pos2)>,
        drag: &mut Option<(String, String)>,
        acts: &mut Vec<Act>,
    ) {
        let (cw, ch, gap_x, gap_y) = file_card_size();
        if list {
            let hr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, 28.0));
            let cols_x = list_columns(hr.right());
            for (x, label) in [(hr.left() + 60.0, "Name"), (cols_x.0, "Location"), (cols_x.1, "Last modified"), (cols_x.2, "Last viewed")] {
                p.text(
                    Pos2::new(x, hr.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium),
                    theme::TEXT_TERTIARY,
                );
            }
            y += 28.0;
        }
        for (i, f) in files.iter().enumerate() {
            let r = if list {
                Rect::from_min_size(Pos2::new(x0, y + i as f32 * theme::HOME_LIST_ROW_H), Vec2::new(inner_w, theme::HOME_LIST_ROW_H))
            } else {
                Rect::from_min_size(Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (ch + gap_y)), Vec2::new(cw, ch))
            };
            if r.bottom() < content.top() || r.top() > content.bottom() {
                continue;
            }
            let is_sel = selected.as_deref() == Some(f.id.as_str());
            let resp = ui.interact(r, Id::new(("home-card", &f.id)), Sense::click_and_drag());
            if resp.double_clicked() {
                acts.push(Act::Open(f.id.clone()));
            } else if resp.clicked() {
                *new_selected = Some(f.id.clone());
            }
            if resp.secondary_clicked() {
                *new_selected = Some(f.id.clone());
                *menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::File(f.id.clone()), at));
            }
            if resp.drag_started() {
                *drag = Some((f.id.clone(), f.name.clone()));
            }
            let tex = h.thumbs.get(ctx, lib, f, budget);
            let board = h.kinds.get(lib, f, budget) == designos_doc::DocumentKind::Board;
            let project = h.snap.project_name(&f.project).to_string();
            let missing = !super::library_file_disk_path(lib, &f.project, &f.id).is_file();
            let star_r = if list {
                Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 12.0), Vec2::splat(24.0))
            } else {
                Rect::from_min_size(Pos2::new(r.left() + cw - 32.0, r.top() + 8.0), Vec2::splat(24.0))
            };
            let star = ui.interact(star_r, Id::new(("home-star", &f.id)), Sense::click());
            if star.clicked() {
                acts.push(Act::Star(f.id.clone(), !f.starred));
            }
            let mut paint = p.clone();
            if dragging && selected.as_deref() == Some(f.id.as_str()) {
                paint.set_opacity(0.5);
            }
            if list {
                list_row(&paint, r, f, &project, tex, board, missing, resp.hovered(), is_sel, now);
            } else {
                card(&paint, r, f, &project, tex, board, missing, resp.hovered(), is_sel, now);
            }
            if missing {
                let mut rm_right = r.right() - theme::HOME_PAD * 0.5;
                let rm_cy = if list { r.center().y } else { r.bottom() - theme::HOME_PAD };
                if button(ui, &paint, &mut rm_right, rm_cy, "Remove", Kind::SecondaryDestructive, &format!("home-remove-{}", f.name)).clicked() {
                    acts.push(Act::RemoveMissing(f.id.clone()));
                }
            }
            widgets::record(ui, &format!("home-kind-{}", if board { "board" } else { "design" }), r);
            if resp.hovered() || star.hovered() || f.starred {
                if star.hovered() || (!list && resp.hovered()) {
                    paint.circle_filled(star_r.center(), 12.0, theme::BG_OVERLAY);
                }
                paint_star(&paint, star_r.center(), f.starred, star.hovered());
            }
            widgets::record(ui, &format!("home-card-{}", f.name), r);
        }
    }

    fn empty_state(h: &Home, ui: &Ui, area: Rect) {
        let (title, body) = if h.searching() {
            (format!("No files match \u{201c}{}\u{201d}", h.search.trim()), "Try a different name or clear the filter.".to_string())
        } else {
            match &h.section {
                Section::Recents | Section::Drafts | Section::AllProjects | Section::Templates | Section::Resources => {
                    ("No files yet".into(), "Create a design file, FigJam board, or import a .fig, .svg or .designos file to get started.".into())
                }
                Section::Starred => ("Nothing starred".into(), "Hover a file thumbnail or right-click a file and choose Star to keep it handy here.".into()),
                Section::Project(..) => ("This project is empty".into(), "Create a design file here, or drag files in from other projects.".into()),
                Section::Trash => ("Trash is empty".into(), "Files and projects you delete wait here until you empty the trash.".into()),
            }
        };
        let p = ui.painter();
        let c = area.center();
        let tile = kit::tile_look(TileKind::Neutral);
        let tile_r = Rect::from_center_size(Pos2::new(c.x, c.y - 44.0), Vec2::splat(theme::CONTROL_XS));
        kit::paint_frame(p, tile_r, theme::RADIUS_CONTROL, &tile);
        icons::paint(p, Icon::File, tile_r.center(), 16.0, tile.icon);
        p.text(Pos2::new(c.x, c.y), Align2::CENTER_CENTER, title, theme::font_text(theme::FONT_SIZE_TEXT_MD, theme::TextWeight::Semibold), theme::TEXT);
        let galley =
            p.layout(body, theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular), theme::TEXT_TERTIARY, 380.0_f32.min(area.width() - 48.0));
        let pos = Pos2::new(c.x - galley.size().x / 2.0, c.y + theme::LINE_TEXT_MD * 0.5 + theme::SPACE_FACT);
        p.galley(pos, galley, theme::TEXT_TERTIARY);
    }

    fn files_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let folders = h.folders();
        let is_drafts = h.section == Section::Drafts && !h.searching();
        if files.is_empty() && folders.is_empty() && !is_drafts {
            empty_state(h, ui, content);
            return;
        }
        let now = h.now_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = content.width() - pad * 2.0;
        let (cw, ch, gap_x, gap_y) = file_card_size();
        let cols = (((inner_w + gap_x) / (cw + gap_x)).floor() as usize).max(1);
        let drafts_strip_h = if is_drafts { 64.0 } else { 0.0 };
        let folder_rows =
            if list { folders.len() as f32 * theme::HOME_LIST_ROW_H } else { folders.len().div_ceil(cols) as f32 * (theme::HOME_FOLDER_H + 12.0) };
        let file_rows = if files.is_empty() {
            0.0
        } else if list {
            files.len() as f32 * theme::HOME_LIST_ROW_H + 28.0
        } else {
            files.len().div_ceil(cols) as f32 * (ch + gap_y)
        };
        let content_h = pad + drafts_strip_h + folder_rows + if folder_rows > 0.0 { 12.0 } else { 0.0 } + file_rows + pad;
        crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
        let scroll = h.scroll;
        let ptr = ui.input(|i| i.pointer.hover_pos());
        let dragging = h.drag.is_some();
        let mut budget = 4u32;
        let selected = h.selected.clone();
        let mut new_selected = None;
        let mut menu = None;
        let mut drag = None;
        let mut new_sort: Option<(super::Sort, bool)> = None;
        let mut zones: Vec<(Rect, DropTo)> = vec![];
        let section = h.section.clone();
        let ctx = ui.ctx().clone();
        let project_id = if let Section::Project(p, _) = &section { Some(p.clone()) } else { None };
        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let mut y = content.top() + pad - scroll;
            let x0 = content.left() + pad;

            // ---- Drafts Quick-Create Strip (`+ New Design File`, `+ New FigJam Board`, `+ New Slide Deck`)
            if is_drafts {
                let quick_items: [(&str, &str, Icon, TileKind, Act, &str); 3] = [
                    ("+ New Design File", "Blank vector & auto-layout canvas", Icon::Pen, TileKind::Design, Act::NewFile, "home-drafts-new-file"),
                    ("+ New FigJam Board", "Infinite whiteboard, stickies & flows", Icon::Sticky, TileKind::Board, Act::NewBoard, "home-drafts-new-board"),
                    ("+ New Slide Deck", "16:9 presentation deck with speaker notes", Icon::Play, TileKind::Deck, Act::NewDeck, "home-drafts-new-deck"),
                ];
                let qw = ((inner_w - 24.0) / 3.0).max(180.0);
                for (qi, (qtitle, qsub, qicon, qcol, qact, qkey)) in quick_items.iter().enumerate() {
                    let qr = Rect::from_min_size(Pos2::new(x0 + qi as f32 * (qw + 12.0), y), Vec2::new(qw, 48.0));
                    let qresp = ui.interact(qr, Id::new(("home-draft-quick", *qkey)), Sense::click());
                    let hot = qresp.hovered();
                    p.rect_filled(qr, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
                    p.rect_stroke(
                        qr,
                        theme::cr(theme::RADIUS_CONTROL),
                        Stroke::new(1.0, if hot { theme::BORDER_STRONG } else { theme::BORDER }),
                        StrokeKind::Inside,
                    );
                    let ib = Rect::from_min_size(Pos2::new(qr.left() + 10.0, qr.center().y - 13.0), Vec2::splat(26.0));
                    let qtile = kit::tile_look(*qcol);
                    p.rect_filled(ib, theme::cr(theme::RADIUS_CONTROL), qtile.fill);
                    icons::paint(&p, *qicon, ib.center(), 14.0, qtile.icon);
                    p.text(
                        Pos2::new(ib.right() + 10.0, qr.center().y - 7.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, qtitle, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), qr.width() - 52.0),
                        theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                        theme::TEXT,
                    );
                    p.text(
                        Pos2::new(ib.right() + 10.0, qr.center().y + 8.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, qsub, &theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular), qr.width() - 52.0),
                        theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
                        theme::TEXT_TERTIARY,
                    );
                    widgets::record(ui, qkey, qr);
                    if qresp.clicked() {
                        acts.push(qact.clone());
                    }
                }
                y += drafts_strip_h;
            }

            // ---- folder tiles
            for (i, (fid, name)) in folders.iter().enumerate() {
                let r = if list {
                    Rect::from_min_size(Pos2::new(x0, y + i as f32 * theme::HOME_LIST_ROW_H), Vec2::new(inner_w, theme::HOME_LIST_ROW_H))
                } else {
                    Rect::from_min_size(
                        Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (theme::HOME_FOLDER_H + 12.0)),
                        Vec2::new(cw, theme::HOME_FOLDER_H),
                    )
                };
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let pid = project_id.clone().or_else(|| h.snap.projects.iter().find(|pr| pr.folders.iter().any(|fl| &fl.id == fid)).map(|pr| pr.id.clone()));
                let Some(pid) = pid else { continue };
                let resp = ui.interact(r, Id::new(("home-folder", fid)), Sense::click());
                let over = dragging && ptr.is_some_and(|q| r.contains(q));
                let fill = if over {
                    theme::BG_SELECTED_HOVER
                } else if resp.hovered() {
                    theme::BORDER_STRONG
                } else {
                    theme::BG_SECONDARY
                };
                p.rect_filled(
                    r,
                    theme::cr(theme::RADIUS_CONTROL),
                    if list {
                        if over || resp.hovered() {
                            fill
                        } else {
                            Color32::TRANSPARENT
                        }
                    } else {
                        fill
                    },
                );
                icons::paint(&p, Icon::Folder, Pos2::new(r.left() + 24.0, r.center().y), 20.0, theme::ICON_SECONDARY);
                p.text(
                    Pos2::new(r.left() + 44.0, r.center().y),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, name, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), r.width() - 60.0),
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                widgets::record(ui, &format!("home-folder-{name}"), r);
                if resp.double_clicked() || resp.clicked() {
                    acts.push(Act::Go(Section::Project(pid.clone(), Some(fid.clone()))));
                }
                if resp.secondary_clicked() {
                    menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::Folder(pid.clone(), fid.clone()), at));
                }
                zones.push((r.intersect(content), DropTo::Folder(pid, fid.clone())));
            }
            y += folder_rows + if folder_rows > 0.0 { 12.0 } else { 0.0 };

            if list && !files.is_empty() {
                let hr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, 28.0));
                let cols_x = list_columns(hr.right());
                /// (x, label, sort key and whether it sorts ascending first)
                type ListColumn<'a> = (f32, &'a str, Option<(super::Sort, bool)>);
                let col_defs: [ListColumn; 4] = [
                    (hr.left() + 60.0, "Name", Some((super::Sort::Name, true))),
                    (cols_x.0, "Location", None),
                    (cols_x.1, "Last modified", Some((super::Sort::LastModified, false))),
                    (cols_x.2, "Last viewed", Some((super::Sort::LastOpened, false))),
                ];
                for (x, label, sort_opt) in col_defs {
                    let col_r = Rect::from_min_size(Pos2::new(x - 4.0, hr.top()), Vec2::new(110.0, hr.height()));
                    let active = sort_opt.is_some_and(|(s, _)| h.sort == s);
                    if let Some(target_sort) = sort_opt {
                        let resp = ui.interact(col_r, Id::new(("home-list-col", label)), Sense::click());
                        let col = if active || resp.hovered() { theme::TEXT } else { theme::TEXT_TERTIARY };
                        p.text(
                            Pos2::new(x, hr.center().y),
                            Align2::LEFT_CENTER,
                            label,
                            theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium),
                            col,
                        );
                        if resp.clicked() {
                            new_sort = Some(target_sort);
                        }
                    } else {
                        p.text(
                            Pos2::new(x, hr.center().y),
                            Align2::LEFT_CENTER,
                            label,
                            theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium),
                            theme::TEXT_TERTIARY,
                        );
                    }
                }
                y += 28.0;
            }
            for (i, f) in files.iter().enumerate() {
                let r = if list {
                    Rect::from_min_size(Pos2::new(x0, y + i as f32 * theme::HOME_LIST_ROW_H), Vec2::new(inner_w, theme::HOME_LIST_ROW_H))
                } else {
                    Rect::from_min_size(Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (ch + gap_y)), Vec2::new(cw, ch))
                };
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let is_sel = selected.as_deref() == Some(f.id.as_str());
                let resp = ui.interact(r, Id::new(("home-card", &f.id)), Sense::click_and_drag());
                if resp.double_clicked() {
                    acts.push(Act::Open(f.id.clone()));
                } else if resp.clicked() {
                    new_selected = Some(f.id.clone());
                }
                if resp.secondary_clicked() {
                    new_selected = Some(f.id.clone());
                    menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::File(f.id.clone()), at));
                }
                if resp.drag_started() {
                    drag = Some((f.id.clone(), f.name.clone()));
                }
                let tex = h.thumbs.get(&ctx, lib, f, &mut budget);
                let board = h.kinds.get(lib, f, &mut budget) == designos_doc::DocumentKind::Board;
                let project = h.snap.project_name(&f.project).to_string();
                let missing = !super::library_file_disk_path(lib, &f.project, &f.id).is_file();
                let star_r = if list {
                    Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 12.0), Vec2::splat(24.0))
                } else {
                    Rect::from_min_size(Pos2::new(r.left() + cw - 32.0, r.top() + 8.0), Vec2::splat(24.0))
                };
                let star = ui.interact(star_r, Id::new(("home-star", &f.id)), Sense::click());
                if star.clicked() {
                    acts.push(Act::Star(f.id.clone(), !f.starred));
                }
                let mut paint = p.clone();
                if dragging && selected.as_deref() == Some(f.id.as_str()) {
                    paint.set_opacity(0.5);
                }
                if list {
                    list_row(&paint, r, f, &project, tex, board, missing, resp.hovered(), is_sel, now);
                } else {
                    card(&paint, r, f, &project, tex, board, missing, resp.hovered(), is_sel, now);
                }
                if missing {
                    let mut rm_right = r.right() - theme::HOME_PAD * 0.5;
                    let rm_cy = if list { r.center().y } else { r.bottom() - theme::HOME_PAD };
                    if button(ui, &paint, &mut rm_right, rm_cy, "Remove", Kind::SecondaryDestructive, &format!("home-remove-{}", f.name)).clicked() {
                        acts.push(Act::RemoveMissing(f.id.clone()));
                    }
                }
                widgets::record(ui, &format!("home-kind-{}", if board { "board" } else { "design" }), r);
                if resp.hovered() || star.hovered() || f.starred {
                    if star.hovered() || (!list && resp.hovered()) {
                        paint.circle_filled(star_r.center(), 12.0, theme::BG_OVERLAY);
                    }
                    paint_star(&paint, star_r.center(), f.starred, star.hovered());
                }
                widgets::record(ui, &format!("home-card-{}", f.name), r);
            }
            if !files.is_empty() {
                y += if list { files.len() as f32 * theme::HOME_LIST_ROW_H } else { files.len().div_ceil(cols) as f32 * (ch + gap_y) };
            }
            let _ = y;
        });
        crate::panels::scrollbar(ui, content, scroll, content_h);
        h.drop_rects.extend(zones);
        if let Some((sort_mode, alpha)) = new_sort {
            h.sort = sort_mode;
            h.sort_alpha = alpha;
        }
        if let Some(s) = new_selected {
            h.selected = Some(s);
        }
        if menu.is_some() {
            h.menu = menu;
        }
        if drag.is_some() {
            h.drag = drag;
        }
    }

    fn paint_star(p: &egui::Painter, c: Pos2, on: bool, hot: bool) {
        let col = if on {
            theme::TEXT_WARNING
        } else if hot {
            theme::ICON
        } else {
            theme::ICON_SECONDARY
        };
        if on {
            icons::paint(p, Icon::Star, c, 14.0, col);
            icons::paint(p, Icon::Star, c, 11.0, col);
            icons::paint(p, Icon::Star, c, 8.0, col);
        } else {
            icons::paint(p, Icon::Star, c, 14.0, col);
        }
    }

    type Tex = Option<(egui::TextureId, Vec2)>;

    fn thumb_area(p: &egui::Painter, area: Rect, tex: Tex, board: bool, radius: u8) {
        if board {
            p.rect_filled(area, theme::cr(radius), theme::rgb(theme::BOARD_BG));
            match tex {
                Some((t, _)) => {
                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                    p.add(egui::epaint::RectShape::filled(area, theme::cr(radius), Color32::WHITE).with_texture(t, uv));
                }
                None => {
                    let mut mesh = egui::Mesh::default();
                    let mut y = area.top() + 8.0;
                    while y < area.bottom() {
                        let mut x = area.left() + 8.0;
                        while x < area.right() {
                            mesh.add_colored_rect(Rect::from_center_size(Pos2::new(x, y), Vec2::splat(1.5)), theme::BOARD_DOT);
                            x += 16.0;
                        }
                        y += 16.0;
                    }
                    p.add(egui::Shape::mesh(mesh));
                    icons::paint(p, Icon::Sticky, area.center(), 24.0, theme::BOARD_DOT);
                }
            }
            return;
        }
        p.rect_filled(area, theme::cr(radius), theme::HOME_THUMB_BG);
        match tex {
            Some((t, size)) => {
                let pad = if area.width() > 80.0 { 6.0 } else { 3.0 };
                let inner = area.shrink(pad);
                let k = (inner.width() / size.x.max(1.0)).min(inner.height() / size.y.max(1.0)).min(2.0);
                let fit = Rect::from_center_size(inner.center(), size * k);
                let inner_cr = CornerRadius::same(if area.width() > 80.0 { 4 } else { 2 });
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                p.add(egui::epaint::RectShape::filled(fit, inner_cr, Color32::WHITE).with_texture(t, uv));
                p.rect_stroke(fit, inner_cr, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
            }
            None => icons::paint(p, Icon::File, area.center(), 24.0, theme::ICON_TERTIARY),
        }
    }

    fn board_badge(p: &egui::Painter, area: Rect) {
        let look = kit::count_badge_look(kit::Tone::Neutral);
        let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
        let w = widgets::text_width(p, "Board", &font) + 28.0;
        let r = Rect::from_min_size(Pos2::new(area.left() + 8.0, area.bottom() - 8.0 - theme::BADGE_SM), Vec2::new(w, theme::BADGE_SM));
        kit::paint_frame(p, r, theme::radius_full(r.height()).round() as u8, &look);
        icons::paint(p, Icon::Sticky, Pos2::new(r.left() + 10.0, r.center().y), 12.0, look.text);
        p.text(Pos2::new(r.left() + 19.0, r.center().y), Align2::LEFT_CENTER, "Board", font, look.text);
    }

    /// Draws a file's type tile (`20x20`, `RADIUS_CONTROL`): the kit's design, board, deck or spec bundle colours with
    /// the matching glyph.
    fn draw_file_type_badge(p: &egui::Painter, center: Pos2, name: &str, board: bool) {
        let low = name.to_ascii_lowercase();
        let (kind, icon) = if board {
            (TileKind::Board, Icon::Sticky)
        } else if low.contains("spec") || low.contains("v2") {
            (TileKind::Spec, Icon::Specs)
        } else if low.contains("deck") || low.contains("slide") {
            (TileKind::Deck, Icon::Play)
        } else {
            (TileKind::Design, Icon::Pen)
        };
        let tile = kit::tile_look(kind);
        let badge_r = Rect::from_center_size(center, Vec2::splat(20.0));
        p.rect_filled(badge_r, theme::cr(theme::RADIUS_CONTROL), tile.fill);
        icons::paint(p, icon, center, 12.0, tile.icon);
    }

    /// A grid file card: `RADIUS_XL` outer radius, `BG_SECONDARY` body, thumbnail stage,
    /// `BORDER` stroke (`BORDER_SELECTED` at `2px` when selected), and bottom metadata row.
    #[allow(clippy::too_many_arguments)]
    fn card(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, missing: bool, hover: bool, selected: bool, now: u64) {
        let card_r = theme::cr(theme::RADIUS_XL);
        p.rect_filled(r, card_r, theme::BG_SECONDARY);
        let thumb = Rect::from_min_size(r.min, Vec2::new(r.width(), theme::HOME_THUMB_H));
        thumb_area(p, thumb, tex, board, theme::RADIUS_XL);
        if board {
            board_badge(p, thumb);
        }
        widgets::hline(p, thumb.left() + 1.0, thumb.right() - 1.0, thumb.bottom(), theme::BORDER);
        let stroke = if selected {
            Stroke::new(2.0, theme::BORDER_SELECTED)
        } else if hover {
            Stroke::new(1.0, theme::BORDER_STRONG)
        } else {
            Stroke::new(1.0, theme::BORDER)
        };
        p.rect_stroke(r, card_r, stroke, StrokeKind::Inside);

        let badge_c = Pos2::new(r.left() + 18.0, thumb.bottom() + theme::SPACE_GROUP + theme::LINE_TEXT_SM * 0.5);
        draw_file_type_badge(p, badge_c, &f.name, board);

        let tx = r.left() + 34.0;
        let w = (r.width() - 42.0).max(40.0);
        let y_title = thumb.bottom() + theme::SPACE_GROUP + theme::LINE_TEXT_SM * 0.5;
        p.text(
            Pos2::new(tx, y_title),
            Align2::LEFT_CENTER,
            widgets::elide(p, &f.name, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), w),
            theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
            theme::TEXT,
        );
        let loc = if project.is_empty() { "Drafts" } else { project };
        let subtitle = if missing { "File not found".to_string() } else { format!("{} \u{b7} In {loc}", edited(now, f.modified)) };
        let sub_color = if missing { theme::TEXT_ERROR } else { theme::TEXT_TERTIARY };
        let y_meta = y_title + theme::LINE_TEXT_SM * 0.5 + theme::LINE_TEXT_XS * 0.5;
        p.text(
            Pos2::new(tx, y_meta),
            Align2::LEFT_CENTER,
            widgets::elide(p, &subtitle, &theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular), w),
            theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular),
            sub_color,
        );
    }

    fn list_columns(right: f32) -> (f32, f32, f32) {
        (right - 520.0, right - 340.0, right - 180.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn list_row(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, missing: bool, hover: bool, selected: bool, now: u64) {
        let row_state = if selected {
            State::Selected
        } else if hover {
            State::Hover
        } else {
            State::Default
        };
        let row_look = kit::row_look(row_state, Surface::Primary);
        kit::paint_frame(p, r, theme::RADIUS_CONTROL, &row_look);
        let th = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 5.0), Vec2::new(48.0, r.height() - 10.0));
        thumb_area(p, th, tex, board, theme::RADIUS_MARK);
        draw_file_type_badge(p, Pos2::new(r.left() + 70.0, r.center().y), &f.name, board);
        let cols = list_columns(r.right());
        let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
        p.text(
            Pos2::new(r.left() + 86.0, r.center().y),
            Align2::LEFT_CENTER,
            widgets::elide(p, &f.name, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), (cols.0 - r.left() - 96.0).max(40.0)),
            theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
            row_look.text,
        );
        let loc = if project.is_empty() { "Drafts" } else { project };
        p.text(Pos2::new(cols.0, r.center().y), Align2::LEFT_CENTER, widgets::elide(p, loc, &font, 150.0), font.clone(), theme::TEXT_TERTIARY);
        let mod_text = if missing { "File not found".to_string() } else { relative(now, f.modified) };
        let mod_color = if missing { theme::TEXT_ERROR } else { theme::TEXT_TERTIARY };
        p.text(Pos2::new(cols.1, r.center().y), Align2::LEFT_CENTER, mod_text, font.clone(), mod_color);
        p.text(Pos2::new(cols.2, r.center().y), Align2::LEFT_CENTER, relative(now, f.opened), font, theme::TEXT_TERTIARY);
    }

    fn trash_view(h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        if h.snap.trash.is_empty() {
            empty_state(h, ui, content);
            return;
        }
        let now = h.now_ms();
        let pad = theme::HOME_PAD;
        let row_h = 52.0;
        let note_h = 36.0;
        let content_h = pad + note_h + h.snap.trash.len() as f32 * row_h + pad;
        crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
        let scroll = h.scroll;
        let items = h.snap.trash.clone();
        let mut menu = None;
        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let x0 = content.left() + pad;
            let w = content.width() - pad * 2.0;
            let mut y = content.top() + pad - scroll;
            p.text(
                Pos2::new(x0, y + 10.0),
                Align2::LEFT_CENTER,
                "Items stay here until you empty the trash. Emptying it deletes them permanently.",
                theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular),
                theme::TEXT_SECONDARY,
            );
            y += note_h;
            for it in items.iter() {
                let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, row_h));
                y += row_h;
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let resp = ui.interact(r, Id::new(("home-trash-row", &it.id)), Sense::click());
                if resp.hovered() {
                    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                if resp.secondary_clicked() {
                    menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::TrashItem(it.id.clone()), at));
                }
                let is_file = it.kind == designos_library::TrashKind::File;
                icons::paint(&p, if is_file { Icon::File } else { Icon::Frame }, Pos2::new(r.left() + 24.0, r.center().y), 20.0, theme::ICON_SECONDARY);
                p.text(
                    Pos2::new(r.left() + 48.0, r.center().y - 9.0),
                    Align2::LEFT_CENTER,
                    &it.name,
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
                    theme::TEXT,
                );
                let from = match (&it.project_name, is_file) {
                    (Some(pn), true) => format!("Deleted {} \u{b7} from {pn}", relative(now, it.trashed)),
                    (_, false) => format!("Project \u{b7} deleted {}", relative(now, it.trashed)),
                    _ => format!("Deleted {}", relative(now, it.trashed)),
                };
                p.text(
                    Pos2::new(r.left() + 48.0, r.center().y + 9.0),
                    Align2::LEFT_CENTER,
                    from,
                    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular),
                    theme::TEXT_TERTIARY,
                );
                let mut right = r.right() - 8.0;
                let b = button(ui, &p, &mut right, r.center().y, "Restore", Kind::Secondary, &format!("home-restore-{}", it.name));
                if b.clicked() {
                    acts.push(Act::Restore(it.id.clone()));
                }
            }
        });
        if menu.is_some() {
            h.menu = menu;
        }
        crate::panels::scrollbar(ui, content, scroll, content_h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, project: &str, folder: Option<&str>, starred: bool, opened: u64, modified: u64) -> FileEntry {
        FileEntry {
            id: id.into(),
            name: name.into(),
            project: project.into(),
            folder: folder.map(Into::into),
            starred,
            created: 50,
            modified,
            opened,
            thumbnail: None,
            branches: vec![],
        }
    }

    #[test]
    fn ui3_home_sections_filters_and_sorts_files() {
        let mut home = Home::new();
        home.snap.files = vec![
            entry("f-design", "Design Token Sheet", "p1", None, false, 300, 100),
            entry("f-board", "Architecture Board", "p1", Some("fol1"), false, 200, 300),
            entry("f-spec", "US-04 Spec Bundle", "p1", None, true, 100, 200),
        ];

        assert_eq!(home.section, Section::Recents);
        assert_eq!(home.filter, FileFilter::AllFiles);
        assert_eq!(home.sort, Sort::LastOpened);
        assert_eq!(home.files().len(), 3);

        // Filter by Boards
        home.filter = FileFilter::Boards;
        let boards = home.files();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].id, "f-board");

        // Filter by SpecBundles
        home.filter = FileFilter::SpecBundles;
        let specs = home.files();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].id, "f-spec");

        // Filter by DesignFiles
        home.filter = FileFilter::DesignFiles;
        let designs = home.files();
        assert_eq!(designs.len(), 1);
        assert_eq!(designs[0].id, "f-design");

        // Reset filter, test Sort::Name
        home.filter = FileFilter::AllFiles;
        home.sort = Sort::Name;
        let sorted = home.files();
        assert_eq!(sorted[0].id, "f-board"); // "Architecture Board"
        assert_eq!(sorted[1].id, "f-design"); // "Design Token Sheet"
        assert_eq!(sorted[2].id, "f-spec"); // "US-04 Spec Bundle"

        // Drafts section (folder.is_none())
        home.section = Section::Drafts;
        assert_eq!(home.files().len(), 2);

        // Starred section
        home.section = Section::Starred;
        assert_eq!(home.files().len(), 1);
        assert_eq!(home.files()[0].id, "f-spec");

        // AllProjects (foldered files), Templates (spec/template files), Resources (token/system files)
        home.section = Section::AllProjects;
        assert_eq!(home.files().len(), 1);
        assert_eq!(home.files()[0].id, "f-board");

        home.section = Section::Templates;
        assert_eq!(home.files().len(), 1);
        assert_eq!(home.files()[0].id, "f-spec");

        home.section = Section::Resources;
        assert_eq!(home.files().len(), 1);
        assert_eq!(home.files()[0].id, "f-design");

        // Verify kind_filter and sort_alpha on Recents via visible_files()
        home.section = Section::Recents;
        home.kind_filter = Some(designos_doc::DocumentKind::Board);
        assert_eq!(home.visible_files().len(), 1);
        assert_eq!(home.visible_files()[0].id, "f-board");
        home.kind_filter = None;
        home.sort = Sort::LastOpened;
        home.sort_alpha = true;
        let alpha = home.visible_files();
        assert_eq!(alpha[0].id, "f-board");
        assert_eq!(alpha[1].id, "f-design");
        assert_eq!(alpha[2].id, "f-spec");
    }

    #[test]
    fn home_sections_do_not_fallback_to_recents_and_templates_create_real_files() {
        let mut home = Home::new();
        home.snap.files = vec![entry("f-welcome", "Welcome", "p1", None, false, 200, 200), entry("f-untitled", "Untitled", "p1", None, false, 300, 300)];

        home.section = Section::Recents;
        assert_eq!(home.visible_files().len(), 2);

        home.section = Section::Drafts;
        assert_eq!(home.visible_files().len(), 2);

        home.section = Section::AllProjects;
        assert!(home.visible_files().is_empty(), "AllProjects must not fall back to un-foldered Recents files");

        home.section = Section::Templates;
        assert!(home.visible_files().is_empty(), "Templates must not fall back to non-template Recents files");

        home.section = Section::Resources;
        assert!(home.visible_files().is_empty(), "Resources must not fall back to non-resource Recents files");

        // Verify Act::OpenStarterSample and Act::NewDeck against a temporary Library
        let tmp = std::env::temp_dir().join(format!("designos_home_tpl_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut app = App::new(designos_doc::Document::new("Init"));
        app.attach_library(tmp.clone());
        app.go_home();
        assert!(app.home.is_some());

        apply(&mut app, Act::OpenStarterSample);
        assert!(app.home.is_none(), "opening starter sample opens the file in the editor");
        assert_eq!(app.doc.name, "DESIGN-OS Studio Starter (1)");
        assert_eq!(app.doc.kind, designos_doc::DocumentKind::Design);

        app.go_home();
        apply(&mut app, Act::NewDeck);
        assert!(app.home.is_none(), "NewDeck opens the new slide deck in the editor");
        assert_eq!(app.doc.kind, designos_doc::DocumentKind::Deck);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn opening_malformed_library_file_warns_user_and_writes_backup_on_save() {
        let tmp = std::env::temp_dir().join(format!("designos_lib_open_repair_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let mut app = App::new(designos_doc::Document::new("Init"));
        app.attach_library(tmp.clone());
        let lib = app.lib.clone().expect("library attached");
        let proj = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("TestProj").unwrap());

        let mut damaged = designos_doc::Document::new("Damaged Library Doc");
        let page = damaged.add_page("Page 1");
        let r = damaged.alloc_id();
        damaged.insert(page, designos_doc::Node::rectangle(r, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
        let entry = lib.create_file(&proj.id, None, "Damaged Library Doc", &damaged).unwrap();

        // Corrupt the stored library file with a dangling child id and cycle
        damaged.nodes.get_mut(&page).unwrap().children.push(designos_doc::NodeId(7777));
        damaged.nodes.get_mut(&r).unwrap().children.push(page);
        damaged.nodes.get_mut(&page).unwrap().parent = Some(r);
        let original_bytes = damaged.to_json().unwrap().into_bytes();
        let disk_path = lib.path("projects").join(&proj.id).join("files").join(format!("{}.designos", entry.id));
        let bak_path = crate::io::backup_path(&disk_path);
        std::fs::write(&disk_path, &original_bytes).unwrap();

        app.toast = None;
        app.open_library_file(&entry.id);

        let toast = app.toast.as_ref().expect("malformed library open must surface a warning toast");
        assert!(toast.error, "repair warning toast should use warning/error styling");
        assert!(toast.text.starts_with("Opened Damaged Library Doc with a warning: repaired malformed document"), "unexpected toast: {}", toast.text);
        assert!(toast.text.contains("1 dangling ids"), "toast must describe the structural repair: {}", toast.text);
        assert!(!bak_path.exists(), "backup is deferred until save");

        app.save_library_file().expect("saving repaired library file succeeds");
        assert_eq!(std::fs::read(&bak_path).unwrap(), original_bytes, "saving a repaired library file must back up the untouched pre-repair bytes");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
