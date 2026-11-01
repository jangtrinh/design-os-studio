//! Canvas rendering on the GPU (`designos-gpu`): the visible page is compiled
//! to a frame while the UI is painted and drawn by an egui paint callback.
//! Pan and zoom only change the view transform: geometry is cached per node,
//! nothing is re-rasterised on the CPU.
//!
//! The CPU tile path (`tiles.rs`) stays the fallback: it is used when the GPU
//! cannot be initialised, when "Use GPU rendering" is off, and for
//! `--screenshot` unless `--gpu` is given (pixel diffs must be deterministic).

use crate::app::App;
use designos_doc::{Document, NodeId};
use designos_gpu::{next_slice, Canvas, Frame, SceneCompiler, DEFAULT_REFINE_MAX_PROGRAMS};
use designos_render::Viewport;
use eframe::egui_wgpu;
use egui::{Rect as ERect, Ui};
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Seconds the frame key must have stayed unchanged (and the view must have
/// seen no scroll or zoom) before the one full-quality refine compile runs.
/// Keeps the uncapped compile out of trackpad pans and momentum scrolls.
pub const IDLE_REFINE_DELAY: f64 = 0.25;

/// A UI frame whose own work took less than this many milliseconds is an idle frame. The refine
/// starts only on a frame that follows one: not in the frame after a document opened, whose
/// layout and first compile are still settling.
pub const IDLE_FRAME_MS: f32 = 16.0;

/// Seconds past the idle delay after which the refine starts whatever the frames before it
/// cost: a UI that never has an idle frame (a slow machine) must still get its full detail.
const IDLE_WAIT_LIMIT: f64 = 1.0;

/// A refine compiles the page again after warming the geometry it deferred; if that still
/// leaves geometry deferred (a node the warm-up could not rebuild the way the compile asks
/// for it) this many compiles in, the frame is taken as it is.
const MAX_REFINE_COMPILES: u32 = 4;

/// Times a refine whose chunks did not all reach the GPU starts over before it gives up
/// and leaves the interactive frame on screen.
const MAX_CHUNK_RESTARTS: u8 = 2;

/// Document nodes one slice of a big page's refine compiles: a slice compiles in a few tens of
/// milliseconds, so the compile of a page of any size never holds a frame for long.
const SLICE_NODES: usize = 3_000;

/// Estimated microseconds one chunk of the refined frame may cost to render (see
/// `designos_gpu::PROGRAM_COST_US`): well inside a 60 fps frame, so the UI stays live while
/// thousands of passes are recorded.
pub const CHUNK_BUDGET_US: f32 = 8_000.0;

type FrameKey = (designos_doc::NodeId, u64, [u32; 2], u32, u32, (u32, u32), usize);

/// The key of the frame `paint` compiles for this page, revision, view and canvas size.
fn frame_key(page: NodeId, rev: u64, vp: &Viewport, size: (u32, u32)) -> FrameKey {
    (page, rev, [vp.offset.x.to_bits(), vp.offset.y.to_bits()], vp.zoom.to_bits(), vp.dpr.to_bits(), size, designos_render::font_generation())
}

/// What a frame of the canvas works on.
struct FrameCtx<'a> {
    doc: &'a Document,
    page: NodeId,
    rev: u64,
    vp: Viewport,
    size: (u32, u32),
    key: FrameKey,
}

/// App-side GPU state.
pub struct GpuView {
    /// Preferences -> "Use GPU rendering".
    pub pref: bool,
    /// The device and renderer initialised.
    pub available: bool,
    /// The canvas started on the CPU tiles because the adapter is a software renderer and the
    /// user had not chosen: Preferences says why. Cleared once the user picks either way.
    pub off_for_software: bool,
    max_texture: u32,
    pub(crate) compiler: SceneCompiler,
    last_key: Option<FrameKey>,
    /// The frame for `last_key` was compiled with the offscreen-program cap
    /// lifted and the zoom-out shortcuts off (the idle full-quality pass), and the GPU
    /// holds it, so there is nothing left to refine.
    refined: bool,
    /// What the refine of the current view is doing between its compiles and its
    /// presentation (see [`Settle`]).
    settle: Settle,
    /// The canvas was last drawn by the GPU, not by the CPU tiles.
    painting: bool,
    /// `egui` input time at which the frame key last changed, or the view last
    /// scrolled/zoomed while the key was unchanged: the start of the idle delay.
    stable_since: f64,
    /// Consecutive frames, since `stable_since`, that followed an idle frame (a frame
    /// whose UI work took under `IDLE_FRAME_MS`): the refine needs one.
    idle_frames: u8,
    /// Milliseconds of work the previous UI frame took (`Shell::draw`).
    last_frame_ms: f32,
    /// Tests that run in debug builds, where every UI frame is slow, fix the frame work.
    pub(crate) frame_work_override: Option<f32>,
    /// Chunks of the current refine whose paint callback took them: callback `i` stores `i + 1`.
    /// A chunk that was handed out and never taken (its frame was not painted) shows up as a
    /// gap here.
    chunk_ack: Arc<AtomicUsize>,
    /// Times the current view's refine started over because a chunk never reached the GPU.
    restarts: u8,
    /// Document nodes of one slice (`SLICE_NODES`; smaller in tests).
    pub(crate) slice_nodes: usize,
    /// Document nodes counted so far to cut slices (each slice counts its own nodes when it starts).
    pub(crate) slice_counted: usize,
    /// Milliseconds the last frame took to compile (CPU side).
    pub last_compile_ms: f32,
    pub last_stats: Option<designos_gpu::FrameStats>,
}

impl Default for GpuView {
    fn default() -> Self {
        let mut compiler = SceneCompiler::new();
        // re-flattening curves for a new zoom level may take at most this long per frame
        compiler.geom.budget = Some(std::time::Duration::from_millis(8));
        Self {
            pref: true,
            available: false,
            off_for_software: false,
            max_texture: 0,
            compiler,
            last_key: None,
            refined: false,
            settle: Settle::None,
            painting: false,
            stable_since: 0.0,
            idle_frames: 0,
            last_frame_ms: 0.0,
            frame_work_override: None,
            chunk_ack: Arc::new(AtomicUsize::new(0)),
            restarts: 0,
            slice_nodes: SLICE_NODES,
            slice_counted: 0,
            last_compile_ms: 0.0,
            last_stats: None,
        }
    }
}

impl GpuView {
    /// Whether the canvas is drawn by the GPU right now.
    pub fn active(&self) -> bool {
        self.pref && self.available
    }

    /// Cached spatial index of the current page's world bounds.
    pub fn scene_index(&self) -> &designos_render::SceneIndex {
        &self.compiler.index
    }

    /// Forgets cached geometry (the document was replaced).
    pub fn reset(&mut self) {
        self.compiler.geom.clear();
        self.compiler.index.clear();
        self.last_key = None;
        self.refined = false;
        self.settle = Settle::None;
        self.idle_frames = 0;
        self.restarts = 0;
    }

    /// Records the work the UI frame that just ended took, for the next frame's `plan`.
    pub fn note_frame_work(&mut self, ms: f32) {
        self.last_frame_ms = self.frame_work_override.unwrap_or(ms);
    }

    /// Whether a capture taken now shows the frame a user sees once the view is at
    /// rest: the canvas is not drawn by the GPU, or the full-quality refine of the
    /// current view has reached the GPU. Until then the frame on screen is the
    /// capped interactive one, whose masks, group opacity and effects past the
    /// offscreen-program cap are degraded.
    pub fn at_rest(&self) -> bool {
        !self.painting || self.refined
    }

