//! Application state shared by all panels and the canvas.

use crate::canvas::tools::{Tool, GROUPS};
use crate::canvas::CanvasState;
use crate::clipboard::Clip;
use crate::fonts::FontLoader;
use crate::panels::layers::LayersState;
use crate::panels::properties::PropsState;
use crate::theme;
use designos_doc::{Document, History, NodeId, Vec2};
use std::path::PathBuf;

/// Page-space point shown at the top-left of the canvas, plus zoom.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub offset: Vec2,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, zoom: 1.0 }
    }
}

// `View` is Copy, but these keep `&self` so call sites read `view.to_screen(..)` uniformly.
#[allow(clippy::wrong_self_convention)]
impl View {
    pub fn to_screen(&self, origin: egui::Pos2, p: Vec2) -> egui::Pos2 {
        egui::pos2(origin.x + (p.x - self.offset.x) * self.zoom, origin.y + (p.y - self.offset.y) * self.zoom)
    }
    pub fn to_page(&self, origin: egui::Pos2, s: egui::Pos2) -> Vec2 {
        Vec2::new((s.x - origin.x) / self.zoom + self.offset.x, (s.y - origin.y) / self.zoom + self.offset.y)
    }
    /// Zooms keeping the page point under `screen` fixed.
    pub fn zoom_at(&mut self, origin: egui::Pos2, screen: egui::Pos2, new_zoom: f32) {
        let z = new_zoom.clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let page = self.to_page(origin, screen);
        self.zoom = z;
        self.offset = Vec2::new(page.x - (screen.x - origin.x) / z, page.y - (screen.y - origin.y) / z);
    }
    /// Fits `bounds` (page space) into a canvas of `size` px with padding.
    pub fn fit(bounds: designos_doc::Rect, size: egui::Vec2) -> View {
        let pad = theme::FIT_PADDING;
        let zw = (size.x - 2.0 * pad).max(10.0) / bounds.w.max(1.0);
        let zh = (size.y - 2.0 * pad).max(10.0) / bounds.h.max(1.0);
        let z = zw.min(zh).clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let cx = bounds.x + bounds.w / 2.0;
        let cy = bounds.y + bounds.h / 2.0;
        View { zoom: z, offset: Vec2::new(cx - size.x / 2.0 / z, cy - size.y / 2.0 / z) }
    }
    /// Fits `bounds` (page space) into a canvas of `size` px with a fractional margin on each side.
    pub fn fit_with_margin(bounds: designos_doc::Rect, size: egui::Vec2, margin_fraction: f32) -> View {
        let frac = margin_fraction.clamp(0.0, 0.45);
        let avail_w = (size.x * (1.0 - 2.0 * frac)).max(10.0);
        let avail_h = (size.y * (1.0 - 2.0 * frac)).max(10.0);
        let zw = avail_w / bounds.w.max(1.0);
        let zh = avail_h / bounds.h.max(1.0);
        let z = zw.min(zh).clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let cx = bounds.x + bounds.w / 2.0;
        let cy = bounds.y + bounds.h / 2.0;
        View { zoom: z, offset: Vec2::new(cx - size.x / 2.0 / z, cy - size.y / 2.0 / z) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailItem {
    File,
    Agents,
    Assets,
    Specs,
    Tools,
    Variables,
    Plugins,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightTab {
    Design,
    Prototype,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuKind {
    Main,
    FileMenu,
    Zoom,
    ToolGroup(usize),
    #[allow(dead_code)]
    FileName,
    /// The "Actions…" command palette (Ctrl+K).
    Palette,
}

#[derive(Default)]
pub struct UiState {
    pub menu: Option<MenuKind>,
    pub menu_anchor: Option<egui::Rect>,
    pub left_collapsed: bool,
    pub left_panel_width: f32,
    pub ai_panel_width: f32,
    pub rail: Option<RailItem>,
    pub right_tab: Option<RightTab>,
    pub dev_mode: bool,
    /// Selection context menu position (canvas or layers right-click).
    pub ctx_menu: Option<egui::Pos2>,
    /// Overlapping layer picker menu position and hit list (`Cmd/Ctrl + Right-Click` on canvas).
    pub layer_picker: Option<(egui::Pos2, Vec<NodeId>)>,
    /// Anchor of the instance "more" menu and of the add-property menu in the inspector.
    pub inst_menu: Option<egui::Rect>,
    pub prop_menu: Option<egui::Rect>,
    /// Active preferred-instance `+` picker for a slot container (`(slot_id, anchor_rect)`).
    pub slot_picker: Option<(NodeId, egui::Rect)>,
    /// Command palette query and highlighted row.
    pub palette_query: String,
    pub palette_sel: usize,
    /// View toggles from the main menu.
    pub pixel_grid: bool,
    pub rulers: bool,
    /// Layout grids of frames are hidden (Ctrl+Shift+4).
    pub hide_layout_grids: bool,
    /// Snap to pixel grid is off (it is on by default, as in Figma).
    pub no_pixel_snap: bool,
    /// Whether the Preferences -> Agent permissions modal is open (`#656`).
    pub permissions_modal_open: bool,
}

#[derive(Clone, Debug)]
pub struct CopiedStyle {
    pub fills: Vec<designos_doc::Paint>,
    pub stroke: designos_doc::Stroke,
    pub effects: Vec<designos_doc::Effect>,
    pub opacity: f32,
    pub blend_mode: designos_doc::BlendMode,
    pub corner_radii: [f32; 4],
    pub corner_smoothing: f32,
    pub text_style: Option<designos_doc::TextStyle>,
}

pub struct Toast {
    pub text: String,
    /// When the toast leaves. Armed on the first paint (`armed`), so a blocked frame before that paint does not spend it.
    pub until: f64,
    pub error: bool,
    /// A file this toast can show in its folder (the toast then carries a "Show in folder" action).
    pub reveal: Option<PathBuf>,
    /// True when no file dialog could open and the toast carries an "Enter a path…" fallback action.
    pub enter_path: bool,
    /// How long the toast stays once it has been painted.
    pub(crate) duration: f64,
    /// False until the shell has painted this toast and set `until` from `duration`.
    pub(crate) armed: bool,
}

impl Toast {
    /// Keeps this toast up for `seconds` from `now`. The first paint arms the same duration again.
    pub(crate) fn stay_for(&mut self, now: f64, seconds: f64) {
        self.duration = seconds;
        self.until = now + seconds;
    }
}

pub struct App {
    pub doc: Document,
    pub hist: History,
    pub page: NodeId,
    pub selection: Vec<NodeId>,
    pub hover: Option<NodeId>,
    pub hover_layer: Option<NodeId>,
    pub tool: Tool,
    /// Last picked tool index per toolbar group.
    pub group_pick: [usize; GROUPS.len()],
    pub view: View,
    pub canvas_rect: egui::Rect,
    pub canvas: CanvasState,
    pub layers: LayersState,
    pub props: PropsState,
    pub vars: crate::variables::VarsState,
    pub assets: crate::panels::assets::AssetsState,
    /// Shared libraries (publishing, enabled libraries, updates).
    pub libs: crate::libraries::LibrariesState,
    pub plugins: crate::plugins::PluginsState,
    pub proto: crate::proto::ProtoState,
    /// Find and replace panel state (Ctrl+F).
    pub find: crate::find::FindState,
    pub ui: UiState,
    pub clip: Option<Clip>,
    pub copied_style: Option<CopiedStyle>,
    pub paste_count: u32,
    /// Reads the system clipboard's Figma HTML payload. The seam the paste paths go through: production
    /// reads the OS pasteboard, tests script it (the pasteboard is global and not thread-safe).
    pub system_html: fn() -> Option<String>,
    /// Shows a file in the system file manager. The seam the "Show in folder" action goes through:
    /// production launches the file manager, tests record the path (a test never opens a window).
    pub reveal_file: fn(&std::path::Path) -> Result<(), String>,
    /// Reads the system clipboard's plain text, for the paste paths that egui-winit sends no text for (Edit > Paste).
    pub system_text: fn() -> Option<String>,
    /// Reads the system clipboard's image. Runs on a worker thread, see `system_paste`.
    pub system_image: fn() -> Option<crate::system_paste::ClipImage>,
    /// Pastes whose clipboard image is still being read.
    pub paste_jobs: std::collections::VecDeque<crate::system_paste::PasteJob>,
    pub toast: Option<Toast>,
    pub file_path: Option<PathBuf>,
    pub now: f64,
    pub keep_tool: bool,
    pub first_fit_done: bool,
    pub fonts: FontLoader,
    /// Document snapshot while a numeric field is being scrubbed (see `widgets::field::SCRUB_FLAG`).
    pub scrub: Option<std::collections::BTreeMap<NodeId, designos_doc::Node>>,
    /// `Document::layout_epoch` when the open scrub began; set by `App::begin_scrub` together with `scrub`.
    pub(crate) scrub_opened_at: u64,
    /// Zoom (as a factor) to apply on the first frame instead of fitting the page (`--zoom`).
    pub startup_zoom: Option<f32>,
    /// The projects library (None until opened; tests run without one).
    pub lib: Option<designos_library::Library>,
    /// The library file open in the editor, if any (autosaved, shown in the header).
    pub lib_file: Option<crate::home::LibFile>,
    /// Some while the home screen (file browser) is showing instead of the editor.
    pub home: Option<crate::home::Home>,
    /// The open modal dialog (prompts, confirmations, shortcuts, About).
    pub dialog: Option<crate::home::Dialog>,
    /// The questions "Save changes to <name>?" that stand between an action and the unsaved edits it would drop.
    pub leave: crate::home::LeaveFlow,
    pub autosave: crate::home::Autosave,
    /// The header file name is being renamed (double-click).
    pub renaming_file: bool,
    /// Comment threads UI (composer, open thread, panel filters).
    pub comments: crate::comments::CommentsState,
    /// Version history of the open library file.
    pub history: crate::history::HistoryState,
    /// Dev mode: code language, annotation / link editors, Compare changes.
    pub dev: crate::dev::DevState,
    /// Board (whiteboard) mode: tool settings, magnet dots and popovers.
    pub board: crate::board::BoardState,
    pub decks: crate::decks::DecksState,
    /// Native EaseUI-styled Spec Reader & Agentic Hub state.
    pub specs: crate::specs::SpecsState,
    /// Native Embedded PTY Agent Terminal (Claude CLI, Codex CLI, Grok CLI, $SHELL).
    pub terminal: crate::terminal::TerminalState,
    /// Screen-reader polite live-region announcer for agent tab transitions (`#687`).
    pub agent_a11y: crate::agent_a11y::Announcer,
    /// Per-session agent cursors and write-flash outlines on the canvas (`#682`).
    pub agent_presence: crate::agent_presence::AgentPresence,
    /// Edge-anchored "N changes -> Show" chips for off-screen agent turns (`#695`).
    pub agent_changes_chip: crate::canvas::agent_changes_chip::AgentChangesChips,
    /// Busy-frame banner, dismissals, free-spot duplicate placement, and concurrent-edit ledger (`#693`).
    pub agent_busy_frames: crate::agent_busy_frames::AgentBusyFrames,
    /// In-Canvas Cmd+K Agent & Design Gate Bar state.
    pub cmdk: crate::cmdk::CmdKState,
    /// Check designs panel: hard-coded values, contrast and detached instances.
    pub check_designs: crate::check_designs::CheckDesignsState,
    /// 1:1 Figma UI3 Multi-Tab Project Workspace state (`US-10`).
    pub tabs: crate::tabs::TabsState,
    /// E5-10: AI image tools routed through the user's agent CLI.
    pub image_jobs: crate::image_jobs::ImageJobsState,
    /// Two-digit rapid opacity keystroke buffer (`(digit, timestamp)` within 400ms).
    pub opacity_buffer: Option<(u8, f64)>,
    /// Non-finite or save warning produced by `save_library_file` / `flush_autosave` during a document
    /// switch, carried into an immediate follow-up `show_toast` so `"Opened X"` does not overwrite it.
    pub(crate) pending_flush_notice: Option<(f64, String)>,
    /// Last web capture import report (`T8.27`) and whether its `Details` sheet is open.
    pub web_capture_report: Option<designos_webcapture::ImportReport>,
    pub web_capture_report_open: bool,
    /// Last `warn` log line emitted when a system file dialog could not open (`#1192`).
    pub last_dialog_warn: Option<String>,
    /// Per-window guard so a second click on Import while a chooser is opening opens nothing new (`#1192`).
    pub dialog_opening: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl App {
    pub fn new(doc: Document) -> Self {
        let page = doc.pages.first().copied().unwrap_or(NodeId(0));
        Self {
            doc,
            hist: History::default(),
            page,
            selection: vec![],
            hover: None,
            hover_layer: None,
            tool: Tool::Move,
            group_pick: [0; GROUPS.len()],
            view: View::default(),
            canvas_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            canvas: CanvasState::default(),
            layers: LayersState::default(),
            props: PropsState::default(),
            vars: Default::default(),
            assets: Default::default(),
            libs: Default::default(),
            plugins: Default::default(),
            proto: Default::default(),
            find: Default::default(),
            ui: UiState { pixel_grid: true, left_panel_width: theme::LEFT_PANEL_WIDTH, ai_panel_width: 400.0, ..UiState::default() },
            clip: None,
            copied_style: None,
            paste_count: 0,
            system_html: crate::clipboard::system_figma_html,
            reveal_file: crate::fig_export::reveal_file,
            system_text: crate::system_paste::system_text,
            system_image: crate::system_paste::system_image,
            paste_jobs: Default::default(),
            toast: None,
            file_path: None,
            now: 0.0,
            keep_tool: false,
            first_fit_done: false,
            fonts: FontLoader::default(),
            scrub: None,
            scrub_opened_at: 0,
            startup_zoom: None,
            lib: None,
            lib_file: None,
            home: None,
            dialog: None,
            leave: Default::default(),
            autosave: Default::default(),
            renaming_file: false,
            comments: Default::default(),
            history: Default::default(),
            dev: Default::default(),
            board: Default::default(),
            decks: Default::default(),
            specs: Default::default(),
            terminal: Default::default(),
            agent_a11y: Default::default(),
            agent_presence: Default::default(),
            agent_changes_chip: Default::default(),
            agent_busy_frames: Default::default(),
            cmdk: Default::default(),
            check_designs: Default::default(),
            tabs: Default::default(),
            image_jobs: Default::default(),
            opacity_buffer: None,
            pending_flush_notice: None,
            web_capture_report: None,
            web_capture_report_open: false,
            last_dialog_warn: None,
            dialog_opening: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Registers fonts finished loading in the background and re-renders.
    pub fn poll_fonts(&mut self) {
        if let Some(done) = self.fonts.poll() {
            if done.loaded > 0 {
                self.changed();
                // text measured with the stand-in keeps its old box until it is fitted to the face that arrived
                self.start_text_refit(&done.families);
            }
            if let Some(first) = done.failed.first() {
                let more = done.failed.len() - 1;
                let suffix = if more > 0 { format!(" (+{more} more)") } else { String::new() };
                self.show_toast(format!("Font unavailable: {first}{suffix}"), true);
            }
        }
        self.step_text_refit();
    }

    pub fn show_toast(&mut self, text: impl Into<String>, error: bool) {
        let mut text = text.into();
        let mut error = error;
        if let Some((at, prev)) = self.pending_flush_notice.take() {
            if (at - self.now).abs() < f64::EPSILON && self.toast.as_ref().is_some_and(|t| t.text == prev) && text != prev {
                text = format!("{text} ({prev})");
                error = true;
            }
        }
        self.toast =
            Some(Toast { text, until: self.now + theme::TOAST_SECONDS, error, reveal: None, enter_path: false, duration: theme::TOAST_SECONDS, armed: false });
    }

    /// A toast that stays long enough to read and offers to show `path` in its folder.
    pub fn show_toast_with_reveal(&mut self, text: impl Into<String>, path: PathBuf) {
        self.show_toast(text, false);
        if let Some(toast) = self.toast.as_mut() {
            toast.reveal = Some(path);
            toast.stay_for(self.now, theme::TOAST_REVEAL_SECONDS);
        }
    }

    /// Marks the document as modified so the canvas texture is re-rendered.
    pub fn changed(&mut self) {
        self.canvas.dirty = true;
    }

    /// Marks transient `in_progress` badge state as updated for session persistence without dirtying
    /// the canvas tile/GPU scene cache (`TileView::rev`).
    pub fn in_progress_changed(&mut self) {
        if self.lib_file.is_some() {
            self.autosave.changed_at = Some(self.now);
            self.autosave.confirmed = false;
        }
    }

    /// Runs one bounded step of the auto-layout subtrees a relayout deferred past its budget and
    /// reports the work still queued. True while subtrees remain, so the caller keeps repainting.
    pub fn step_pending_relayout(&mut self) -> bool {
        if self.doc.pending_relayout().is_empty() {
            return false;
        }
        let remaining = self.doc.flush_pending_relayout_step();
        self.relayout_moved_geometry();
        remaining > 0
    }

    /// Finishes every deferred auto-layout subtree. Anything that writes the document out (a save,
    /// a version snapshot, an export) calls this first: the queue is not part of the file, so what
    /// is still queued would be stored with the stale layout and nothing would finish it on reopen.
    pub fn settle_pending_relayout(&mut self) {
        if self.doc.pending_relayout().is_empty() {
            return;
        }
        self.doc.flush_pending_relayout();
        self.relayout_moved_geometry();
    }

    /// The deferred layout moved nodes behind the canvas's back: bump the revision (the CPU scene
    /// index and the autosave watch it), drop the cached tiles and GPU geometry, and tell the
    /// user how much is still unfinished, or retire that notice once nothing is.
    pub(crate) fn relayout_moved_geometry(&mut self) {
        self.changed();
        self.canvas.tiles.invalidate_all();
        match self.doc.relayout_notice() {
            Some(notice) if self.toast.as_ref().is_none_or(|t| !t.error) => self.show_toast(notice, false),
            Some(_) => {}
            None => {
                if self.toast.as_ref().is_some_and(|t| !t.error && t.text.starts_with(designos_doc::RELAYOUT_NOTICE_PREFIX)) {
                    self.toast = None;
                }
            }
        }
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection.contains(&id)
    }

    pub fn select_only(&mut self, id: NodeId) {
        self.selection = vec![id];
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Refreshes the `#693` busy-frame banner state from the current selection and any active canvas edit gesture.
    pub fn sync_busy_frame_banner(&mut self) {
        use crate::canvas::{Interaction, ResizeKind};
        let gesture_node = match &self.canvas.inter {
            Interaction::Pending(p) => Some(p.node),
            Interaction::Move(m) => m.ids.first().copied(),
            Interaction::Resize(r) => match &r.kind {
                ResizeKind::Single { id, .. } => Some(*id),
                ResizeKind::Multi { nodes, .. } => nodes.first().map(|n| n.0),
            },
            Interaction::Rotate(r) => Some(r.id),
            Interaction::CornerRadius(c) => Some(c.id),
            Interaction::AutoLayoutDrag(a) => Some(a.id),
            Interaction::ShapeControl(s) => Some(s.id),
            _ => self.canvas.text.as_ref().map(|t| t.id).or_else(|| self.canvas.vedit.as_ref().map(|v| v.id)),
        };
        self.agent_busy_frames.sync_banner(&self.doc, &self.agent_presence, &self.selection, gesture_node);
    }

    /// Drops selection/hover entries that no longer exist or left the page.
    pub fn sanitize(&mut self) {
        self.doc.prune_in_progress();
        let doc = &self.doc;
        let page = self.page;
        self.selection.retain(|id| doc.get(*id).is_some() && doc.page_of(*id) == Some(page));
        if self.hover.map(|h| doc.get(h).is_none()).unwrap_or(false) {
            self.hover = None;
        }
        if !doc.nodes.contains_key(&self.page) {
            self.page = doc.pages.first().copied().unwrap_or(NodeId(0));
        }
        self.sync_busy_frame_banner();
    }

    #[allow(dead_code)]
    pub fn title(&self) -> String {
        theme::APP_TITLE.to_string()
    }

    pub fn open_agent_permissions(&mut self) {
        self.ui.permissions_modal_open = true;
    }

    pub fn set_agent_permission_mode(&mut self, mode: crate::agent_permissions::AgentPermissionMode) {
        self.terminal.permission_mode = mode;
        for entry in &mut self.tabs.entries {
            entry.terminal.permission_mode = mode;
        }
        if let Some(lib) = &self.lib {
            // A failed save leaves the older mode on disk. The picked mode keeps applying in this process
            // (`save_permission_mode`), and the user is told it did not persist.
            if crate::agent_permissions::save_permission_mode(lib.root(), mode).is_err() {
                self.terminal.status_msg = Some("Mode not saved. Check folder permissions.".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_roundtrip_and_zoom_anchor() {
        let mut v = View { offset: Vec2::new(100.0, 50.0), zoom: 2.0 };
        let origin = egui::pos2(300.0, 10.0);
        let screen = egui::pos2(500.0, 200.0);
        let before = v.to_page(origin, screen);
        v.zoom_at(origin, screen, 4.0);
        let after = v.to_page(origin, screen);
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
        let s = v.to_screen(origin, before);
        assert!((s.x - screen.x).abs() < 1e-3);
    }

    #[test]
    fn fit_centers_bounds() {
        let v = View::fit(designos_doc::Rect::new(0.0, 0.0, 1000.0, 500.0), egui::vec2(800.0, 600.0));
        assert!(v.zoom < 1.0);
        let c = v.to_screen(egui::Pos2::ZERO, Vec2::new(500.0, 250.0));
        assert!((c.x - 400.0).abs() < 1.0 && (c.y - 300.0).abs() < 1.0);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut v = View::default();
        v.zoom_at(egui::Pos2::ZERO, egui::Pos2::ZERO, 10_000.0);
        assert_eq!(v.zoom, theme::ZOOM_MAX);
    }
}
