//! E5-10: AI image tools routed through the user's installed agent CLI
//! (`Remove background`, `Isolate subject`, `Expand…`, `Erase…`).
//!
//! No model weights or inference runtime are bundled in the binary. For each job,
//! `designos` writes `<session>/image-jobs/<job>/input.png` (plus `mask.png` for `Erase`)
//! and `request.json`, sends a clear task prompt to the active agent tab, and watches
//! `output.png` off the UI thread. When `output.png` is complete and passes size/content
//! validation, a new image layer (`<name> · <op>`) is inserted above the original and the
//! original is hidden in a single undo step (`Tx` / `Command::Batch`).

use crate::app::App;
use crate::edit::Tx;
use crate::terminal::{parse_agent_status_payload, runtime_binary_available, AgentCliPreset, AgentTabStatus};
use crate::theme;
use crate::widgets;
use crate::widgets::field::{self, Prefix};
use designos_doc::{Affine, BlendMode, Command, Document, ImageScaleMode, Node, NodeId, NodeKind, Paint, PaintKind, Vec2};
use egui::{Align2, Area, Color32, Context, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// Tooltip shown on disabled AI image commands when no agent CLI is installed.
pub const DISABLED_TOOLTIP: &str = "Needs an agent CLI. Set one up in Agents.";

/// Maximum padding per side for `Expand…` (px).
pub const EXPAND_MAX_SIDE_PX: u32 = 4096;

/// Default time `output.png` must remain unchanged before the watcher reads it.
const DEFAULT_STABLE_WINDOW: Duration = Duration::from_millis(500);

/// Default timeout for a running image job (5 minutes).
const DEFAULT_JOB_TIMEOUT: Duration = Duration::from_secs(300);

/// Supported AI image operations (`E5-10`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageOp {
    RemoveBackground,
    IsolateSubject,
    Expand { top: u32, right: u32, bottom: u32, left: u32 },
    Erase { instruction: String, region: Option<[u32; 4]> },
}

impl ImageOp {
    /// Slug written to `request.json` under `"op"`.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::RemoveBackground => "remove_background",
            Self::IsolateSubject => "isolate_subject",
            Self::Expand { .. } => "expand",
            Self::Erase { .. } => "erase",
        }
    }

    /// Short label appended to the new layer's name (`"<name> · <label>"`).
    pub fn layer_suffix(&self) -> &'static str {
        match self {
            Self::RemoveBackground => "Remove background",
            Self::IsolateSubject => "Isolate subject",
            Self::Expand { .. } => "Expand",
            Self::Erase { .. } => "Erase",
        }
    }

    /// Human-readable instruction for `request.json` and the agent prompt.
    pub fn instruction_text(&self) -> String {
        match self {
            Self::RemoveBackground => "Remove background and write a transparent RGBA PNG".to_string(),
            Self::IsolateSubject => "Isolate the main subject on a transparent RGBA background".to_string(),
            Self::Expand { top, right, bottom, left } => {
                format!("Expand canvas by top={top}px, right={right}px, bottom={bottom}px, left={left}px and outpaint the new border area")
            }
            Self::Erase { instruction, region } => match region {
                Some([x, y, w, h]) => {
                    format!("Erase '{}' in mask region [x={x}, y={y}, w={w}, h={h}] (white in mask.png) and inpaint cleanly", instruction.trim())
                }
                None => format!("Erase '{}' and inpaint cleanly", instruction.trim()),
            },
        }
    }
}

/// Lifecycle state of an [`ImageJob`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageJobState {
    Queued,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

/// Padding sides written into `request.json` for `Expand`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandSides {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// JSON payload written to `<session>/image-jobs/<job>/request.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRequestPayload {
    pub op: String,
    pub instruction: String,
    pub input: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<ExpandSides>,
    pub expected_size: [u32; 2],
}

/// A single AI image job targeting a node's image fill.
#[derive(Clone, Debug)]
pub struct ImageJob {
    pub id: String,
    pub node: NodeId,
    pub fill_index: usize,
    pub op: ImageOp,
    pub input_path: PathBuf,
    pub mask_path: Option<PathBuf>,
    pub output_path: PathBuf,
    pub request_path: PathBuf,
    /// Dimensions of the exported visible `input.png` crop in image pixels.
    pub input_crop_size: (u32, u32),
    pub expected_size: (u32, u32),
    pub started_at: Instant,
    pub state: ImageJobState,
}

/// Outcome sent back from the off-thread `output.png` watcher.
#[derive(Debug)]
enum WatcherOutcome {
    Completed { job_id: String, png_bytes: Vec<u8>, warning: Option<String> },
    Failed { job_id: String, reason: String },
}

struct RunningWatcher {
    job_id: String,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<WatcherOutcome>,
}

/// Popover state for `Expand…` (top / right / bottom / left px or `25%` preset).
#[derive(Clone, Debug)]
pub struct ExpandPopoverState {
    pub node: NodeId,
    pub fill_index: usize,
    pub anchor: Rect,
    /// Cached visible crop dimensions `(w, h)` in image pixels so the `+25%` preset uses image pixels without re-decoding.
    pub crop_dims: (u32, u32),
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// Popover state for `Erase…` (instruction text + optional rectangle drawn on the image).
#[derive(Clone, Debug)]
pub struct ErasePopoverState {
    pub node: NodeId,
    pub fill_index: usize,
    pub anchor: Rect,
    pub instruction: String,
    pub focus_requested: bool,
    /// Cached visible crop dimensions `(w, h)` in image pixels so canvas rendering never decodes `doc.images` per frame.
    pub crop_dims: (u32, u32),
    pub scale_mode: ImageScaleMode,
    /// Optional mask rectangle in exported crop pixel coordinates `[x, y, w, h]`.
    pub region: Option<[u32; 4]>,
    /// Drag start in page coordinates when drawing the mask rectangle on the canvas.
    pub drag_start_page: Option<Pos2>,
}

/// Per-document state for AI image tools (`App::image_jobs`).
pub struct ImageJobsState {
    pub jobs: Vec<ImageJob>,
    queue: VecDeque<String>,
    watcher: Option<RunningWatcher>,
    next_seq: u64,
    /// Open `AI` menu popup for a specific `(node, fill_index, anchor)`.
    pub open_fill_menu: Option<(NodeId, usize, Rect)>,
    pub expand_popover: Option<ExpandPopoverState>,
    pub erase_popover: Option<ErasePopoverState>,
    /// Override for agent CLI availability in deterministic unit tests.
    pub override_agent_installed: Option<bool>,
    /// Optional custom root directory for `image-jobs` (used by tests or custom sessions).
    pub custom_jobs_root: Option<PathBuf>,
    /// Optional shell script or command executed when a job starts (test seam for stub agents).
    pub agent_script: Option<String>,
    /// Time `output.png` must stay unchanged before decoding (500ms default, shorter in tests).
    pub stable_window: Duration,
    /// Maximum runtime before a job times out (5 minutes default).
    pub job_timeout: Duration,
    last_session_root: Option<PathBuf>,
}

impl Default for ImageJobsState {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            queue: VecDeque::new(),
            watcher: None,
            next_seq: 1,
            open_fill_menu: None,
            expand_popover: None,
            erase_popover: None,
            override_agent_installed: None,
            custom_jobs_root: None,
            agent_script: None,
            stable_window: DEFAULT_STABLE_WINDOW,
            job_timeout: DEFAULT_JOB_TIMEOUT,
            last_session_root: None,
        }
    }
}

impl ImageJobsState {
    /// Creates a fresh `ImageJobsState` for a new tab while preserving test/runtime configuration overrides.
    pub fn new_inheriting_config_from(other: &Self) -> Self {
        Self {
            override_agent_installed: other.override_agent_installed,
            agent_script: other.agent_script.clone(),
            stable_window: other.stable_window,
            job_timeout: other.job_timeout,
            ..Self::default()
        }
    }

    /// Moves this tab's `ImageJobsState` out for parking in `ProjectTab`, leaving a clean state with the same config overrides.
    pub fn take_for_park(&mut self) -> Self {
        let replacement = Self::new_inheriting_config_from(self);
        std::mem::replace(self, replacement)
    }

    /// Cancels all active/queued image jobs and removes this tab's `<session_dir>/image-jobs` folder.
    pub fn cancel_and_cleanup(&mut self, fallback_root: Option<PathBuf>) {
        if let Some(w) = self.watcher.take() {
            w.cancel.store(true, Ordering::SeqCst);
        }
        self.queue.clear();
        for job in &mut self.jobs {
            if matches!(job.state, ImageJobState::Running | ImageJobState::Queued) {
                job.state = ImageJobState::Cancelled;
            }
        }
        let root = self.last_session_root.take().or_else(|| self.custom_jobs_root.clone()).or(fallback_root);
        if let Some(root) = root {
            let _ = crate::agent_session_files::remove_session_dir_all(&root);
        }
        self.open_fill_menu = None;
        self.expand_popover = None;
        self.erase_popover = None;
    }

    /// True if `node` currently has a `Queued` or `Running` AI image job.
    pub fn active_job_for_node(&self, node: NodeId) -> Option<&ImageJob> {
        self.jobs.iter().find(|j| j.node == node && matches!(j.state, ImageJobState::Running | ImageJobState::Queued))
    }

    /// True while any job is `Queued` or `Running`.
    pub fn has_active_jobs(&self) -> bool {
        self.jobs.iter().any(|j| matches!(j.state, ImageJobState::Running | ImageJobState::Queued))
    }
}

const AGENT_CLI_PRESETS: [AgentCliPreset; 4] = [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok];

fn is_preset_installed(image_jobs: &ImageJobsState, preset: AgentCliPreset) -> bool {
    if !matches!(preset, AgentCliPreset::Claude | AgentCliPreset::Codex | AgentCliPreset::Antigravity | AgentCliPreset::Grok) {
        return false;
    }
    if let Some(ov) = image_jobs.override_agent_installed {
        return ov;
    }
    runtime_binary_available(preset.binary_name())
}

fn tab_is_usable_for_preset(tab: &crate::terminal::TerminalTab, preset: AgentCliPreset) -> bool {
    tab.preset == preset && tab.start_failure.is_none() && !tab.exited_notice_shown
}

fn preset_has_failed_tab(terminal: &crate::terminal::TerminalState, preset: AgentCliPreset) -> bool {
    terminal.tabs.iter().any(|t| (t.preset == preset || t.spawn_preset == preset) && (t.start_failure.is_some() || t.exited_notice_shown))
        && !terminal.tabs.iter().any(|t| tab_is_usable_for_preset(t, preset))
}

/// Resolves the agent CLI preset to use for `terminal` and `image_jobs`: prefers an already-running
/// usable agent tab, then `terminal.active_runtime` if it is an installed agent CLI that can be started,
/// then the first installed agent CLI preset whose tab is running or can be started (`E5-10` §3 rule 2).
pub fn preferred_agent_cli_preset_for(terminal: &crate::terminal::TerminalState, image_jobs: &ImageJobsState) -> Option<AgentCliPreset> {
    if terminal.session_dir.as_os_str().is_empty() {
        return None;
    }
    if let Some(active) = terminal.active_tab() {
        if is_preset_installed(image_jobs, active.preset) && tab_is_usable_for_preset(active, active.preset) {
            return Some(active.preset);
        }
    }
    if is_preset_installed(image_jobs, terminal.active_runtime) && !preset_has_failed_tab(terminal, terminal.active_runtime) {
        return Some(terminal.active_runtime);
    }
    for tab in &terminal.tabs {
        if is_preset_installed(image_jobs, tab.preset) && tab_is_usable_for_preset(tab, tab.preset) {
            return Some(tab.preset);
        }
    }
    AGENT_CLI_PRESETS.into_iter().find(|&p| is_preset_installed(image_jobs, p) && !preset_has_failed_tab(terminal, p))
}

/// Resolves the agent CLI preset to use for the active file (`E5-10` §3 rule 2).
pub fn preferred_agent_cli_preset(app: &App) -> Option<AgentCliPreset> {
    preferred_agent_cli_preset_for(&app.terminal, &app.image_jobs)
}

/// Returns true when at least one agent CLI (`Claude`, `Codex`, `Antigravity`, `Grok`) is installed
/// and an agent tab for this file is running or can be started (`E5-10` §3 rule 2).
pub fn has_available_agent_cli(app: &App) -> bool {
    preferred_agent_cli_preset(app).is_some()
}

/// Finds the topmost visible `PaintKind::Image` fill on `node`, returning `(fill_index, image_ref, scale_mode, total_image_fills)`.
pub fn topmost_visible_image_fill(node: &Node) -> Option<(usize, String, ImageScaleMode, usize)> {
    let mut count = 0usize;
    let mut chosen: Option<(usize, String, ImageScaleMode)> = None;
    for (idx, paint) in node.fills.iter().enumerate() {
        if let PaintKind::Image { image_ref, scale_mode } = &paint.kind {
            count += 1;
            if paint.visible {
                chosen = Some((idx, image_ref.clone(), *scale_mode));
            }
        }
    }
    chosen.map(|(idx, r, sm)| (idx, r, sm, count))
}