    /// What this frame needs: the cached output, the capped interactive
    /// compile, or a step of the refine of a view that has gone idle.
    /// Idle = same frame key as the last compile for `IDLE_REFINE_DELAY`
    /// seconds of input time `now`, all geometry ready, no pointer gesture in
    /// progress, no scroll or zoom (`view_input`) in this frame, and a previous
    /// frame whose own work was short (`last_frame_ms`).
    ///
    /// The refine is a sequence of frames, none of which records more than a budget of CPU
    /// or GPU work (see [`Job`]): a full compile, geometry warm-up frames from the list that
    /// compile left deferred, a second full compile once that list is empty, and the chunks
    /// of the result, one per frame. A big page goes through that in slices of its top-level
    /// nodes, so no compile covers more than a slice. The frame on screen stays until the
    /// last chunk is in. A change of view or document, or a scroll or zoom, abandons the
    /// sequence; a pointer gesture pauses it; a chunk the GPU never took restarts it.
    fn plan(&mut self, key: FrameKey, now: f64, pointer_down: bool, view_input: bool) -> FramePlan {
        if let Settle::Refine(job) = &self.settle {
            if self.last_key == Some(key) && !view_input {
                if pointer_down {
                    return FramePlan::Reuse;
                }
                let chunks = job.chunks;
                let step = match job.step {
                    Step::Compile { .. } => return FramePlan::Refine,
                    Step::Warm { .. } => return FramePlan::Warm,
                    Step::Submit { .. } => FramePlan::Submit,
                    Step::Confirm => FramePlan::Finish,
                };
                // every chunk handed out so far must have reached the GPU
                if self.chunk_ack.load(Ordering::SeqCst) == chunks {
                    return step;
                }
                return self.restart_refine(now);
            }
            self.settle = Settle::None;
        }
        if self.refined && self.last_key == Some(key) {
            // also a refine taken as it was after `MAX_REFINE_COMPILES`, whatever it left deferred
            return FramePlan::Reuse;
        }
        let still = self.last_key == Some(key) && self.compiler.geom.stale_hits == 0;
        if !still {
            self.stable_since = now;
            self.idle_frames = 0;
            return FramePlan::Interactive;
        }
        if pointer_down {
            return FramePlan::Reuse;
        }
        if view_input {
            self.stable_since = now;
            self.idle_frames = 0;
        }
        let remaining = IDLE_REFINE_DELAY - (now - self.stable_since);
        if self.last_frame_ms < IDLE_FRAME_MS {
            self.idle_frames = self.idle_frames.saturating_add(1);
        } else {
            self.idle_frames = 0;
        }
        if remaining > 0.0 {
            return FramePlan::Wait(Duration::from_secs_f64(remaining));
        }
        if self.idle_frames >= 1 || -remaining >= IDLE_WAIT_LIMIT {
            FramePlan::Refine
        } else {
            // the frame before this one was busy (a document just opened): look again soon
            FramePlan::Wait(Duration::from_millis(16))
        }
    }

    /// Drops a refine whose chunks did not all reach the GPU and starts the idle delay over,
    /// so it begins again from the first chunk; the third time, the interactive frame stays.
    fn restart_refine(&mut self, now: f64) -> FramePlan {
        self.settle = Settle::None;
        self.refined = false;
        self.stable_since = now;
        self.idle_frames = 0;
        if self.restarts >= MAX_CHUNK_RESTARTS {
            self.give_up();
            return FramePlan::Reuse;
        }
        self.restarts += 1;
        FramePlan::Wait(Duration::from_secs_f64(IDLE_REFINE_DELAY))
    }

    /// Starts the refine of a big page in slices of its top-level nodes `top` (see
    /// `SceneCompiler::visible_top_level`), the first of which is `first`: the next
    /// `FramePlan::Refine` compiles it.
    fn begin_refine(&mut self, top: Vec<NodeId>, first: Range<usize>) {
        self.refined = false;
        self.settle = Settle::Refine(Job { top: Arc::new(top), sliced: true, cur: first, step: Step::Compile { passes: 0 }, chunks: 0, programs: 0 });
    }

    /// The top-level nodes the next refine compile covers, when the page is refined in slices.
    fn current_slice(&self) -> Option<(Arc<Vec<NodeId>>, Range<usize>)> {
        match &self.settle {
            Settle::Refine(job) if job.sliced => Some((job.top.clone(), job.cur.clone())),
            _ => None,
        }
    }

    /// The offscreen programs the next refine compile may open: what is left of
    /// `DEFAULT_REFINE_MAX_PROGRAMS` after the slices compiled so far (root program included).
    fn refine_program_limit(&self) -> usize {
        let used = match &self.settle {
            Settle::Refine(job) => job.programs,
            Settle::None => 0,
        };
        DEFAULT_REFINE_MAX_PROGRAMS.saturating_sub(used).max(1)
    }

    /// Whether the refine under way goes through the GPU in chunks whatever their number.
    fn sliced(&self) -> bool {
        matches!(&self.settle, Settle::Refine(job) if job.sliced)
    }

    /// Records the compile of `key` and returns the repaint it needs: another frame while
    /// geometry is still deferred or the view may now go idle, none once the refined frame
    /// is cached. A refine compile that left geometry deferred starts the warm-up
    /// (`FramePlan::Warm` frames) instead; one that did not is final: of a whole page it is
    /// the finished frame, which `GpuView::begin_submit` may still take over to submit in
    /// chunks; of a slice it waits for `begin_submit`.
    fn compiled(&mut self, key: FrameKey, refine: bool) -> Repaint {
        let stale = self.compiler.geom.stale_hits > 0;
        self.last_key = Some(key);
        self.refined = false;
        if !refine {
            self.settle = Settle::None;
            self.restarts = 0;
            return Repaint::Now;
        }
        let mut job = match std::mem::replace(&mut self.settle, Settle::None) {
            Settle::Refine(job) => job,
            Settle::None => Job::whole_page(),
        };
        let passes = match job.step {
            Step::Compile { passes } => passes,
            _ => 0,
        } + 1;
        if stale && passes < MAX_REFINE_COMPILES {
            job.step = Step::Warm { passes };
            self.settle = Settle::Refine(job);
            return Repaint::Now;
        }
        if job.sliced {
            // this slice is compiled: its chunks come next (`begin_submit`)
            job.step = Step::Compile { passes };
            self.settle = Settle::Refine(job);
            return Repaint::Now;
        }
        self.refined = true;
        Repaint::None
    }

    /// Whether the frame of the last `compiled` call goes to the screen whole: not that of a
    /// refine compile that left geometry deferred (it exists to learn what to build) nor of a
    /// slice (it is submitted in chunks). The frame on screen stays until the refine completes.
    fn presents_last_compile(&self) -> bool {
        !matches!(&self.settle, Settle::Refine(job) if matches!(job.step, Step::Warm { .. }))
    }

    /// Records a warm-up frame that built geometry from the deferred list, `left` entries
    /// remaining; the frame after the last one is the second full compile.
    fn warmed(&mut self, left: usize) -> Repaint {
        if let Settle::Refine(job) = &mut self.settle {
            if let Step::Warm { passes } = job.step {
                if left == 0 {
                    job.step = Step::Compile { passes };
                }
            }
        }
        Repaint::Now
    }

    /// Takes over the frame of the refine compile that just finished (`compiled` said it
    /// was final) to submit it in `ranges` of root ops, one per frame; the page appears
    /// when the last of the last slice is in. A whole page in a single range has nothing
    /// to spread: this does nothing then and the frame goes out whole.
    fn begin_submit(&mut self, frame: Arc<Frame>, ranges: Vec<Range<usize>>) -> Repaint {
        if !self.sliced() && ranges.len() < 2 {
            return Repaint::None;
        }
        let mut job = match std::mem::replace(&mut self.settle, Settle::None) {
            Settle::Refine(job) => job,
            Settle::None => Job::whole_page(),
        };
        if job.chunks == 0 {
            self.chunk_ack.store(0, Ordering::SeqCst);
        }
        job.programs += frame.programs.len().saturating_sub(1);
        job.step = Step::Submit { frame, ranges, next: 0 };
        self.refined = false;
        self.settle = Settle::Refine(job);
        Repaint::Now
    }

    /// Gives up a refine that cannot go out in chunks: the frame on screen stays.
    fn give_up(&mut self) {
        self.settle = Settle::None;
        self.refined = true;
    }

    /// The chunk this `FramePlan::Submit` frame submits. After the last chunk of a slice
    /// the next slice is cut (`doc`) and compiled; after the last of the last slice the
    /// refine waits for the callbacks' acknowledgement (`FramePlan::Finish`).
    fn next_chunk(&mut self, doc: &Document) -> Option<Chunk> {
        let slice_nodes = self.slice_nodes;
        let ack = self.chunk_ack.clone();
        let mut counted = 0;
        let Settle::Refine(job) = &mut self.settle else { return None };
        let Step::Submit { frame, ranges, next } = &mut job.step else { return None };
        let ops = ranges.get(*next)?.clone();
        let last_in_slice = *next + 1 == ranges.len();
        let last_slice = !job.sliced || job.cur.end >= job.top.len();
        let chunk = Chunk { frame: frame.clone(), ops, index: job.chunks, last: last_in_slice && last_slice, slice_done: last_in_slice, ack };
        *next += 1;
        job.chunks += 1;
        if last_in_slice {
            if last_slice {
                job.step = Step::Confirm;
            } else {
                let cut = next_slice(doc, &job.top, job.cur.end, slice_nodes);
                counted += cut.counted;
                job.cur = cut.range;
                job.step = Step::Compile { passes: 0 };
            }
        }
        self.slice_counted += counted;
        Some(chunk)
    }

