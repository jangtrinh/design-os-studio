//! Canvas rendering on the GPU (`designos-gpu`): the visible page is compiled
//! to a frame while the UI is painted and drawn by an egui paint callback.
//! Pan and zoom only change the view transform: geometry is cached per node,
//! nothing is re-rasterised on the CPU.
//!
//! The CPU tile path (`tiles.rs`) stays the fallback: it is used when the GPU
//! cannot be initialised, when "Use GPU rendering" is off, and for
//! `--screenshot` unless `--gpu` is given (pixel diffs must be deterministic).

use crate::app::App;
use designos_gpu::{Canvas, Frame, SceneCompiler};
use designos_render::Viewport;
use eframe::egui_wgpu;
use egui::{Rect as ERect, Ui};
use std::sync::Arc;
use std::time::Duration;

/// Seconds the frame key must have stayed unchanged (and the view must have
/// seen no scroll or zoom) before the one full-quality refine compile runs.
/// Keeps the uncapped compile out of trackpad pans and momentum scrolls.
pub const IDLE_REFINE_DELAY: f64 = 0.25;

type FrameKey = (designos_doc::NodeId, u64, [u32; 2], u32, u32, (u32, u32), usize);

/// App-side GPU state.
pub struct GpuView {
    /// Preferences -> "Use GPU rendering".
    pub pref: bool,
    /// The device and renderer initialised.
    pub available: bool,
    max_texture: u32,
    compiler: SceneCompiler,
    last_key: Option<FrameKey>,
    /// The frame for `last_key` was compiled with the offscreen-program cap
    /// lifted (the idle full-quality pass), so there is nothing left to refine.
    refined: bool,
    /// The canvas was last drawn by the GPU, not by the CPU tiles.
    painting: bool,
    /// `egui` input time at which the frame key last changed, or the view last
    /// scrolled/zoomed while the key was unchanged: the start of the idle delay.
    stable_since: f64,
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
            max_texture: 0,
            compiler,
            last_key: None,
            refined: false,
            painting: false,
            stable_since: 0.0,
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
    }

    /// Whether a capture taken now shows the frame a user sees once the view is at
    /// rest: the canvas is not drawn by the GPU, or the one full-quality refine
    /// compile of the current view has run. Until then the frame on screen is the
    /// capped interactive one, whose masks, group opacity and effects past the
    /// offscreen-program cap are degraded.
    pub fn at_rest(&self) -> bool {
        !self.painting || self.refined
    }

    /// What this frame needs: the cached output, the capped interactive
    /// compile, or the one uncapped compile of a view that has gone idle.
    /// Idle = same frame key as the last compile for `IDLE_REFINE_DELAY`
    /// seconds of input time `now`, all geometry ready, no pointer gesture in
    /// progress and no scroll or zoom (`view_input`) in this frame.
    fn plan(&mut self, key: FrameKey, now: f64, pointer_down: bool, view_input: bool) -> FramePlan {
        let still = self.last_key == Some(key) && self.compiler.geom.stale_hits == 0;
        if !still {
            self.stable_since = now;
            return FramePlan::Interactive;
        }
        if self.refined || pointer_down {
            return FramePlan::Reuse;
        }
        if view_input {
            self.stable_since = now;
        }
        let remaining = IDLE_REFINE_DELAY - (now - self.stable_since);
        if remaining > 0.0 {
            FramePlan::Wait(Duration::from_secs_f64(remaining))
        } else {
            FramePlan::Refine
        }
    }

    /// Records the compile of `key` and returns the repaint it needs: another
    /// frame while geometry is still deferred or the view may now go idle, none
    /// once the refined frame is cached.
    fn compiled(&mut self, key: FrameKey, refine: bool) -> Repaint {
        let stale = self.compiler.geom.stale_hits > 0;
        self.last_key = Some(key);
        self.refined = refine && !stale;
        if stale || !refine {
            Repaint::Now
        } else {
            Repaint::None
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

#[derive(Debug, PartialEq, Eq)]
enum FramePlan {
    /// Re-blit the cached output (`Cb { frame: None }`) and ask for nothing.
    Reuse,
    /// Re-blit the cached output; the view is unchanged but has not been idle
    /// for `IDLE_REFINE_DELAY` yet: come back after this long.
    Wait(Duration),
    /// Compile with the interactive fast paths and the 64-program cap.
    Interactive,
    /// Compile once more with the program cap lifted and cache that frame.
    Refine,
}

struct Cb {
    frame: Option<Arc<Frame>>,
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
        if let (Some(c), Some(frame)) = (resources.get_mut::<Canvas>(), &self.frame) {
            c.prepare(device, encoder, frame);
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
    let vp = Viewport { offset: app.view.offset, zoom: app.view.zoom, dpr: ppp };
    let key: FrameKey =
        (app.page, rev, [vp.offset.x.to_bits(), vp.offset.y.to_bits()], vp.zoom.to_bits(), vp.dpr.to_bits(), size, designos_render::font_generation());
    let (doc, page) = (&app.doc, app.page);
    let g = &mut app.canvas.gpu;
    let (now, pointer_down, view_input) =
        ui.input(|i| (i.time, i.pointer.any_down(), i.smooth_scroll_delta != egui::Vec2::ZERO || (i.zoom_delta() - 1.0).abs() > 1e-4));
    let plan = g.plan(key, now, pointer_down, view_input);
    let wait = match &plan {
        FramePlan::Reuse => Some(Repaint::None),
        FramePlan::Wait(d) => Some(Repaint::After(*d)),
        FramePlan::Interactive | FramePlan::Refine => None,
    };
    if let Some(repaint) = wait {
        repaint.request(ui.ctx());
        ui.painter().with_clip_rect(rect).add(egui_wgpu::Callback::new_paint_callback(rect, Cb { frame: None }));
        return true;
    }
    let start = std::time::Instant::now();
    let refine = plan == FramePlan::Refine;
    g.compiler.opts.uncap_programs = refine;
    let frame = g.compiler.compile(doc, page, rev, &vp, size);
    g.compiler.opts.uncap_programs = false;
    g.last_stats = Some(frame.stats());
    g.last_compile_ms = start.elapsed().as_secs_f32() * 1000.0;
    // some meshes may still have the previous zoom's flattening, or the view
    // may now be idle: come back for another frame (no timer needed); nothing
    // once the refined frame is cached
    g.compiled(key, refine).request(ui.ctx());
    ui.painter().with_clip_rect(rect).add(egui_wgpu::Callback::new_paint_callback(rect, Cb { frame: Some(Arc::new(frame)) }));
    app.canvas.last_render_ms = start.elapsed().as_secs_f32() * 1000.0;
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

    #[test]
    fn deferred_geometry_after_a_refine_asks_for_another_frame() {
        let mut g = view(false);
        g.compiler.geom.stale_hits = 2;
        assert_eq!(g.compiled(key(1), true), Repaint::Now);
        assert!(!g.refined, "a refine with deferred geometry is not final");
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
}