/// True if the current selection has a single layer with at least one visible `PaintKind::Image` fill.
pub fn selection_target_image_fill(app: &App) -> Option<(NodeId, usize, String, ImageScaleMode, usize)> {
    if app.selection.len() != 1 {
        return None;
    }
    let id = app.selection[0];
    let node = app.doc.get(id)?;
    let (fill_idx, image_ref, scale_mode, count) = topmost_visible_image_fill(node)?;
    Some((id, fill_idx, image_ref, scale_mode, count))
}

/// Decodes an image stored in `doc.images` by content sniffing (PNG, JPEG, WebP).
fn decode_doc_image(doc: &Document, image_ref: &str) -> Result<image::RgbaImage, String> {
    let bytes = doc.images.get(image_ref).ok_or_else(|| format!("Image asset '{image_ref}' is missing from the document"))?;
    let dyn_img = image::load_from_memory(bytes).map_err(|e| format!("Cannot decode source image '{image_ref}': {e}"))?;
    Ok(dyn_img.to_rgba8())
}

/// Reads the header dimensions `(w, h)` of an image in `doc.images` without decoding every pixel when possible.
fn doc_image_dimensions(doc: &Document, image_ref: &str) -> Option<(u32, u32)> {
    let bytes = doc.images.get(image_ref)?;
    if let Ok(reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format() {
        if let Ok(dims) = reader.into_dimensions() {
            return Some((dims.0.max(1), dims.1.max(1)));
        }
    }
    decode_doc_image(doc, image_ref).ok().map(|img| img.dimensions())
}

/// Computes the visible crop rectangle `(crop_x, crop_y, crop_w, crop_h)` in source image pixels
/// matching `designos-render`'s `PaintKind::Image` shader (`Fill` / `Crop` center-cover vs `Fit`).
pub fn visible_image_crop_rect(raw_w: u32, raw_h: u32, layer_size: Vec2, scale_mode: ImageScaleMode) -> (u32, u32, u32, u32) {
    let rw = raw_w.max(1);
    let rh = raw_h.max(1);
    let iw = rw as f32;
    let ih = rh as f32;
    let lx = layer_size.x.max(1.0);
    let ly = layer_size.y.max(1.0);
    match scale_mode {
        ImageScaleMode::Fill | ImageScaleMode::Crop => {
            let s = (lx / iw).max(ly / ih).max(1e-6);
            let crop_w = (lx / s).round().clamp(1.0, iw) as u32;
            let crop_h = (ly / s).round().clamp(1.0, ih) as u32;
            let crop_x = rw.saturating_sub(crop_w) / 2;
            let crop_y = rh.saturating_sub(crop_h) / 2;
            (crop_x, crop_y, crop_w, crop_h)
        }
        ImageScaleMode::Fit | ImageScaleMode::Tile => (0, 0, rw, rh),
    }
}

/// Extracts the visible crop of `decoded` at native pixel resolution (`E5-10` §3 rule 3, §8.1).
fn crop_visible_image(decoded: &image::RgbaImage, layer_size: Vec2, scale_mode: ImageScaleMode) -> image::RgbaImage {
    let (raw_w, raw_h) = decoded.dimensions();
    let (cx, cy, cw, ch) = visible_image_crop_rect(raw_w, raw_h, layer_size, scale_mode);
    if cx == 0 && cy == 0 && cw == raw_w && ch == raw_h {
        decoded.clone()
    } else {
        image::imageops::crop_imm(decoded, cx, cy, cw, ch).to_image()
    }
}

/// Computes the cached visible crop dimensions `(crop_w, crop_h)` and `ImageScaleMode` for `(node, fill_index)`.
fn node_fill_visible_crop_dims(doc: &Document, node: NodeId, fill_index: usize) -> Option<((u32, u32), ImageScaleMode)> {
    let n = doc.get(node)?;
    let paint = n.fills.get(fill_index)?;
    let PaintKind::Image { image_ref, scale_mode } = &paint.kind else {
        return None;
    };
    let (raw_w, raw_h) = doc_image_dimensions(doc, image_ref).unwrap_or_else(|| (n.size.x.round().max(1.0) as u32, n.size.y.round().max(1.0) as u32));
    let (_, _, cw, ch) = visible_image_crop_rect(raw_w, raw_h, n.size, *scale_mode);
    Some(((cw, ch), *scale_mode))
}

/// Returns `(x, y, w, h)` in node-local coordinates for the rendered image content rect inside `layer_size`.
fn local_image_content_rect(layer_size: Vec2, scale_mode: ImageScaleMode, crop_dims: (u32, u32)) -> (f32, f32, f32, f32) {
    let lx = layer_size.x.max(1.0);
    let ly = layer_size.y.max(1.0);
    let cw = crop_dims.0.max(1) as f32;
    let ch = crop_dims.1.max(1) as f32;
    match scale_mode {
        ImageScaleMode::Fit => {
            let s = (lx / cw).min(ly / ch).max(1e-6);
            let w = cw * s;
            let h = ch * s;
            ((lx - w) * 0.5, (ly - h) * 0.5, w.max(1e-6), h.max(1e-6))
        }
        _ => (0.0, 0.0, lx, ly),
    }
}

/// Maps a canvas drag `(start_page, cur_page)` through `world.invert()` into the exported crop's pixel grid `(0..crop_w, 0..crop_h)`.
pub fn page_drag_to_image_region(
    world: Affine,
    layer_size: Vec2,
    scale_mode: ImageScaleMode,
    crop_dims: (u32, u32),
    start_page: Pos2,
    cur_page: Pos2,
) -> Option<[u32; 4]> {
    let inv = world.invert()?;
    let p0 = inv.apply(Vec2::new(start_page.x, start_page.y));
    let p1 = inv.apply(Vec2::new(cur_page.x, cur_page.y));

    let (cx, cy, cw_local, ch_local) = local_image_content_rect(layer_size, scale_mode, crop_dims);
    let min_x = (p0.x.min(p1.x) - cx).clamp(0.0, cw_local);
    let min_y = (p0.y.min(p1.y) - cy).clamp(0.0, ch_local);
    let max_x = (p0.x.max(p1.x) - cx).clamp(0.0, cw_local);
    let max_y = (p0.y.max(p1.y) - cy).clamp(0.0, ch_local);
    if (max_x - min_x) < 1.0 || (max_y - min_y) < 1.0 {
        return None;
    }

    let cw = crop_dims.0.max(1) as f32;
    let ch = crop_dims.1.max(1) as f32;
    let sx = cw / cw_local;
    let sy = ch / ch_local;
    let min_ix: f32 = min_x * sx;
    let min_iy: f32 = min_y * sy;
    let max_ix: f32 = max_x * sx;
    let max_iy: f32 = max_y * sy;

    if (max_ix - min_ix) < 1.0 || (max_iy - min_iy) < 1.0 {
        return None;
    }
    let rx = (min_ix.floor() as u32).min(crop_dims.0.saturating_sub(1));
    let ry = (min_iy.floor() as u32).min(crop_dims.1.saturating_sub(1));
    let rw = ((max_ix - min_ix).ceil().max(1.0) as u32).min(crop_dims.0.saturating_sub(rx));
    let rh = ((max_iy - min_iy).ceil().max(1.0) as u32).min(crop_dims.1.saturating_sub(ry));
    Some([rx, ry, rw, rh])
}

/// Encodes an [`image::RgbaImage`] as PNG bytes.
fn encode_png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    image::DynamicImage::ImageRgba8(img.clone()).write_to(&mut cursor, image::ImageFormat::Png).map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out)
}

/// Builds the black/white `mask.png` (`#FFFFFF` = erase region on `#000000` background) of size `(w, h)`.
fn build_erase_mask_png(w: u32, h: u32, region: [u32; 4]) -> Result<Vec<u8>, String> {
    let mut img = image::RgbaImage::from_pixel(w.max(1), h.max(1), image::Rgba([0, 0, 0, 255]));
    let [rx, ry, rw, rh] = region;
    let x0 = rx.min(w);
    let y0 = ry.min(h);
    let x1 = rx.saturating_add(rw).min(w);
    let y1 = ry.saturating_add(rh).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
        }
    }
    encode_png(&img)
}

impl App {
    /// Resolves `<session_dir>/image-jobs` for the current file's isolated session (`E5-10` §3 rule 3).
    pub fn image_jobs_root(&self) -> PathBuf {
        if let Some(custom) = &self.image_jobs.custom_jobs_root {
            return custom.clone();
        }
        self.terminal.session_dir.join("image-jobs")
    }

    /// Cancels all active/queued image jobs and removes this file's `<session_dir>/image-jobs` folder on file close.
    pub fn cleanup_image_jobs_on_file_close(&mut self) {
        let fallback = self.image_jobs_root();
        self.image_jobs.cancel_and_cleanup(Some(fallback));
    }

    /// Cancels any running or queued job on `node` (or the currently running job if `node` is `None`).
    pub fn cancel_image_job(&mut self, node: Option<NodeId>) {
        let mut cancelled_running = false;
        for job in &mut self.image_jobs.jobs {
            if node.is_none_or(|n| job.node == n) && matches!(job.state, ImageJobState::Running | ImageJobState::Queued) {
                if job.state == ImageJobState::Running {
                    cancelled_running = true;
                }
                job.state = ImageJobState::Cancelled;
            }
        }
        self.image_jobs.queue.retain(|jid| self.image_jobs.jobs.iter().any(|j| &j.id == jid && j.state == ImageJobState::Queued));
        if cancelled_running {
            if let Some(w) = self.image_jobs.watcher.take() {
                w.cancel.store(true, Ordering::SeqCst);
            }
            self.pump_next_queued_image_job();
        }
        self.changed();
    }