    /// One frame of the canvas: does what `plan` decided and returns what the paint callback
    /// does and the repaint the frame asks for.
    fn step(&mut self, plan: FramePlan, cx: &FrameCtx) -> (Cb, Repaint) {
        let started = std::time::Instant::now();
        match plan {
            FramePlan::Reuse => (Cb::Blit, Repaint::None),
            FramePlan::Wait(d) => (Cb::Blit, Repaint::After(d)),
            FramePlan::Warm => {
                // geometry from the list of the last refine compile, within the 8 ms budget: the
                // page is not walked and nothing is submitted
                let left = self.compiler.warm_deferred(cx.doc, cx.rev);
                self.last_compile_ms = started.elapsed().as_secs_f32() * 1000.0;
                (Cb::Blit, self.warmed(left))
            }
            FramePlan::Submit => {
                let chunk = self.next_chunk(cx.doc);
                self.last_compile_ms = 0.0;
                (chunk.map_or(Cb::Blit, Cb::Chunk), Repaint::Now)
            }
            FramePlan::Finish => {
                // the last chunk reached the GPU: the page is on screen
                self.settle = Settle::None;
                self.refined = true;
                self.restarts = 0;
                (Cb::Blit, Repaint::None)
            }
            FramePlan::Interactive | FramePlan::Refine => self.compile_step(plan == FramePlan::Refine, cx, started),
        }
    }

    /// An interactive compile, or a compile of the refine: options, the compile of the page or
    /// of its current slice, what the compile means for the refine, and what goes to the GPU.
    fn compile_step(&mut self, refine: bool, cx: &FrameCtx, started: std::time::Instant) -> (Cb, Repaint) {
        if refine && matches!(self.settle, Settle::None) {
            // a big page is compiled in slices of its top-level nodes, so that no frame holds
            // the whole compile; only the first slice is counted now
            if let Some(top) = self.compiler.visible_top_level(cx.doc, cx.page, cx.rev, &cx.vp, cx.size) {
                let cut = next_slice(cx.doc, &top, 0, self.slice_nodes);
                self.slice_counted = cut.counted;
                if cut.range.end < top.len() {
                    self.begin_refine(top, cut.range);
                }
            }
        }
        self.compiler.opts.uncap_programs = refine;
        self.compiler.opts.full_detail = refine;
        self.compiler.opts.refine_max_programs = if refine { self.refine_program_limit() } else { DEFAULT_REFINE_MAX_PROGRAMS };
        let frame = match self.current_slice() {
            Some((top, range)) if refine => self.compiler.compile_slice(cx.doc, cx.page, cx.rev, &cx.vp, cx.size, &top[range]),
            _ => self.compiler.compile(cx.doc, cx.page, cx.rev, &cx.vp, cx.size),
        };
        self.compiler.opts.uncap_programs = false;
        self.compiler.opts.full_detail = false;
        self.compiler.opts.refine_max_programs = DEFAULT_REFINE_MAX_PROGRAMS;
        self.last_compile_ms = started.elapsed().as_secs_f32() * 1000.0;
        // some meshes may still have the previous zoom's flattening, or the view
        // may now be idle: come back for another frame (no timer needed); nothing
        // once the refined frame is cached
        let mut repaint = self.compiled(cx.key, refine);
        if !self.presents_last_compile() {
            // a refine that left geometry deferred only learns what to build
            return (Cb::Blit, repaint);
        }
        if self.sliced() {
            // a slice goes to the GPU in chunks, whatever their number
            let frame = Arc::new(frame);
            match frame.plan_chunks(CHUNK_BUDGET_US) {
                Some(ranges) => repaint = self.begin_submit(frame, ranges),
                None => {
                    self.give_up();
                    repaint = Repaint::None;
                }
            }
            return (Cb::Blit, repaint);
        }
        self.last_stats = Some(frame.stats());
        let frame = Arc::new(frame);
        // a finished refine of a big page goes to the GPU in chunks, one per frame
        match refine.then(|| frame.plan_chunks(CHUNK_BUDGET_US)).flatten() {
            Some(ranges) if ranges.len() > 1 => {
                repaint = self.begin_submit(frame, ranges);
                (Cb::Blit, repaint)
            }
            _ => (Cb::Frame(frame), repaint),
        }
    }

    /// Largest texture edge the device supports.
    pub fn set_max_texture(&mut self, max: u32) {
        self.compiler.opts.max_texture = max;
        self.max_texture = max;
    }
}

/// The repaint a frame asks egui for.
#[derive(Debug, PartialEq, Eq)]
enum Repaint {
    /// Nothing: the view is at rest (0% idle CPU).
    None,
    /// Another frame right away.
    Now,
    /// One frame after the delay (no timer thread).
    After(Duration),
}

impl Repaint {
    fn request(&self, ctx: &egui::Context) {
        match self {
            Repaint::None => {}
            Repaint::Now => ctx.request_repaint(),
            Repaint::After(d) => ctx.request_repaint_after(*d),
        }
    }
}

/// Where the refine of the current view stands between the frame that starts it and the one
/// that puts its page on screen.
enum Settle {
    /// No refine under way.
    None,
    Refine(Job),
}

/// A refine under way.
struct Job {
    /// The top-level nodes of the page in paint order. With `sliced`, `cur` is the slice being
    /// compiled, warmed or submitted (the next one is cut when it starts, so the page's nodes
    /// are not all counted up front); otherwise the whole page is one compile.
    top: Arc<Vec<NodeId>>,
    sliced: bool,
    cur: Range<usize>,
    step: Step,
    /// Chunks handed to the GPU so far, over all slices (the refine's page texture counts them).
    chunks: usize,
    /// Offscreen programs of the slices compiled so far: the refine as a whole stays within
    /// `DEFAULT_REFINE_MAX_PROGRAMS`, as one frame did.
    programs: usize,
}

impl Job {
    fn whole_page() -> Self {
        Job { top: Arc::new(Vec::new()), sliced: false, cur: 0..0, step: Step::Compile { passes: 0 }, chunks: 0, programs: 0 }
    }
}

enum Step {
    /// The next frame is a full compile (of the slice); `passes` compiles of it have run.
    Compile { passes: u32 },
    /// A compile left geometry deferred: each frame builds some of the nodes on
    /// `compiler.geom.deferred` within the 8 ms budget, without compiling anything.
    Warm { passes: u32 },
    /// The compiled frame goes to the GPU in `ranges` of root ops, chunk `next` in the next
    /// frame, into a page texture of its own.
    Submit { frame: Arc<Frame>, ranges: Vec<Range<usize>>, next: usize },
    /// Every chunk was handed out: the next frame checks that they all reached the GPU.
    Confirm,
}

/// One piece of the refined frame, for one UI frame.
struct Chunk {
    frame: Arc<Frame>,
    ops: Range<usize>,
    /// Counts on over the slices of a page.
    index: usize,
    /// The last chunk of the page: its page texture goes on screen.
    last: bool,
    /// The last chunk of its frame (slice): the GPU frees what the frame's programs held.
    slice_done: bool,
    /// Set to `index + 1` by the paint callback once the GPU took the chunk.
    ack: Arc<AtomicUsize>,
}

#[derive(Debug, PartialEq, Eq)]
enum FramePlan {
    /// Re-blit the cached output (`Cb::Blit`) and ask for nothing.
    Reuse,
    /// Re-blit the cached output; the view is unchanged but has not been idle
    /// for `IDLE_REFINE_DELAY` yet: come back after this long.
    Wait(Duration),
    /// Compile with the interactive fast paths and the 64-program cap.
    Interactive,
    /// Compile with the program cap lifted and the zoom-out shortcuts off: the first
    /// compile of a refine, or the one after its geometry was built.
    Refine,
    /// Build deferred geometry from the list of the last refine compile; no page walk,
    /// nothing submitted.
    Warm,
    /// Submit the next chunk of the refined frame into the refine's own page texture.
    Submit,
    /// The last chunk reached the GPU: the refine is done and the view at rest.
    Finish,
}

/// What the paint callback of a frame does before it blits the canvas.
enum Cb {
    /// Nothing: blit the page the canvas holds.
    Blit,
    /// Render a whole frame (an interactive compile, or a refine that fits one frame's
    /// budget) and show it.
    Frame(Arc<Frame>),
    /// Render one chunk of a refine into the refine's own page texture; the page on
    /// screen changes only with the last one.
    Chunk(Chunk),
}

