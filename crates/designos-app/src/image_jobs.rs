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
use crate::terminal::{runtime_binary_available, AgentCliPreset, SessionEnv};
use crate::theme;
use crate::widgets;
use crate::widgets::field::{self, Prefix};
use designos_doc::{Affine, BlendMode, Command, Document, ImageScaleMode, Node, NodeId, NodeKind, Paint, PaintKind, Vec2};
use egui::{Align2, Area, Color32, Context, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Tooltip shown on disabled AI image commands when no agent CLI is installed.
pub const DISABLED_TOOLTIP: &str = "Set up an agent CLI in the Agents panel to use AI image tools";

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
    /// Optional mask rectangle in image pixel coordinates `[x, y, w, h]`.
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
    /// True if `node` currently has a `Queued` or `Running` AI image job.
    pub fn active_job_for_node(&self, node: NodeId) -> Option<&ImageJob> {
        self.jobs.iter().find(|j| j.node == node && matches!(j.state, ImageJobState::Running | ImageJobState::Queued))
    }

    /// True while any job is `Queued` or `Running`.
    pub fn has_active_jobs(&self) -> bool {
        self.jobs.iter().any(|j| matches!(j.state, ImageJobState::Running | ImageJobState::Queued))
    }
}

/// Returns true when at least one agent CLI (`Claude`, `Codex`, `Antigravity`, `Grok`) is installed.
pub fn has_available_agent_cli(app: &App) -> bool {
    if let Some(ov) = app.image_jobs.override_agent_installed {
        return ov;
    }
    [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok]
        .iter()
        .any(|preset: &AgentCliPreset| runtime_binary_available(preset.binary_name()))
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

/// Encodes an [`image::RgbaImage`] as PNG bytes.
fn encode_png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    image::DynamicImage::ImageRgba8(img.clone()).write_to(&mut cursor, image::ImageFormat::Png).map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out)
}