    /// Opens the `Expand…` popover for `(node, fill_index)` anchored at `anchor`.
    pub fn open_expand_image_popover(&mut self, node: NodeId, fill_index: usize, anchor: Rect) {
        if !has_available_agent_cli(self) {
            self.show_toast(DISABLED_TOOLTIP, true);
            return;
        }
        if !self.check_image_fill_editable(node, fill_index) {
            return;
        }
        let ((crop_w, crop_h), _) = node_fill_visible_crop_dims(&self.doc, node, fill_index).unwrap_or(((100, 100), ImageScaleMode::Fill));
        let pad_x = ((crop_w as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
        let pad_y = ((crop_h as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
        self.image_jobs.erase_popover = None;
        self.image_jobs.expand_popover =
            Some(ExpandPopoverState { node, fill_index, anchor, crop_dims: (crop_w, crop_h), top: pad_y, right: pad_x, bottom: pad_y, left: pad_x });
    }

    /// Opens the `Erase…` popover for `(node, fill_index)` anchored at `anchor`.
    pub fn open_erase_image_popover(&mut self, node: NodeId, fill_index: usize, anchor: Rect) {
        if !has_available_agent_cli(self) {
            self.show_toast(DISABLED_TOOLTIP, true);
            return;
        }
        if !self.check_image_fill_editable(node, fill_index) {
            return;
        }
        let (crop_dims, scale_mode) = node_fill_visible_crop_dims(&self.doc, node, fill_index).unwrap_or(((100, 100), ImageScaleMode::Fill));
        self.image_jobs.expand_popover = None;
        self.image_jobs.erase_popover = Some(ErasePopoverState {
            node,
            fill_index,
            anchor,
            instruction: String::new(),
            focus_requested: true,
            crop_dims,
            scale_mode,
            region: None,
            drag_start_page: None,
        });
    }

    /// Verifies that `(node, fill_index)` is an editable non-tiled `PaintKind::Image` fill.
    fn check_image_fill_editable(&mut self, node: NodeId, fill_index: usize) -> bool {
        let Some(n) = self.doc.get(node) else {
            self.show_toast("Select an image layer first", true);
            return false;
        };
        let Some(paint) = n.fills.get(fill_index) else {
            self.show_toast("Image fill not found on layer", true);
            return false;
        };
        match &paint.kind {
            PaintKind::Image { scale_mode, .. } => {
                if *scale_mode == ImageScaleMode::Tile {
                    self.show_toast("Tiled image fills cannot be edited with AI image tools (ambiguous source)", true);
                    return false;
                }
                true
            }
            _ => {
                self.show_toast("Selected fill is not an image", true);
                false
            }
        }
    }

    /// Enqueues or immediately starts an [`ImageOp`] on `(node, fill_index)`.
    pub fn start_image_job(&mut self, node: NodeId, fill_index: usize, op: ImageOp) -> Option<String> {
        if !has_available_agent_cli(self) {
            self.show_toast(DISABLED_TOOLTIP, true);
            return None;
        }
        if !self.check_image_fill_editable(node, fill_index) {
            return None;
        }
        if let ImageOp::Expand { top, right, bottom, left } = &op {
            if *top == 0 && *right == 0 && *bottom == 0 && *left == 0 {
                self.show_toast("Set at least one side above 0 px", true);
                return None;
            }
            if *top > EXPAND_MAX_SIDE_PX || *right > EXPAND_MAX_SIDE_PX || *bottom > EXPAND_MAX_SIDE_PX || *left > EXPAND_MAX_SIDE_PX {
                self.show_toast(format!("Expand padding must be between 0 and {EXPAND_MAX_SIDE_PX} px"), true);
                return None;
            }
        }
        if let ImageOp::Erase { instruction, .. } = &op {
            if instruction.trim().is_empty() {
                self.show_toast("Describe what to erase", true);
                return None;
            }
        }

        let (image_ref, scale_mode, layer_size) = match self.doc.get(node) {
            Some(n) => match n.fills.get(fill_index).map(|p| &p.kind) {
                Some(PaintKind::Image { image_ref, scale_mode }) => (image_ref.clone(), *scale_mode, n.size),
                _ => return None,
            },
            None => return None,
        };

        let decoded = match decode_doc_image(&self.doc, &image_ref) {
            Ok(img) => img,
            Err(e) => {
                self.show_toast(e, true);
                return None;
            }
        };
        let cropped = crop_visible_image(&decoded, layer_size, scale_mode);
        let (orig_w, orig_h) = cropped.dimensions();
        let expected_size = match &op {
            ImageOp::Expand { top, right, bottom, left } => {
                (orig_w.saturating_add(*left).saturating_add(*right), orig_h.saturating_add(*top).saturating_add(*bottom))
            }
            _ => (orig_w, orig_h),
        };

        let seq = self.image_jobs.next_seq;
        self.image_jobs.next_seq += 1;
        let job_id = format!("job-{seq:04}");
        let root = self.image_jobs_root();
        self.image_jobs.last_session_root = Some(root.clone());
        let job_dir = root.join(&job_id);
        if let Err(e) = crate::agent_session_files::create_session_dir_all(&job_dir) {
            self.show_toast(format!("Cannot create image job folder: {}", crate::agent_session_files::with_next_step(&e)), true);
            return None;
        }

        let input_path = job_dir.join("input.png");
        let output_path = job_dir.join("output.png");
        let request_path = job_dir.join("request.json");
        let _ = crate::agent_session_files::remove_session_file(&output_path);

        let input_png = match encode_png(&cropped) {
            Ok(b) => b,
            Err(e) => {
                self.show_toast(e, true);
                return None;
            }
        };
        if let Err(e) = crate::agent_session_files::atomic_write_session_file(&input_path, &input_png) {
            self.show_toast(format!("Cannot write the input image: {}", crate::agent_session_files::with_next_step(&e)), true);
            return None;
        }

        // Write `mask.png` only when an explicit region rectangle was drawn (`E5-10` §3 rule 1, rule 3).
        let mask_path = if let ImageOp::Erase { region: Some(region_rect), .. } = &op {
            let mp = job_dir.join("mask.png");
            match build_erase_mask_png(orig_w, orig_h, *region_rect) {
                Ok(mask_bytes) => {
                    if let Err(e) = crate::agent_session_files::atomic_write_session_file(&mp, &mask_bytes) {
                        self.show_toast(format!("Cannot write the erase mask: {}", crate::agent_session_files::with_next_step(&e)), true);
                        return None;
                    }
                    Some(mp)
                }
                Err(e) => {
                    self.show_toast(e, true);
                    return None;
                }
            }
        } else {
            None
        };

        let expand_sides = match &op {
            ImageOp::Expand { top, right, bottom, left } => Some(ExpandSides { top: *top, right: *right, bottom: *bottom, left: *left }),
            _ => None,
        };

        let payload = JobRequestPayload {
            op: op.slug().to_string(),
            instruction: op.instruction_text(),
            input: input_path.to_string_lossy().into_owned(),
            output: output_path.to_string_lossy().into_owned(),
            mask: mask_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            expand: expand_sides,
            expected_size: [expected_size.0, expected_size.1],
        };
        let req_json = match serde_json::to_string_pretty(&payload) {
            Ok(s) => s,
            Err(e) => {
                self.show_toast(format!("Cannot serialize request.json: {e}"), true);
                return None;
            }
        };
        if let Err(e) = crate::agent_session_files::atomic_write_session_file(&request_path, req_json.as_bytes()) {
            self.show_toast(format!("Cannot write the job request: {}", crate::agent_session_files::with_next_step(&e)), true);
            return None;
        }

        let job = ImageJob {
            id: job_id.clone(),
            node,
            fill_index,
            op,
            input_path,
            mask_path,
            output_path,
            request_path,
            input_crop_size: (orig_w, orig_h),
            expected_size,
            started_at: Instant::now(),
            state: ImageJobState::Queued,
        };
        self.image_jobs.jobs.push(job);
        self.image_jobs.queue.push_back(job_id.clone());

        if self.image_jobs.watcher.is_some() {
            let pos = self.image_jobs.queue.len();
            self.show_toast(format!("Queued image job · {pos} in queue"), false);
        } else {
            self.pump_next_queued_image_job();
        }
        self.changed();
        Some(job_id)
    }

    /// Starts the next queued job if no job is currently running.
    fn pump_next_queued_image_job(&mut self) {
        let preset = preferred_agent_cli_preset(self).unwrap_or(AgentCliPreset::Claude);
        let fp = self.file_path.clone();
        pump_next_queued_image_job_for(&mut self.image_jobs, &mut self.terminal, fp.as_deref(), preset);
    }

    /// Polls the background `output.png` watcher and applies finished jobs on the UI thread
    /// (for both the active tab and any background tabs parked in `self.tabs.entries`).
    pub fn poll_image_jobs(&mut self) -> bool {
        let mut any_handled = false;

        if let Some(outcome) = try_recv_watcher(&mut self.image_jobs) {
            any_handled = true;
            match outcome {
                WatcherOutcome::Completed { job_id, png_bytes, warning } => {
                    if let Some(job) = self.image_jobs.jobs.iter().find(|j| j.id == job_id).cloned() {
                        if job.state != ImageJobState::Cancelled {
                            match self.apply_completed_image_job(&job, png_bytes) {
                                Ok(new_id) => {
                                    if let Some(j) = self.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                                        j.state = ImageJobState::Done;
                                    }
                                    self.selection = vec![new_id];
                                    if let Some(w) = warning {
                                        self.show_toast(w, false);
                                    } else {
                                        self.show_toast(format!("Applied {}", job.op.layer_suffix()), false);
                                    }
                                }
                                Err(err) => {
                                    if let Some(j) = self.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                                        j.state = ImageJobState::Failed(err.clone());
                                    }
                                    self.show_toast(err, true);
                                }
                            }
                        }
                    }
                }
                WatcherOutcome::Failed { job_id, reason } => {
                    if let Some(j) = self.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                        if j.state != ImageJobState::Cancelled {
                            j.state = ImageJobState::Failed(reason.clone());
                            self.show_toast(reason, true);
                        }
                    }
                }
            }
            self.pump_next_queued_image_job();
            self.changed();
        }

        let tab_len = self.tabs.entries.len();
        for idx in 0..tab_len {
            let is_parked = self.tabs.parked_in_home || self.home.is_some() || !self.tabs.is_hist_in_app() || idx != self.tabs.active_idx;
            if !is_parked {
                continue;
            }
            let Some(outcome) = try_recv_watcher(&mut self.tabs.entries[idx].image_jobs) else {
                continue;
            };
            any_handled = true;
            let mut toast_to_show: Option<(String, bool)> = None;
            {
                let tab = &mut self.tabs.entries[idx];
                match outcome {
                    WatcherOutcome::Completed { job_id, png_bytes, warning } => {
                        if let Some(job) = tab.image_jobs.jobs.iter().find(|j| j.id == job_id).cloned() {
                            if job.state != ImageJobState::Cancelled {
                                match build_completed_image_job_tx(&mut tab.doc, &job, png_bytes)
                                    .and_then(|(new_id, tx)| tx.commit(&mut tab.doc, &mut tab.hist).map(|_| new_id).map_err(|e| format!("Edit failed: {e}")))
                                {
                                    Ok(new_id) => {
                                        if let Some(j) = tab.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                                            j.state = ImageJobState::Done;
                                        }
                                        tab.selection = vec![new_id];
                                        if let (Some(lib), Some(lf)) = (&self.lib, &tab.lib_file) {
                                            // The job is a step of the tab's history, so the dot already shows it. A write
                                            // that took the whole document leaves the tab as the file holds it.
                                            if Self::save_doc_to_library(lib, lf, &mut tab.doc, &mut tab.hist).is_ok() {
                                                tab.hist.mark_clean();
                                                tab.dirty = false;
                                                // The file now holds the document: its tab must not read the write as another program's.
                                                tab.disk = crate::home::DiskState {
                                                    saved_hash: crate::home::doc_hash(&tab.doc),
                                                    disk_mtime: std::fs::metadata(crate::home::library_file_disk_path(lib, &lf.project_id, &lf.id))
                                                        .ok()
                                                        .and_then(|m| m.modified().ok()),
                                                    conflict_mtime: None,
                                                };
                                            }
                                        }
                                        let msg = warning.unwrap_or_else(|| format!("Applied {}", job.op.layer_suffix()));
                                        toast_to_show = Some((msg, false));
                                    }
                                    Err(err) => {
                                        if let Some(j) = tab.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                                            j.state = ImageJobState::Failed(err.clone());
                                        }
                                        toast_to_show = Some((err, true));
                                    }
                                }
                            }
                        }
                    }
                    WatcherOutcome::Failed { job_id, reason } => {
                        if let Some(j) = tab.image_jobs.jobs.iter_mut().find(|j| j.id == job_id) {
                            if j.state != ImageJobState::Cancelled {
                                j.state = ImageJobState::Failed(reason.clone());
                                toast_to_show = Some((reason, true));
                            }
                        }
                    }
                }
                let fp = tab.file_path.clone();
                let tab_preset = preferred_agent_cli_preset_for(&tab.terminal, &tab.image_jobs).unwrap_or(AgentCliPreset::Claude);
                pump_next_queued_image_job_for(&mut tab.image_jobs, &mut tab.terminal, fp.as_deref(), tab_preset);
            }
            if let Some((msg, is_err)) = toast_to_show {
                self.show_toast(msg, is_err);
            }
            self.changed();
        }

        any_handled
    }

    /// Inserts the result layer above the original and hides the original in one undo step (`Tx`).
    fn apply_completed_image_job(&mut self, job: &ImageJob, png_bytes: Vec<u8>) -> Result<NodeId, String> {
        let (new_id, tx) = build_completed_image_job_tx(&mut self.doc, job, png_bytes)?;
        if !self.run_tx(tx) {
            return Err("Failed to commit AI image edit".to_string());
        }
        Ok(new_id)
    }
}

fn try_recv_watcher(image_jobs: &mut ImageJobsState) -> Option<WatcherOutcome> {
    let outcome = match &image_jobs.watcher {
        Some(w) => match w.rx.try_recv() {
            Ok(out) => Some(out),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(WatcherOutcome::Failed { job_id: w.job_id.clone(), reason: "Image job stopped unexpectedly. Try again.".to_string() })
            }
        },
        None => None,
    };
    if outcome.is_some() {
        image_jobs.watcher = None;
    }
    outcome
}

fn pump_next_queued_image_job_for(
    image_jobs: &mut ImageJobsState,
    terminal: &mut crate::terminal::TerminalState,
    file_path: Option<&std::path::Path>,
    preset: AgentCliPreset,
) {
    if image_jobs.watcher.is_some() {
        return;
    }
    while let Some(next_id) = image_jobs.queue.pop_front() {
        let Some(idx) = image_jobs.jobs.iter().position(|j| j.id == next_id && j.state == ImageJobState::Queued) else {
            continue;
        };
        image_jobs.jobs[idx].state = ImageJobState::Running;
        image_jobs.jobs[idx].started_at = Instant::now();
        let job = image_jobs.jobs[idx].clone();

        let prompt = format!(
            "Image task {}: {}. Read {}, write the result as PNG to {}. Keep the same pixel size unless told otherwise. Do not change other files.",
            job.id,
            job.op.instruction_text(),
            job.input_path.display(),
            job.output_path.display()
        );

        let status_file_path = terminal.hook_paths().status();
        // Clear any stale `done` status file before starting a new job.
        if let Some(raw) = crate::agent_session_files::read_session_text(&status_file_path) {
            if matches!(parse_agent_status_payload(raw.trim()), Some(AgentTabStatus::Done)) {
                let _ = crate::agent_session_files::remove_session_file(&status_file_path);
            }
        }

        if image_jobs.agent_script.is_none() {
            terminal.submit_system_image_prompt(preset, &prompt, file_path);
        }

        let tab_status: Option<Arc<Mutex<AgentTabStatus>>> = terminal.active_tab().map(|t| Arc::clone(&t.agent_status));
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_bg = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        let stable_window = image_jobs.stable_window;
        let timeout = image_jobs.job_timeout;
        let script = image_jobs.agent_script.clone();
        let job_id = job.id.clone();

        std::thread::spawn(move || {
            let outcome = run_watcher_thread(job, script, stable_window, timeout, cancel_bg, tab_status, status_file_path);
            let _ = tx.send(outcome);
        });

        image_jobs.watcher = Some(RunningWatcher { job_id, cancel, rx });
        break;
    }
}