impl egui_wgpu::CallbackTrait for Cb {
    fn prepare(
        &self,
        device: &egui_wgpu::wgpu::Device,
        _queue: &egui_wgpu::wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<egui_wgpu::wgpu::CommandBuffer> {
        match (self, resources.get_mut::<Canvas>()) {
            (Cb::Blit, _) => {}
            (Cb::Frame(frame), Some(c)) => c.prepare(device, encoder, frame),
            (Cb::Chunk(k), Some(c)) => {
                if c.prepare_chunk(device, &k.frame, k.ops.clone(), k.index, k.last, k.slice_done) {
                    k.ack.store(k.index + 1, Ordering::SeqCst);
                }
            }
            // no renderer to take it (the canvas is not on the GPU): nothing to void
            (Cb::Chunk(k), None) => k.ack.store(k.index + 1, Ordering::SeqCst),
            (Cb::Frame(_), None) => {}
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut egui_wgpu::wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        if let Some(c) = resources.get::<Canvas>() {
            c.paint(pass);
        }
    }
}

impl App {
    /// Preferences -> "Use GPU rendering": switches between the GPU canvas and the CPU tiles.
    pub fn toggle_gpu(&mut self) {
        if !self.canvas.gpu.available {
            self.show_toast("GPU rendering is not available on this system", true);
            return;
        }
        self.canvas.gpu.pref = !self.canvas.gpu.pref;
        self.canvas.gpu.off_for_software = false;
        self.canvas.gpu.last_key = None;
        let on = self.canvas.gpu.pref;
        if let Some(lib) = &self.lib {
            if let Err(e) = crate::comments::settings::save_flag(lib.root(), "use_gpu", on) {
                self.show_toast(e, true);
            }
        }
        // the tile cache may be stale after a long GPU session
        self.canvas.tiles.invalidate_all();
    }
}

/// Creates the GPU renderer on eframe's device. Returns false (and the app keeps
/// using the CPU tiles) when that is not possible.
pub fn init(rs: &egui_wgpu::RenderState) -> Option<u32> {
    match Canvas::new(&rs.device, &rs.queue, rs.target_format, 1) {
        Ok(c) => {
            let max = c.max_texture_dimension();
            rs.renderer.write().callback_resources.insert(c);
            Some(max)
        }
        Err(e) => {
            eprintln!("designos: GPU rendering unavailable, using the CPU renderer ({e})");
            None
        }
    }
}

/// Paints the page with the GPU into `rect`; false when the CPU path must do it.
pub fn paint(app: &mut App, ui: &Ui, rect: ERect) -> bool {
    app.canvas.gpu.painting = false;
    if !app.canvas.gpu.active() {
        return false;
    }
    let ppp = ui.ctx().pixels_per_point();
    let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
    if size.0 > app.canvas.gpu.max_texture.max(1) || size.1 > app.canvas.gpu.max_texture.max(1) {
        return false;
    }
    app.canvas.gpu.painting = true;
    let rev = super::tiles::take_rev(app);
    let runtime_opacities =
        if app.canvas.tiles.unlimited_budget { Vec::new() } else { app.agent_presence.active_top_level_insert_opacities(&app.doc, app.page, app.now) };
    let vp = Viewport { offset: app.view.offset, zoom: app.view.zoom, dpr: ppp };
    let key = frame_key(app.page, rev, &vp, size);
    let (doc, page) = (&app.doc, app.page);
    let g = &mut app.canvas.gpu;
    if g.compiler.opts.runtime_node_opacities != runtime_opacities {
        g.compiler.opts.runtime_node_opacities = runtime_opacities;
        g.last_key = None;
        g.refined = false;
        g.settle = Settle::None;
    }
    let (now, pointer_down, view_input) =
        ui.input(|i| (i.time, i.pointer.any_down(), i.smooth_scroll_delta != egui::Vec2::ZERO || (i.zoom_delta() - 1.0).abs() > 1e-4));
    let plan = g.plan(key, now, pointer_down, view_input);
    let started = std::time::Instant::now();
    let (cb, repaint) = g.step(plan, &FrameCtx { doc, page, rev, vp, size, key });
    repaint.request(ui.ctx());
    ui.painter().with_clip_rect(rect).add(egui_wgpu::Callback::new_paint_callback(rect, cb));
    app.canvas.last_render_ms = started.elapsed().as_secs_f32() * 1000.0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(rev: u64) -> FrameKey {
        (designos_doc::NodeId(0), rev, [0, 0], 1.0f32.to_bits(), 1.0f32.to_bits(), (100, 100), 0)
    }

    /// A view whose key last changed at input time 0 and whose last compile was for `key(1)`.
    fn view(refined: bool) -> GpuView {
        GpuView { last_key: Some(key(1)), refined, stable_since: 0.0, ..GpuView::default() }
    }

    fn waits(plan: FramePlan) -> f64 {
        match plan {
            FramePlan::Wait(d) => d.as_secs_f64(),
            other => panic!("expected Wait, got {other:?}"),
        }
    }

    #[test]
    fn changed_view_compiles_interactively() {
        assert_eq!(GpuView::default().plan(key(1), 0.0, false, false), FramePlan::Interactive);
        assert_eq!(view(false).plan(key(2), 1.0, false, false), FramePlan::Interactive, "a document change returns to the capped compile");
    }

    #[test]
    fn key_change_restarts_the_idle_delay() {
        let mut g = view(false);
        assert_eq!(g.plan(key(2), 5.0, false, false), FramePlan::Interactive);
        g.last_key = Some(key(2));
        assert!((waits(g.plan(key(2), 5.1, false, false)) - 0.15).abs() < 1e-9, "delay counts from the key change at t=5.0");
        assert_eq!(g.plan(key(2), 5.25, false, false), FramePlan::Refine);
    }

    #[test]
    fn no_refine_before_the_idle_delay() {
        let mut g = view(false);
        assert!((waits(g.plan(key(1), 0.0, false, false)) - IDLE_REFINE_DELAY).abs() < 1e-9);
        assert!((waits(g.plan(key(1), 0.10, false, false)) - 0.15).abs() < 1e-9);
        assert!(waits(g.plan(key(1), 0.249, false, false)) > 0.0, "one millisecond short is still waiting");
    }

    #[test]
    fn refines_once_the_key_stayed_unchanged_for_the_delay() {
        assert_eq!(view(false).plan(key(1), 0.25, false, false), FramePlan::Refine);
        assert_eq!(view(false).plan(key(1), 3.0, false, false), FramePlan::Refine);
    }

    #[test]
    fn scroll_or_zoom_delta_resets_the_timer() {
        let mut g = view(false);
        assert!(waits(g.plan(key(1), 0.20, false, true)) > 0.24, "input in the last frame restarts the full delay");
        assert!((waits(g.plan(key(1), 0.30, false, false)) - 0.15).abs() < 1e-9, "timer runs from the last input at t=0.20");
        assert!(waits(g.plan(key(1), 0.449, false, false)) > 0.0);
        assert_eq!(g.plan(key(1), 0.45, false, false), FramePlan::Refine);
        assert!(waits(view(false).plan(key(1), 9.0, false, true)) > 0.24, "even long after the key changed, a scroll frame never refines");
    }

    #[test]
    fn refine_waits_for_gesture_and_ready_geometry() {
        assert_eq!(view(false).plan(key(1), 5.0, true, false), FramePlan::Reuse, "no refine while the pointer is down");
        let mut g = view(false);
        g.compiler.geom.stale_hits = 3;
        assert_eq!(g.plan(key(1), 5.0, false, false), FramePlan::Interactive, "deferred geometry keeps the interactive compile");
    }

    #[test]
    fn refined_view_reuses_without_repaint() {
        for (t, down, input) in [(0.0, false, false), (0.3, false, false), (9.0, false, false), (9.0, true, false), (9.0, false, true)] {
            assert_eq!(view(true).plan(key(1), t, down, input), FramePlan::Reuse, "t={t} down={down} input={input}");
        }
        assert_eq!(GpuView::default().compiled(key(1), true), Repaint::None, "the refined compile asks for no further frame");
    }

    #[test]
    fn idle_sequence_refines_exactly_once_then_goes_silent() {
        let mut g = GpuView::default();
        // t=0: key changes -> capped compile, one repaint to find out whether the view is idle
        assert_eq!(g.plan(key(1), 0.0, false, false), FramePlan::Interactive);
        assert_eq!(g.compiled(key(1), false), Repaint::Now);
        // t=0.01: unchanged, still inside the delay -> blit and one wake-up after the rest
        let d = waits(g.plan(key(1), 0.01, false, false));
        assert!((d - 0.24).abs() < 1e-9);
        // the wake-up frame refines and asks for nothing
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        assert_eq!(g.compiled(key(1), true), Repaint::None);
        // every later idle frame reuses the cached output and never asks again
        for t in [0.3, 1.0, 60.0] {
            assert_eq!(g.plan(key(1), t, false, false), FramePlan::Reuse);
        }
    }

    /// A frame compiled from an empty page: only the shape matters to the settle sequence.
    fn some_frame() -> Arc<Frame> {
        let mut doc = designos_doc::Document::new("t");
        let page = doc.add_page("p");
        let vp = Viewport { offset: designos_doc::Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
        Arc::new(designos_gpu::SceneCompiler::new().compile(&doc, page, 1, &vp, (8, 8)))
    }

    fn pieces(n: usize) -> Vec<Range<usize>> {
        (0..n).map(|i| i..i + 1).collect()
    }

    /// What a refine compile that met `n` deferred nodes leaves behind.
    fn defer(g: &mut GpuView, n: usize) {
        g.compiler.geom.stale_hits = n as u32;
        g.compiler.geom.deferred = (0..n).map(|i| (designos_doc::NodeId(i as u64), 0)).collect();
    }

    /// One warm-up frame: builds one node of the list.
    fn warm_one(g: &mut GpuView) -> usize {
        g.compiler.geom.deferred.pop();
        g.compiler.geom.deferred.len()
    }

    #[test]
    fn deferred_geometry_after_a_refine_asks_for_another_frame() {
        let mut g = view(false);
        defer(&mut g, 2);
        assert_eq!(g.compiled(key(1), true), Repaint::Now);
        assert!(!g.refined, "a refine with deferred geometry is not final");
    }

    #[test]
    fn deferred_geometry_is_built_from_the_list_and_the_page_is_not_compiled_while_it_is_not_empty() {
        let mut g = view(false);
        let mut full_compiles = 0;
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        full_compiles += 1;
        defer(&mut g, 5);
        assert_eq!(g.compiled(key(1), true), Repaint::Now);
        assert!(!g.presents_last_compile(), "the compile that learns the list shows nothing");
        let mut warm_frames = 0;
        loop {
            let plan = g.plan(key(1), 0.26 + warm_frames as f64 * 0.01, false, false);
            match plan {
                FramePlan::Warm => {
                    warm_frames += 1;
                    let left = warm_one(&mut g);
                    assert_eq!(g.warmed(left), Repaint::Now);
                    if left == 0 {
                        break;
                    }
                }
                other => panic!("with {} nodes still on the list the plan must be Warm, got {other:?}", g.compiler.geom.deferred.len()),
            }
        }
        assert_eq!(warm_frames, 5, "one list entry per frame in this model: no frame compiled the page");
        // only now the page is compiled again, with its geometry cached
        assert_eq!(g.plan(key(1), 0.4, false, false), FramePlan::Refine);
        full_compiles += 1;
        g.compiler.geom.stale_hits = 0;
        g.compiler.geom.deferred.clear();
        assert_eq!(g.compiled(key(1), true), Repaint::None);
        assert!(g.refined && g.presents_last_compile());
        assert_eq!(full_compiles, 2, "one compile to learn the list, one once it is empty");
        for t in [0.5, 1.0, 60.0] {
            assert_eq!(g.plan(key(1), t, false, false), FramePlan::Reuse, "silent once the page is cached");
        }
    }

    #[test]
    fn a_compile_that_still_defers_is_warmed_for_again_but_only_so_often() {
        let mut g = view(false);
        let mut compiles = 0;
        for round in 0..10 {
            let plan = g.plan(key(1), 1.0 + round as f64, false, false);
            match plan {
                FramePlan::Refine => {
                    compiles += 1;
                    defer(&mut g, 1);
                    g.compiled(key(1), true);
                }
                FramePlan::Warm => {
                    let left = warm_one(&mut g);
                    g.warmed(left);
                }
                FramePlan::Reuse => break,
                other => panic!("round {round}: unexpected {other:?}"),
            }
        }
        assert_eq!(compiles, MAX_REFINE_COMPILES, "the frame is taken as it is after {MAX_REFINE_COMPILES} compiles");
        assert!(g.refined);
    }

    #[test]
    fn chunks_are_submitted_one_per_frame_in_order() {
        let (doc, _) = cards_doc(3);
        let mut g = GpuView { painting: true, last_key: Some(key(1)), stable_since: 0.0, ..GpuView::default() };
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        assert_eq!(g.compiled(key(1), true), Repaint::None);
        assert_eq!(g.begin_submit(some_frame(), pieces(3)), Repaint::Now, "three chunks to come");
        assert!(!g.refined && !g.at_rest(), "the interactive frame is still what is on screen");
        let mut seen = Vec::new();
        for frame_no in 0..3 {
            assert_eq!(g.plan(key(1), 0.3 + frame_no as f64 * 0.016, false, false), FramePlan::Submit);
            let c = g.next_chunk(&doc).expect("a chunk");
            seen.push((c.index, c.ops.clone(), c.last, c.slice_done));
            c.ack.store(c.index + 1, Ordering::SeqCst);
        }
        assert_eq!(seen, vec![(0, 0..1, false, false), (1, 1..2, false, false), (2, 2..3, true, true)]);
        assert!(g.next_chunk(&doc).is_none(), "no chunk is left");
    }

    #[test]
    fn a_frame_that_is_one_chunk_is_not_spread() {
        let mut g = view(false);
        g.compiled(key(1), true);
        assert_eq!(g.begin_submit(some_frame(), pieces(1)), Repaint::None);
        assert!(g.refined, "it goes out whole, like any frame");
        assert_eq!(g.plan(key(1), 1.0, false, false), FramePlan::Reuse);
    }

    #[test]
    fn refine_chunks_stay_within_the_per_frame_budget() {
        let mut doc = designos_doc::Document::new("blurred");
        let page = doc.add_page("p");
        for i in 0..300 {
            let id = doc.alloc_id();
            let mut n = designos_doc::Node::rectangle(id, designos_doc::Rect::new((i % 20) as f32 * 50.0, (i / 20) as f32 * 50.0, 40.0, 40.0));
            n.effects = vec![designos_doc::Effect::LayerBlur { radius: 3.0, visible: true }];
            doc.insert(page, n).unwrap();
        }
        let mut c = designos_gpu::SceneCompiler::new();
        c.geom.budget = Some(Duration::from_millis(8));
        c.opts.uncap_programs = true;
        c.opts.full_detail = true;
        let vp = Viewport { offset: designos_doc::Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
        let frame = c.compile(&doc, page, 1, &vp, (1200, 800));
        let ranges = frame.plan_chunks(CHUNK_BUDGET_US).expect("cuttable");
        assert!(ranges.len() > 3, "300 blurred layers do not fit one {CHUNK_BUDGET_US} us frame, got {} chunks", ranges.len());
        for r in &ranges {
            let cost = frame.estimate_chunk_us(r.clone());
            assert!(cost <= CHUNK_BUDGET_US || r.len() == 1, "chunk {r:?} is estimated at {cost} us");
        }
    }

    #[test]
    fn an_input_event_mid_settle_abandons_it_and_a_gesture_only_pauses_it() {
        type Phase = (&'static str, fn(&mut GpuView), FramePlan);
        let phases: [Phase; 3] = [
            (
                "warm",
                |g| {
                    defer(g, 3);
                    g.compiled(key(1), true);
                },
                FramePlan::Warm,
            ),
            (
                "compile",
                |g| {
                    defer(g, 1);
                    g.compiled(key(1), true);
                    let l = warm_one(g);
                    g.warmed(l);
                },
                FramePlan::Refine,
            ),
            (
                "submit",
                |g| {
                    g.compiled(key(1), true);
                    g.begin_submit(some_frame(), pieces(4));
                },
                FramePlan::Submit,
            ),
        ];
        for (name, setup, expected) in phases {
            // a scroll or zoom, the view unchanged so far
            let mut g = view(false);
            setup(&mut g);
            assert_eq!(g.plan(key(1), 0.3, false, false), expected, "{name}: under way");
            let plan = g.plan(key(1), 0.31, false, true);
            assert!(matches!(plan, FramePlan::Interactive | FramePlan::Wait(_)), "{name}: a scroll abandons the settle, got {plan:?}");
            assert!(matches!(g.settle, Settle::None), "{name}: nothing left of it");
            // the view or the document changed
            let mut g = view(false);
            setup(&mut g);
            let mut moved = key(1);
            moved.2 = [1.0f32.to_bits(), 0];
            assert_eq!(g.plan(moved, 0.3, false, false), FramePlan::Interactive, "{name}: a moved view starts over");
            assert!(matches!(g.settle, Settle::None), "{name}: nothing left of it");
            assert_eq!(g.plan(key(2), 0.4, false, false), FramePlan::Interactive, "{name}: and so does an edit");
            // a pointer gesture pauses it
            let mut g = view(false);
            setup(&mut g);
            assert_eq!(g.plan(key(1), 0.3, true, false), FramePlan::Reuse, "{name}: paused during a gesture");
            assert_eq!(g.plan(key(1), 0.4, false, false), expected, "{name}: resumes where it was");
        }
    }

    #[test]
    fn moving_the_view_abandons_a_refine_in_progress() {
        let mut g = view(false);
        defer(&mut g, 2);
        g.compiled(key(1), true);
        let mut moved = key(1);
        moved.2 = [1.0f32.to_bits(), 0];
        assert_eq!(g.plan(moved, 0.3, false, false), FramePlan::Interactive);
        g.compiler.geom.stale_hits = 0;
        g.compiled(moved, false);
        assert!(matches!(g.plan(moved, 0.31, false, false), FramePlan::Wait(_)), "after the interactive frame the idle delay applies again");
    }

    #[test]
    fn viewport_change_after_refine_returns_to_interactive() {
        let mut moved = key(1);
        moved.2 = [1.0f32.to_bits(), 0];
        assert_eq!(view(true).plan(moved, 1.0, false, false), FramePlan::Interactive);
    }

    #[test]
    fn reset_forgets_the_refined_frame() {
        let mut g = view(true);
        g.reset();
        assert_eq!(g.plan(key(1), 1.0, false, false), FramePlan::Interactive);
    }

    #[test]
    fn a_canvas_not_drawn_by_the_gpu_is_always_at_rest() {
        assert!(GpuView::default().at_rest());
    }

    #[test]
    fn gpu_view_is_at_rest_only_once_the_refine_compile_has_run() {
        let mut g = GpuView { painting: true, ..GpuView::default() };
        assert!(!g.at_rest(), "nothing compiled yet");
        assert_eq!(g.plan(key(1), 0.0, false, false), FramePlan::Interactive);
        g.compiled(key(1), false);
        assert!(!g.at_rest(), "the capped interactive frame is on screen");
        assert!(matches!(g.plan(key(1), 0.1, false, false), FramePlan::Wait(_)));
        assert!(!g.at_rest(), "still waiting for the idle delay");
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        g.compiled(key(1), true);
        assert!(g.at_rest(), "the refined frame is cached");
        assert_eq!(g.plan(key(2), 1.0, false, false), FramePlan::Interactive);
        g.compiled(key(2), false);
        assert!(!g.at_rest(), "a changed view leaves rest again");
    }

    #[test]
    fn refine_with_deferred_geometry_is_not_at_rest() {
        let mut g = GpuView { painting: true, ..GpuView::default() };
        g.compiler.geom.stale_hits = 1;
        g.compiled(key(1), true);
        assert!(!g.at_rest());
    }

    // ------------------------------------------------------------------ the canvas end to end, no UI

    fn test_vp(zoom: f32) -> Viewport {
        Viewport { offset: designos_doc::Vec2::new(0.0, 0.0), zoom, dpr: 1.0 }
    }

    /// Runs the canvas as `paint` does, frame after frame at 16 ms of input time, until the view is at
    /// rest. `take` decides whether a chunk's paint callback ran (the ack it would store).
    struct Drive {
        cbs: Vec<Cb>,
        frames: usize,
    }

    fn drive(g: &mut GpuView, doc: &Document, page: NodeId, vp: &Viewport, size: (u32, u32), mut take: impl FnMut(&Chunk) -> bool) -> Drive {
        let key = frame_key(page, 1, vp, size);
        let cx = FrameCtx { doc, page, rev: 1, vp: *vp, size, key };
        let mut run = Drive { cbs: Vec::new(), frames: 0 };
        for n in 0..60_000 {
            let plan = g.plan(key, n as f64 * 0.016, false, false);
            let (cb, _) = g.step(plan, &cx);
            if let Cb::Chunk(k) = &cb {
                if take(k) {
                    k.ack.store(k.index + 1, Ordering::SeqCst);
                }
            }
            run.cbs.push(cb);
            run.frames += 1;
            if g.refined {
                return run;
            }
        }
        panic!("the view never came to rest in {} frames", run.frames);
    }

    fn painting_view() -> GpuView {
        GpuView { painting: true, ..GpuView::default() }
    }

    /// `count` one-word labels, 12 px type: a few device pixels tall at zoom 0.2.
    fn labels_doc(count: usize) -> (Document, NodeId) {
        let mut doc = Document::new("labels");
        let page = doc.add_page("Page");
        for i in 0..count {
            let id = doc.alloc_id();
            let style = designos_doc::TextStyle { font_size: 12.0, fills: vec![designos_doc::Paint::solid(designos_doc::Color::BLACK)], ..Default::default() };
            let rect = designos_doc::Rect::new((i % 8) as f32 * 120.0 + 20.0, (i / 8) as f32 * 40.0 + 20.0, 60.0, 16.0);
            doc.insert(page, designos_doc::Node::text(id, rect, format!("Label {i}"), style)).unwrap();
        }
        (doc, page)
    }

    /// `count` frames of three nodes each: the frame, a label and a layer-blurred swatch.
    fn cards_doc(count: usize) -> (Document, NodeId) {
        let mut doc = Document::new("cards");
        let page = doc.add_page("Page");
        for i in 0..count {
            let id = doc.alloc_id();
            let (x, y) = ((i % 8) as f32 * 120.0 + 10.0, (i / 8) as f32 * 120.0 + 10.0);
            let card = doc.insert(page, designos_doc::Node::frame(id, designos_doc::Rect::new(x, y, 100.0, 100.0))).unwrap();
            let tid = doc.alloc_id();
            let style = designos_doc::TextStyle { font_size: 14.0, fills: vec![designos_doc::Paint::solid(designos_doc::Color::BLACK)], ..Default::default() };
            doc.insert(card, designos_doc::Node::text(tid, designos_doc::Rect::new(8.0, 8.0, 80.0, 18.0), format!("Card {i}"), style)).unwrap();
            let rid = doc.alloc_id();
            let mut swatch = designos_doc::Node::rectangle(rid, designos_doc::Rect::new(20.0, 40.0, 60.0, 40.0));
            swatch.effects = vec![designos_doc::Effect::LayerBlur { radius: 3.0, visible: true }];
            doc.insert(card, swatch).unwrap();
        }
        (doc, page)
    }

    /// Three containers of 1,500 layer-blurred swatches (one offscreen program each) followed by nine
    /// small rectangles: three slices of 3,000 nodes whose programs sum past 4,096.
    fn blurred_containers_doc() -> (Document, NodeId) {
        let mut doc = Document::new("containers");
        let page = doc.add_page("Page");
        for c in 0..3 {
            let id = doc.alloc_id();
            let frame = doc.insert(page, designos_doc::Node::frame(id, designos_doc::Rect::new(c as f32 * 1300.0, 0.0, 1250.0, 800.0))).unwrap();
            for i in 0..1_500 {
                let sid = doc.alloc_id();
                let mut n = designos_doc::Node::rectangle(sid, designos_doc::Rect::new((i % 50) as f32 * 24.0 + 4.0, (i / 50) as f32 * 24.0 + 4.0, 16.0, 16.0));
                n.effects = vec![designos_doc::Effect::LayerBlur { radius: 2.0, visible: true }];
                doc.insert(frame, n).unwrap();
            }
        }
        for i in 0..9 {
            let id = doc.alloc_id();
            doc.insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(i as f32 * 20.0, 900.0, 10.0, 10.0))).unwrap();
        }
        (doc, page)
    }

    fn frames_of(run: &Drive) -> Vec<Arc<Frame>> {
        let mut seen: Vec<Arc<Frame>> = Vec::new();
        for cb in &run.cbs {
            let f = match cb {
                Cb::Chunk(k) => &k.frame,
                Cb::Frame(f) => f,
                Cb::Blit => continue,
            };
            if !seen.iter().any(|s| Arc::ptr_eq(s, f)) {
                seen.push(f.clone());
            }
        }
        seen
    }

    /// What the refine compile of the canvas draws is the full-detail frame, not the zoom-out
    /// frame: its triangles are the unbudgeted compile's, and more than the interactive frame's.
    #[test]
    fn the_canvas_refine_draws_full_detail() {
        let (doc, page) = labels_doc(6);
        let (vp, size) = (test_vp(0.2), (1000, 1000));
        let mut g = painting_view();
        let run = drive(&mut g, &doc, page, &vp, size, |_| true);
        let frames: Vec<Arc<Frame>> = run.cbs.iter().filter_map(|c| if let Cb::Frame(f) = c { Some(f.clone()) } else { None }).collect();
        assert!(frames.len() >= 2, "an interactive frame, then the refined one: {} frames shown", frames.len());
        let (interactive, refined) = (frames[0].stats(), frames.last().unwrap().stats());
        let reference = designos_gpu::SceneCompiler::new().compile(&doc, page, 1, &vp, size).stats();
        assert_eq!(refined.triangles, reference.triangles, "the refine draws every glyph the unbudgeted compile draws");
        assert!(refined.triangles > interactive.triangles, "{} refined against {} interactive triangles", refined.triangles, interactive.triangles);
    }

    /// A refine compile that left geometry deferred shows nothing: the glyphs it could not draw yet
    /// must not reach the screen.
    #[test]
    fn a_refine_compile_that_left_geometry_deferred_goes_to_the_screen_as_nothing() {
        let (doc, page) = labels_doc(60);
        let (vp, size) = (test_vp(0.2), (1000, 1000));
        let mut g = painting_view();
        g.compiler.geom.budget = Some(Duration::from_micros(1));
        let key = frame_key(page, 1, &vp, size);
        let cx = FrameCtx { doc: &doc, page, rev: 1, vp, size, key };
        let plan = g.plan(key, 0.0, false, false);
        assert_eq!(plan, FramePlan::Interactive);
        let (cb, _) = g.step(plan, &cx);
        assert!(matches!(cb, Cb::Frame(_)), "interactive frames always show");
        let plan = g.plan(key, 0.3, false, false);
        assert_eq!(plan, FramePlan::Refine);
        let (cb, repaint) = g.step(plan, &cx);
        assert!(g.compiler.geom.stale_hits > 0, "a 1 microsecond budget defers the labels");
        assert!(matches!(cb, Cb::Blit), "a refine that left geometry deferred must not be shown");
        assert_eq!(repaint, Repaint::Now);
        assert!(matches!(&g.settle, Settle::Refine(Job { step: Step::Warm { .. }, .. })));
    }

    /// The slices of a refine share what is left of the program limit: the third gets what the first two left.
    #[test]
    fn later_slices_get_what_is_left_of_the_program_limit() {
        let (doc, page) = blurred_containers_doc();
        let (vp, size) = (test_vp(0.2), (1000, 800));
        let mut g = painting_view();
        let run = drive(&mut g, &doc, page, &vp, size, |_| true);
        let slices: Vec<usize> = frames_of(&run).iter().skip(1).map(|f| f.programs.len()).collect();
        assert_eq!(slices.len(), 3, "three slices: {slices:?}");
        assert_eq!((slices[0], slices[1]), (1_501, 1_501), "the first two slices open a program per swatch");
        assert!(slices[2] <= DEFAULT_REFINE_MAX_PROGRAMS - 3_000, "the third gets what is left of {DEFAULT_REFINE_MAX_PROGRAMS}, opened {}", slices[2]);
        assert!(slices[2] < 1_501, "so it cannot open one per swatch");
    }

    /// A paint callback that never ran leaves a gap in the acknowledgements: the refine starts over
    /// from the first chunk, and only the run that reaches the GPU whole puts the view at rest.
    #[test]
    fn a_chunk_that_never_reached_the_gpu_restarts_the_refine_from_the_first_chunk() {
        let (doc, page) = {
            let mut doc = Document::new("blurred");
            let page = doc.add_page("p");
            for i in 0..300 {
                let id = doc.alloc_id();
                let mut n = designos_doc::Node::rectangle(id, designos_doc::Rect::new((i % 20) as f32 * 50.0, (i / 20) as f32 * 50.0, 40.0, 40.0));
                n.effects = vec![designos_doc::Effect::LayerBlur { radius: 3.0, visible: true }];
                doc.insert(page, n).unwrap();
            }
            (doc, page)
        };
        let (vp, size) = (test_vp(1.0), (1200, 800));
        let mut g = painting_view();
        let mut dropped = false;
        let run = drive(&mut g, &doc, page, &vp, size, |c| {
            if c.index == 1 && !dropped {
                dropped = true;
                return false;
            }
            true
        });
        let indices: Vec<usize> = run.cbs.iter().filter_map(|c| if let Cb::Chunk(k) = c { Some(k.index) } else { None }).collect();
        let total = frames_of(&run).last().unwrap().stats().draws; // any positive number: the run had chunks
        assert!(total > 0);
        assert_eq!(&indices[..2], &[0, 1], "the first run hands out chunk 1, whose callback never runs");
        assert_eq!(indices[2], 0, "the next chunk handed out is the first again: {indices:?}");
        assert!(indices.len() > 4 && g.refined, "the second run goes through, {} chunks in all", indices.len());
        let last = run.cbs.iter().rev().find_map(|c| if let Cb::Chunk(k) = c { Some(k.last) } else { None });
        assert_eq!(last, Some(true));
    }

    #[test]
    fn the_view_is_not_at_rest_until_the_last_chunk_is_acknowledged() {
        let (doc, _) = cards_doc(3);
        let page = doc.pages[0];
        let mut g = painting_view();
        g.last_key = Some(key(1));
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        g.compiled(key(1), true);
        g.begin_submit(some_frame(), pieces(3));
        for i in 0..3 {
            assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Submit);
            let c = g.next_chunk(&doc).expect("chunk");
            assert!(!g.at_rest(), "chunk {i} was handed out, none is at the GPU yet");
            c.ack.store(c.index + 1, Ordering::SeqCst);
        }
        assert!(!g.at_rest(), "all handed out and acknowledged, but the frame after the last one confirms");
        assert_eq!(g.plan(key(1), 0.4, false, false), FramePlan::Finish);
        let cx = FrameCtx { doc: &doc, page, rev: 1, vp: test_vp(1.0), size: (100, 100), key: key(1) };
        let (cb, repaint) = g.step(FramePlan::Finish, &cx);
        assert!(matches!(cb, Cb::Blit));
        assert_eq!(repaint, Repaint::None);
        assert!(g.at_rest());
        assert_eq!(g.plan(key(1), 0.5, false, false), FramePlan::Reuse);
    }

    #[test]
    fn a_refine_whose_chunks_keep_missing_the_gpu_gives_up_after_two_restarts() {
        let (doc, _) = cards_doc(3);
        let mut g = painting_view();
        g.last_key = Some(key(1));
        let mut attempts = 0;
        for round in 0..20 {
            let t = 1.0 + round as f64;
            match g.plan(key(1), t, false, false) {
                FramePlan::Refine => {
                    g.compiled(key(1), true);
                    g.begin_submit(some_frame(), pieces(3));
                }
                FramePlan::Submit => {
                    let c = g.next_chunk(&doc).expect("chunk");
                    if c.index == 0 {
                        attempts += 1;
                        c.ack.store(1, Ordering::SeqCst);
                    } // chunk 1 and 2 never run
                }
                FramePlan::Wait(_) | FramePlan::Interactive => {}
                FramePlan::Reuse => break,
                other => panic!("round {round}: {other:?}"),
            }
        }
        assert_eq!(attempts, 1 + MAX_CHUNK_RESTARTS as usize, "the first run and two restarts");
        assert!(g.at_rest(), "then the interactive frame stays and the view is at rest");
        assert_eq!(g.plan(key(1), 99.0, false, false), FramePlan::Reuse);
    }

    // ------------------------------------------------------------------ the first frames after a document opens

    /// A document has opened: its first frame (compile, layout) took 300 ms. The next frame is already
    /// past the idle delay, but the refine waits for a frame that follows an idle one.
    #[test]
    fn the_refine_waits_for_the_frame_after_an_idle_frame() {
        let mut g = GpuView::default();
        assert_eq!(g.plan(key(1), 0.0, false, false), FramePlan::Interactive);
        g.compiled(key(1), false);
        g.note_frame_work(300.0);
        let after_busy = g.plan(key(1), 0.30, false, false);
        g.note_frame_work(3.0);
        let after_idle = g.plan(key(1), 0.32, false, false);
        eprintln!("frame after a 300 ms frame: {after_busy:?}; next frame after a 3 ms frame: {after_idle:?}");
        assert_eq!(after_busy, FramePlan::Wait(Duration::from_millis(16)), "the frame before was a 300 ms frame");
        assert_eq!(after_idle, FramePlan::Refine, "an idle frame came: the refine may start");
    }

    #[test]
    fn a_busy_frame_between_idle_ones_delays_the_refine_by_a_frame() {
        let mut g = GpuView::default();
        g.plan(key(1), 0.0, false, false);
        g.compiled(key(1), false);
        g.note_frame_work(2.0);
        assert!(matches!(g.plan(key(1), 0.1, false, false), FramePlan::Wait(_)));
        g.note_frame_work(60.0);
        assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Wait(Duration::from_millis(16)), "the previous frame was busy");
        g.note_frame_work(2.0);
        assert_eq!(g.plan(key(1), 0.32, false, false), FramePlan::Refine);
    }

    #[test]
    fn a_ui_with_no_idle_frame_still_gets_its_refine() {
        let mut g = GpuView::default();
        g.plan(key(1), 0.0, false, false);
        g.compiled(key(1), false);
        g.note_frame_work(40.0);
        for t in [0.3, 0.6, 1.0, 1.2] {
            assert!(matches!(g.plan(key(1), t, false, false), FramePlan::Wait(_)), "t={t}");
        }
        assert_eq!(g.plan(key(1), 1.3, false, false), FramePlan::Refine, "a second past the idle delay it starts whatever the frames cost");
    }

    /// The UI frame work is measured once per frame by the shell and handed to the canvas.
    #[test]
    fn the_frame_work_comes_from_the_shell_or_from_a_test_override() {
        let mut g = GpuView::default();
        g.note_frame_work(42.0);
        assert_eq!(g.last_frame_ms, 42.0);
        g.frame_work_override = Some(1.0);
        g.note_frame_work(42.0);
        assert_eq!(g.last_frame_ms, 1.0);
    }

    // ------------------------------------------------------------------ slices of a big page

    /// A page of 18 cards of three nodes, cut in slices of 18 nodes (6 cards): three slices.
    fn sliced_view() -> (GpuView, Document, NodeId) {
        let (doc, page) = cards_doc(18);
        let mut g = GpuView { painting: true, last_key: Some(key(1)), stable_since: 0.0, slice_nodes: 18, ..GpuView::default() };
        assert_eq!(g.plan(key(1), 0.25, false, false), FramePlan::Refine);
        let top = g.compiler.visible_top_level(&doc, page, 1, &test_vp(1.0), (1200, 800)).expect("an indexed page");
        let first = next_slice(&doc, &top, 0, 18).range;
        assert_eq!(first, 0..6);
        g.begin_refine(top, first);
        (g, doc, page)
    }

    #[test]
    fn a_big_page_is_refined_one_slice_at_a_time_and_is_at_rest_after_the_last_acknowledgement() {
        let (mut g, doc, page) = sliced_view();
        let mut chunks = Vec::new();
        for slice in 0..3usize {
            assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Refine, "slice {slice}: its compile");
            let (top, range) = g.current_slice().expect("a slice");
            assert_eq!(range, slice * 6..slice * 6 + 6, "slice {slice} covers its own six cards");
            assert_eq!(top.len(), 18);
            // the compile meets deferred geometry the first time round
            defer(&mut g, 2);
            assert_eq!(g.compiled(key(1), true), Repaint::Now);
            assert!(!g.presents_last_compile());
            while g.plan(key(1), 0.3, false, false) == FramePlan::Warm {
                let left = warm_one(&mut g);
                g.warmed(left);
            }
            assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Refine, "slice {slice}: compiled again once its list is empty");
            assert_eq!(g.current_slice().unwrap().1, slice * 6..slice * 6 + 6, "the same slice");
            g.compiler.geom.stale_hits = 0;
            g.compiler.geom.deferred.clear();
            assert_eq!(g.compiled(key(1), true), Repaint::Now, "a slice is not the end of the refine");
            assert!(!g.refined);
            assert_eq!(g.begin_submit(some_frame(), pieces(2)), Repaint::Now);
            for _ in 0..2 {
                assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Submit);
                let c = g.next_chunk(&doc).expect("chunk");
                chunks.push((c.index, c.last));
                c.ack.store(c.index + 1, Ordering::SeqCst);
                assert!(!g.at_rest(), "not before the page is confirmed");
            }
        }
        let expected: Vec<(usize, bool)> = (0..6).map(|i| (i, i == 5)).collect();
        assert_eq!(chunks, expected, "chunks count on over the slices and only the very last one finishes the page");
        assert_eq!(g.plan(key(1), 1.0, false, false), FramePlan::Finish);
        let cx = FrameCtx { doc: &doc, page, rev: 1, vp: test_vp(1.0), size: (1200, 800), key: key(1) };
        g.step(FramePlan::Finish, &cx);
        assert!(g.at_rest());
        assert_eq!(g.plan(key(1), 1.1, false, false), FramePlan::Reuse);
    }

    /// The first refine frame cuts the first slice only: it counts that slice's nodes, not the page's.
    #[test]
    fn the_first_refine_frame_counts_only_the_nodes_of_the_first_slice() {
        let (doc, page) = cards_doc(60);
        let (vp, size) = (test_vp(0.5), (1200, 800));
        let mut g = GpuView { painting: true, slice_nodes: 30, ..GpuView::default() };
        let key = frame_key(page, 1, &vp, size);
        let cx = FrameCtx { doc: &doc, page, rev: 1, vp, size, key };
        let plan = g.plan(key, 0.0, false, false);
        g.step(plan, &cx);
        assert_eq!(g.slice_counted, 0, "an interactive frame cuts nothing");
        // interactive frames until their geometry is ready and the idle delay has passed
        let mut n = 1.0;
        let plan = loop {
            let plan = g.plan(key, n * 0.3, false, false);
            if plan == FramePlan::Refine {
                break plan;
            }
            g.step(plan, &cx);
            n += 1.0;
            assert!(n < 100.0);
        };
        assert_eq!(g.slice_counted, 0, "nothing was cut before the refine");
        g.step(plan, &cx);
        assert!(g.sliced(), "180 nodes in slices of 30 are a sliced page");
        assert_eq!(g.current_slice().unwrap().1, 0..10);
        assert!(g.slice_counted <= 33, "the slice's 30 nodes and the card that did not fit, counted {}", g.slice_counted);
    }

    #[test]
    fn a_slice_with_a_single_chunk_is_still_submitted_as_a_chunk() {
        let (mut g, doc, _) = sliced_view();
        g.compiled(key(1), true);
        assert_eq!(g.begin_submit(some_frame(), pieces(1)), Repaint::Now, "slices of a page always go through the refine's page texture");
        assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Submit);
        let c = g.next_chunk(&doc).unwrap();
        assert_eq!((c.index, c.last), (0, false));
        c.ack.store(1, Ordering::SeqCst);
        assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Refine, "the next slice follows");
        assert_eq!(g.current_slice().unwrap().1, 6..12, "cut when it starts");
    }

    #[test]
    fn an_input_event_abandons_a_sliced_refine_in_every_step() {
        // mid-way: slice 1 is being compiled
        let (mut g, doc, _) = sliced_view();
        g.compiled(key(1), true);
        g.begin_submit(some_frame(), pieces(2));
        for _ in 0..2 {
            let c = g.next_chunk(&doc).unwrap();
            c.ack.store(c.index + 1, Ordering::SeqCst);
        }
        assert_eq!(g.plan(key(1), 0.3, false, false), FramePlan::Refine, "slice 1");
        let plan = g.plan(key(1), 0.31, false, true);
        assert!(matches!(plan, FramePlan::Interactive | FramePlan::Wait(_)), "a scroll abandons the refine, got {plan:?}");
        assert!(g.current_slice().is_none() && !g.sliced());
        assert!(!g.at_rest(), "nothing of it reached the screen");
    }

    #[test]
    fn a_refine_that_cannot_go_out_in_chunks_gives_up_and_leaves_the_frame_on_screen() {
        let (mut g, _, _) = sliced_view();
        g.compiled(key(1), true);
        g.give_up();
        assert!(g.at_rest());
        assert_eq!(g.plan(key(1), 1.0, false, false), FramePlan::Reuse);
    }
}