/// Builds the black/white `mask.png` (`#FFFFFF` = erase region on `#000000` background) of size `(w, h)`.
fn build_erase_mask_png(w: u32, h: u32, region: Option<[u32; 4]>) -> Result<Vec<u8>, String> {
    let mut img = image::RgbaImage::from_pixel(w.max(1), h.max(1), image::Rgba([0, 0, 0, 255]));
    let [rx, ry, rw, rh] = region.unwrap_or([0, 0, w, h]);
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
    /// Resolves `<session>/image-jobs` for the current file and active agent tab.
    pub fn image_jobs_root(&self) -> PathBuf {
        if let Some(custom) = &self.image_jobs.custom_jobs_root {
            return custom.clone();
        }
        let tab_id = self.terminal.tabs.get(self.terminal.active_tab).map(|t| t.id).unwrap_or(1);
        SessionEnv::for_session_id(&format!("tab-{tab_id}")).session_dir.join("image-jobs")
    }

    /// Cancels all active/queued image jobs and removes the `<session>/image-jobs` folder on file close.
    pub fn cleanup_image_jobs_on_file_close(&mut self) {
        if let Some(w) = self.image_jobs.watcher.take() {
            w.cancel.store(true, Ordering::SeqCst);
        }
        self.image_jobs.queue.clear();
        for job in &mut self.image_jobs.jobs {
            if matches!(job.state, ImageJobState::Running | ImageJobState::Queued) {
                job.state = ImageJobState::Cancelled;
            }
        }
        let root = self.image_jobs.last_session_root.take().unwrap_or_else(|| self.image_jobs_root());
        if root.exists() {
            let _ = std::fs::remove_dir_all(&root);
        }
        self.image_jobs.open_fill_menu = None;
        self.image_jobs.expand_popover = None;
        self.image_jobs.erase_popover = None;
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
        let (w, h) = self.doc.get(node).map(|n| (n.size.x.round().max(1.0) as u32, n.size.y.round().max(1.0) as u32)).unwrap_or((100, 100));
        let pad_x = ((w as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
        let pad_y = ((h as f32 * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
        self.image_jobs.erase_popover = None;
        self.image_jobs.expand_popover = Some(ExpandPopoverState { node, fill_index, anchor, top: pad_y, right: pad_x, bottom: pad_y, left: pad_x });
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
        self.image_jobs.expand_popover = None;
        self.image_jobs.erase_popover =
            Some(ErasePopoverState { node, fill_index, anchor, instruction: String::new(), focus_requested: true, region: None, drag_start_page: None });
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
                self.show_toast("Expand requires at least one side greater than 0 px", true);
                return None;
            }
            if *top > EXPAND_MAX_SIDE_PX || *right > EXPAND_MAX_SIDE_PX || *bottom > EXPAND_MAX_SIDE_PX || *left > EXPAND_MAX_SIDE_PX {
                self.show_toast(format!("Expand padding must be between 0 and {EXPAND_MAX_SIDE_PX} px"), true);
                return None;
            }
        }
        if let ImageOp::Erase { instruction, .. } = &op {
            if instruction.trim().is_empty() {
                self.show_toast("Enter what to erase before running Erase", true);
                return None;
            }
        }

        let image_ref = match self.doc.get(node).and_then(|n| n.fills.get(fill_index)).map(|p| &p.kind) {
            Some(PaintKind::Image { image_ref, .. }) => image_ref.clone(),
            _ => return None,
        };

        let decoded = match decode_doc_image(&self.doc, &image_ref) {
            Ok(img) => img,
            Err(e) => {
                self.show_toast(e, true);
                return None;
            }
        };
        let (orig_w, orig_h) = decoded.dimensions();
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
        if let Err(e) = std::fs::create_dir_all(&job_dir) {
            self.show_toast(format!("Cannot create image job folder: {e}"), true);
            return None;
        }

        let input_path = job_dir.join("input.png");
        let output_path = job_dir.join("output.png");
        let request_path = job_dir.join("request.json");
        let _ = std::fs::remove_file(&output_path);

        let input_png = match encode_png(&decoded) {
            Ok(b) => b,
            Err(e) => {
                self.show_toast(e, true);
                return None;
            }
        };
        if let Err(e) = std::fs::write(&input_path, &input_png) {
            self.show_toast(format!("Cannot write {}: {e}", input_path.display()), true);
            return None;
        }

        let mask_path = if let ImageOp::Erase { region, .. } = &op {
            let mp = job_dir.join("mask.png");
            match build_erase_mask_png(orig_w, orig_h, *region) {
                Ok(mask_bytes) => {
                    if let Err(e) = std::fs::write(&mp, mask_bytes) {
                        self.show_toast(format!("Cannot write {}: {e}", mp.display()), true);
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
        if let Err(e) = std::fs::write(&request_path, req_json) {
            self.show_toast(format!("Cannot write {}: {e}", request_path.display()), true);
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
            expected_size,
            started_at: Instant::now(),
            state: ImageJobState::Queued,
        };
        self.image_jobs.jobs.push(job);
        self.image_jobs.queue.push_back(job_id.clone());

        if self.image_jobs.watcher.is_some() {
            let pos = self.image_jobs.queue.len();
            self.show_toast(format!("Queued image job ({pos} in queue)"), false);
        } else {
            self.pump_next_queued_image_job();
        }
        self.changed();
        Some(job_id)
    }

    /// Starts the next queued job if no job is currently running.
    fn pump_next_queued_image_job(&mut self) {
        if self.image_jobs.watcher.is_some() {
            return;
        }
        while let Some(next_id) = self.image_jobs.queue.pop_front() {
            let Some(idx) = self.image_jobs.jobs.iter().position(|j| j.id == next_id && j.state == ImageJobState::Queued) else {
                continue;
            };
            self.image_jobs.jobs[idx].state = ImageJobState::Running;
            self.image_jobs.jobs[idx].started_at = Instant::now();
            let job = self.image_jobs.jobs[idx].clone();

            let prompt = format!(
                "Image task {}: {}. Read {}, write the result as PNG to {}. Keep the same pixel size unless told otherwise. Do not change other files.",
                job.id,
                job.op.instruction_text(),
                job.input_path.display(),
                job.output_path.display()
            );

            if self.image_jobs.agent_script.is_none() {
                let fp = self.file_path.clone();
                self.terminal.submit_chat_prompt(&prompt, fp.as_deref());
            }

            let cancel = Arc::new(AtomicBool::new(false));
            let cancel_bg = Arc::clone(&cancel);
            let (tx, rx) = mpsc::channel();
            let stable_window = self.image_jobs.stable_window;
            let timeout = self.image_jobs.job_timeout;
            let script = self.image_jobs.agent_script.clone();
            let job_id = job.id.clone();

            std::thread::spawn(move || {
                let outcome = run_watcher_thread(job, script, stable_window, timeout, cancel_bg);
                let _ = tx.send(outcome);
            });

            self.image_jobs.watcher = Some(RunningWatcher { job_id, cancel, rx });
            break;
        }
    }

    /// Polls the background `output.png` watcher and applies finished jobs on the UI thread.
    pub fn poll_image_jobs(&mut self) -> bool {
        let outcome = match &self.image_jobs.watcher {
            Some(w) => match w.rx.try_recv() {
                Ok(out) => Some(out),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(WatcherOutcome::Failed { job_id: w.job_id.clone(), reason: "Image job watcher disconnected unexpectedly".to_string() })
                }
            },
            None => None,
        };
        let Some(outcome) = outcome else {
            return false;
        };
        self.image_jobs.watcher = None;

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
        true
    }

    /// Inserts the result layer above the original and hides the original in one undo step (`Tx`).
    fn apply_completed_image_job(&mut self, job: &ImageJob, png_bytes: Vec<u8>) -> Result<NodeId, String> {
        let orig = self.doc.get(job.node).cloned().ok_or_else(|| "Target image layer no longer exists".to_string())?;
        let parent = orig.parent.ok_or_else(|| "Target image layer has no parent container".to_string())?;
        let orig_index = self.doc.get(parent).and_then(|p| p.children.iter().position(|&c| c == job.node)).unwrap_or(0);

        let mut key_seq = self.doc.images.len() + 1;
        let new_image_key = loop {
            let candidate = format!("ai-{}-{key_seq}", job.id);
            if !self.doc.images.contains_key(&candidate) {
                break candidate;
            }
            key_seq += 1;
        };

        let new_id = self.doc.alloc_id();
        let mut new_node = orig.clone();
        new_node.id = new_id;
        new_node.parent = Some(parent);
        new_node.children.clear();
        new_node.name = format!("{} · {}", orig.name, job.op.layer_suffix());
        new_node.visible = true;
        new_node.kind = NodeKind::Rectangle;

        let new_paint = Paint {
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
            kind: PaintKind::Image { image_ref: new_image_key.clone(), scale_mode: ImageScaleMode::Fill },
        };
        if job.fill_index < new_node.fills.len() {
            new_node.fills[job.fill_index] = new_paint;
        } else {
            new_node.fills = vec![new_paint];
        }

        if let ImageOp::Expand { top, right, bottom, left } = job.op {
            new_node.size = Vec2::new(orig.size.x + left as f32 + right as f32, orig.size.y + top as f32 + bottom as f32);
            new_node.transform = orig.transform * Affine::translate(-(left as f32), -(top as f32));
        }

        let mut add = BTreeMap::new();
        add.insert(new_image_key, png_bytes);

        let mut tx = Tx::new();
        tx.cmd(&mut self.doc, Command::SetImages { add, remove: Vec::new() }).map_err(|e| format!("Failed to store result image: {e}"))?;
        tx.cmd(&mut self.doc, Command::Insert { parent, index: orig_index + 1, nodes: vec![new_node] })
            .map_err(|e| format!("Failed to insert result layer: {e}"))?;
        if let Some(orig_mut) = tx.ed.node(&mut self.doc, job.node) {
            orig_mut.visible = false;
        }
        if !self.run_tx(tx) {
            return Err("Failed to commit AI image edit".to_string());
        }
        Ok(new_id)
    }
}

fn run_watcher_thread(job: ImageJob, script: Option<String>, stable_window: Duration, timeout: Duration, cancel: Arc<AtomicBool>) -> WatcherOutcome {
    if let Some(cmd_str) = script {
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(&cmd_str)
            .env("DESIGNOS_IMAGE_JOB_ID", &job.id)
            .env("DESIGNOS_IMAGE_OP", job.op.slug())
            .env("DESIGNOS_IMAGE_REQUEST", &job.request_path)
            .env("DESIGNOS_IMAGE_INPUT", &job.input_path)
            .env("DESIGNOS_IMAGE_OUTPUT", &job.output_path);
        if let Some(mp) = &job.mask_path {
            cmd.env("DESIGNOS_IMAGE_MASK", mp);
        }
        let _ = cmd.output();
    }

    let start = Instant::now();
    let poll_interval = stable_window.checked_div(2).unwrap_or(Duration::from_millis(10)).clamp(Duration::from_millis(5), Duration::from_millis(100));
    let mut last_sig: Option<(u64, Option<std::time::SystemTime>, Instant)> = None;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return WatcherOutcome::Failed { job_id: job.id, reason: "Cancelled".to_string() };
        }
        if start.elapsed() > timeout {
            return WatcherOutcome::Failed { job_id: job.id, reason: "AI image tool timed out after 5 minutes".to_string() };
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
            }
        }
        std::thread::sleep(poll_interval);
    }
}

fn validate_output_file(job: &ImageJob) -> WatcherOutcome {
    let raw = match std::fs::read(&job.output_path) {
        Ok(b) => b,
        Err(e) => {
            return WatcherOutcome::Failed { job_id: job.id.clone(), reason: format!("Cannot read output.png: {e}") };
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
        return WatcherOutcome::Failed { job_id: job.id.clone(), reason: format!("expected {exp_w}x{exp_h}, got {got_w}x{got_h}") };
    }

    let warning = if matches!(job.op, ImageOp::RemoveBackground | ImageOp::IsolateSubject) {
        let has_alpha = decoded.pixels().any(|px| px.0[3] < 255);
        if !has_alpha {
            Some("Warning: output image has no transparent pixels".to_string())
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

/// Renders the `Agent working…` chip and `Cancel` button on a layer row with an active image job (`panels/layers.rs`).
/// Returns the x coordinate of the left edge of the chip so the layer name elides cleanly before it.
pub fn layer_row_progress_chip(app: &mut App, ui: &mut egui::Ui, p: &egui::Painter, row_rect: Rect, right_limit: f32, node: NodeId) -> Option<f32> {
    let job = app.image_jobs.active_job_for_node(node)?;
    let status_text = match job.state {
        ImageJobState::Queued => "Queued…",
        _ => "Agent working…",
    };
    let font = theme::font_caption();
    let status_w = widgets::text_width(p, status_text, &font);
    let cancel_w = widgets::text_width(p, "Cancel", &font) + 12.0;
    let chip_w = status_w + 8.0 + cancel_w + 12.0;
    let chip_h = 20.0;
    let chip_right = right_limit - 4.0;
    let chip_left = (chip_right - chip_w).max(row_rect.left() + 48.0);
    let chip_rect = Rect::from_min_size(Pos2::new(chip_left, row_rect.center().y - chip_h * 0.5), egui::vec2(chip_right - chip_left, chip_h));
    widgets::record(ui, &format!("layer-ai-chip:{}", node.0), chip_rect);
    p.rect_filled(chip_rect, theme::cr_pill(chip_h), theme::BG_SECONDARY);
    p.rect_stroke(chip_rect, theme::cr_pill(chip_h), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    p.text(Pos2::new(chip_rect.left() + 6.0, chip_rect.center().y), Align2::LEFT_CENTER, status_text, font.clone(), theme::TEXT_SECONDARY);

    let cancel_hit = Rect::from_min_size(Pos2::new(chip_rect.right() - cancel_w - 2.0, row_rect.center().y - 12.0), egui::vec2(cancel_w.max(24.0), 24.0));
    let cancel_vis = Rect::from_min_size(Pos2::new(chip_rect.right() - cancel_w - 2.0, chip_rect.top() + 2.0), egui::vec2(cancel_w, chip_h - 4.0));
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
    let pos = Pos2::new(anchor.left() - 140.0, anchor.bottom() + 4.0);
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
    let layer_size = node_ref.size;

    let screen = ctx.content_rect();
    let w = 236.0;
    let h = if total_fills > 1 { 176.0 } else { 156.0 };
    let inspector_left = screen.right() - theme::RIGHT_PANEL_WIDTH;
    let target_x = if pop.anchor.center().x >= inspector_left { inspector_left - w - 8.0 } else { pop.anchor.left() - w - 8.0 };
    let x = target_x.clamp(screen.left() + 8.0, (screen.right() - w - 8.0).max(screen.left() + 8.0));
    let y = pop.anchor.top().clamp(screen.top() + 8.0, (screen.bottom() - h - 8.0).max(screen.top() + 8.0));
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

        // 25% preset button
        let preset_r = Rect::from_min_size(Pos2::new(rect.right() - pad - 52.0, cy - 2.0), egui::vec2(52.0, 24.0));
        widgets::record(ui, "ai-expand-preset-25", preset_r);
        let pr_resp = ui.interact(preset_r, ui.id().with("ai-expand-25"), Sense::click());
        p.rect_filled(preset_r, theme::cr(theme::RADIUS_CONTROL), if pr_resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        p.text(preset_r.center(), Align2::CENTER_CENTER, "+25%", theme::font_mono(), theme::TEXT);
        if pr_resp.on_hover_text("Set each side to 25% of image dimensions").clicked() {
            let px = ((layer_size.x * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
            let py = ((layer_size.y * 0.25).round() as u32).clamp(1, EXPAND_MAX_SIDE_PX);
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
    let x = target_x.clamp(screen.left() + 8.0, (screen.right() - w - 8.0).max(screen.left() + 8.0));
    let y = pop.anchor.top().clamp(screen.top() + 8.0, (screen.bottom() - h - 8.0).max(screen.top() + 8.0));
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
                Pos2::new(input_r.left() + 8.0, input_r.center().y),
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
    let image_dims = node
        .fills
        .get(pop.fill_index)
        .and_then(|p| match &p.kind {
            PaintKind::Image { image_ref, .. } => decode_doc_image(&app.doc, image_ref).ok(),
            _ => None,
        })
        .map(|img| img.dimensions())
        .unwrap_or((node.size.x.round().max(1.0) as u32, node.size.y.round().max(1.0) as u32));

    let world = app.doc.world_transform(node.id);
    let node_page_min = Pos2::new(world.tx, world.ty);
    let node_page_size = egui::vec2(node.size.x.max(1.0), node.size.y.max(1.0));

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
            let min_x = (start.x.min(cur.x) - node_page_min.x).clamp(0.0, node_page_size.x);
            let min_y = (start.y.min(cur.y) - node_page_min.y).clamp(0.0, node_page_size.y);
            let max_x = (start.x.max(cur.x) - node_page_min.x).clamp(0.0, node_page_size.x);
            let max_y = (start.y.max(cur.y) - node_page_min.y).clamp(0.0, node_page_size.y);
            if (max_x - min_x) >= 2.0 && (max_y - min_y) >= 2.0 {
                let sx = image_dims.0 as f32 / node_page_size.x;
                let sy = image_dims.1 as f32 / node_page_size.y;
                let rx = (min_x * sx).floor() as u32;
                let ry = (min_y * sy).floor() as u32;
                let rw = ((max_x - min_x) * sx).ceil().max(1.0) as u32;
                let rh = ((max_y - min_y) * sy).ceil().max(1.0) as u32;
                pop.region = Some([rx, ry, rw, rh]);
            }
            if released {
                pop.drag_start_page = None;
            }
        }
    }

    if let Some([rx, ry, rw, rh]) = pop.region {
        let sx = node_page_size.x / image_dims.0.max(1) as f32;
        let sy = node_page_size.y / image_dims.1.max(1) as f32;
        let p0 = Vec2::new(node_page_min.x + rx as f32 * sx, node_page_min.y + ry as f32 * sy);
        let p1 = Vec2::new(node_page_min.x + (rx + rw) as f32 * sx, node_page_min.y + (ry + rh) as f32 * sy);
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
        assert_eq!(DISABLED_TOOLTIP, "Set up an agent CLI in the Agents panel to use AI image tools");
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
        assert!(toast.text.contains("expected 100x100, got 80x60"), "unexpected toast: {}", toast.text);
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
}