fn build_completed_image_job_tx(doc: &mut Document, job: &ImageJob, png_bytes: Vec<u8>) -> Result<(NodeId, Tx), String> {
    let orig = doc.get(job.node).cloned().ok_or_else(|| "Target image layer no longer exists".to_string())?;
    let parent = orig.parent.ok_or_else(|| "Target image layer has no parent container".to_string())?;
    let orig_index = doc.get(parent).and_then(|p| p.children.iter().position(|&c| c == job.node)).unwrap_or(0);

    let mut key_seq = doc.images.len() + 1;
    let new_image_key = loop {
        let candidate = format!("ai-{}-{key_seq}", job.id);
        if !doc.images.contains_key(&candidate) {
            break candidate;
        }
        key_seq += 1;
    };

    let new_id = doc.alloc_id();
    let mut new_node = orig.clone();
    new_node.id = new_id;
    new_node.parent = Some(parent);
    new_node.children.clear();
    new_node.name = format!("{} · {}", orig.name, job.op.layer_suffix());
    new_node.visible = true;
    new_node.kind = NodeKind::Rectangle;

    let (orig_scale_mode, orig_opacity, orig_blend) = orig
        .fills
        .get(job.fill_index)
        .and_then(|p| match &p.kind {
            PaintKind::Image { scale_mode, .. } => Some((*scale_mode, p.opacity, p.blend)),
            _ => None,
        })
        .unwrap_or((ImageScaleMode::Fill, 1.0, BlendMode::Normal));

    let new_paint = Paint {
        visible: true,
        opacity: orig_opacity,
        blend: orig_blend,
        kind: PaintKind::Image { image_ref: new_image_key.clone(), scale_mode: orig_scale_mode },
    };
    if job.fill_index < new_node.fills.len() {
        new_node.fills[job.fill_index] = new_paint;
    } else {
        new_node.fills = vec![new_paint];
    }

    if let ImageOp::Expand { top, right, bottom, left } = job.op {
        let cw = job.input_crop_size.0.max(1) as f32;
        let ch = job.input_crop_size.1.max(1) as f32;
        let (scale_x, scale_y) = match orig_scale_mode {
            ImageScaleMode::Fit => {
                let s = (orig.size.x.max(1.0) / cw).min(orig.size.y.max(1.0) / ch);
                (s, s)
            }
            _ => (orig.size.x.max(1.0) / cw, orig.size.y.max(1.0) / ch),
        };
        let pad_left = left as f32 * scale_x;
        let pad_right = right as f32 * scale_x;
        let pad_top = top as f32 * scale_y;
        let pad_bottom = bottom as f32 * scale_y;
        new_node.size = Vec2::new(orig.size.x + pad_left + pad_right, orig.size.y + pad_top + pad_bottom);
        new_node.transform = orig.transform * Affine::translate(-pad_left, -pad_top);
    }

    let mut add = BTreeMap::new();
    add.insert(new_image_key, png_bytes);

    let mut tx = Tx::new();
    tx.cmd(doc, Command::SetImages { add, remove: Vec::new() }).map_err(|e| format!("Failed to store result image: {e}"))?;
    tx.cmd(doc, Command::Insert { parent, index: orig_index + 1, nodes: vec![new_node] }).map_err(|e| format!("Failed to insert result layer: {e}"))?;
    if let Some(orig_mut) = tx.ed.node(doc, job.node) {
        orig_mut.visible = false;
    }
    Ok((new_id, tx))
}

fn is_agent_run_done(tab_status: &Option<Arc<Mutex<AgentTabStatus>>>, status_file_path: &std::path::Path) -> bool {
    if let Some(raw) = crate::agent_session_files::read_session_text(status_file_path) {
        if let Some(parsed) = parse_agent_status_payload(raw.trim()) {
            if let Some(ts) = tab_status {
                if let Ok(mut guard) = ts.lock() {
                    *guard = parsed.clone();
                }
            }
            if matches!(parsed, AgentTabStatus::Done) {
                return true;
            }
        }
    }
    if let Some(ts) = tab_status {
        if let Ok(guard) = ts.lock() {
            if matches!(*guard, AgentTabStatus::Done) {
                return true;
            }
        }
    }
    false
}

fn run_stub_agent_script(cmd_str: &str, job: &ImageJob, cancel: &AtomicBool) {
    let mut all_handled_in_process = true;
    for raw_step in cmd_str.split("&&") {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        let step = raw_step.trim();
        if let Some(rest) = step.strip_prefix("sleep ") {
            if let Ok(secs) = rest.trim().parse::<f64>() {
                let deadline = Instant::now() + Duration::from_secs_f64(secs);
                while Instant::now() < deadline {
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                continue;
            }
        }
        if let Some(rest) = step.strip_prefix("cp \"") {
            if let Some(src) = rest.strip_suffix("\" \"$DESIGNOS_IMAGE_OUTPUT\"") {
                if let Some(parent) = job.output_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(src, &job.output_path);
                continue;
            }
        }
        all_handled_in_process = false;
        break;
    }
    if all_handled_in_process {
        return;
    }

    #[cfg(unix)]
    let mut cmd = {
        let mut c = std::process::Command::new("/bin/sh");
        c.arg("-c").arg(cmd_str);
        c
    };
    #[cfg(not(unix))]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(cmd_str);
        c
    };
    cmd.env("DESIGNOS_IMAGE_JOB_ID", &job.id)
        .env("DESIGNOS_IMAGE_OP", job.op.slug())
        .env("DESIGNOS_IMAGE_REQUEST", &job.request_path)
        .env("DESIGNOS_IMAGE_INPUT", &job.input_path)
        .env("DESIGNOS_IMAGE_OUTPUT", &job.output_path);
    if let Some(mp) = &job.mask_path {
        cmd.env("DESIGNOS_IMAGE_MASK", mp);
    }
    let _ = cmd.output();
}

fn run_watcher_thread(
    job: ImageJob,
    script: Option<String>,
    stable_window: Duration,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    tab_status: Option<Arc<Mutex<AgentTabStatus>>>,
    status_file_path: PathBuf,
) -> WatcherOutcome {
    if let Some(cmd_str) = script {
        run_stub_agent_script(&cmd_str, &job, &cancel);
    }

    let start = Instant::now();
    let poll_interval = stable_window.checked_div(2).unwrap_or(Duration::from_millis(10)).clamp(Duration::from_millis(5), Duration::from_millis(100));
    let mut last_sig: Option<(u64, Option<std::time::SystemTime>, Instant)> = None;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return WatcherOutcome::Failed { job_id: job.id, reason: "Cancelled".to_string() };
        }
        if start.elapsed() > timeout {
            return WatcherOutcome::Failed { job_id: job.id, reason: "AI image job timed out after 5 minutes. Try again.".to_string() };
        }

        if let Ok(meta) = std::fs::metadata(&job.output_path) {
            let len = meta.len();
            let mtime = meta.modified().ok();
            if len > 0 {
                match &mut last_sig {
                    Some((prev_len, prev_mtime, stable_since)) if *prev_len == len && *prev_mtime == mtime => {
                        if stable_since.elapsed() >= stable_window {
                            return validate_output_file(&job);
                        }
                    }
                    _ => {
                        last_sig = Some((len, mtime, Instant::now()));
                    }
                }
            } else if is_agent_run_done(&tab_status, &status_file_path) {
                return WatcherOutcome::Failed { job_id: job.id, reason: "Agent returned no image. Try again.".to_string() };
            }
        } else if is_agent_run_done(&tab_status, &status_file_path) {
            return WatcherOutcome::Failed { job_id: job.id, reason: "Agent returned no image. Try again.".to_string() };
        }
        std::thread::sleep(poll_interval);
    }
}

fn validate_output_file(job: &ImageJob) -> WatcherOutcome {
    let raw = match crate::agent_session_files::read_session_file(&job.output_path) {
        Ok(b) => b,
        Err(e) => {
            return WatcherOutcome::Failed {
                job_id: job.id.clone(),
                reason: format!("Cannot read output.png: {}", crate::agent_session_files::with_next_step(&e)),
            };
        }
    };
    // Decode by content sniffing so a JPEG saved with a .png extension still decodes (§8.3).
    let decoded = match image::load_from_memory(&raw) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            return WatcherOutcome::Failed { job_id: job.id.clone(), reason: format!("Agent wrote unreadable image output: {e}") };
        }
    };
    let (got_w, got_h) = decoded.dimensions();
    let (exp_w, exp_h) = job.expected_size;
    if (got_w, got_h) != (exp_w, exp_h) {
        return WatcherOutcome::Failed { job_id: job.id.clone(), reason: format!("Result is {got_w}×{got_h}, expected {exp_w}×{exp_h}. Try again.") };
    }

    let warning = if matches!(job.op, ImageOp::RemoveBackground | ImageOp::IsolateSubject) {
        let has_alpha = decoded.pixels().any(|px| px.0[3] < 255);
        if !has_alpha {
            Some("Applied, but the result has no transparency".to_string())
        } else {
            None
        }
    } else {
        None
    };

    let png_bytes = match encode_png(&decoded) {
        Ok(b) => b,
        Err(e) => {
            return WatcherOutcome::Failed { job_id: job.id.clone(), reason: e };
        }
    };

    WatcherOutcome::Completed { job_id: job.id.clone(), png_bytes, warning }
}

// ---------------------------------------------------------------- UI Surfaces

/// Renders the 24x24 `AI` menu button on an image fill row in the Inspector (`panels/properties.rs`).
pub fn paint_row_ai_button(app: &mut App, ui: &mut egui::Ui, rect: Rect, node: NodeId, fill_index: usize) {
    widgets::record(ui, &format!("ai-fill-btn:{fill_index}"), rect);
    let is_open = app.image_jobs.open_fill_menu.is_some_and(|(n, idx, _)| n == node && idx == fill_index);
    let resp = ui.interact(rect, ui.id().with(("ai-fill-btn", node.0, fill_index)), Sense::click());
    let bg = if is_open {
        theme::BG_SELECTED
    } else if resp.hovered() {
        theme::BG_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), bg);
    }
    let col = if is_open || resp.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "AI", theme::font_caption_strong(), col);
    if resp.on_hover_text("AI image tools").clicked() {
        if is_open {
            app.image_jobs.open_fill_menu = None;
        } else {
            app.image_jobs.open_fill_menu = Some((node, fill_index, rect));
        }
    }
}

