//! The projects library UI: the 1:1 Figma UI3 Home screen (file browser) shown at launch and
//! from File -> "Back to files", the dialogs it shares with the editor, and the
//! glue that opens library files in the editor.
//!
//! The data lives in `designos-library`; this module draws the Figma UI3 Home interface (`S04`)
//! and turns user interactions into [`Act`]s that [`apply`] runs against the library.

pub mod dialog;
mod session;
pub mod thumb;
pub mod time;
#[path = "../updater.rs"]
pub mod updater;

pub use dialog::{Dialog, Intent};
pub(crate) use session::doc_hash;
pub use session::{Autosave, LibFile};

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
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    MobileBanking,
    WebDashboard,
    DesignSystem,
    FigJamBoard,
    SlideDeck,
    VariablesKit,
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
    CreateFromTemplate(TemplateKind),
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
    for a in acts {
        apply(app, a);
    }
}

fn menus(h: &mut Home, ctx: &egui::Context, acts: &mut Vec<Act>) {
    use crate::widgets::menu::{popup_menu, MenuItem, MenuResult};
    if let Some(anchor) = h.filter_menu {
        let items: Vec<MenuItem> = FileFilter::ALL.iter().map(|f| MenuItem::new(f.label()).checked(*f == h.filter)).collect();
        match popup_menu(ctx, egui::Id::new("home-filter-menu"), anchor.left_bottom() + Vec2::new(0.0, 4.0), 168.0, &items, Some(anchor)) {
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
        match popup_menu(ctx, egui::Id::new("home-sort-menu"), anchor.left_bottom() + Vec2::new(0.0, 4.0), 168.0, &items, Some(anchor)) {
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
    match popup_menu(ctx, egui::Id::new("home-context-menu"), pos, theme::MENU_WIDTH, &items, None) {
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
                (MenuItem::new("Export .fig\u{2026}"), Act::ExportFig(id.clone())),
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
    let font = theme::font_body();
    let w = crate::widgets::text_width(&p, &name, &font).min(220.0) + 24.0;
    let r = Rect::from_min_size(ptr + Vec2::new(12.0, 8.0), Vec2::new(w, 28.0));
    p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_BRAND);
    let shown = crate::widgets::elide(&p, &name, &font, 200.0);
    p.text(r.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER, shown, font, egui::Color32::WHITE);
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    if released {
        if let Some((_, to)) = h.drop_rects.iter().rev().find(|(r, _)| r.contains(ptr)) {
            acts.push(Act::Drop(id, to.clone()));
        }
        h.drag = None;
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
        Act::CreateFromTemplate(kind) => {
            let (title, mut doc) = match kind {
                TemplateKind::MobileBanking => ("Marlow \u{2014} Mobile Banking Kit", crate::showcase::document()),
                TemplateKind::WebDashboard => {
                    let mut s = crate::showcase::library_samples();
                    let doc = s.remove(1);
                    ("Marlow \u{2014} SaaS Web Dashboard", doc)
                }
                TemplateKind::DesignSystem => {
                    let mut s = crate::showcase::library_samples();
                    let doc = s.remove(2);
                    ("Marlow \u{2014} Design System & Tokens", doc)
                }
                TemplateKind::FigJamBoard => ("Launch Brainstorm Board", crate::board::demo::document()),
                TemplateKind::SlideDeck => ("Product Exploration Deck", crate::decks::demo::document()),
                TemplateKind::VariablesKit => ("Design Token Sheet", crate::demo::variables_document()),
            };
            let target = app
                .home_target()
                .or_else(|| app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))))
                .or_else(|| lib.list_projects().first().map(|p| (p.id.clone(), None)))
                .or_else(|| ok(app, lib.create_project("My project")).map(|p| (p.id, None)));
            if let Some((proj_id, folder_id)) = target {
                doc.name = title.to_string();
                if let Some(entry) = ok(app, lib.create_file(&proj_id, folder_id.as_deref(), title, &doc)) {
                    thumb::refresh(&lib, &entry.id, &doc);
                    app.open_library_file(&entry.id);
                }
            }
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
            app.terminal.ensure_default_tab(app.file_path.as_deref());
            changed = false;
        }
        Act::Import => {
            let paths = rfd::FileDialog::new()
                .add_filter("Design files", &["designos", "fig", "json", "svg"])
                .add_filter("design:os studio", &["designos"])
                .add_filter("Figma file", &["fig"])
                .add_filter("SVG", &["svg"])
                .add_filter("JSON", &["json"])
                .pick_files();
            if let Some(paths) = paths {
                let target = match app.home_target() {
                    Some(t) => Some(t),
                    None => app.home.as_ref().and_then(|h| h.snap.projects.first().map(|p| (p.id.clone(), None))),
                };
                let target = match target {
                    Some(t) => Some(t),
                    None => ok(app, lib.create_project("My project")).map(|p| (p.id, None)),
                };
                if let Some((p, f)) = target {
                    app.import_into_library(&paths, &p, f.as_deref());
                }
            }
        }
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
            if let Ok(doc) = lib.load_file(&id) {
                match designos_fig::export_fig(&doc) {
                    Ok(bytes) => {
                        let safe_name: String = doc.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
                        let stem = if safe_name.trim_matches('_').is_empty() { "export".to_string() } else { safe_name };
                        let out_dir = lib.path("exports");
                        let _ = std::fs::create_dir_all(&out_dir);
                        let out_path = out_dir.join(format!("{stem}.fig"));
                        let saved_path = if std::fs::write(&out_path, &bytes).is_ok() {
                            out_path
                        } else {
                            let tmp_path = std::env::temp_dir().join(format!("{stem}.fig"));
                            let _ = std::fs::write(&tmp_path, &bytes);
                            tmp_path
                        };
                        app.show_toast(format!("Exported {} ({} bytes .fig v106) -> {}", doc.name, bytes.len(), saved_path.display()), false);
                    }
                    Err(e) => app.show_toast(format!("Export .fig failed: {e}"), true),
                }
            }
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
// Inline Left Sidebar (`264px`, `#18181B` Zinc-900 background, `#27272A` active pill)
// ─────────────────────────────────────────────────────────────────────────────
mod sidebar {
    use super::{Act, DropTo, Home, Section, Target};
    use crate::icons::{self, Icon};
    use crate::theme;
    use crate::widgets;
    use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

    const ROW_H: f32 = 28.0;
    const INDENT: f32 = 16.0;

    // Unified Zinc Dark Palette tokens (`V1.2` / `V2.1`)
    const UI3_SIDEBAR_BG: Color32 = Color32::from_rgb(0x18, 0x18, 0x1B);
    const UI3_SURFACE: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x22);
    const UI3_PILL_ACTIVE: Color32 = Color32::from_rgb(0x27, 0x27, 0x2A);
    const UI3_PILL_HOVER: Color32 = Color32::from_rgb(0x23, 0x23, 0x29);
    const UI3_SEARCH_BG: Color32 = UI3_SURFACE;
    const UI3_BORDER: Color32 = Color32::from_rgb(0x27, 0x27, 0x2A);
    const UI3_TEXT_PRI: Color32 = theme::TEXT;
    const UI3_TEXT_SEC: Color32 = theme::TEXT_SECONDARY;
    const UI3_BLUE: Color32 = Color32::from_rgb(0x0D, 0x99, 0xFF);

    pub fn show(h: &mut Home, ui: &mut Ui, rect: Rect, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, UI3_SIDEBAR_BG);
        widgets::vline(&p, rect.right() - theme::BORDER_WIDTH, rect.top(), rect.bottom(), UI3_BORDER);
        let x0 = rect.left();
        let w = rect.width() - theme::BORDER_WIDTH;
        let mut y = rect.top();

        // 1. Top Workspace Switcher Header Row (`V1.1`: Workspace Avatar + "Personal Workspace" + Version Pill)
        let ws_rect = Rect::from_min_size(Pos2::new(x0 + 8.0, y + 10.0), Vec2::new(w - 44.0, 32.0));
        let ws_resp = ui.interact(ws_rect, Id::new("home-workspace-header"), Sense::click());
        widgets::record(ui, "home-workspace-header", ws_rect);
        if ws_resp.hovered() {
            p.rect_filled(ws_rect, CornerRadius::same(6), UI3_PILL_HOVER);
        }
        let avatar_r = Rect::from_center_size(Pos2::new(x0 + 22.0, ws_rect.center().y), Vec2::splat(20.0));
        p.rect_filled(avatar_r, CornerRadius::same(5), Color32::from_rgb(0x27, 0x27, 0x2A));
        p.rect_stroke(avatar_r, CornerRadius::same(5), Stroke::new(1.0, Color32::from_rgb(0x3F, 0x3F, 0x46)), StrokeKind::Inside);
        icons::paint(&p, Icon::Folder, avatar_r.center(), 12.0, theme::BG_BRAND);
        let ws_font = egui::FontId::new(12.0, egui::FontFamily::Name(theme::FAMILY_MEDIUM.into()));
        p.text(Pos2::new(x0 + 38.0, ws_rect.center().y), Align2::LEFT_CENTER, "Personal Workspace", ws_font, UI3_TEXT_PRI);
        let ver = format!("v{}", env!("CARGO_PKG_VERSION"));
        let mut ver_font = theme::font_mono();
        ver_font.size = 10.0;
        let ver_w = widgets::text_width(&p, &ver, &ver_font) + 10.0;
        let ver_r = Rect::from_center_size(Pos2::new(ws_rect.right() - ver_w * 0.5 - 4.0, ws_rect.center().y), Vec2::new(ver_w, 16.0));
        p.rect_filled(ver_r, theme::cr_pill(ver_r.height()), UI3_PILL_ACTIVE);
        p.text(ver_r.center(), Align2::CENTER_CENTER, ver, ver_font, UI3_TEXT_SEC);
        if ws_resp.clicked() {
            acts.push(Act::Go(Section::Recents));
        }

        // Right notification/activity icon button
        let bell_r = Rect::from_min_size(Pos2::new(x0 + w - 34.0, y + 14.0), Vec2::splat(24.0));
        let _ = widgets::icon_button(ui, bell_r, "home-notifications", Icon::Eye, false, "Workspace activity");
        widgets::record(ui, "home-notifications", bell_r);
        y += 48.0;

        // 2. Search Bar (`height: 32px`, `#2C2C2C` fill, `6px` radius)
        let sr = Rect::from_min_size(Pos2::new(x0 + 12.0, y), Vec2::new(w - 24.0, 32.0));
        p.rect_filled(sr, theme::cr(theme::RADIUS_CONTROL), UI3_SEARCH_BG);
        p.rect_stroke(sr, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);
        icons::paint(&p, Icon::Search, Pos2::new(sr.left() + 14.0, sr.center().y), 14.0, theme::ICON_SECONDARY);
        let tr = Rect::from_min_max(Pos2::new(sr.left() + 28.0, sr.top()), sr.max - Vec2::new(8.0, 0.0));
        let tid = Id::new("home-search");
        let mut text = h.search.clone();
        let te = egui::TextEdit::singleline(&mut text)
            .id(tid)
            .font(theme::font_body())
            .text_color(theme::TEXT)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .vertical_align(egui::Align::Center)
            .hint_text(egui::RichText::new("Search").font(theme::font_body()).color(theme::TEXT_TERTIARY))
            .min_size(tr.size())
            .desired_width(tr.width());
        ui.put(tr, te);
        widgets::record(ui, "home-search", sr);
        if text != h.search {
            h.search = text;
            h.selected = None;
            h.scroll = 0.0;
        }
        y += 32.0 + 10.0;

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
        widgets::hline(&p, x0 + 12.0, x0 + w - 12.0, y, UI3_BORDER);
        y += 8.0;

        // 4. Workspace Section Header (`FILES & PROJECTS`) + `+` New Project button
        let ws_badge_c = Pos2::new(x0 + 20.0, y + 12.0);
        p.circle_filled(ws_badge_c, 7.0, UI3_BLUE.gamma_multiply(0.25));
        p.text(ws_badge_c, Align2::CENTER_CENTER, "P", theme::font_caption_strong(), UI3_BLUE);
        p.text(Pos2::new(x0 + 32.0, y + 12.0), Align2::LEFT_CENTER, "FILES & PROJECTS", theme::font_caption_strong(), theme::TEXT_SECONDARY);
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
                ui.painter().text(Pos2::new(x0 + 20.0, list.top() + 16.0), Align2::LEFT_CENTER, "No projects yet", theme::font_body(), theme::TEXT_TERTIARY);
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
                    icons::paint(ui.painter(), if open { Icon::ChevronDown } else { Icon::ChevronRight }, c, 12.0, theme::ICON_TERTIARY);
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
        widgets::hline(&p, x0 + 12.0, x0 + w - 12.0, sy, UI3_BORDER);
        sy += 6.0;
        let star_r = Rect::from_min_size(Pos2::new(x0 + 8.0, sy), Vec2::new(w - 16.0, ROW_H));
        let star_selected = h.section == Section::Starred && !h.searching();
        let star_resp = row(ui, star_r, Id::new(("home-nav", "starred")), Icon::Star, "Starred", 0.0, star_selected, false, "starred");
        icons::paint(
            &p,
            if h.starred_expanded { Icon::ChevronDown } else { Icon::ChevronRight },
            Pos2::new(star_r.right() - 12.0, star_r.center().y),
            11.0,
            theme::ICON_TERTIARY,
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
                    p.rect_filled(sr_row, CornerRadius::same(5), UI3_PILL_HOVER);
                }
                icons::paint(&p, Icon::File, Pos2::new(sr_row.left() + 12.0, sr_row.center().y), 13.0, UI3_BLUE);
                let stxt = widgets::elide(&p, &sf.name, &theme::font_caption(), sr_row.width() - 28.0);
                p.text(Pos2::new(sr_row.left() + 24.0, sr_row.center().y), Align2::LEFT_CENTER, stxt, theme::font_caption(), theme::TEXT_SECONDARY);
                if sresp.clicked() {
                    acts.push(Act::Open(sf.id.clone()));
                }
                sy += 25.0;
            }
        }

        // 7. Auto-Update & Upstream Figma Watchdog Status Card (`US-09` / `DIR-11`)
        let trash_h = 46.0;
        let updater_r = Rect::from_min_size(Pos2::new(x0 + 8.0, rect.bottom() - trash_h - 46.0), Vec2::new(w - 16.0, 40.0));
        super::updater::show_sidebar_card_in_rect(ui, updater_r, &mut h.updater);

        // 8. Bottom `Trash` Row (`V3.3` / `D85`: separate count pill badge instead of parenthetical string)
        widgets::hline(&p, x0, x0 + w, rect.bottom() - trash_h, UI3_BORDER);
        let tr = Rect::from_min_size(Pos2::new(x0 + 8.0, rect.bottom() - trash_h + 9.0), Vec2::new(w - 16.0, ROW_H));
        let n = h.snap.trash.len();
        let trash_resp = row(ui, tr, Id::new("home-trash"), Icon::Trash, "Trash", 0.0, h.section == Section::Trash && !h.searching(), false, "trash");
        if n > 0 {
            let badge_r = Rect::from_center_size(Pos2::new(tr.right() - 22.0, tr.center().y), Vec2::new(22.0, 16.0));
            p.rect_filled(badge_r, CornerRadius::same(8), Color32::from_rgb(0x27, 0x27, 0x2A));
            p.text(badge_r.center(), Align2::CENTER_CENTER, n.to_string(), theme::font_mono(), UI3_TEXT_SEC);
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

    /// One sidebar row: icon, label, hover / `#27272A` selected / drop-target highlight.
    #[allow(clippy::too_many_arguments)]
    fn row(ui: &mut Ui, r: Rect, id: Id, icon: Icon, label: &str, indent: f32, selected: bool, drop_hover: bool, record: &str) -> Response {
        let resp = ui.interact(r, id, Sense::click());
        let p = ui.painter();
        if drop_hover {
            p.rect_filled(r, CornerRadius::same(6), theme::BG_SELECTED_HOVER);
        } else if selected {
            p.rect_filled(r, CornerRadius::same(6), UI3_PILL_ACTIVE);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(6), UI3_PILL_HOVER);
        }
        let ix = r.left() + 12.0 + indent;
        icons::paint(p, icon, Pos2::new(ix + 14.0, r.center().y), 15.0, if selected { Color32::WHITE } else { theme::ICON_SECONDARY });
        let font = if selected { theme::font_body_strong() } else { theme::font_body() };
        let shown = widgets::elide(p, label, &font, r.right() - (ix + 30.0) - 8.0);
        p.text(Pos2::new(ix + 30.0, r.center().y), Align2::LEFT_CENTER, shown, font, if selected { Color32::WHITE } else { theme::TEXT });
        widgets::record(ui, &format!("home-{record}"), r);
        resp
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline Main Area (Top Header, Creation Strip, Filter/Sort Bar, Grid/List Cards)
// ─────────────────────────────────────────────────────────────────────────────
mod main_view {
    use super::time::{edited, relative, wall_ms};
    use super::{Act, DropTo, Home, Section, Target, TemplateKind, ViewMode};
    use crate::icons::{self, Icon};
    use crate::theme;
    use crate::widgets;
    use designos_library::{FileEntry, Library};
    use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

    // Unified Zinc Dark Mode tokens (`V1.2` / `V2.1`)
    const UI3_MAIN_BG: Color32 = Color32::from_rgb(0x12, 0x12, 0x15);
    const UI3_SURFACE: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x22);
    const UI3_BORDER: Color32 = Color32::from_rgb(0x27, 0x27, 0x2A);
    const UI3_BLUE: Color32 = Color32::from_rgb(0x0D, 0x99, 0xFF);
    const UI3_BLUE_HOVER: Color32 = Color32::from_rgb(0x0B, 0x84, 0xE0);
    const UI3_TEAL: Color32 = Color32::from_rgb(0x14, 0xAE, 0x5C);
    const UI3_AMBER: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
    const UI3_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x3D);

    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        PrimaryBlue,
        Secondary,
        Danger,
    }

    pub fn show(lib: &Library, h: &mut Home, ui: &mut Ui, rect: Rect, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, UI3_MAIN_BG);
        let pad = theme::HOME_PAD;
        let trash = h.section == Section::Trash && !h.searching();

        // 1. Top Header & Creation Strip (`+ Design file`, `+ FigJam board`, `+ Spec bundle`, `Import`, context `+ New folder`)
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), theme::HOME_HEADER_H));
        breadcrumb(h, ui, &p, Pos2::new(rect.left() + pad, head.center().y), acts);
        let mut right = rect.right() - pad;
        if trash {
            if !h.snap.trash.is_empty() {
                let r = button(ui, &p, &mut right, head.center().y, "Empty trash", Kind::Danger, "home-empty-trash");
                if r.clicked() {
                    acts.push(Act::EmptyTrash);
                }
            }
        } else {
            let r = button(ui, &p, &mut right, head.center().y, "+ Design file", Kind::PrimaryBlue, "home-new-file");
            if r.clicked() {
                acts.push(Act::NewFile);
            }
            right -= 8.0;
            let r = button(ui, &p, &mut right, head.center().y, "+ FigJam board", Kind::Secondary, "home-new-board");
            if r.clicked() {
                acts.push(Act::NewBoard);
            }
            right -= 8.0;
            let r = button(ui, &p, &mut right, head.center().y, "+ Spec bundle", Kind::Secondary, "home-new-spec");
            if r.clicked() {
                acts.push(Act::OpenSpecs);
            }
            right -= 8.0;
            let r = button(ui, &p, &mut right, head.center().y, "Import", Kind::Secondary, "home-import");
            if r.clicked() {
                acts.push(Act::Import);
            }
            if !h.searching() {
                let folder_target = match &h.section {
                    Section::Project(pid, folder) => Some((pid.clone(), folder.clone())),
                    Section::AllProjects => h.snap.projects.first().map(|pr| (pr.id.clone(), None)),
                    _ => None,
                };
                if let Some((pid, parent_folder)) = folder_target {
                    right -= 8.0;
                    let r = button(ui, &p, &mut right, head.center().y, "+ New folder", Kind::Secondary, "home-new-folder");
                    if r.clicked() {
                        acts.push(Act::NewFolder(pid, parent_folder));
                    }
                }
            }
        }
        let mut top = head.bottom();

        // 2. Sub-Header Filter & Sort Bar (`Filter: All files v`, `Sort: Last viewed v`, Grid/List toggle)
        if !trash {
            let y = top + 4.0;
            let filter_lbl = format!("Filter: {}", h.filter.label());
            let fw = widgets::text_width(&p, &filter_lbl, &theme::font_body()) + 36.0;
            let fr = Rect::from_min_size(Pos2::new(rect.left() + pad, y), Vec2::new(fw, 28.0));
            let fresp = ui.interact(fr, Id::new("home-filter"), Sense::click());
            p.rect_filled(fr, theme::cr_pill(fr.height()), if h.filter_menu.is_some() || fresp.hovered() { UI3_BORDER } else { UI3_SURFACE });
            p.rect_stroke(fr, theme::cr_pill(fr.height()), Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);
            p.text(Pos2::new(fr.left() + 10.0, fr.center().y), Align2::LEFT_CENTER, &filter_lbl, theme::font_body(), theme::TEXT);
            icons::paint(&p, Icon::ChevronDown, Pos2::new(fr.right() - 12.0, fr.center().y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
            widgets::record(ui, "home-filter", fr);
            if fresp.clicked() {
                h.filter_menu = if h.filter_menu.is_some() { None } else { Some(fr) };
                h.sort_menu = None;
            }

            let sort_lbl = format!("Sort: {}", h.sort.label());
            let sw = widgets::text_width(&p, &sort_lbl, &theme::font_body()) + 36.0;
            let sr = Rect::from_min_size(Pos2::new(fr.right() + 8.0, y), Vec2::new(sw, 28.0));
            let sresp = ui.interact(sr, Id::new("home-sort"), Sense::click());
            p.rect_filled(sr, theme::cr_pill(sr.height()), if h.sort_menu.is_some() || sresp.hovered() { UI3_BORDER } else { UI3_SURFACE });
            p.rect_stroke(sr, theme::cr_pill(sr.height()), Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);
            p.text(Pos2::new(sr.left() + 10.0, sr.center().y), Align2::LEFT_CENTER, &sort_lbl, theme::font_body(), theme::TEXT);
            icons::paint(&p, Icon::ChevronDown, Pos2::new(sr.right() - 12.0, sr.center().y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
            widgets::record(ui, "home-sort", sr);
            if sresp.clicked() {
                h.sort_menu = if h.sort_menu.is_some() { None } else { Some(sr) };
                h.filter_menu = None;
            }

            let mut x = rect.right() - pad;
            for (mode, icon, key, tip) in
                [(ViewMode::List, Icon::ListView, "home-view-list", "List view"), (ViewMode::Grid, Icon::GridView, "home-view-grid", "Grid view")]
            {
                let br = Rect::from_min_size(Pos2::new(x - 28.0, y), Vec2::splat(28.0));
                x -= 28.0 + 4.0;
                if widgets::icon_button(ui, br, key, icon, h.view == mode, tip).clicked() {
                    acts.push(Act::SetView(mode));
                }
                widgets::record(ui, key, br);
            }
            top += 38.0;
        }
        widgets::hline(&p, rect.left(), rect.right(), top, UI3_BORDER);
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

    /// Title, or clickable "Project / Folder / ..." path inside a project.
    fn breadcrumb(h: &Home, ui: &mut Ui, p: &egui::Painter, at: Pos2, acts: &mut Vec<Act>) {
        let font = theme::font_title();
        let mut segs: Vec<(String, Section)> = vec![];
        match &h.section {
            Section::Project(pid, folder) if !h.searching() => {
                segs.push((h.snap.project_name(pid).to_string(), Section::Project(pid.clone(), None)));
                if let Some(pr) = h.snap.projects.iter().find(|x| &x.id == pid) {
                    let mut chain = vec![];
                    let mut cur = folder.clone();
                    while let Some(fid) = cur {
                        let Some(f) = pr.folders.iter().find(|x| x.id == fid) else { break };
                        chain.push((f.name.clone(), Section::Project(pid.clone(), Some(f.id.clone()))));
                        cur = f.parent.clone();
                    }
                    chain.reverse();
                    segs.extend(chain);
                }
            }
            _ => segs.push((h.title(), h.section.clone())),
        }
        let mut x = at.x;
        let last = segs.len() - 1;
        for (i, (label, section)) in segs.into_iter().enumerate() {
            let w = widgets::text_width(p, &label, &font);
            let r = Rect::from_min_size(Pos2::new(x, at.y - 14.0), Vec2::new(w, 28.0));
            let color = if i == last { theme::TEXT } else { theme::TEXT_SECONDARY };
            if i < last {
                let resp = ui.interact(r, Id::new(("home-crumb", i)), Sense::click());
                if resp.hovered() {
                    p.rect_filled(r.expand2(Vec2::new(4.0, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                if resp.clicked() {
                    acts.push(Act::Go(section));
                }
            }
            p.text(Pos2::new(x, at.y), Align2::LEFT_CENTER, &label, font.clone(), color);
            x += w;
            if i < last {
                p.text(Pos2::new(x + 8.0, at.y), Align2::LEFT_CENTER, "/", font.clone(), theme::TEXT_TERTIARY);
                x += 24.0;
            }
        }
    }

    /// Right-aligned button; moves `right` to its left edge.
    fn button(ui: &mut Ui, p: &egui::Painter, right: &mut f32, cy: f32, label: &str, kind: Kind, record: &str) -> Response {
        let font = theme::font_body_strong();
        let w = widgets::text_width(p, label, &font) + 26.0;
        let r = Rect::from_min_size(Pos2::new(*right - w, cy - 16.0), Vec2::new(w, 32.0));
        *right = r.left();
        let resp = ui.interact(r, Id::new(("home-button", record)), Sense::click());
        let radius = theme::cr_pill(r.height());
        let down = resp.is_pointer_button_down_on();
        match kind {
            Kind::PrimaryBlue => {
                let bg = if down || resp.hovered() { UI3_BLUE_HOVER } else { UI3_BLUE };
                p.rect_filled(r, radius, bg);
                p.text(r.center(), Align2::CENTER_CENTER, label, font, Color32::WHITE);
            }
            Kind::Secondary => {
                let bg = if down || resp.hovered() { UI3_BORDER } else { UI3_SURFACE };
                p.rect_filled(r, radius, bg);
                p.rect_stroke(r, radius, Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);
                p.text(r.center(), Align2::CENTER_CENTER, label, font, theme::TEXT);
            }
            Kind::Danger => {
                let bg = if resp.hovered() { theme::DANGER.gamma_multiply(0.2) } else { Color32::TRANSPARENT };
                p.rect_filled(r, radius, bg);
                p.rect_stroke(r, radius, Stroke::new(1.0, theme::DANGER), StrokeKind::Inside);
                p.text(r.center(), Align2::CENTER_CENTER, label, font, theme::DANGER);
            }
        }
        widgets::record(ui, record, r);
        resp
    }

    fn section_heading(p: &egui::Painter, x: f32, y: f32, title: &str, subtitle: Option<&str>) -> f32 {
        p.text(Pos2::new(x, y + 10.0), Align2::LEFT_CENTER, title, theme::font_body_strong(), theme::TEXT);
        if let Some(sub) = subtitle {
            let tw = widgets::text_width(p, title, &theme::font_body_strong());
            p.text(Pos2::new(x + tw + 10.0, y + 10.0), Align2::LEFT_CENTER, sub, theme::font_caption(), theme::TEXT_SECONDARY);
        }
        28.0
    }

    /// Dedicated view for `Section::Templates` (`"Templates & Specs"`):
    /// 6 starter template & spec cards + user's matching template/spec files below.
    fn templates_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let now = wall_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(240.0);
        let (tcw, tch, tgap_x, tgap_y) = (240.0_f32, 188.0_f32, 16.0_f32, 16.0_f32);
        let tcols = (((inner_w + tgap_x) / (tcw + tgap_x)).floor() as usize).max(1);
        let template_rows = 6usize.div_ceil(tcols) as f32 * (tch + tgap_y);

        let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
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

        /// (title, badge, accent, icon, description, button label, action, record key)
        type TemplateCard<'a> = (&'a str, &'a str, Color32, Icon, &'a str, &'a str, Act, &'a str);
        let cards: [TemplateCard; 6] = [
            (
                "Marlow \u{2014} Mobile Banking Kit",
                "Design Kit",
                UI3_BLUE,
                Icon::Pen,
                "iOS finance screens, components & prototype flows",
                "Use template",
                Act::CreateFromTemplate(TemplateKind::MobileBanking),
                "home-template-mobile",
            ),
            (
                "Marlow \u{2014} SaaS Web Dashboard",
                "Web App",
                UI3_TEAL,
                Icon::Frame,
                "Analytics overview, tables, sidebar & dark/light cards",
                "Use template",
                Act::CreateFromTemplate(TemplateKind::WebDashboard),
                "home-template-web",
            ),
            (
                "Marlow \u{2014} Design System & Tokens",
                "Component Library",
                UI3_AMBER,
                Icon::Component,
                "Reusable buttons, inputs, badges & semantic variables",
                "Use template",
                Act::CreateFromTemplate(TemplateKind::DesignSystem),
                "home-template-system",
            ),
            (
                "Launch Brainstorm Board",
                "FigJam Board",
                UI3_ORANGE,
                Icon::Sticky,
                "Sticky notes, connectors, sections & retro grid",
                "Use template",
                Act::CreateFromTemplate(TemplateKind::FigJamBoard),
                "home-template-board",
            ),
            (
                "Product Exploration Deck",
                "Slide Deck",
                UI3_BLUE,
                Icon::Play,
                "16:9 presentation slides with speaker notes & transitions",
                "Use template",
                Act::CreateFromTemplate(TemplateKind::SlideDeck),
                "home-template-deck",
            ),
            (
                "PRODUCT:OS Spec Bundle v2",
                "Spec Driven",
                UI3_TEAL,
                Icon::Specs,
                "Interactive PRD, User Stories, Architecture & Gates G1\u{2013}G5",
                "Open Spec Hub",
                Act::OpenSpecs,
                "home-template-specs",
            ),
        ];

        crate::panels::clipped(ui, content, |ui| {
            let p = ui.painter().clone();
            let x0 = content.left() + pad;
            let mut y = content.top() + pad - scroll;

            y += section_heading(&p, x0, y, "Starter Templates & UI Kits", Some("Click any kit to create an editable file in your workspace"));

            for (i, (title, badge, accent, icon, desc, btn_lbl, act, record)) in cards.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + (i % tcols) as f32 * (tcw + tgap_x), y + (i / tcols) as f32 * (tch + tgap_y)), Vec2::new(tcw, tch));
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                let resp = ui.interact(r, Id::new(("home-tpl-card", *record)), Sense::click());
                let hot = resp.hovered();
                p.rect_filled(r, CornerRadius::same(8), UI3_SURFACE);
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(if hot { 1.5 } else { 1.0 }, if hot { *accent } else { UI3_BORDER }), StrokeKind::Inside);

                // Top preview banner (72px)
                let banner = Rect::from_min_size(r.min + Vec2::new(1.0, 1.0), Vec2::new(r.width() - 2.0, 72.0));
                p.rect_filled(banner, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, accent.gamma_multiply(0.14));
                let icon_box = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 14.0), Vec2::splat(28.0));
                p.rect_filled(icon_box, CornerRadius::same(6), *accent);
                icons::paint(&p, *icon, icon_box.center(), 15.0, Color32::WHITE);

                // Category pill on top-right of banner
                let bw = widgets::text_width(&p, badge, &theme::font_caption_strong()) + 14.0;
                let pill_r = Rect::from_min_size(Pos2::new(r.right() - bw - 12.0, r.top() + 16.0), Vec2::new(bw, 20.0));
                p.rect_filled(pill_r, theme::cr_pill(pill_r.height()), accent.gamma_multiply(0.24));
                p.text(pill_r.center(), Align2::CENTER_CENTER, *badge, theme::font_caption_strong(), *accent);

                // Subtle decorative wireframe bars inside banner
                let bar_y = banner.bottom() - 14.0;
                p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 12.0, bar_y), Vec2::new(64.0, 4.0)), CornerRadius::same(2), accent.gamma_multiply(0.45));
                p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 82.0, bar_y), Vec2::new(40.0, 4.0)), CornerRadius::same(2), accent.gamma_multiply(0.25));

                // Title & description
                p.text(
                    Pos2::new(r.left() + 12.0, banner.bottom() + 16.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, title, &theme::font_body_strong(), r.width() - 24.0),
                    theme::font_body_strong(),
                    theme::TEXT,
                );
                let galley = p.layout((*desc).to_string(), theme::font_caption(), theme::TEXT_SECONDARY, r.width() - 24.0);
                p.galley(Pos2::new(r.left() + 12.0, banner.bottom() + 28.0), galley, theme::TEXT_SECONDARY);

                // Bottom CTA button
                let btn_r = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.bottom() - 36.0), Vec2::new(r.width() - 24.0, 26.0));
                let btn_bg = if hot { *accent } else { UI3_MAIN_BG };
                p.rect_filled(btn_r, theme::cr_pill(btn_r.height()), btn_bg);
                p.rect_stroke(btn_r, theme::cr_pill(btn_r.height()), Stroke::new(1.0, if hot { *accent } else { UI3_BORDER }), StrokeKind::Inside);
                p.text(btn_r.center(), Align2::CENTER_CENTER, *btn_lbl, theme::font_caption_strong(), if hot { Color32::WHITE } else { theme::TEXT });

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
        let now = wall_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(260.0);

        let (pcw, pch, pgap_x, pgap_y) = (260.0_f32, 168.0_f32, 16.0_f32, 16.0_f32);
        let pcols = (((inner_w + pgap_x) / (pcw + pgap_x)).floor() as usize).max(1);
        let proj_total = h.snap.projects.len() + 1; // +1 for `+ New project` card
        let proj_rows_h = proj_total.div_ceil(pcols) as f32 * (pch + pgap_y);

        let all_folders: Vec<(String, String, String, String)> =
            h.snap.projects.iter().flat_map(|pr| pr.folders.iter().map(move |fl| (pr.id.clone(), pr.name.clone(), fl.id.clone(), fl.name.clone()))).collect();
        let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
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
                let plus_r = Rect::from_min_size(Pos2::new(r.right() - 74.0, r.top() + 8.0), Vec2::new(66.0, 22.0));
                let plus_resp = ui.interact(plus_r, Id::new(("home-proj-new-folder", &pr.id)), Sense::click());
                let card_resp = ui.interact(r, Id::new(("home-proj-card", &pr.id)), Sense::click());
                let over = dragging && ptr.is_some_and(|q| r.contains(q));
                let hot = card_resp.hovered() || plus_resp.hovered();

                p.rect_filled(r, CornerRadius::same(8), if over { theme::BG_SELECTED_HOVER } else { UI3_SURFACE });
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(if hot { 1.5 } else { 1.0 }, if hot { UI3_BLUE } else { UI3_BORDER }), StrokeKind::Inside);

                // Top 2x2 preview grid (96px tall)
                let preview = Rect::from_min_size(r.min + Vec2::new(8.0, 8.0), Vec2::new(r.width() - 16.0, 92.0));
                p.rect_filled(preview, CornerRadius::same(6), UI3_MAIN_BG);
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
                        p.rect_filled(cell, CornerRadius::same(4), UI3_SURFACE.gamma_multiply(0.55));
                    }
                }

                // Top-right `+ Folder` mini button
                p.rect_filled(plus_r, CornerRadius::same(5), if plus_resp.hovered() { UI3_BLUE } else { Color32::from_black_alpha(170) });
                p.text(plus_r.center(), Align2::CENTER_CENTER, "+ Folder", theme::font_caption_strong(), Color32::WHITE);
                widgets::record(ui, &format!("home-project-new-folder-{}", pr.name), plus_r);

                // Bottom metadata
                let icon_c = Pos2::new(r.left() + 18.0, preview.bottom() + 24.0);
                p.rect_filled(Rect::from_center_size(icon_c, Vec2::splat(22.0)), CornerRadius::same(5), UI3_BLUE.gamma_multiply(0.22));
                icons::paint(&p, Icon::Frame, icon_c, 13.0, UI3_BLUE);

                p.text(
                    Pos2::new(r.left() + 36.0, preview.bottom() + 16.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, &pr.name, &theme::font_body_strong(), r.width() - 48.0),
                    theme::font_body_strong(),
                    theme::TEXT,
                );
                let sub = format!(
                    "{} file{} \u{b7} {} folder{}",
                    proj_files.len(),
                    if proj_files.len() == 1 { "" } else { "s" },
                    pr.folders.len(),
                    if pr.folders.len() == 1 { "" } else { "s" },
                );
                p.text(Pos2::new(r.left() + 36.0, preview.bottom() + 33.0), Align2::LEFT_CENTER, sub, theme::font_caption(), theme::TEXT_SECONDARY);

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
                p.rect_filled(np_r, CornerRadius::same(8), if hot { UI3_SURFACE } else { UI3_MAIN_BG });
                p.rect_stroke(np_r, CornerRadius::same(8), Stroke::new(1.0, if hot { UI3_BLUE } else { UI3_BORDER }), StrokeKind::Inside);
                let c = np_r.center();
                p.circle_filled(Pos2::new(c.x, c.y - 14.0), 18.0, UI3_BLUE.gamma_multiply(0.18));
                icons::paint(&p, Icon::Plus, Pos2::new(c.x, c.y - 14.0), 16.0, UI3_BLUE);
                p.text(Pos2::new(c.x, c.y + 14.0), Align2::CENTER_CENTER, "+ New project", theme::font_body_strong(), theme::TEXT);
                p.text(Pos2::new(c.x, c.y + 31.0), Align2::CENTER_CENTER, "Organize files and nested folders", theme::font_caption(), theme::TEXT_SECONDARY);
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
                    UI3_BORDER
                } else {
                    UI3_SURFACE
                };
                p.rect_filled(r, CornerRadius::same(8), fill);
                icons::paint(&p, Icon::Folder, Pos2::new(r.left() + 22.0, r.center().y), 18.0, UI3_TEAL);
                let label = format!("{pname} / {fname}");
                p.text(
                    Pos2::new(r.left() + 42.0, r.center().y),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, &label, &theme::font_body_strong(), r.width() - 54.0),
                    theme::font_body_strong(),
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
                p.rect_filled(nf_r, CornerRadius::same(8), if hot { UI3_BORDER } else { UI3_SURFACE });
                p.rect_stroke(nf_r, CornerRadius::same(8), Stroke::new(1.0, if hot { UI3_TEAL } else { UI3_BORDER }), StrokeKind::Inside);
                icons::paint(&p, Icon::Plus, Pos2::new(nf_r.left() + 22.0, nf_r.center().y), 16.0, UI3_TEAL);
                p.text(Pos2::new(nf_r.left() + 42.0, nf_r.center().y), Align2::LEFT_CENTER, "+ New folder", theme::font_body_strong(), theme::TEXT);
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
        let now = wall_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = (content.width() - pad * 2.0).max(260.0);

        let (rcw, rch, rgap_x, rgap_y) = (260.0_f32, 172.0_f32, 16.0_f32, 16.0_f32);
        let rcols = (((inner_w + rgap_x) / (rcw + rgap_x)).floor() as usize).max(1);
        let hub_rows_h = 4usize.div_ceil(rcols) as f32 * (rch + rgap_y);

        let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
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
        type HubCard<'a> = (&'a str, Icon, Color32, &'a str, HubButton<'a>, Option<HubButton<'a>>);
        let hub_cards: [HubCard; 4] = [
            (
                "Shared Component Libraries",
                Icon::Assets,
                UI3_BLUE,
                &lib_sub,
                ("Manage Libraries", Act::OpenLibrariesModal, "home-resource-libraries"),
                Some(("Create Library Kit", Act::CreateFromTemplate(TemplateKind::DesignSystem), "home-resource-create-kit")),
            ),
            (
                "Design Tokens & Variables",
                Icon::Variables,
                UI3_TEAL,
                "Color, Number, String & Boolean collections \u{b7} Light/Dark modes",
                ("Open Variables", Act::OpenVariables, "home-resource-variables"),
                Some(("New Token Sheet", Act::CreateFromTemplate(TemplateKind::VariablesKit), "home-resource-token-sheet")),
            ),
            (
                "Plugins & Automation",
                Icon::Component,
                UI3_AMBER,
                "Installed studio plugins, XPath query & 17-rule Design Linter",
                ("Open Plugins", Act::OpenPlugins, "home-resource-plugins"),
                None,
            ),
            (
                "Agent CLI & MCP Server",
                Icon::Terminal,
                UI3_ORANGE,
                "Per-file Claude Code, Codex, Grok & Antigravity + 24 MCP tools",
                ("Open Agent Terminal", Act::OpenAgents, "home-resource-agents"),
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

            for (i, (title, icon, accent, desc, (btn1_lbl, btn1_act, btn1_key), btn2_opt)) in hub_cards.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0 + (i % rcols) as f32 * (rcw + rgap_x), y + (i / rcols) as f32 * (rch + rgap_y)), Vec2::new(rcw, rch));
                if r.bottom() < content.top() || r.top() > content.bottom() {
                    continue;
                }
                p.rect_filled(r, CornerRadius::same(8), UI3_SURFACE);
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);

                let icon_box = Rect::from_min_size(Pos2::new(r.left() + 14.0, r.top() + 14.0), Vec2::splat(32.0));
                p.rect_filled(icon_box, CornerRadius::same(7), accent.gamma_multiply(0.2));
                icons::paint(&p, *icon, icon_box.center(), 17.0, *accent);

                p.text(
                    Pos2::new(r.left() + 14.0, r.top() + 62.0),
                    Align2::LEFT_CENTER,
                    widgets::elide(&p, title, &theme::font_body_strong(), r.width() - 28.0),
                    theme::font_body_strong(),
                    theme::TEXT,
                );
                let galley = p.layout((*desc).to_string(), theme::font_caption(), theme::TEXT_SECONDARY, r.width() - 28.0);
                p.galley(Pos2::new(r.left() + 14.0, r.top() + 76.0), galley, theme::TEXT_SECONDARY);

                if let Some((btn2_lbl, btn2_act, btn2_key)) = btn2_opt {
                    let bw = (r.width() - 34.0) / 2.0;
                    let b1 = Rect::from_min_size(Pos2::new(r.left() + 14.0, r.bottom() - 38.0), Vec2::new(bw, 26.0));
                    let b2 = Rect::from_min_size(Pos2::new(b1.right() + 6.0, r.bottom() - 38.0), Vec2::new(bw, 26.0));
                    let r1 = ui.interact(b1, Id::new(("home-res-b1", *btn1_key)), Sense::click());
                    let r2 = ui.interact(b2, Id::new(("home-res-b2", *btn2_key)), Sense::click());
                    p.rect_filled(b1, theme::cr_pill(b1.height()), if r1.hovered() { *accent } else { UI3_MAIN_BG });
                    p.rect_stroke(b1, theme::cr_pill(b1.height()), Stroke::new(1.0, *accent), StrokeKind::Inside);
                    p.text(b1.center(), Align2::CENTER_CENTER, *btn1_lbl, theme::font_caption_strong(), Color32::WHITE);
                    widgets::record(ui, btn1_key, b1);
                    if r1.clicked() {
                        acts.push(btn1_act.clone());
                    }

                    p.rect_filled(b2, theme::cr_pill(b2.height()), if r2.hovered() { UI3_BORDER } else { UI3_MAIN_BG });
                    p.rect_stroke(b2, theme::cr_pill(b2.height()), Stroke::new(1.0, UI3_BORDER), StrokeKind::Inside);
                    p.text(b2.center(), Align2::CENTER_CENTER, *btn2_lbl, theme::font_caption_strong(), theme::TEXT);
                    widgets::record(ui, btn2_key, b2);
                    if r2.clicked() {
                        acts.push(btn2_act.clone());
                    }
                } else {
                    let b1 = Rect::from_min_size(Pos2::new(r.left() + 14.0, r.bottom() - 38.0), Vec2::new(r.width() - 28.0, 26.0));
                    let r1 = ui.interact(b1, Id::new(("home-res-b1", *btn1_key)), Sense::click());
                    p.rect_filled(b1, theme::cr_pill(b1.height()), if r1.hovered() { *accent } else { UI3_MAIN_BG });
                    p.rect_stroke(b1, theme::cr_pill(b1.height()), Stroke::new(1.0, *accent), StrokeKind::Inside);
                    p.text(b1.center(), Align2::CENTER_CENTER, *btn1_lbl, theme::font_caption_strong(), Color32::WHITE);
                    widgets::record(ui, btn1_key, b1);
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
                    p.rect_filled(row_r, CornerRadius::same(8), if resp.hovered() { UI3_BORDER } else { UI3_SURFACE });
                    icons::paint(&p, Icon::Book, Pos2::new(row_r.left() + 22.0, row_r.center().y), 18.0, UI3_BLUE);
                    p.text(Pos2::new(row_r.left() + 44.0, row_r.center().y - 8.0), Align2::LEFT_CENTER, &lib_idx.name, theme::font_body_strong(), theme::TEXT);
                    let meta = format!(
                        "v{} \u{b7} {} components \u{b7} {} styles \u{b7} {} variables",
                        lib_idx.latest_version(),
                        lib_idx.components,
                        lib_idx.styles,
                        lib_idx.variables,
                    );
                    p.text(Pos2::new(row_r.left() + 44.0, row_r.center().y + 9.0), Align2::LEFT_CENTER, meta, theme::font_caption(), theme::TEXT_SECONDARY);
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
        let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
        if list {
            let hr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, 28.0));
            let cols_x = list_columns(hr.right());
            for (x, label) in [(hr.left() + 60.0, "Name"), (cols_x.0, "Location"), (cols_x.1, "Last modified"), (cols_x.2, "Last viewed")] {
                p.text(Pos2::new(x, hr.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), theme::TEXT_TERTIARY);
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
            let star_r = if list {
                Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 12.0), Vec2::splat(24.0))
            } else {
                Rect::from_min_size(Pos2::new(r.left() + cw - 32.0, r.top() + 8.0), Vec2::splat(24.0))
            };
            let star = ui.interact(star_r, Id::new(("home-star", &f.id)), Sense::click());
            if star.clicked() {
                acts.push(Act::Star(f.id.clone(), !f.starred));
            }
            if list {
                list_row(p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
            } else {
                card(p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
            }
            widgets::record(ui, &format!("home-kind-{}", if board { "board" } else { "design" }), r);
            if resp.hovered() || star.hovered() || f.starred {
                if star.hovered() || (!list && resp.hovered()) {
                    p.circle_filled(star_r.center(), 12.0, Color32::from_black_alpha(if f.starred && !star.hovered() { 80 } else { 150 }));
                }
                paint_star(p, star_r.center(), f.starred, star.hovered());
            }
            widgets::record(ui, &format!("home-card-{}", f.name), r);
            if dragging && selected.as_deref() == Some(f.id.as_str()) {
                p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), Color32::from_black_alpha(90));
            }
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
        icons::paint(p, Icon::File, Pos2::new(c.x, c.y - 44.0), 32.0, theme::ICON_TERTIARY);
        p.text(Pos2::new(c.x, c.y), Align2::CENTER_CENTER, title, theme::font_large_strong(), theme::TEXT);
        let galley = p.layout(body, theme::font_body(), theme::TEXT_SECONDARY, 380.0_f32.min(area.width() - 48.0));
        let pos = Pos2::new(c.x - galley.size().x / 2.0, c.y + 18.0);
        p.galley(pos, galley, theme::TEXT_SECONDARY);
    }

    fn files_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        let files = h.files();
        let folders = h.folders();
        let is_drafts = h.section == Section::Drafts && !h.searching();
        let is_getting_started = matches!(&h.section, Section::Project(pid, None) if h.snap.project_name(pid) == "Getting started") && !h.searching();
        if files.is_empty() && folders.is_empty() && !is_drafts && !is_getting_started {
            empty_state(h, ui, content);
            return;
        }
        let now = wall_ms();
        let pad = theme::HOME_PAD;
        let list = h.view == ViewMode::List;
        let inner_w = content.width() - pad * 2.0;
        let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
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
        let starter_kits_h = if is_getting_started { 164.0 } else { 0.0 };
        let content_h = pad + drafts_strip_h + folder_rows + if folder_rows > 0.0 { 12.0 } else { 0.0 } + file_rows + starter_kits_h + pad;
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
                let quick_items: [(&str, &str, Icon, Color32, Act, &str); 3] = [
                    ("+ New Design File", "Blank vector & auto-layout canvas", Icon::Pen, UI3_BLUE, Act::NewFile, "home-drafts-new-file"),
                    ("+ New FigJam Board", "Infinite whiteboard, stickies & flows", Icon::Sticky, UI3_AMBER, Act::NewBoard, "home-drafts-new-board"),
                    ("+ New Slide Deck", "16:9 presentation deck with speaker notes", Icon::Play, UI3_TEAL, Act::NewDeck, "home-drafts-new-deck"),
                ];
                let qw = ((inner_w - 24.0) / 3.0).max(180.0);
                for (qi, (qtitle, qsub, qicon, qcol, qact, qkey)) in quick_items.iter().enumerate() {
                    let qr = Rect::from_min_size(Pos2::new(x0 + qi as f32 * (qw + 12.0), y), Vec2::new(qw, 48.0));
                    let qresp = ui.interact(qr, Id::new(("home-draft-quick", *qkey)), Sense::click());
                    let hot = qresp.hovered();
                    p.rect_filled(qr, CornerRadius::same(8), if hot { UI3_BORDER } else { UI3_SURFACE });
                    p.rect_stroke(qr, CornerRadius::same(8), Stroke::new(1.0, if hot { *qcol } else { UI3_BORDER }), StrokeKind::Inside);
                    let ib = Rect::from_min_size(Pos2::new(qr.left() + 10.0, qr.center().y - 13.0), Vec2::splat(26.0));
                    p.rect_filled(ib, CornerRadius::same(6), qcol.gamma_multiply(0.22));
                    icons::paint(&p, *qicon, ib.center(), 14.0, *qcol);
                    p.text(
                        Pos2::new(ib.right() + 10.0, qr.center().y - 7.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, qtitle, &theme::font_body_strong(), qr.width() - 52.0),
                        theme::font_body_strong(),
                        theme::TEXT,
                    );
                    p.text(
                        Pos2::new(ib.right() + 10.0, qr.center().y + 8.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, qsub, &theme::font_caption(), qr.width() - 52.0),
                        theme::font_caption(),
                        theme::TEXT_SECONDARY,
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
                    UI3_BORDER
                } else {
                    UI3_SURFACE
                };
                p.rect_filled(
                    r,
                    CornerRadius::same(8),
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
                    widgets::elide(&p, name, &theme::font_body_strong(), r.width() - 60.0),
                    theme::font_body_strong(),
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
                        p.text(Pos2::new(x, hr.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), col);
                        if resp.clicked() {
                            new_sort = Some(target_sort);
                        }
                    } else {
                        p.text(Pos2::new(x, hr.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), theme::TEXT_TERTIARY);
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
                let star_r = if list {
                    Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 12.0), Vec2::splat(24.0))
                } else {
                    Rect::from_min_size(Pos2::new(r.left() + cw - 32.0, r.top() + 8.0), Vec2::splat(24.0))
                };
                let star = ui.interact(star_r, Id::new(("home-star", &f.id)), Sense::click());
                if star.clicked() {
                    acts.push(Act::Star(f.id.clone(), !f.starred));
                }
                if list {
                    list_row(&p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
                } else {
                    card(&p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
                }
                widgets::record(ui, &format!("home-kind-{}", if board { "board" } else { "design" }), r);
                if resp.hovered() || star.hovered() || f.starred {
                    if star.hovered() || (!list && resp.hovered()) {
                        p.circle_filled(star_r.center(), 12.0, Color32::from_black_alpha(if f.starred && !star.hovered() { 80 } else { 150 }));
                    }
                    paint_star(&p, star_r.center(), f.starred, star.hovered());
                }
                widgets::record(ui, &format!("home-card-{}", f.name), r);
                if dragging && selected.as_deref() == Some(f.id.as_str()) {
                    p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), Color32::from_black_alpha(90));
                }
            }
            if !files.is_empty() {
                y += if list { files.len() as f32 * theme::HOME_LIST_ROW_H } else { files.len().div_ceil(cols) as f32 * (ch + gap_y) };
            }

            // ---- Starter Kits row inside `Getting started`
            if is_getting_started {
                y += 12.0;
                y += section_heading(
                    &p,
                    x0,
                    y,
                    "Starter Kits \u{2014} Click to add to Getting started",
                    Some("Populate this project with full multi-screen samples"),
                );
                let starters: [(&str, &str, Icon, Color32, TemplateKind, &str); 4] = [
                    ("Mobile Banking Kit", "4 iOS screens & prototype", Icon::Pen, UI3_BLUE, TemplateKind::MobileBanking, "home-starter-mobile"),
                    ("SaaS Web Dashboard", "1440px analytics UI & table", Icon::Frame, UI3_TEAL, TemplateKind::WebDashboard, "home-starter-web"),
                    ("Design System & Tokens", "Component sets & variables", Icon::Component, UI3_AMBER, TemplateKind::DesignSystem, "home-starter-system"),
                    ("Launch Brainstorm Board", "FigJam stickies & connectors", Icon::Sticky, UI3_ORANGE, TemplateKind::FigJamBoard, "home-starter-board"),
                ];
                let sw = ((inner_w - 36.0) / 4.0).max(170.0);
                for (si, (stitle, ssub, sicon, scol, skind, skey)) in starters.iter().enumerate() {
                    let sr = Rect::from_min_size(Pos2::new(x0 + si as f32 * (sw + 12.0), y), Vec2::new(sw, 96.0));
                    let sresp = ui.interact(sr, Id::new(("home-starter-kit", *skey)), Sense::click());
                    let hot = sresp.hovered();
                    p.rect_filled(sr, CornerRadius::same(8), UI3_SURFACE);
                    p.rect_stroke(sr, CornerRadius::same(8), Stroke::new(1.0, if hot { *scol } else { UI3_BORDER }), StrokeKind::Inside);
                    let ib = Rect::from_min_size(Pos2::new(sr.left() + 12.0, sr.top() + 12.0), Vec2::splat(24.0));
                    p.rect_filled(ib, CornerRadius::same(6), scol.gamma_multiply(0.22));
                    icons::paint(&p, *sicon, ib.center(), 13.0, *scol);
                    p.text(
                        Pos2::new(sr.left() + 12.0, sr.top() + 50.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, stitle, &theme::font_body_strong(), sr.width() - 24.0),
                        theme::font_body_strong(),
                        theme::TEXT,
                    );
                    p.text(
                        Pos2::new(sr.left() + 12.0, sr.top() + 68.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(&p, ssub, &theme::font_caption(), sr.width() - 24.0),
                        theme::font_caption(),
                        theme::TEXT_SECONDARY,
                    );
                    widgets::record(ui, skey, sr);
                    if sresp.clicked() {
                        acts.push(Act::CreateFromTemplate(*skind));
                    }
                }
            }
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
            Color32::from_rgb(0xFF, 0xCD, 0x29)
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
                    icons::paint(p, Icon::Sticky, area.center(), 24.0, Color32::from_black_alpha(110));
                }
            }
            return;
        }
        let _ = theme::HOME_THUMB_BG;
        p.rect_filled(area, theme::cr(radius), Color32::from_rgb(0x18, 0x18, 0x1B));
        match tex {
            Some((t, size)) => {
                let pad = if area.width() > 80.0 { 6.0 } else { 3.0 };
                let inner = area.shrink(pad);
                let k = (inner.width() / size.x.max(1.0)).min(inner.height() / size.y.max(1.0)).min(2.0);
                let fit = Rect::from_center_size(inner.center(), size * k);
                let inner_cr = CornerRadius::same(if area.width() > 80.0 { 4 } else { 2 });
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                p.add(egui::epaint::RectShape::filled(fit, inner_cr, Color32::WHITE).with_texture(t, uv));
                p.rect_stroke(fit, inner_cr, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
            }
            None => icons::paint(p, Icon::File, area.center(), 24.0, theme::ICON_TERTIARY),
        }
    }

    fn board_badge(p: &egui::Painter, area: Rect) {
        let font = theme::font_caption_strong();
        let w = widgets::text_width(p, "Board", &font) + 28.0;
        let r = Rect::from_min_size(Pos2::new(area.left() + 8.0, area.bottom() - 8.0 - 20.0), Vec2::new(w, 20.0));
        p.rect_filled(r, theme::cr(10), theme::BG_MENU);
        icons::paint(p, Icon::Sticky, Pos2::new(r.left() + 10.0, r.center().y), 12.0, theme::ICON);
        p.text(Pos2::new(r.left() + 19.0, r.center().y), Align2::LEFT_CENTER, "Board", font, theme::TEXT);
    }

    /// Draws the Figma UI3 File-Type Badge icon (`20x20` rounded badge: `#0D99FF` Design,
    /// `#F59E0B` Board, `#14AE5C` Deck, `#FF6A3D` Spec bundle).
    fn draw_file_type_badge(p: &egui::Painter, center: Pos2, name: &str, board: bool) {
        let low = name.to_ascii_lowercase();
        let (bg, icon) = if board {
            (UI3_AMBER, Icon::Sticky)
        } else if low.contains("spec") || low.contains("v2") {
            (UI3_ORANGE, Icon::Specs)
        } else if low.contains("deck") || low.contains("slide") {
            (UI3_TEAL, Icon::Play)
        } else {
            (UI3_BLUE, Icon::Pen)
        };
        let badge_r = Rect::from_center_size(center, Vec2::splat(20.0));
        p.rect_filled(badge_r, CornerRadius::same(5), bg);
        icons::paint(p, icon, center, 12.0, Color32::WHITE);
    }

    /// A grid file card (`P1.3` / `V5.2` / `D92`): `8px` outer card radius with `UI3_SURFACE` body,
    /// `#18181B` thumbnail stage (`pad = 6.0`, `4px` inner preview radius + `1px` subtle frame),
    /// `UI3_BORDER` stroke (`#0D99FF` `2px` when selected), and bottom metadata row.
    #[allow(clippy::too_many_arguments)]
    fn card(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, hover: bool, selected: bool, now: u64) {
        p.rect_filled(r, CornerRadius::same(8), UI3_SURFACE);
        let thumb = Rect::from_min_size(r.min, Vec2::new(r.width(), theme::HOME_THUMB_H));
        thumb_area(p, thumb, tex, board, 8);
        if board {
            board_badge(p, thumb);
        }
        widgets::hline(p, thumb.left() + 1.0, thumb.right() - 1.0, thumb.bottom(), UI3_BORDER);
        let stroke = if selected {
            Stroke::new(2.0, UI3_BLUE)
        } else if hover {
            Stroke::new(1.0, theme::TEXT_TERTIARY)
        } else {
            Stroke::new(1.0, UI3_BORDER)
        };
        p.rect_stroke(r, CornerRadius::same(8), stroke, StrokeKind::Inside);

        let badge_c = Pos2::new(r.left() + 18.0, thumb.bottom() + 24.0);
        draw_file_type_badge(p, badge_c, &f.name, board);

        let tx = r.left() + 34.0;
        let w = (r.width() - 42.0).max(40.0);
        let y_title = thumb.bottom() + 17.0;
        p.text(Pos2::new(tx, y_title), Align2::LEFT_CENTER, widgets::elide(p, &f.name, &theme::font_body_strong(), w), theme::font_body_strong(), theme::TEXT);
        let loc = if project.is_empty() { "Drafts" } else { project };
        let subtitle = format!("{} \u{b7} In {loc}", edited(now, f.modified));
        p.text(
            Pos2::new(tx, y_title + 16.0),
            Align2::LEFT_CENTER,
            widgets::elide(p, &subtitle, &theme::font_caption(), w),
            theme::font_caption(),
            theme::TEXT_SECONDARY,
        );
    }

    fn list_columns(right: f32) -> (f32, f32, f32) {
        (right - 520.0, right - 340.0, right - 180.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn list_row(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, hover: bool, selected: bool, now: u64) {
        if selected {
            p.rect_filled(r, CornerRadius::same(6), UI3_SURFACE);
            p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, UI3_BLUE), StrokeKind::Inside);
        } else if hover {
            p.rect_filled(r, CornerRadius::same(6), theme::BG_HOVER);
        }
        let th = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 5.0), Vec2::new(48.0, r.height() - 10.0));
        thumb_area(p, th, tex, board, theme::RADIUS_MARK);
        draw_file_type_badge(p, Pos2::new(r.left() + 70.0, r.center().y), &f.name, board);
        let cols = list_columns(r.right());
        let font = theme::font_body();
        p.text(
            Pos2::new(r.left() + 86.0, r.center().y),
            Align2::LEFT_CENTER,
            widgets::elide(p, &f.name, &theme::font_body_strong(), (cols.0 - r.left() - 96.0).max(40.0)),
            theme::font_body_strong(),
            theme::TEXT,
        );
        let loc = if project.is_empty() { "Drafts" } else { project };
        p.text(Pos2::new(cols.0, r.center().y), Align2::LEFT_CENTER, widgets::elide(p, loc, &font, 150.0), font.clone(), theme::TEXT_SECONDARY);
        p.text(Pos2::new(cols.1, r.center().y), Align2::LEFT_CENTER, relative(now, f.modified), font.clone(), theme::TEXT_SECONDARY);
        p.text(Pos2::new(cols.2, r.center().y), Align2::LEFT_CENTER, relative(now, f.opened), font, theme::TEXT_SECONDARY);
    }

    fn trash_view(h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
        if h.snap.trash.is_empty() {
            empty_state(h, ui, content);
            return;
        }
        let now = wall_ms();
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
                theme::font_body(),
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
                p.text(Pos2::new(r.left() + 48.0, r.center().y - 9.0), Align2::LEFT_CENTER, &it.name, theme::font_body_strong(), theme::TEXT);
                let from = match (&it.project_name, is_file) {
                    (Some(pn), true) => format!("Deleted {} \u{b7} from {pn}", relative(now, it.trashed)),
                    (_, false) => format!("Project \u{b7} deleted {}", relative(now, it.trashed)),
                    _ => format!("Deleted {}", relative(now, it.trashed)),
                };
                p.text(Pos2::new(r.left() + 48.0, r.center().y + 9.0), Align2::LEFT_CENTER, from, theme::font_body(), theme::TEXT_SECONDARY);
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

        // Verify Act::CreateFromTemplate and Act::NewDeck against a temporary Library
        let tmp = std::env::temp_dir().join(format!("designos_home_tpl_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut app = App::new(designos_doc::Document::new("Init"));
        app.attach_library(tmp.clone());
        app.go_home();
        assert!(app.home.is_some());

        apply(&mut app, Act::CreateFromTemplate(TemplateKind::FigJamBoard));
        assert!(app.home.is_none(), "creating from template opens the file in the editor");
        assert_eq!(app.doc.name, "Launch Brainstorm Board");
        assert_eq!(app.doc.kind, designos_doc::DocumentKind::Board);

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