/// Renders the `Working…` chip and `Cancel` button on a layer row with an active image job (`panels/layers.rs`).
/// Returns the x coordinate of the left edge of the chip so the layer name elides cleanly before it.
pub fn layer_row_progress_chip(app: &mut App, ui: &mut egui::Ui, p: &egui::Painter, row_rect: Rect, right_limit: f32, node: NodeId) -> Option<f32> {
    let job = app.image_jobs.active_job_for_node(node)?;
    let status_text = match job.state {
        ImageJobState::Queued => "Queued…",
        _ => "Working…",
    };
    let font = theme::font_caption();
    let status_w = widgets::text_width(p, status_text, &font);
    let cancel_w = widgets::text_width(p, "Cancel", &font) + 12.0;
    let chip_w = status_w + 8.0 + cancel_w + 12.0;
    let chip_h = 20.0;
    let chip_right = right_limit - 4.0;
    let chip_left = (chip_right - chip_w).max(row_rect.left() + theme::SPACE_REGION);
    let chip_rect = Rect::from_min_size(Pos2::new(chip_left, row_rect.center().y - chip_h * 0.5), egui::vec2(chip_right - chip_left, chip_h));
    widgets::record(ui, &format!("layer-ai-chip:{}", node.0), chip_rect);
    p.rect_filled(chip_rect, theme::cr_pill(chip_h), theme::BG_SECONDARY);
    p.rect_stroke(chip_rect, theme::cr_pill(chip_h), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    p.text(
        Pos2::new(chip_rect.left() + theme::INSET_CONTROL - theme::INSET_SEGMENT, chip_rect.center().y),
        Align2::LEFT_CENTER,
        status_text,
        font.clone(),
        theme::TEXT_SECONDARY,
    );

    let cancel_hit = Rect::from_min_size(
        Pos2::new(chip_rect.right() - cancel_w - theme::INSET_SEGMENT, row_rect.center().y - theme::SPACE_GROUP),
        egui::vec2(cancel_w.max(24.0), 24.0),
    );
    let cancel_vis = Rect::from_min_size(
        Pos2::new(chip_rect.right() - cancel_w - theme::INSET_SEGMENT, chip_rect.top() + theme::INSET_SEGMENT),
        egui::vec2(cancel_w, chip_h - 4.0),
    );
    widgets::record(ui, &format!("layer-ai-cancel:{}", node.0), cancel_hit);
    let c_resp = ui.interact(cancel_hit, ui.id().with(("layer-ai-cancel", node.0)), Sense::click());
    if c_resp.hovered() {
        p.rect_filled(cancel_vis, theme::cr_pill(chip_h - 4.0), theme::BG_HOVER);
    }
    p.text(cancel_vis.center(), Align2::CENTER_CENTER, "Cancel", font, if c_resp.hovered() { theme::TEXT } else { theme::TEXT_SECONDARY });
    if c_resp.on_hover_text("Cancel AI image job").clicked() {
        app.cancel_image_job(Some(node));
    }
    Some(chip_left - 4.0)
}

/// Renders open AI image popovers (`AI` fill menu, `Expand…` popover, `Erase…` popover + canvas mask overlay).
pub fn show_popovers(app: &mut App, ctx: &Context) {
    show_fill_ai_menu(app, ctx);
    show_expand_popover(app, ctx);
    show_erase_popover(app, ctx);
}

fn show_fill_ai_menu(app: &mut App, ctx: &Context) {
    let Some((node, fill_index, anchor)) = app.image_jobs.open_fill_menu else {
        return;
    };
    let available = has_available_agent_cli(app);
    let tip = if available { "" } else { DISABLED_TOOLTIP };
    let items = vec![
        widgets::menu::MenuItem::new("Remove background").disabled(!available).tooltip(tip),
        widgets::menu::MenuItem::new("Isolate subject").disabled(!available).tooltip(tip).sep(),
        widgets::menu::MenuItem::new("Expand…").disabled(!available).tooltip(tip),
        widgets::menu::MenuItem::new("Erase…").disabled(!available).tooltip(tip),
    ];
    let menu_dx = theme::SPACE_REGION * 3.0 - theme::SPACE_ROW;
    let pos = Pos2::new(anchor.left() - menu_dx, anchor.bottom() + theme::SPACE_ROW);
    match widgets::menu::popup_menu(ctx, Id::new(("ai-image-fill-menu", node.0, fill_index)), pos, 180.0, &items, Some(anchor)) {
        widgets::menu::MenuResult::Picked(idx) => {
            app.image_jobs.open_fill_menu = None;
            match idx {
                0 => {
                    app.start_image_job(node, fill_index, ImageOp::RemoveBackground);
                }
                1 => {
                    app.start_image_job(node, fill_index, ImageOp::IsolateSubject);
                }
                2 => {
                    app.open_expand_image_popover(node, fill_index, anchor);
                }
                3 => {
                    app.open_erase_image_popover(node, fill_index, anchor);
                }
                _ => {}
            }
        }
        widgets::menu::MenuResult::Closed => {
            app.image_jobs.open_fill_menu = None;
        }
        widgets::menu::MenuResult::Open => {}
    }
}

fn show_expand_popover(app: &mut App, ctx: &Context) {
    let Some(mut pop) = app.image_jobs.expand_popover.take() else {
        return;
    };
    let Some(node_ref) = app.doc.get(pop.node) else {
        return;
    };
    let total_fills = node_ref.fills.iter().filter(|p| matches!(p.kind, PaintKind::Image { .. })).count();

    let screen = ctx.content_rect();
    let w = 236.0;
    let h = if total_fills > 1 { 176.0 } else { 156.0 };
    let inspector_left = screen.right() - theme::RIGHT_PANEL_WIDTH;
    let target_x = if pop.anchor.center().x >= inspector_left { inspector_left - w - 8.0 } else { pop.anchor.left() - w - 8.0 };
    let x = target_x.clamp(screen.left() + theme::SPACE_INLINE, (screen.right() - w - theme::SPACE_INLINE).max(screen.left() + theme::SPACE_INLINE));
    let y = pop.anchor.top().clamp(screen.top() + theme::SPACE_INLINE, (screen.bottom() - h - theme::SPACE_INLINE).max(screen.top() + theme::SPACE_INLINE));
    let rect = Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h));

    let mut run_clicked = false;
    let mut close_clicked = false;

    Area::new(Id::new("ai-expand-popover")).order(Order::Foreground).fixed_pos(rect.min).interactable(true).show(ctx, |ui| {
        ui.allocate_exact_size(rect.size(), Sense::click());
        widgets::record(ui, "ai-expand-popover", rect);
        let p = ui.painter();
        widgets::menu_panel(p, rect, theme::RADIUS_FRAME);

        let pad = theme::INSET_POPOVER;
        let mut cy = rect.top() + pad;
        p.text(Pos2::new(rect.left() + pad, cy + 8.0), Align2::LEFT_CENTER, "Expand image", theme::font_body_strong(), theme::TEXT);

        // 25% preset button (uses visible crop pixel dimensions, not layer point size)
        let preset_r = Rect::from_min_size(Pos2::new(rect.right() - pad - 52.0, cy - 2.0), egui::vec2(52.0, 24.0));
        widgets::record(ui, "ai-expand-preset-25", preset_r);
        let pr_resp = ui.interact(preset_r, ui.id().with("ai-expand-25"), Sense::click());
        p.rect_filled(preset_r, theme::cr(theme::RADIUS_CONTROL), if pr_resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        p.text(preset_r.center(), Align2::CENTER_CENTER, "+25%", theme::font_mono(), theme::TEXT);
        if pr_resp.on_hover_text("Set each side to 25% of image dimensions").clicked() {
            let px = ((pop.crop_dims.0 as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
            let py = ((pop.crop_dims.1 as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
            pop.top = py;
            pop.bottom = py;
            pop.left = px;
            pop.right = px;
        }
        cy += 24.0;

        if total_fills > 1 {
            p.text(
                Pos2::new(rect.left() + pad, cy + 6.0),
                Align2::LEFT_CENTER,
                format!("Target: Image fill #{} (topmost visible)", pop.fill_index + 1),
                theme::font_caption(),
                theme::TEXT_SECONDARY,
            );
            cy += 20.0;
        }

        let col_w = (w - pad * 2.0 - 8.0) * 0.5;
        let r_top = Rect::from_min_size(Pos2::new(rect.left() + pad, cy), egui::vec2(col_w, 24.0));
        let r_right = Rect::from_min_size(Pos2::new(rect.left() + pad + col_w + 8.0, cy), egui::vec2(col_w, 24.0));
        if let Some(v) = field::num_field(ui, r_top, "ai-expand-top", Prefix::Text("T"), Some(pop.top as f32), "px") {
            pop.top = (v.round().max(0.0) as u32).min(EXPAND_MAX_SIDE_PX);
        }
        if let Some(v) = field::num_field(ui, r_right, "ai-expand-right", Prefix::Text("R"), Some(pop.right as f32), "px") {
            pop.right = (v.round().max(0.0) as u32).min(EXPAND_MAX_SIDE_PX);
        }
        cy += 32.0;

        let r_bottom = Rect::from_min_size(Pos2::new(rect.left() + pad, cy), egui::vec2(col_w, 24.0));
        let r_left = Rect::from_min_size(Pos2::new(rect.left() + pad + col_w + 8.0, cy), egui::vec2(col_w, 24.0));
        if let Some(v) = field::num_field(ui, r_bottom, "ai-expand-bottom", Prefix::Text("B"), Some(pop.bottom as f32), "px") {
            pop.bottom = (v.round().max(0.0) as u32).min(EXPAND_MAX_SIDE_PX);
        }
        if let Some(v) = field::num_field(ui, r_left, "ai-expand-left", Prefix::Text("L"), Some(pop.left as f32), "px") {
            pop.left = (v.round().max(0.0) as u32).min(EXPAND_MAX_SIDE_PX);
        }
        cy += 36.0;

        let cancel_r = Rect::from_min_size(Pos2::new(rect.left() + pad, cy), egui::vec2(col_w, 24.0));
        let run_r = Rect::from_min_size(Pos2::new(rect.left() + pad + col_w + 8.0, cy), egui::vec2(col_w, 24.0));
        widgets::record(ui, "ai-expand-cancel", cancel_r);
        widgets::record(ui, "ai-expand-run", run_r);
        let c_resp = ui.interact(cancel_r, ui.id().with("ai-expand-cancel"), Sense::click());
        ui.painter().rect_filled(cancel_r, theme::cr(theme::RADIUS_CONTROL), if c_resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        ui.painter().text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", theme::font_body(), theme::TEXT);
        if c_resp.clicked() {
            close_clicked = true;
        }

        let r_resp = ui.interact(run_r, ui.id().with("ai-expand-run"), Sense::click());
        ui.painter().rect_filled(run_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_BRAND);
        ui.painter().text(run_r.center(), Align2::CENTER_CENTER, "Expand", theme::font_body_strong(), Color32::WHITE);
        if r_resp.clicked() {
            run_clicked = true;
        }
    });

    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    if run_clicked {
        app.start_image_job(pop.node, pop.fill_index, ImageOp::Expand { top: pop.top, right: pop.right, bottom: pop.bottom, left: pop.left });
    } else if !close_clicked && !esc {
        app.image_jobs.expand_popover = Some(pop);
    }
}

fn show_erase_popover(app: &mut App, ctx: &Context) {
    let Some(mut pop) = app.image_jobs.erase_popover.take() else {
        return;
    };
    let Some(node_ref) = app.doc.get(pop.node).cloned() else {
        return;
    };
    let total_fills = node_ref.fills.iter().filter(|p| matches!(p.kind, PaintKind::Image { .. })).count();

    let screen = ctx.content_rect();
    let w = 248.0;
    let h = if total_fills > 1 { 164.0 } else { 144.0 };
    let inspector_left = screen.right() - theme::RIGHT_PANEL_WIDTH;
    let target_x = if pop.anchor.center().x >= inspector_left { inspector_left - w - 8.0 } else { pop.anchor.left() - w - 8.0 };
    let x = target_x.clamp(screen.left() + theme::SPACE_INLINE, (screen.right() - w - theme::SPACE_INLINE).max(screen.left() + theme::SPACE_INLINE));
    let y = pop.anchor.top().clamp(screen.top() + theme::SPACE_INLINE, (screen.bottom() - h - theme::SPACE_INLINE).max(screen.top() + theme::SPACE_INLINE));
    let rect = Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h));

    // Optional canvas drag-to-mask interaction while the Erase popover is open.
    draw_and_interact_erase_mask_overlay(app, ctx, &node_ref, rect, &mut pop);

    let mut run_clicked = false;
    let mut close_clicked = false;

    Area::new(Id::new("ai-erase-popover")).order(Order::Foreground).fixed_pos(rect.min).interactable(true).show(ctx, |ui| {
        ui.allocate_exact_size(rect.size(), Sense::click());
        widgets::record(ui, "ai-erase-popover", rect);
        let p = ui.painter();
        widgets::menu_panel(p, rect, theme::RADIUS_FRAME);

        let pad = theme::INSET_POPOVER;
        let mut cy = rect.top() + pad;
        p.text(Pos2::new(rect.left() + pad, cy + 8.0), Align2::LEFT_CENTER, "Erase from image", theme::font_body_strong(), theme::TEXT);
        cy += 22.0;

        if total_fills > 1 {
            p.text(
                Pos2::new(rect.left() + pad, cy + 6.0),
                Align2::LEFT_CENTER,
                format!("Target: Image fill #{} (topmost visible)", pop.fill_index + 1),
                theme::font_caption(),
                theme::TEXT_SECONDARY,
            );
            cy += 20.0;
        }

        let input_r = Rect::from_min_size(Pos2::new(rect.left() + pad, cy), egui::vec2(w - pad * 2.0, 24.0));
        widgets::record(ui, "ai-erase-instruction", input_r);
        ui.painter().rect_filled(input_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        let input_id = ui.id().with("ai-erase-instruction-input");
        if pop.focus_requested {
            ui.memory_mut(|m| m.request_focus(input_id));
            pop.focus_requested = false;
        }
        if let Some(committed) = field::edit_text(ui, input_r, input_id, &pop.instruction, theme::font_body(), theme::TEXT, 8.0) {
            pop.instruction = committed;
        } else if let Some(live) = ui.data(|d| d.get_temp::<String>(input_id)) {
            pop.instruction = live;
        }
        if pop.instruction.is_empty() && !ui.memory(|m| m.has_focus(input_id)) {
            ui.painter().text(
                Pos2::new(input_r.left() + theme::INSET_CONTROL, input_r.center().y),
                Align2::LEFT_CENTER,
                "e.g. the person on the left",
                theme::font_body(),
                theme::TEXT_TERTIARY,
            );
        }
        cy += 28.0;

        let region_text = match pop.region {
            Some([rx, ry, rw, rh]) => format!("Mask: {rx},{ry} {rw}×{rh}px"),
            None => "Drag on image to mark erase region (optional)".to_string(),
        };
        ui.painter().text(
            Pos2::new(rect.left() + pad, cy + 6.0),
            Align2::LEFT_CENTER,
            region_text,
            if pop.region.is_some() { theme::font_mono() } else { theme::font_caption() },
            theme::TEXT_SECONDARY,
        );
        cy += 22.0;

        let col_w = (w - pad * 2.0 - 8.0) * 0.5;
        let cancel_r = Rect::from_min_size(Pos2::new(rect.left() + pad, cy), egui::vec2(col_w, 24.0));
        let run_r = Rect::from_min_size(Pos2::new(rect.left() + pad + col_w + 8.0, cy), egui::vec2(col_w, 24.0));
        widgets::record(ui, "ai-erase-cancel", cancel_r);
        widgets::record(ui, "ai-erase-run", run_r);
        let c_resp = ui.interact(cancel_r, ui.id().with("ai-erase-cancel"), Sense::click());
        ui.painter().rect_filled(cancel_r, theme::cr(theme::RADIUS_CONTROL), if c_resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        ui.painter().text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", theme::font_body(), theme::TEXT);
        if c_resp.clicked() {
            close_clicked = true;
        }

        let r_resp = ui.interact(run_r, ui.id().with("ai-erase-run"), Sense::click());
        ui.painter().rect_filled(run_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_BRAND);
        ui.painter().text(run_r.center(), Align2::CENTER_CENTER, "Erase", theme::font_body_strong(), Color32::WHITE);
        if r_resp.clicked() {
            run_clicked = true;
        }
    });

    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    if run_clicked {
        let instruction = pop.instruction.clone();
        let region = pop.region;
        if app.start_image_job(pop.node, pop.fill_index, ImageOp::Erase { instruction, region }).is_none() {
            app.image_jobs.erase_popover = Some(pop);
        }
    } else if !close_clicked && !esc {
        app.image_jobs.erase_popover = Some(pop);
    }
}

fn draw_and_interact_erase_mask_overlay(app: &App, ctx: &Context, node: &Node, popover_rect: Rect, pop: &mut ErasePopoverState) {
    let image_dims = pop.crop_dims;
    let world = app.doc.world_transform(node.id);
    let node_size = Vec2::new(node.size.x.max(1.0), node.size.y.max(1.0));

    let (pressed, down, released, ptr) =
        ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos()));
    if let Some(pos) = ptr {
        if pressed && app.canvas_rect.contains(pos) && !popover_rect.contains(pos) {
            let p_page = app.view.to_page(app.canvas_rect.min, pos);
            pop.drag_start_page = Some(Pos2::new(p_page.x, p_page.y));
        }
        if (down || released) && pop.drag_start_page.is_some() {
            let start = pop.drag_start_page.unwrap();
            let cur_v = app.view.to_page(app.canvas_rect.min, pos);
            let cur = Pos2::new(cur_v.x, cur_v.y);
            if let Some(region) = page_drag_to_image_region(world, node_size, pop.scale_mode, image_dims, start, cur) {
                pop.region = Some(region);
            }
            if released {
                pop.drag_start_page = None;
            }
        }
    }

    if let Some([rx, ry, rw, rh]) = pop.region {
        let (cx, cy, cw_local, ch_local) = local_image_content_rect(node_size, pop.scale_mode, image_dims);
        let sx = cw_local / image_dims.0.max(1) as f32;
        let sy = ch_local / image_dims.1.max(1) as f32;
        let local_x0 = cx + rx as f32 * sx;
        let local_y0 = cy + ry as f32 * sy;
        let local_x1 = cx + (rx + rw) as f32 * sx;
        let local_y1 = cy + (ry + rh) as f32 * sy;
        let p0 = world.apply(Vec2::new(local_x0, local_y0));
        let p1 = world.apply(Vec2::new(local_x1, local_y1));
        let s0 = app.view.to_screen(app.canvas_rect.min, p0);
        let s1 = app.view.to_screen(app.canvas_rect.min, p1);
        let mask_screen = Rect::from_two_pos(s0, s1);
        Area::new(Id::new("ai-erase-mask-overlay")).order(Order::Middle).fixed_pos(mask_screen.min).interactable(false).show(ctx, |ui| {
            let painter = ui.painter();
            painter.rect_filled(mask_screen, 0.0, theme::CANVAS_MARQUEE_FILL);
            painter.rect_stroke(mask_screen, 0.0, Stroke::new(1.5, theme::CANVAS_SELECTION), StrokeKind::Inside);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::Rect as DRect;
    use std::path::Path;
    use std::sync::atomic::AtomicU64;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("designos-e5-10-test-{}-{label}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Builds a test `App` with one 100x100 layer at `(0, 0)` holding an opaque RGBA PNG fill.
    fn sample_app_with_image_layer(temp_dir: &Path) -> (App, NodeId, String) {
        let mut doc = Document::new("Image Tools Spec");
        let page = doc.add_page("Page 1");
        let mut img = image::RgbaImage::from_pixel(100, 100, image::Rgba([220, 40, 40, 255]));
        for y in 20..80 {
            for x in 20..80 {
                img.put_pixel(x, y, image::Rgba([40, 120, 240, 255]));
            }
        }
        let png_bytes = encode_png(&img).expect("encode test PNG");
        let img_key = "source-hero-png".to_string();
        doc.images.insert(img_key.clone(), png_bytes);

        let nid = doc.alloc_id();
        let mut node = Node::rectangle(nid, DRect::new(0.0, 0.0, 100.0, 100.0));
        node.name = "Hero Photo".to_string();
        node.fills = vec![Paint {
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
            kind: PaintKind::Image { image_ref: img_key.clone(), scale_mode: ImageScaleMode::Fill },
        }];
        doc.insert(page, node).unwrap();

        let mut app = App::new(doc);
        app.selection = vec![nid];
        app.image_jobs.override_agent_installed = Some(true);
        app.image_jobs.custom_jobs_root = Some(temp_dir.join("image-jobs"));
        app.image_jobs.stable_window = Duration::from_millis(15);
        app.image_jobs.job_timeout = Duration::from_secs(5);
        (app, nid, img_key)
    }

    /// Writes a PNG file with `(w, h)` and transparent corners (`alpha = 0` at `(0,0)`).
    fn write_rgba_png(path: &Path, w: u32, h: u32, transparent_corners: bool) {
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([30, 200, 100, 255]));
        if transparent_corners {
            img.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
            img.put_pixel(w - 1, 0, image::Rgba([0, 0, 0, 0]));
            img.put_pixel(0, h - 1, image::Rgba([0, 0, 0, 0]));
            img.put_pixel(w - 1, h - 1, image::Rgba([0, 0, 0, 0]));
        }
        let bytes = encode_png(&img).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn wait_for_job_completion(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if app.poll_image_jobs() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for image job watcher");
    }

    #[test]
    fn remove_background_inserts_result_one_undo() {
        let tmp = TestDir::new("rm-bg");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());

        // Prepare a stub RGBA PNG with transparent corners and have the stub agent script copy it to $DESIGNOS_IMAGE_OUTPUT.
        let stub_png = tmp.path().join("stub_rgba_transparent.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        let job_id = app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts");
        let req_text = std::fs::read_to_string(tmp.path().join("image-jobs").join(&job_id).join("request.json")).unwrap();
        let req: JobRequestPayload = serde_json::from_str(&req_text).unwrap();
        assert_eq!(req.op, "remove_background");
        assert_eq!(req.expected_size, [100, 100]);

        wait_for_job_completion(&mut app);

        let page_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_children.len(), 2, "new image layer inserted above original");
        let new_id = page_children[1];
        assert_ne!(new_id, orig_id);
        let new_node = app.doc.get(new_id).unwrap();
        assert_eq!(new_node.name, "Hero Photo · Remove background");
        assert!(new_node.visible);
        assert!(!app.doc.get(orig_id).unwrap().visible, "original layer is hidden");

        // One Undo reverts both the inserted layer and the hidden original layer.
        app.undo();
        let children_after_undo = &app.doc.get(app.page).unwrap().children;
        assert_eq!(children_after_undo, &[orig_id]);
        assert!(app.doc.get(orig_id).unwrap().visible, "single undo restores original visibility");
    }

    #[test]
    fn no_agent_disables_commands() {
        let tmp = TestDir::new("no-agent");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.override_agent_installed = Some(false);

        assert!(!has_available_agent_cli(&app));
        assert_eq!(DISABLED_TOOLTIP, "Needs an agent CLI. Set one up in Agents.");
        assert!(app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).is_none(), "starting an image job without an installed agent CLI is refused");
        assert_eq!(app.toast.as_ref().map(|t| t.text.as_str()), Some(DISABLED_TOOLTIP));
    }

    #[test]
    fn original_kept() {
        let tmp = TestDir::new("orig-kept");
        let (mut app, orig_id, orig_img_key) = sample_app_with_image_layer(tmp.path());
        let orig_bytes = app.doc.images.get(&orig_img_key).cloned().unwrap();

        let stub_png = tmp.path().join("stub_isolate.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::IsolateSubject).expect("job starts");
        wait_for_job_completion(&mut app);

        let orig_after = app.doc.get(orig_id).expect("original layer must remain in document");
        assert!(!orig_after.visible, "original layer is hidden, never deleted");
        assert_eq!(app.doc.images.get(&orig_img_key), Some(&orig_bytes), "original doc.images bytes must remain intact");
    }

    #[test]
    fn size_mismatch_rejected() {
        let tmp = TestDir::new("size-mismatch");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());

        // Stub agent writes an 80x60 PNG instead of the expected 100x100.
        let wrong_png = tmp.path().join("wrong_80x60.png");
        write_rgba_png(&wrong_png, 80, 60, true);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", wrong_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts");
        wait_for_job_completion(&mut app);

        let page_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_children, &[orig_id], "document layers must be unchanged when output.png has wrong dimensions");
        assert!(app.doc.get(orig_id).unwrap().visible, "original layer remains visible on failure");
        let toast = app.toast.as_ref().expect("error toast shown");
        assert!(toast.error);
        assert!(toast.text.contains("Result is 80×60, expected 100×100"), "unexpected toast: {}", toast.text);
    }

    #[test]
    fn no_bundled_model_crate() {
        let lock_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
        let lock_text = std::fs::read_to_string(&lock_path).unwrap_or_else(|e| panic!("cannot read {}: {e}", lock_path.display()));
        for forbidden in ["ort", "tract", "candle", "tch"] {
            let exact = format!("name = \"{forbidden}\"");
            let prefix = format!("name = \"{forbidden}-");
            assert!(!lock_text.contains(&exact) && !lock_text.contains(&prefix), "Cargo.lock must not bundle ML inference crate '{forbidden}'");
        }
    }

    #[test]
    fn expand_bounds() {
        let tmp = TestDir::new("expand-bounds");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());

        // Expand 10 px each side on a 100x100 layer at (0, 0) expects a 120x120 output.png
        // and produces a 120x120 layer at (-10, -10).
        let expanded_png = tmp.path().join("expanded_120x120.png");
        write_rgba_png(&expanded_png, 120, 120, false);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", expanded_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::Expand { top: 10, right: 10, bottom: 10, left: 10 }).expect("expand job starts");
        wait_for_job_completion(&mut app);

        let page_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_children.len(), 2);
        let new_id = page_children[1];
        let new_node = app.doc.get(new_id).unwrap();
        assert_eq!(new_node.size, Vec2::new(120.0, 120.0));
        assert_eq!((new_node.transform.tx, new_node.transform.ty), (-10.0, -10.0));
    }

    #[test]
    fn default_flow_without_agent_script_invokes_agent_cli_as_system_turn() {
        let tmp = TestDir::new("default-cli-flow");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.terminal.bind_session(&format!("test-default-cli-{}", std::process::id()));
        // A stub `claude` that enables bracketed paste and records each submitted line.
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let recv = tmp.path().join("recv.txt");
        let stub = bin_dir.join("claude");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\nprintf '\\033[?2004hReady> '\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\" >> '{}'\ndone\n", recv.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        app.terminal.path_prepend.push(bin_dir);
        // Do NOT set `app.image_jobs.agent_script`: exercise the real default terminal submission flow.
        app.image_jobs.agent_script = None;

        let job_id = app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts via terminal");
        let needle = format!("Image task {job_id}");
        let deadline = Instant::now() + Duration::from_secs(6);
        let mut received = String::new();
        while Instant::now() < deadline {
            let _ = app.terminal.poll_live_agent_bridge();
            received = std::fs::read_to_string(&recv).unwrap_or_default();
            if received.contains(&needle) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let shells: Vec<(AgentCliPreset, String)> = app
            .terminal
            .tabs
            .iter()
            .filter(|t| t.is_shell_process())
            .map(|t| (t.spawn_preset, String::from_utf8_lossy(&t.written_bytes).into_owned()))
            .collect();
        let (spawn_preset, spawned_command, is_shell) =
            app.terminal.active_tab().map(|t| (t.spawn_preset, t.spawned_command.clone(), t.is_shell_process())).expect("active agent tab spawned or selected");
        app.cancel_image_job(Some(orig_id));
        app.terminal.shutdown();

        assert!(!is_shell, "the image prompt must go to an agent CLI tab, never a shell");
        assert_eq!(spawn_preset, AgentCliPreset::Claude);
        assert!(spawned_command.as_deref().is_some_and(|c| c.contains("claude")), "the tab must launch the agent binary: {spawned_command:?}");
        assert!(received.contains(&needle), "the agent CLI must receive the image task prompt: {received:?}");
        assert!(shells.is_empty(), "no shell tab may exist or receive the prompt: {shells:?}");
    }

    /// The status word a hook left in the hook folder is what a new job looks at, never a file in the session folder.
    #[test]
    fn a_done_status_left_in_the_hook_folder_is_cleared_when_an_image_job_starts() {
        let tmp = TestDir::new("stale-done");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.terminal.bind_session(&format!("test-stale-done-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&app.terminal.session_dir);
        let status = app.terminal.prepared_hook_paths().status();
        std::fs::write(&status, r#"{"state":"done"}"#).unwrap();
        app.image_jobs.agent_script = Some("sleep 0".into());

        let _ = app.start_image_job(orig_id, 0, ImageOp::RemoveBackground);

        assert!(!status.exists(), "a done status an earlier run left must not end the new job at once");
        app.cleanup_image_jobs_on_file_close();
        crate::agent_approval_channel::remove_channel(&app.terminal.session_dir);
        app.terminal.shutdown();
    }

    #[test]
    fn finished_agent_without_output_fails_immediately_and_advances_queue() {
        let tmp = TestDir::new("done-no-output");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.terminal.bind_session(&format!("test-done-no-output-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&app.terminal.session_dir);
        // Keep the full 5-minute timeout so the test proves the watcher stops on `done` without waiting for timeout.
        app.image_jobs.job_timeout = Duration::from_secs(300);
        app.image_jobs.agent_script = None;

        let job1 = app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job 1 starts");
        let job2 = app.start_image_job(orig_id, 0, ImageOp::IsolateSubject).expect("job 2 queues");
        assert_eq!(app.image_jobs.jobs[0].state, ImageJobState::Running);
        assert_eq!(app.image_jobs.jobs[1].state, ImageJobState::Queued);

        // Prepare a stub script for job 2 so once job 1 fails immediately on `done`, job 2 runs and completes.
        let stub_png = tmp.path().join("job2_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        // Signal that the agent finished (`done`) without writing output.png for job 1.
        std::fs::write(app.terminal.prepared_hook_paths().status(), r#"{"state":"done"}"#).unwrap();

        // Wait for job 1 to fail immediately (< 1s, nowhere near the 300s timeout).
        let t0 = Instant::now();
        wait_for_job_completion(&mut app);
        assert!(t0.elapsed() < Duration::from_secs(2), "watcher should fail immediately when agent is done without output");
        assert!(
            matches!(app.image_jobs.jobs.iter().find(|j| j.id == job1).map(|j| &j.state), Some(ImageJobState::Failed(_))),
            "job 1 must fail when agent finishes without output.png"
        );

        // And job 2 must have started and completed right after job 1 failed.
        wait_for_job_completion(&mut app);
        assert_eq!(app.image_jobs.jobs.iter().find(|j| j.id == job2).map(|j| &j.state), Some(&ImageJobState::Done));
        app.cleanup_image_jobs_on_file_close();
        app.terminal.shutdown();
    }

    #[test]
    fn erase_without_region_omits_mask_file_and_field() {
        let tmp = TestDir::new("erase-no-mask");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        let stub_png = tmp.path().join("erase_out.png");
        write_rgba_png(&stub_png, 100, 100, false);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        let job_no_region = app
            .start_image_job(orig_id, 0, ImageOp::Erase { instruction: "the person on the left".into(), region: None })
            .expect("erase without region starts");
        let job_dir = tmp.path().join("image-jobs").join(&job_no_region);
        assert!(!job_dir.join("mask.png").exists(), "mask.png must NOT be written when no region was drawn");
        let req_raw = std::fs::read_to_string(job_dir.join("request.json")).unwrap();
        let req: JobRequestPayload = serde_json::from_str(&req_raw).unwrap();
        assert!(req.mask.is_none(), "request.json must omit mask when region is None");
        assert!(!req_raw.contains("\"mask\""), "serialized request.json must not contain mask field: {req_raw}");
        wait_for_job_completion(&mut app);

        // With an explicit region, mask.png IS written with only that rectangle white.
        let job_with_region =
            app.start_image_job(orig_id, 0, ImageOp::Erase { instruction: "sign".into(), region: Some([10, 20, 30, 40]) }).expect("erase with region starts");
        let job2_dir = tmp.path().join("image-jobs").join(&job_with_region);
        assert!(job2_dir.join("mask.png").exists(), "mask.png must be written when region is Some");
        let mask_img = image::open(job2_dir.join("mask.png")).unwrap().to_rgba8();
        assert_eq!(mask_img.get_pixel(0, 0).0, [0, 0, 0, 255], "outside region must be black");
        assert_eq!(mask_img.get_pixel(15, 25).0, [255, 255, 255, 255], "inside region must be white");
        wait_for_job_completion(&mut app);
    }

    #[test]
    fn visible_crop_hidpi_expand_and_scale_mode_preserved() {
        let tmp = TestDir::new("crop-hidpi");
        let (mut app, orig_id, img_key) = sample_app_with_image_layer(tmp.path());

        // Replace source bitmap with a 400x200 hi-DPI landscape image on the 100x100 layer using ImageScaleMode::Crop.
        // Center cover crop of 400x200 into 100x100 aspect ratio is the middle 200x200 square (x in 100..300, y in 0..200).
        let mut wide = image::RgbaImage::from_pixel(400, 200, image::Rgba([10, 20, 30, 255]));
        for y in 0..200 {
            for x in 100..300 {
                wide.put_pixel(x, y, image::Rgba([200, 100, 50, 255]));
            }
        }
        app.doc.images.insert(img_key.clone(), encode_png(&wide).unwrap());
        app.doc.get_mut(orig_id).unwrap().fills[0].kind = PaintKind::Image { image_ref: img_key, scale_mode: ImageScaleMode::Crop };

        // Expand by 50 image px each side on the 200x200 visible crop -> expected output is 300x300 image px,
        // and the 100x100 layer (0.5 layer units per image px) grows by 25 layer units each side to 150x150 at (-25, -25).
        let expanded_png = tmp.path().join("expanded_300x300.png");
        write_rgba_png(&expanded_png, 300, 300, false);
        app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", expanded_png.display()));

        let job_id = app.start_image_job(orig_id, 0, ImageOp::Expand { top: 50, right: 50, bottom: 50, left: 50 }).expect("expand starts");
        let input_img = image::open(tmp.path().join("image-jobs").join(&job_id).join("input.png")).unwrap().to_rgba8();
        assert_eq!(input_img.dimensions(), (200, 200), "input.png must be the 200x200 visible center crop, not the raw 400x200 bitmap");
        assert_eq!(input_img.get_pixel(0, 0).0, [200, 100, 50, 255], "cropped input.png must come from center x=100..300");

        wait_for_job_completion(&mut app);

        let page_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_children.len(), 2);
        let new_node = app.doc.get(page_children[1]).unwrap();
        assert_eq!(new_node.size, Vec2::new(150.0, 150.0), "layer size must scale expansion by layer-units-per-image-pixel (150/300 == 100/200)");
        assert_eq!((new_node.transform.tx, new_node.transform.ty), (-25.0, -25.0));
        match &new_node.fills[0].kind {
            PaintKind::Image { scale_mode, .. } => assert_eq!(*scale_mode, ImageScaleMode::Crop, "source scale_mode must be preserved"),
            other => panic!("expected image fill, got {other:?}"),
        }

        // Also verify `page_drag_to_image_region` respects both world scale/translation and Fit letterbox rect:
        // 400x200 image in a 100x100 layer under Fit occupies local y in 25..75, x in 0..100.
        // With a 2x scaled world transform at (50, 100), local (10, 35)..(60, 60) maps to page (70, 170)..(170, 220).
        let world = Affine { a: 2.0, b: 0.0, c: 0.0, d: 2.0, tx: 50.0, ty: 100.0 };
        let region =
            page_drag_to_image_region(world, Vec2::new(100.0, 100.0), ImageScaleMode::Fit, (400, 200), Pos2::new(70.0, 170.0), Pos2::new(170.0, 220.0))
                .expect("drag within Fit image content rect resolves");
        // local x: 10..60 of 0..100 -> 40..240 px (width 200); local y: 35..60 within 25..75 (10..35 of 50) -> 40..140 px (height 100).
        assert_eq!(region, [40, 40, 200, 100]);
    }

    #[test]
    fn jobs_rooted_in_file_session_dir_and_isolated_across_files() {
        let tmp1 = TestDir::new("sess-1");
        let tmp2 = TestDir::new("sess-2");
        let (mut app1, orig1, _) = sample_app_with_image_layer(tmp1.path());
        let (mut app2, orig2, _) = sample_app_with_image_layer(tmp2.path());
        app1.image_jobs.custom_jobs_root = None;
        app2.image_jobs.custom_jobs_root = None;
        app1.terminal.bind_session(&format!("test-file-a-{}", std::process::id()));
        app2.terminal.bind_session(&format!("test-file-b-{}", std::process::id()));

        assert_eq!(app1.image_jobs_root(), app1.terminal.session_dir.join("image-jobs"));
        assert_eq!(app2.image_jobs_root(), app2.terminal.session_dir.join("image-jobs"));
        assert_ne!(app1.image_jobs_root(), app2.image_jobs_root());

        let stub = tmp1.path().join("stub.png");
        write_rgba_png(&stub, 100, 100, true);
        let script = format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub.display());
        app1.image_jobs.agent_script = Some(script.clone());
        app2.image_jobs.agent_script = Some(script);

        app1.start_image_job(orig1, 0, ImageOp::RemoveBackground).unwrap();
        app2.start_image_job(orig2, 0, ImageOp::RemoveBackground).unwrap();
        wait_for_job_completion(&mut app1);
        wait_for_job_completion(&mut app2);

        assert!(app1.image_jobs_root().exists());
        assert!(app2.image_jobs_root().exists());

        // Closing file 1 removes only file 1's image-jobs directory, leaving file 2's intact.
        app1.cleanup_image_jobs_on_file_close();
        assert!(!app1.image_jobs_root().exists(), "file 1 image-jobs dir must be removed on close");
        assert!(app2.image_jobs_root().exists(), "file 2 image-jobs dir must remain intact when file 1 closes");

        app2.cleanup_image_jobs_on_file_close();
    }

    #[test]
    fn multi_tab_switch_and_close_isolate_image_jobs_and_cleanup() {
        let tmp = TestDir::new("multi-tab-isolation");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;

        // Build a second document whose image layer has the exact same NodeId (`orig_id`) as Tab 0.
        let mut doc_b = Document::new("Tab B Doc");
        let page_b = doc_b.add_page("Page 1");
        let nid_b = doc_b.alloc_id();
        assert_eq!(nid_b, orig_id, "fresh Document allocates the same NodeId as Tab 0");
        let mut node_b = Node::rectangle(nid_b, DRect::new(0.0, 0.0, 100.0, 100.0));
        node_b.name = "Tab B Layer".to_string();
        doc_b.insert(page_b, node_b).unwrap();

        // Initialize Tab 0 and open Tab 1 (`doc_b`), then switch back to Tab 0.
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        tabs.open_new_tab(&mut app, "Tab B", doc_b, crate::tabs::ProjectTabKind::Design);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;

        let tab_a_jobs_dir = app.image_jobs_root();
        let stub_png = tmp.path().join("tab_a_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        // Delay slightly so the tab switch happens while the job is in flight.
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on Tab A");
        assert!(tab_a_jobs_dir.exists(), "Tab A image-jobs directory created");

        // Switch to Tab B BEFORE Tab A's job finishes, then poll to completion while Tab B is active.
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(1, &mut app);
        app.tabs = tabs;
        let tab_b_jobs_dir = app.image_jobs_root();
        assert_ne!(tab_a_jobs_dir, tab_b_jobs_dir, "Tab A and Tab B must have distinct session image-jobs roots");

        wait_for_job_completion(&mut app);

        // Tab B must NOT have been mutated by Tab A's completed image job despite sharing `orig_id`.
        let page_b_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_b_children, &[orig_id], "Tab B must not receive Tab A's output layer");
        assert!(app.doc.get(orig_id).unwrap().visible, "Tab B's colliding NodeId layer must stay visible");
        assert!(!app.hist.can_undo(), "Tab B undo history must stay clean");

        // Switch back to Tab A: the completed result must be on Tab A and revertible in one undo step.
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;
        let page_a_children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(page_a_children.len(), 2, "Tab A must receive its completed output layer");
        assert!(!app.doc.get(orig_id).unwrap().visible, "Tab A's original layer must be hidden");
        app.undo();
        assert_eq!(app.doc.get(app.page).unwrap().children, vec![orig_id]);
        assert!(app.doc.get(orig_id).unwrap().visible);

        // Start a long-running job on Tab A, switch to Tab B, create Tab B's image-jobs dir, then close Tab A (background tab).
        app.image_jobs.agent_script = Some("sleep 5".to_string());
        app.start_image_job(orig_id, 0, ImageOp::IsolateSubject).expect("second job starts on Tab A");
        assert!(tab_a_jobs_dir.exists());

        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(1, &mut app);
        std::fs::create_dir_all(&tab_b_jobs_dir).unwrap();
        assert!(tabs.close_tab(0, &mut app), "closing background Tab A succeeds");
        app.tabs = tabs;

        assert!(!tab_a_jobs_dir.exists(), "closing Tab A while Tab B stays open must remove Tab A's image-jobs dir");
        assert!(tab_b_jobs_dir.exists(), "Tab B's image-jobs dir must remain intact when Tab A closes");

        let mut tabs = std::mem::take(&mut app.tabs);
        assert!(tabs.close_tab(0, &mut app), "closing last tab succeeds");
        app.tabs = tabs;
        assert!(!tab_b_jobs_dir.exists(), "closing Tab B removes Tab B's image-jobs dir");
    }

    #[test]
    fn job_completing_while_at_home_survives_reopening_same_library_file_with_one_undo() {
        let tmp = TestDir::new("home-reopen-same");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;

        let lib_dir = tmp.path().join("library");
        let lib = designos_library::Library::open(&lib_dir).unwrap();
        let proj = lib.create_project("Project").unwrap();
        let entry_a = lib.create_file(&proj.id, None, "File A", &app.doc).unwrap();
        app.lib = Some(lib.clone());

        app.open_library_file(&entry_a.id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;

        let stub_png = tmp.path().join("home_a_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on File A");
        app.go_home();
        assert!(app.home.is_some(), "Home screen is shown after Back to files");

        wait_for_job_completion(&mut app);
        assert_eq!(lib.load_file(&entry_a.id).unwrap().get(app.page).unwrap().children.len(), 2, "completed job at Home must be saved to File A on disk");

        // Reopen File A from the Home grid (`open_library_file`).
        app.open_library_file(&entry_a.id);
        assert!(app.home.is_none(), "opening File A leaves Home");
        let children = &app.doc.get(app.page).unwrap().children;
        assert_eq!(children.len(), 2, "File A in editor must contain the completed result layer");
        let new_id = children[1];
        assert!(!app.doc.get(orig_id).unwrap().visible, "original layer must be hidden");
        assert_eq!(app.selection, vec![new_id], "completed result layer must be selected");

        // One Undo removes the result layer and restores the original layer's visibility.
        app.undo();
        assert_eq!(app.doc.get(app.page).unwrap().children, vec![orig_id]);
        assert!(app.doc.get(orig_id).unwrap().visible);

        assert!(app.close_tab(0));
    }

    #[test]
    fn a_job_that_landed_on_a_parked_library_tab_and_was_written_shows_no_dot() {
        let tmp = TestDir::new("parked-lib-dot");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        let mut doc_b = Document::new("File B");
        doc_b.add_page("Page 1");
        let lib = designos_library::Library::open(tmp.path().join("library")).unwrap();
        let proj = lib.create_project("Project").unwrap();
        let entry_a = lib.create_file(&proj.id, None, "File A", &app.doc).unwrap();
        let entry_b = lib.create_file(&proj.id, None, "File B", &doc_b).unwrap();
        app.lib = Some(lib.clone());
        app.open_library_file(&entry_a.id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;
        let stub_png = tmp.path().join("parked_dot_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));
        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on File A");
        app.go_home();
        app.open_library_file(&entry_b.id);
        wait_for_job_completion(&mut app);
        assert_eq!(lib.load_file(&entry_a.id).unwrap().get(app.page).unwrap().children.len(), 2, "the result is written to File A");
        assert!(!crate::tabs::tab_is_unsaved(&app, 0), "File A's tab holds what its file holds: no dot");
        // back on it: still no dot, and the one undo that takes the layer away is an unsaved edit
        app.switch_tab(0);
        assert!(!app.has_unsaved_edits());
        app.undo();
        assert!(app.has_unsaved_edits());
        assert!(app.close_tab(1));
        assert!(app.close_tab(0));
    }

    #[test]
    fn a_job_that_landed_on_a_parked_library_tab_does_not_rewrite_its_file_when_the_tab_comes_back() {
        let tmp = TestDir::new("parked-lib-no-rewrite");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        let mut doc_b = Document::new("File B");
        doc_b.add_page("Page 1");
        let lib = designos_library::Library::open(tmp.path().join("library")).unwrap();
        let proj = lib.create_project("Project").unwrap();
        let entry_a = lib.create_file(&proj.id, None, "File A", &app.doc).unwrap();
        let entry_b = lib.create_file(&proj.id, None, "File B", &doc_b).unwrap();
        app.lib = Some(lib.clone());
        app.open_library_file(&entry_a.id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;
        let stub_png = tmp.path().join("parked_no_rewrite_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));
        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on File A");
        app.go_home();
        app.open_library_file(&entry_b.id);
        wait_for_job_completion(&mut app);
        let written = lib.load_file(&entry_a.id).unwrap().get(app.page).unwrap().children.len();
        assert_eq!(written, 2, "the result was written to File A while its tab was parked");
        let modified = lib.file(&entry_a.id).unwrap().modified;
        // back on the tab: the write was this app's own, so nothing is read again and nothing is written
        app.switch_tab(0);
        crate::canvas::tiles::take_rev(&mut app);
        let saves = app.autosave.saves;
        let ctx = egui::Context::default();
        for step in 1..=40 {
            app.now = 100.0 + step as f64 * 0.1;
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.autosave_tick(ui.ctx());
            });
        }
        assert_eq!(app.autosave.saves, saves, "autosave wrote nothing");
        assert_eq!(lib.file(&entry_a.id).unwrap().modified, modified, "File A was not rewritten");
        assert!(!app.has_unsaved_edits(), "and the tab shows no dot");
        assert!(app.close_tab(1));
        assert!(app.close_tab(0));
    }

    #[test]
    fn undoing_a_job_that_landed_on_a_parked_tab_back_to_the_saved_state_clears_the_dot() {
        let tmp = TestDir::new("parked-undo-dot");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        let mut doc_b = Document::new("Tab B Doc");
        doc_b.add_page("Page 1");
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        tabs.open_new_tab(&mut app, "Tab B", doc_b, crate::tabs::ProjectTabKind::Design);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;
        let stub_png = tmp.path().join("parked_undo_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));
        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on Tab A");
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(1, &mut app);
        app.tabs = tabs;
        wait_for_job_completion(&mut app);
        assert!(crate::tabs::tab_is_unsaved(&app, 0), "the job is an edit nothing has saved");
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;
        assert!(app.has_unsaved_edits());
        app.undo();
        assert!(!app.has_unsaved_edits(), "the job is a step of the undo history, so undoing it is back at the saved state");
    }

    #[test]
    fn job_on_file_a_completes_after_go_home_and_opening_file_b_from_home() {
        let tmp = TestDir::new("home-open-other");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;

        let mut doc_b = Document::new("File B");
        let page_b = doc_b.add_page("Page 1");
        let nid_b = doc_b.alloc_id();
        assert_eq!(nid_b, orig_id, "File B uses colliding NodeId");
        let mut node_b = Node::rectangle(nid_b, DRect::new(0.0, 0.0, 100.0, 100.0));
        node_b.name = "File B Rect".to_string();
        doc_b.insert(page_b, node_b).unwrap();

        let lib_dir = tmp.path().join("library");
        let lib = designos_library::Library::open(&lib_dir).unwrap();
        let proj = lib.create_project("Project").unwrap();
        let entry_a = lib.create_file(&proj.id, None, "File A", &app.doc).unwrap();
        let entry_b = lib.create_file(&proj.id, None, "File B", &doc_b).unwrap();
        app.lib = Some(lib.clone());

        app.open_library_file(&entry_a.id);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;

        let stub_png = tmp.path().join("home_b_out.png");
        write_rgba_png(&stub_png, 100, 100, true);
        app.image_jobs.agent_script = Some(format!("sleep 0.05 && cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", stub_png.display()));

        app.start_image_job(orig_id, 0, ImageOp::RemoveBackground).expect("job starts on File A");
        app.go_home();
        app.open_library_file(&entry_b.id);
        assert_eq!(app.tabs.entries.len(), 2, "File A stays parked in tab 0 and File B opens in tab 1");
        assert_eq!(app.tabs.active_idx, 1);

        wait_for_job_completion(&mut app);

        // File B (currently active) must remain untouched.
        assert_eq!(app.doc.get(app.page).unwrap().children, vec![orig_id], "File B must not receive File A's output layer");
        assert!(app.doc.get(orig_id).unwrap().visible);
        assert!(!app.hist.can_undo(), "File B undo stack must remain clean");

        // File A on disk and in Tab 0 must have the result layer and revert in one undo step.
        assert_eq!(lib.load_file(&entry_a.id).unwrap().get(app.page).unwrap().children.len(), 2);
        app.switch_tab(0);
        assert_eq!(app.doc.get(app.page).unwrap().children.len(), 2, "File A must have the completed output layer");
        assert!(!app.doc.get(orig_id).unwrap().visible);
        app.undo();
        assert_eq!(app.doc.get(app.page).unwrap().children, vec![orig_id]);
        assert!(app.doc.get(orig_id).unwrap().visible);

        assert!(app.close_tab(1));
        assert!(app.close_tab(0));
    }

    /// Removes the session folder of a test, links first, whatever the test does.
    #[cfg(unix)]
    struct SessionFolder(PathBuf);

    #[cfg(unix)]
    impl Drop for SessionFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_planted_as_the_image_jobs_folder_receives_no_job_file_and_loses_nothing() {
        let tmp = TestDir::new("link-image-jobs");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        app.terminal.bind_session(&format!("test-link-jobs-{}", std::process::id()));
        let session = app.terminal.session_dir.clone();
        let _session = SessionFolder(session.clone());
        crate::agent_session_files::ensure_session_dir(&session).expect("the session folder");
        let victim = tmp.path().join("victim");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("keep.txt"), b"mine").unwrap();
        std::os::unix::fs::symlink(&victim, session.join("image-jobs")).expect("plant the link");
        app.image_jobs.agent_script = Some("sleep 0".into());

        let started = app.start_image_job(orig_id, 0, ImageOp::RemoveBackground);

        let names = |dir: &Path| -> Vec<String> { std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect() };
        assert_eq!(names(&victim), ["keep.txt"], "a job file went through the link");
        assert!(started.is_none(), "a job must not start in a folder behind a link");
        app.cleanup_image_jobs_on_file_close();
        assert_eq!(std::fs::read(victim.join("keep.txt")).unwrap(), b"mine", "closing the file removed what the link points at");
    }

    #[cfg(unix)]
    #[test]
    fn closing_a_file_whose_session_folder_is_a_link_removes_nothing_in_the_target_of_the_link() {
        let tmp = TestDir::new("link-session-cleanup");
        let (mut app, _orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        let victim = tmp.path().join("victim");
        let job = victim.join("image-jobs").join("job-0001");
        std::fs::create_dir_all(&job).unwrap();
        std::fs::write(job.join("request.json"), b"mine").unwrap();
        let linked = tmp.path().join("agent-sessions").join("file-1");
        std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&victim, &linked).unwrap();
        app.terminal.session_dir = linked;

        app.cleanup_image_jobs_on_file_close();

        assert_eq!(std::fs::read(job.join("request.json")).ok().as_deref(), Some(&b"mine"[..]), "closing the file removed what the link points at");
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_at_agent_status_json_does_not_freeze_the_start_of_an_image_job() {
        use crate::agent_session_files::test_support::{make_fifo, timed_with_fifo_watchdog};
        let tmp = TestDir::new("fifo-status");
        let (mut app, orig_id, _) = sample_app_with_image_layer(tmp.path());
        app.image_jobs.custom_jobs_root = None;
        app.terminal.bind_session(&format!("test-fifo-status-{}", std::process::id()));
        let session = app.terminal.session_dir.clone();
        let _session = SessionFolder(session.clone());
        crate::agent_session_files::ensure_session_dir(&session).expect("the session folder");
        let fifo = app.terminal.prepared_hook_paths().status();
        make_fifo(&fifo);
        app.image_jobs.agent_script = Some("sleep 0".into());

        let (started, took) = timed_with_fifo_watchdog(&fifo, Duration::from_secs(30), || app.start_image_job(orig_id, 0, ImageOp::RemoveBackground));

        assert!(took < Duration::from_secs(30), "starting a job was blocked on the FIFO for {took:?}");
        assert!(started.is_some(), "a FIFO is no stale status: the job starts");
    }
}
