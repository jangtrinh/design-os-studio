//! Scene compiler: walks the page exactly like the CPU renderer's
//! `draw_children` / `draw_node` / `draw_body` and records a [`Frame`].
//!
//! Where the CPU renderer draws into a pixmap, this records draws into the
//! current *program*; where it allocates a layer pixmap, this opens a new
//! program (bounded to the node's pixels) and composites it afterwards.

use crate::frame::*;
use crate::geom::{bucket_of, GeomCache, NodeGeom};
use crate::mesh::Mesh;
use crate::paintconv::{PaintCache, PaintInst};
use crate::pipelines::{FORMAT, MSAA};
use designos_doc as d;
use designos_doc::{resolve_node, Document, Effect, Node, NodeId, NodeKind, Vec2};
use designos_render::board;
use designos_render::tiny_skia as sk;
use designos_render::{mask_frame_box, mask_stencil, node_margin, render_node_into, SceneIndex, StrokeClip, Viewport};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Decides which nodes the CPU renderer draws instead of the GPU.
pub type FallbackFn = Arc<dyn Fn(&Node) -> bool + Send + Sync>;

/// Default VRAM ceiling of the idle refine compile: 512 MiB.
///
/// Evidence: every offscreen program is accounted with the same per-pixel
/// formula `Pool::bytes` (resources.rs) uses for the real pool, summed over the
/// whole frame. That sum over-counts (the pool recycles equally sized free
/// targets), so the real pool stays below it. 512 MiB is about five full
/// 2880x1800 retina programs of MSAA colour + resolve + blur targets, far under
/// the multi-GiB working set of the integrated GPUs the editor runs on, while a
/// page at rest still gets thousands of regular-sized blurred layers. The value
/// is a conservative owner-reviewed default, not a measured crash threshold.
pub const DEFAULT_REFINE_VRAM_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

/// Default offscreen-program count ceiling for the idle refine compile: 4,096
/// programs (the root program plus up to 4,095 offscreen layer/mask/effect
/// programs).
///
/// Evidence: on the 112,474-node (`723,136` Kiwi records) `VSF - PCP.fig`
/// benchmark fixture at 2880x1800 (`1440x900 @2x`) across zooms `0.05..=4.0`,
/// an uncapped refine opens at most ~1,200 programs, whereas a synthetic page
/// of 400,000 tiny (`2x2`) blurred nodes would otherwise open ~400,001 `8x8`
/// programs (~1.28 KB each) inside the 512 MiB byte budget and stall CPU/GPU
/// pass encoding. 4,096 is >3x the largest real-document refine while bounding
/// pathological tiny-node pass explosions (`CLAUDE.md` §4.4).
///
/// This is a CPU/GPU work bound, not the Metal device limit. What the Metal
/// backend cannot run is a submission with more than 2,047 render passes (4,096
/// command buffers, two per pass), and a few hundred programs reach that: a
/// blurred layer alone is 4 passes. The executor splits such frames into
/// several submissions (`submit.rs`), so no program count can lose the device.
pub const DEFAULT_REFINE_MAX_PROGRAMS: usize = 4_096;

/// Compile-time knobs.
#[derive(Clone)]
pub struct CompileOptions {
    /// Largest texture edge the device supports (0 = unlimited).
    pub max_texture: u32,
    /// Nodes for which this returns true are rendered by the CPU renderer into
    /// a texture instead of by the GPU. Used by tests and as an escape hatch.
    pub force_fallback: Option<FallbackFn>,
    /// Render at this multiple of the output size and box-filter down (1 = off).
    /// 2 gives 16 samples per pixel together with the 4x MSAA.
    pub supersample: u32,
    /// Lifts the offscreen-program caps of interactive mode, for the one
    /// full-quality compile of an idle view. Headless compiles never cap.
    pub uncap_programs: bool,
    /// With `uncap_programs`: offscreen programs are only opened while the
    /// accounted GPU bytes of all targets of the frame (including the root
    /// output target) stay within this budget (see `program_bytes`); past it
    /// the compile degrades exactly like the capped interactive path (scissor
    /// clip, skipped effect). Ignored by capped and headless compiles.
    pub refine_vram_budget_bytes: u64,
    /// With `uncap_programs`: total programs in the frame (`frame.programs.len()`,
    /// including root program 0) are capped at this count backstop alongside
    /// `refine_vram_budget_bytes`. Ignored by capped and headless compiles.
    pub refine_max_programs: usize,
    /// Runtime-only per-node opacity multipliers (`0.0..=1.0`) applied during
    /// interactive canvas rendering without mutating `Document` or bumping `rev`.
    pub runtime_node_opacities: Vec<(NodeId, f32)>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            max_texture: 0,
            force_fallback: None,
            supersample: 0,
            uncap_programs: false,
            refine_vram_budget_bytes: DEFAULT_REFINE_VRAM_BUDGET_BYTES,
            refine_max_programs: DEFAULT_REFINE_MAX_PROGRAMS,
            runtime_node_opacities: Vec::new(),
        }
    }
}

/// Persistent compiler state: geometry, paint and index caches.
#[derive(Default)]
pub struct SceneCompiler {
    pub geom: GeomCache,
    paints: PaintCache,
    pub index: SceneIndex,
    pub opts: CompileOptions,
    unit_rect: Option<Arc<Mesh>>,
    /// Accounted GPU bytes of the offscreen targets of the last compiled frame.
    pub last_vram_bytes: u64,
}

impl SceneCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compiles the visible part of `page` seen through `vp` into a frame of
    /// `size` device pixels. `rev` must change whenever the document does.
    pub fn compile(&mut self, doc: &Document, page: NodeId, rev: u64, vp: &Viewport, size: (u32, u32)) -> Frame {
        self.index.ensure(doc, page, rev);
        let s = vp.zoom * vp.dpr;
        let ts = sk::Transform::from_row(s, 0.0, 0.0, s, -vp.offset.x * s, -vp.offset.y * s);
        self.compile_with(doc, page, rev, ts, size, false)
    }

    /// Compiles one node (a page: its children over the page background) with
    /// its own top-left at (0, 0), scaled by `scale`; the counterpart of
    /// `designos_render::render_node` (used for Present frames and exports).
    pub fn compile_node(&mut self, doc: &Document, node: NodeId, rev: u64, scale: f32, size: (u32, u32)) -> Frame {
        let ts = sk::Transform::from_scale(scale, scale);
        self.compile_with(doc, node, rev, ts, size, true)
    }

    fn compile_with(&mut self, doc: &Document, root_node: NodeId, rev: u64, logical_ts: sk::Transform, size: (u32, u32), single: bool) -> Frame {
        self.geom.trim(rev);
        self.geom.begin_frame();
        let unit = self.unit_rect.get_or_insert_with(|| Arc::new(Mesh::rect(0.0, 0.0, 1.0, 1.0))).clone();
        let ss = self.opts.supersample.clamp(1, 4);
        let clampi = |v: f32| if v.is_finite() { v.clamp(-1.0e9, 1.0e9) as i32 } else { 0 };
        let grid = (clampi(-logical_ts.tx.round()), clampi(-logical_ts.ty.round()));
        let root_ts = logical_ts.post_scale(ss as f32, ss as f32);
        let out_size = size;
        let size = (size.0 * ss, size.1 * ss);
        let root_rect = IRect::new(0, 0, size.0 as i32, size.1 as i32);
        let live = !single && self.geom.budget.is_some();
        let mut cx = Ctx {
            doc,
            geom: &mut self.geom,
            paints: &mut self.paints,
            index: &self.index,
            opts: &self.opts,
            rev,
            root_ts,
            root_rect,
            grid,
            progs: Vec::new(),
            stack: Vec::new(),
            clips: Vec::new(),
            clip_ids: HashMap::new(),
            unit,
            cull: !single,
            live,
            ss,
            vram: root_bytes(out_size, size, ss),
            reserved: (0, 0),
            stencil_root: None,
            stencil_children_skipped: false,
        };
        let node = doc.get(root_node).map(|n| resolve_node(doc, n));
        let bg = match node.as_deref().map(|n| &n.kind) {
            Some(NodeKind::Page { background }) => [background.r, background.g, background.b, background.a],
            _ if single => [0.0; 4],
            _ => [0x5A as f32 / 255.0, 0x5A as f32 / 255.0, 0x5A as f32 / 255.0, 1.0],
        };
        let mut root = Program::new(root_rect);
        root.clear = [bg[0].clamp(0.0, 1.0) * bg[3], bg[1].clamp(0.0, 1.0) * bg[3], bg[2].clamp(0.0, 1.0) * bg[3], bg[3].clamp(0.0, 1.0)];
        cx.progs.push(root);
        cx.stack.push(0);
        let full = cx.clip_for(ClipDesc { scissor: root_rect, mask: None });
        if let Some(n) = node.as_deref() {
            if single && !matches!(n.kind, NodeKind::Page { .. }) {
                cx.draw_node(n, root_ts, full);
            } else if cx.cull
                && n.children.len() > 8
                && !cx.index.has_top_mask()
                && root_ts.kx == 0.0
                && root_ts.ky == 0.0
                && root_ts.sx > 0.0
                && root_ts.sy > 0.0
            {
                let wx0 = -root_ts.tx / root_ts.sx - 64.0;
                let wy0 = -root_ts.ty / root_ts.sy - 64.0;
                let wx1 = (root_rect.w() as f32 - root_ts.tx) / root_ts.sx + 64.0;
                let wy1 = (root_rect.h() as f32 - root_ts.ty) / root_ts.sy + 64.0;
                let mut children = Vec::new();
                cx.index.query_top_level((wx0, wy0, wx1, wy1), &mut children);
                cx.draw_children(&children, root_ts, full);
            } else {
                cx.draw_children(&n.children, root_ts, full);
            }
        }
        let vram = cx.vram;
        let frame = Frame { size, out_size, ss, root: 0, programs: cx.progs, clips: cx.clips };
        self.last_vram_bytes = vram;
        frame
    }
}

// ---------------------------------------------------------------- VRAM accounting

/// Bytes per pixel of every offscreen target (`pipelines::FORMAT`, Rgba8Unorm).
fn texel_bytes() -> u64 {
    FORMAT.block_copy_size(None).map_or(4, u64::from)
}

/// Edge length of a pooled target: `resources.rs` rounds every request up to a
/// power of two (8..=64) or a multiple of 64.
fn pool_extent(v: i32) -> u64 {
    let v = v.max(1) as u64;
    if v <= 64 {
        v.next_power_of_two().max(8)
    } else {
        v.div_ceil(64) * 64
    }
}

/// Pixels of the pooled target a `rect`-sized request ends up with.
fn pooled_pixels(rect: IRect) -> u64 {
    pool_extent(rect.w()) * pool_extent(rect.h())
}

/// GPU bytes of the root output target (`exec.rs` `ensure_output`: single-sample
/// `output` at `out_size` plus `MSAA` colour target `output_ms` at `size`, and
/// when supersampling `ss > 1` the pooled root program targets).
fn root_bytes(out_size: (u32, u32), size: (u32, u32), ss: u32) -> u64 {
    let out_px = out_size.0.max(1) as u64 * out_size.1.max(1) as u64;
    let ms_px = size.0.max(1) as u64 * size.1.max(1) as u64;
    let base = (out_px + ms_px * MSAA as u64) * texel_bytes();
    if ss > 1 {
        base + program_bytes(IRect::new(0, 0, size.0 as i32, size.1 as i32))
    } else {
        base
    }
}

/// GPU bytes of one offscreen program: the 4x MSAA colour target (`ms_pool`,
/// resources.rs `sample_count: MSAA`) plus its single-sample resolve target
/// (`plain_pool`), both `FORMAT`, both at the pooled extent
/// (exec.rs `run_program`: `ms_pool.acquire` + `plain_pool.acquire`).
fn program_bytes(rect: IRect) -> u64 {
    pooled_pixels(rect) * texel_bytes() * (MSAA as u64 + 1)
}

/// Extra bytes while a program's gaussian blur runs: the horizontal-pass target
/// and the vertical-pass result, both `plain_pool` (exec.rs `blur`).
fn blur_bytes(rect: IRect) -> u64 {
    pooled_pixels(rect) * texel_bytes() * 2
}

/// Extra bytes of a displacement post: its destination target (exec.rs).
fn displace_bytes(rect: IRect) -> u64 {
    pooled_pixels(rect) * texel_bytes()
}

/// Scratch targets of a backdrop op over `pad`, all `plain_pool` (exec.rs
/// `backdrop`): blur = backdrop copy, two blur targets, snapshot; glass =
/// backdrop copy, frost blur (2), mask blur (2), output, snapshot.
fn backdrop_bytes(glass: bool, pad: IRect) -> u64 {
    pooled_pixels(pad) * texel_bytes() * if glass { 7 } else { 4 }
}

fn post_bytes(post: &Post, rect: IRect) -> u64 {
    match post {
        Post::Blur { .. } => blur_bytes(rect),
        Post::Displace { .. } => displace_bytes(rect),
    }
}

/// What a clip-mask program that the VRAM budget refuses degrades to.
enum Degrade {
    /// Clip to this rectangle instead (what the capped path does for a frame's children).
    Scissor(IRect),
    /// Leave the clip as it is (what the capped path does for strokes).
    Keep,
}

// ---------------------------------------------------------------- helpers

type FBox = (f32, f32, f32, f32);

fn xform_bounds(ts: &sk::Transform, x: f32, y: f32, w: f32, h: f32) -> FBox {
    let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (px, py) in pts {
        let tx = ts.sx * px + ts.kx * py + ts.tx;
        let ty = ts.ky * px + ts.sy * py + ts.ty;
        x0 = x0.min(tx);
        y0 = y0.min(ty);
        x1 = x1.max(tx);
        y1 = y1.max(ty);
    }
    (x0, y0, x1, y1)
}

fn ts_scale(ts: &sk::Transform) -> f32 {
    (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt().max(1e-6)
}

fn to_sk(a: &d::Affine) -> sk::Transform {
    sk::Transform::from_row(a.a, a.b, a.c, a.d, a.tx, a.ty)
}

fn hits(bb: FBox, r: IRect) -> bool {
    !(r.is_empty() || bb.2 < r.x0 as f32 || bb.3 < r.y0 as f32 || bb.0 > r.x1 as f32 || bb.1 > r.y1 as f32)
}

fn draws_children(n: &Node) -> bool {
    !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_))
}

fn special_blend(b: d::BlendMode) -> bool {
    !matches!(b, d::BlendMode::Normal | d::BlendMode::PassThrough)
}

/// How a finished shadow layer composites over what is beneath it.
fn shadow_mode(b: d::BlendMode) -> CompMode {
    if special_blend(b) {
        CompMode::Blend(b)
    } else {
        CompMode::Over
    }
}

#[inline]
fn visible_fills(n: &Node) -> impl Iterator<Item = &d::Paint> + Clone {
    let slice = match &n.kind {
        NodeKind::Text(t) => t.style.fills.as_slice(),
        _ => n.fills.as_slice(),
    };
    slice.iter().filter(|p| p.visible)
}

#[inline]
fn visible_strokes(n: &Node) -> impl Iterator<Item = &d::Paint> + Clone {
    let active = n.stroke.weight > 0.0 || n.stroke.side_weights.is_some();
    let slice = if active { n.stroke.paints.as_slice() } else { &[] };
    slice.iter().filter(|p| p.visible)
}

#[inline]
fn has_visible_fills(n: &Node) -> bool {
    visible_fills(n).next().is_some()
}

#[inline]
fn has_visible_strokes(n: &Node) -> bool {
    visible_strokes(n).next().is_some()
}

fn identity_like(ts: &sk::Transform) -> bool {
    ts.kx.abs() < 1e-5 && ts.ky.abs() < 1e-5 && ts.sx > 0.0 && ts.sy > 0.0
}

struct Ctx<'a> {
    doc: &'a Document,
    geom: &'a mut GeomCache,
    paints: &'a mut PaintCache,
    index: &'a SceneIndex,
    opts: &'a CompileOptions,
    rev: u64,
    root_ts: sk::Transform,
    root_rect: IRect,
    grid: (i32, i32),
    progs: Vec<Program>,
    stack: Vec<ProgId>,
    clips: Vec<ClipDesc>,
    clip_ids: HashMap<ClipDesc, ClipId>,
    unit: Arc<Mesh>,
    /// Index-based culling is valid (not while drawing offset silhouettes).
    cull: bool,
    /// A frame-budgeted (interactive) compile of a whole page. Unlike `cull`
    /// this stays set while a silhouette draws its children, so the resource
    /// limits of the idle refine keep binding there.
    live: bool,
    /// Supersampling factor of the target.
    ss: u32,
    /// Accounted GPU bytes of the offscreen targets opened so far.
    vram: u64,
    /// Programs and bytes that effects being drawn have been promised but not
    /// opened yet (see `reserving`); the refine limits count them as used.
    reserved: (usize, u64),
    /// The mask whose stencil is being drawn (see `draw_mask`).
    stencil_root: Option<NodeId>,
    /// The children of `stencil_root` were left out for being too small at this
    /// zoom.
    stencil_children_skipped: bool,
}

impl Ctx<'_> {
    // ----- programs and clips ---------------------------------------------

    fn cur(&self) -> ProgId {
        *self.stack.last().expect("program stack")
    }

    fn push_op(&mut self, op: Op) {
        let c = self.cur() as usize;
        self.progs[c].ops.push(op);
    }

    /// `rect` as the texture the device can actually hold (extreme blur radii).
    fn clamp_rect(&self, mut rect: IRect) -> IRect {
        let max = self.opts.max_texture as i32;
        if max > 0 {
            rect.x1 = rect.x1.min(rect.x0 + max);
            rect.y1 = rect.y1.min(rect.y0 + max);
        }
        rect
    }

    fn new_prog(&mut self, rect: IRect) -> ProgId {
        let rect = self.clamp_rect(rect);
        self.vram += program_bytes(rect);
        self.progs.push(Program::new(rect));
        (self.progs.len() - 1) as ProgId
    }

    /// Adds a post-processing step to program `id`, accounting its targets.
    fn push_post(&mut self, id: ProgId, post: Post) {
        let prog = &mut self.progs[id as usize];
        self.vram += post_bytes(&post, prog.rect);
        prog.post.push(post);
    }

    fn push_backdrop(&mut self, op: BackdropOp) {
        self.vram += backdrop_bytes(matches!(op.kind, BackdropKind::Glass(_)), op.pad);
        self.push_op(Op::Backdrop(Box::new(op)));
    }

    /// Bytes of a program that would be opened for `rect`.
    fn program_cost(&self, rect: IRect) -> u64 {
        program_bytes(self.clamp_rect(rect))
    }

    fn blur_cost(&self, rect: IRect) -> u64 {
        blur_bytes(self.clamp_rect(rect))
    }

    fn clip_for(&mut self, c: ClipDesc) -> ClipId {
        if let Some(id) = self.clip_ids.get(&c) {
            return *id;
        }
        let id = self.clips.len() as ClipId;
        self.clips.push(c);
        self.clip_ids.insert(c, id);
        id
    }

    fn full_clip(&mut self, rect: IRect) -> ClipId {
        self.clip_for(ClipDesc { scissor: rect, mask: None })
    }

    /// Area of the current program still visible through `clip`.
    fn visible(&self, clip: ClipId) -> IRect {
        self.progs[self.cur() as usize].rect.intersect(self.clips[clip as usize].scissor)
    }

    fn with_prog<R>(&mut self, id: ProgId, f: impl FnOnce(&mut Self) -> R) -> R {
        self.stack.push(id);
        let r = f(self);
        self.stack.pop();
        r
    }

    fn narrow_clip(&mut self, clip: ClipId, rect: IRect) -> ClipId {
        let c = self.clips[clip as usize];
        let s = c.scissor.intersect(rect);
        self.clip_for(ClipDesc { scissor: s, mask: c.mask })
    }

    /// A mask program covering `mesh` (through `ts`) restricted by `parent`;
    /// returns the clip that limits draws to it (`invert`: to its complement
    /// within `roi`). `None` when nothing can remain visible.
    fn mask_clip(&mut self, mesh: &Arc<Mesh>, ts: sk::Transform, parent: ClipId, invert: bool, roi: IRect, degrade: Degrade) -> Option<ClipId> {
        let pdesc = self.clips[parent as usize];
        let bb = xform_bounds(&ts, mesh.bounds.0, mesh.bounds.1, mesh.bounds.2 - mesh.bounds.0, mesh.bounds.3 - mesh.bounds.1);
        let region = if invert { roi } else { IRect::outward(bb).intersect(roi) };
        let region = region.intersect(pdesc.scissor).intersect(self.progs[self.cur() as usize].rect);
        if region.is_empty() {
            return None;
        }
        if self.over_budget(1, self.program_cost(region)) {
            return Some(match degrade {
                Degrade::Scissor(r) => self.narrow_clip(parent, r),
                Degrade::Keep => parent,
            });
        }
        let prog = self.new_prog(region);
        let white = solid_inst([1.0, 1.0, 1.0, 1.0]);
        let inner = self.full_clip(region);
        let with_parent = self.clip_for(ClipDesc { scissor: region, mask: pdesc.mask });
        self.with_prog(prog, |c| {
            if invert {
                // everything inside the parent clip ...
                c.draw_quad(region, white.clone(), DrawBlend::Over, with_parent);
                // ... minus the shape
                c.draw_mesh(mesh, ts, white, DrawBlend::Erase, inner);
            } else {
                c.draw_mesh(mesh, ts, white, DrawBlend::Over, with_parent);
            }
        });
        Some(self.clip_for(ClipDesc { scissor: region, mask: Some(prog) }))
    }

    // ----- low level draws --------------------------------------------------

    fn draw_mesh(&mut self, mesh: &Arc<Mesh>, ts: sk::Transform, mut pi: PaintInst, blend: DrawBlend, clip: ClipId) {
        let bb = xform_bounds(&ts, mesh.bounds.0, mesh.bounds.1, mesh.bounds.2 - mesh.bounds.0, mesh.bounds.3 - mesh.bounds.1);
        if mesh.is_empty() || !hits(bb, self.visible(clip)) {
            return;
        }
        pi.inst.m = [ts.sx, ts.ky, ts.kx, ts.sy];
        pi.inst.t[0] = ts.tx;
        pi.inst.t[1] = ts.ty;
        self.push_op(Op::Draw(Box::new(DrawOp { mesh: mesh.clone(), inst: pi.inst, blend, clip, image: pi.image, ramp: pi.ramp })));
    }

    /// Fills `rect` (root pixels) with a paint.
    fn draw_quad(&mut self, rect: IRect, pi: PaintInst, blend: DrawBlend, clip: ClipId) {
        let ts = sk::Transform::from_row(rect.w() as f32, 0.0, 0.0, rect.h() as f32, rect.x0 as f32, rect.y0 as f32);
        let unit = self.unit.clone();
        self.draw_mesh(&unit, ts, pi, blend, clip);
    }

    // ----- tree -------------------------------------------------------------

    /// Interactive approximations (greeking, sub-pixel culling, effect skips)
    /// apply to index-culled drawing only: a silhouette draws its children at
    /// full detail. The offscreen-program and VRAM limits do not follow this
    /// switch: they follow `live` (see `prog_cap_hit` and `over_budget`).
    fn interactive(&self) -> bool {
        self.cull && self.geom.budget.is_some()
    }

    /// The idle refine compile: interactive mode with its program cap lifted
    /// (`CompileOptions::uncap_programs`). Its limits hold for the whole page,
    /// also inside silhouette subtrees where `interactive()` is off.
    fn refining(&self) -> bool {
        self.live && self.opts.uncap_programs
    }

    /// Whether interactive mode forbids opening more offscreen programs once
    /// `limit` exist (replaced by `CompileOptions::refine_max_programs`,
    /// counting reserved programs, when `CompileOptions::uncap_programs` is
    /// set). Holds for the whole page, silhouette subtrees included.
    fn prog_cap_hit(&self, limit: usize) -> bool {
        if self.refining() {
            return self.progs.len().saturating_add(self.reserved.0) >= self.opts.refine_max_programs;
        }
        self.live && self.progs.len() >= limit
    }

    /// Whether the idle refine compile must not open `progs` more offscreen
    /// targets worth `bytes` more: the frame would exceed
    /// `refine_max_programs` or `refine_vram_budget_bytes`. Only the uncapped
    /// interactive compile is bound; callers degrade exactly like the
    /// `prog_cap_hit` fallback of the same site.
    fn over_budget(&self, progs: usize, bytes: u64) -> bool {
        self.refining()
            && (self.progs.len().saturating_add(self.reserved.0).saturating_add(progs) > self.opts.refine_max_programs
                || self.vram.saturating_add(self.reserved.1).saturating_add(bytes) > self.opts.refine_vram_budget_bytes)
    }

    /// Runs `f` with `progs` programs and `bytes` bytes held back for targets
    /// that the caller opens only after `f` returns. An effect checks the cost
    /// of all its targets once, but draws children (which open targets of their
    /// own) between opening them; without the hold those children could use up
    /// what the effect was already promised.
    fn reserving<R>(&mut self, progs: usize, bytes: u64, f: impl FnOnce(&mut Self) -> R) -> R {
        let held = self.reserved;
        self.reserved = (held.0.saturating_add(progs), held.1.saturating_add(bytes));
        let r = f(self);
        self.reserved = held;
        r
    }

    fn zoom_lod(&self) -> bool {
        self.interactive() && ts_scale(&self.root_ts) < 0.45
    }

    fn draw_children(&mut self, children: &[NodeId], parent_ts: sk::Transform, clip: ClipId) {
        let doc = self.doc;
        let vis = self.visible(clip);
        if vis.is_empty() {
            return;
        }
        let zoom_lod = self.zoom_lod();
        for (i, cid) in children.iter().enumerate() {
            if self.cull && !board::is_synthetic(*cid) {
                let Some((b, is_mask)) = self.index.cull_info(*cid) else {
                    continue;
                };
                if !is_mask {
                    let bb = xform_bounds(&self.root_ts, b.0, b.1, b.2 - b.0, b.3 - b.1);
                    let bw = bb.2 - bb.0;
                    let bh = bb.3 - bb.1;
                    if !hits(bb, vis) || (zoom_lod && ((bw < 1.25 && bh < 1.25) || (bw.max(bh) < 4.0 && bw.min(bh) < 0.35))) {
                        continue;
                    }
                }
            }
            let Some(child) = board::get(doc, *cid) else { continue };
            let child = resolve_node(doc, &child);
            let child: &Node = &child;
            let ts = parent_ts.pre_concat(to_sk(&child.transform));
            if child.is_mask {
                if !child.visible {
                    continue;
                }
                self.draw_mask(child, children, i, parent_ts, ts, clip);
                return;
            }
            self.draw_node(child, ts, clip);
        }
    }

    /// Called where a node leaves its children out for being too small: when
    /// the node is the mask whose stencil is being drawn, `draw_mask` stands in
    /// its box.
    fn note_stencil_children_skipped(&mut self, n: &Node) {
        if self.stencil_root == Some(n.id) {
            self.stencil_children_skipped = true;
        }
    }

    /// Whether program `id` (or any child program it composites) recorded a draw.
    fn prog_has_draws(&self, id: ProgId) -> bool {
        let p = &self.progs[id as usize];
        p.pixels.is_some()
            || p.ops.iter().any(|op| match op {
                Op::Draw(_) | Op::Backdrop(_) => true,
                Op::Composite(c) => self.prog_has_draws(c.src),
            })
    }

    /// A mask node clips the siblings that follow it.
    fn draw_mask(&mut self, mask: &Node, siblings: &[NodeId], i: usize, parent_ts: sk::Transform, ts: sk::Transform, clip: ClipId) {
        let doc = self.doc;
        let vis = self.visible(clip);
        let mut rect = vis;
        if self.cull {
            if let Some(b) = self.index.bounds_of(mask.id) {
                rect = rect.intersect(IRect::outward(xform_bounds(&self.root_ts, b.0, b.1, b.2 - b.0, b.3 - b.1)).grow(1));
            }
        }
        if rect.is_empty() {
            return;
        }
        let rest = &siblings[i + 1..];
        if self.prog_cap_hit(64) || self.over_budget(2, 2 * self.program_cost(rect)) {
            let lclip = self.narrow_clip(clip, rect);
            self.draw_children(rest, parent_ts, lclip);
            return;
        }
        let needs_back = rest.iter().any(|c| doc.get(*c).is_some_and(|c| self.has_bg_blur(c)));
        let mask_cost = self.program_cost(rect);
        let layer = self.new_prog(rect);
        self.progs[layer as usize].needs_backdrop = needs_back;
        let lclip = self.narrow_clip(clip, rect);
        // the mask program below is opened after the siblings: keep its slot
        self.reserving(1, mask_cost, |c| c.with_prog(layer, |c| c.draw_children(rest, parent_ts, lclip)));
        let mask_node = mask_stencil(mask);
        let mprog = self.new_prog(rect);
        let mclip = self.full_clip(rect);
        let deferred = self.geom.deferred_empty;
        let enclosing = (self.stencil_root.replace(mask.id), std::mem::take(&mut self.stencil_children_skipped));
        self.with_prog(mprog, |c| {
            c.draw_node(&mask_node, ts, mclip);
            // A frame mask takes its alpha from its children. If this frame left
            // them out (geometry not built yet, or a frame too small for its
            // children) and nothing else drew into the stencil, it is empty and
            // hides every sibling the mask cuts: stand in the frame's box until
            // the children draw.
            if !c.prog_has_draws(mprog) && (c.geom.deferred_empty != deferred || c.stencil_children_skipped) {
                if let Some(frame_box) = mask_frame_box(mask) {
                    c.draw_node(&frame_box, ts, mclip);
                }
            }
        });
        (self.stencil_root, self.stencil_children_skipped) = enclosing;
        let pclip = self.full_clip(rect);
        self.push_op(Op::Composite(Box::new(CompositeOp {
            src: layer,
            rect,
            opacity: 1.0,
            tint: None,
            aux: Some((mprog, AuxMode::Mul)),
            mode: CompMode::Over,
            clip: pclip,
        })));
    }

    /// Whether `n` or any drawn descendant has a visible background blur.
    fn has_bg_blur(&self, n: &Node) -> bool {
        if !n.visible {
            return false;
        }
        (!matches!(n.kind, NodeKind::Unknown { .. })
            && n.effects
                .iter()
                .any(|e| matches!(e, Effect::BackgroundBlur { visible: true, radius } if *radius > 0.0) || matches!(e, Effect::Glass(g) if g.visible)))
            || (draws_children(n) && n.children.iter().any(|c| self.doc.get(*c).is_some_and(|c| self.has_bg_blur(c))))
    }

    fn bbox(&self, n: &Node, ts: &sk::Transform) -> FBox {
        let m = node_margin(n);
        xform_bounds(ts, -m, -m, n.size.x + 2.0 * m, n.size.y + 2.0 * m)
    }

    /// Pixels a layer for `n` needs: its drawn subtree (from the index) grown
    /// by `pad`, limited to what is visible (grown by `pad` too).
    fn layer_rect(&self, n: &Node, ts: &sk::Transform, clip: ClipId, pad: i32) -> IRect {
        let vis = self.visible(clip).grow(pad);
        let sub = if self.cull {
            match self.index.bounds_of(n.id) {
                Some(b) => IRect::outward(xform_bounds(&self.root_ts, b.0, b.1, b.2 - b.0, b.3 - b.1)).grow(1 + pad),
                None => vis,
            }
        } else {
            let bb = self.bbox(n, ts);
            let own = IRect::outward(bb).grow(1 + pad);
            if draws_children(n) {
                vis
            } else {
                own
            }
        };
        sub.intersect(vis)
    }

    fn draw_node(&mut self, n: &Node, ts: sk::Transform, clip: ClipId) {
        if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
            return;
        }
        if matches!(n.kind, NodeKind::Unknown { .. }) {
            if !n.children.is_empty() {
                self.draw_children(&n.children, ts, clip);
            }
            return;
        }
        if n.kind.is_board_item() {
            // stickies, connectors, shapes with text and stamps draw as their lowered form
            if let Some(body) = board::lower(self.doc, n) {
                self.draw_node(&body, ts, clip);
            }
            return;
        }
        if let Some(f) = &self.opts.force_fallback {
            if f(n) && !self.has_bg_blur(n) {
                self.fallback(n, ts, clip);
                return;
            }
        }
        let clips_content = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
        let bb = self.bbox(n, &ts);
        let has_kids = draws_children(n);
        let vis = self.visible(clip);
        if (!has_kids || clips_content) && !hits(bb, vis) {
            return;
        }
        let bw = bb.2 - bb.0;
        let bh = bb.3 - bb.1;
        if self.zoom_lod() && !has_kids && self.stencil_root != Some(n.id) && ((bw < 1.25 && bh < 1.25) || (bw.max(bh) < 4.0 && bw.min(bh) < 0.35)) {
            return;
        }
        let paints = visible_fills(n).count() + visible_strokes(n).count();
        let has_effects = n.effects.iter().any(|e| {
            matches!(e, Effect::DropShadow { visible: true, .. } | Effect::InnerShadow { visible: true, .. } | Effect::BackgroundBlur { visible: true, .. })
                || matches!(e, Effect::Glass(g) if g.visible)
                || matches!(e, Effect::Noise(n) if n.visible)
        });
        let textured = n.effects.iter().any(|e| matches!(e, Effect::Texture(t) if t.visible && t.radius > 0.0));
        let layer_blur = n
            .effects
            .iter()
            .filter_map(|e| match e {
                Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => Some(*radius),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        let special = special_blend(n.blend);
        let runtime_mul = self.opts.runtime_node_opacities.iter().find(|&&(id, _)| id == n.id).map(|&(_, a)| a.clamp(0.0, 1.0)).unwrap_or(1.0);
        let opacity = (n.opacity * runtime_mul).clamp(0.0, 1.0);
        if opacity <= 0.0 && !special {
            return;
        }
        let need_layer = (special || layer_blur > 0.0 || textured || (opacity < 1.0 && (has_kids || paints > 1 || has_effects))) && !self.prog_cap_hit(64);
        if !need_layer {
            self.draw_body(n, ts, clip, opacity, false);
            return;
        }
        let sigma = layer_blur * ts_scale(&ts) / 2.0;
        let pad = if layer_blur > 0.0 { (3.0 * sigma).ceil() as i32 + 1 } else { 0 };
        let tex_pad = n
            .effects
            .iter()
            .filter_map(|e| match e {
                Effect::Texture(t) if t.visible && t.radius > 0.0 => Some((t.radius * ts_scale(&ts)).ceil() as i32 + 3),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let rect = self.layer_rect(n, &ts, clip, pad.max(tex_pad));
        if rect.is_empty() {
            return;
        }
        let blur_cost = if layer_blur > 0.0 { self.blur_cost(rect) } else { 0 };
        if self.over_budget(1, self.program_cost(rect) + blur_cost) {
            self.draw_body(n, ts, clip, opacity, false);
            return;
        }
        let layer = self.new_prog(rect);
        self.progs[layer as usize].needs_backdrop = self.has_bg_blur(n);
        // draws inside the layer keep the parent's clip (mask and scissor)
        let lclip = self.narrow_clip(clip, rect);
        // the blur targets are accounted only once the body (and its children) drew
        self.reserving(0, blur_cost, |c| c.with_prog(layer, |c| c.draw_body(n, ts, lclip, 1.0, true)));
        if layer_blur > 0.0 {
            self.push_post(layer, Post::Blur { sigma, clamp: false });
        }
        let mode = if special { CompMode::Blend(n.blend) } else { CompMode::Over };
        let pclip = self.full_clip(rect);
        self.push_op(Op::Composite(Box::new(CompositeOp { src: layer, rect, opacity, tint: None, aux: None, mode, clip: pclip })));
    }

    /// CPU-rendered replacement for a whole node (never drops content).
    fn fallback(&mut self, n: &Node, ts: sk::Transform, clip: ClipId) {
        let rect = self.layer_rect(n, &ts, clip, 0);
        let (w, h) = (rect.w() as u32, rect.h() as u32);
        if w == 0 || h == 0 {
            return;
        }
        let Some(mut pm) = sk::Pixmap::new(w, h) else { return };
        let Some(inv) = to_sk(&n.transform).invert() else { return };
        let parent_ts = ts.pre_concat(inv).post_translate(-(rect.x0 as f32), -(rect.y0 as f32));
        render_node_into(self.doc, n.id, parent_ts, self.root_ts.post_translate(-(rect.x0 as f32), -(rect.y0 as f32)), &mut pm);
        let prog = self.new_prog(rect);
        self.progs[prog as usize].pixels = Some(Arc::new(PixelData { w, h, rgba: pm.data().to_vec() }));
        let mode = if special_blend(n.blend) { CompMode::Blend(n.blend) } else { CompMode::Over };
        // the node's clip applies to the whole fallback image
        self.push_op(Op::Composite(Box::new(CompositeOp { src: prog, rect, opacity: 1.0, tint: None, aux: None, mode, clip })));
    }

    // ----- node body --------------------------------------------------------

    /// `isolated`: the node got its own layer program (the texture effects
    /// displace that whole program, so they need it).
    fn draw_body(&mut self, n: &Node, ts: sk::Transform, clip: ClipId, alpha: f32, isolated: bool) {
        self.draw_body_inner(n, ts, clip, alpha);
        if self.prog_cap_hit(64) || !isolated {
            return;
        }
        for e in &n.effects {
            let Effect::Texture(t) = e else { continue };
            if !t.visible || t.radius <= 0.0 {
                continue;
            }
            let scale = ts_scale(&ts);
            let amp = t.radius.max(0.0) * scale;
            let g = self.geom_of(n, &ts);
            let rect = self.progs[self.cur() as usize].rect;
            let clip_rect = t.clip_to_shape.then(|| IRect::outward(self.bbox(n, &ts)).grow(amp.ceil() as i32 + 3).intersect(rect)).filter(|r| !r.is_empty());
            if self.over_budget(usize::from(clip_rect.is_some()), displace_bytes(rect) + clip_rect.map_or(0, |r| self.program_cost(r))) {
                continue;
            }
            // the displacement targets are accounted only once the post is pushed
            let clip_to = clip_rect.map(|r| self.reserving(0, displace_bytes(rect), |c| c.sil_program(n, &g, ts, r, 0.0, 0.0, 0.0, d::SilhouetteMode::Opaque)));
            let cell = (t.size.max(0.1) * scale / self.ss as f32).max(1.0);
            let c = self.cur();
            self.push_post(c, Post::Displace { amp, cell, seed: t.seed, grid: self.grid, clip_to, ss: self.ss });
        }
    }

    fn geom_of(&mut self, n: &Node, ts: &sk::Transform) -> Rc<NodeGeom> {
        let bucket = bucket_of(ts_scale(ts));
        self.geom.get(self.doc, n, bucket, self.rev)
    }

    fn draw_body_inner(&mut self, n: &Node, ts: sk::Transform, clip: ClipId, alpha: f32) {
        let scale = ts_scale(&ts);
        let bb = self.bbox(n, &ts);
        let inner_bb = xform_bounds(&ts, 0.0, 0.0, n.size.x, n.size.y);
        let interactive = self.interactive();
        let zoom_lod = self.zoom_lod();
        let sh = (inner_bb.3 - inner_bb.1).abs();
        let sw = (inner_bb.2 - inner_bb.0).abs();

        if zoom_lod {
            let leaf_lod_px = if scale < 0.35 { 5.0 } else { 2.5 };
            if let NodeKind::Text(t) = &n.kind {
                let font_px = t.style.font_size * scale;
                if font_px < 3.0 || sh < 3.5 || sw < 2.0 {
                    if font_px >= 1.0 && sh >= 1.25 && sw >= 2.0 {
                        if let Some(p) = visible_fills(n).next() {
                            let unit = self.unit.clone();
                            let h_bar = n.size.y.min(t.style.font_size * 0.7).max(1.0);
                            let qts = ts.pre_scale(n.size.x.max(1.0), h_bar);
                            self.fill_with(&unit, qts, n.size, p, alpha * 0.65, clip);
                        }
                    }
                    return;
                }
            } else if !draws_children(n) && sw.max(sh) < leaf_lod_px && n.size.x > 0.0 && n.size.y > 0.0 {
                if let Some(p) = visible_fills(n).next().or_else(|| visible_strokes(n).next()) {
                    let unit = self.unit.clone();
                    let qts = ts.pre_scale(n.size.x, n.size.y);
                    self.fill_with(&unit, qts, n.size, p, alpha, clip);
                }
                return;
            }
        }

        // Exact zero-tessellation unit-quad fast-path for sharp axis-aligned rectangles & frames at all zoom levels
        // (plus sub-pixel corner-radius / stroke approximation when zoomed out in `zoom_lod`).
        if matches!(
            n.kind,
            NodeKind::Rectangle | NodeKind::Section | NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }
        ) && n.fill_geometry.is_none()
            && n.stroke_geometry.is_none()
            && (n.effects.is_empty() || (zoom_lod && scale < 0.25))
            && (!has_visible_strokes(n) || (zoom_lod && scale < 0.25 && n.stroke.weight * scale < 0.75))
            && n.size.x > 0.0
            && n.size.y > 0.0
        {
            let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
            let max_r = radii.iter().copied().fold(0.0f32, f32::max);
            let max_r_px = max_r * scale;
            let rect_r_px = if scale < 0.35 { 2.0 } else { 0.75 };
            let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
            if (max_r <= 0.0 || (zoom_lod && max_r_px <= rect_r_px))
                && (!clips || identity_like(&ts))
                && visible_fills(n).all(|p| matches!(p.kind, d::PaintKind::Solid(_)))
            {
                let unit = self.unit.clone();
                let qts = ts.pre_scale(n.size.x, n.size.y);
                for p in visible_fills(n) {
                    self.fill_with(&unit, qts, n.size, p, alpha, clip);
                }
                if draws_children(n) {
                    if zoom_lod && (sw.max(sh) < 6.0 || sw.min(sh) < 2.0) {
                        self.note_stencil_children_skipped(n);
                        return;
                    }
                    if clips {
                        let (x0, y0) = (ts.tx, ts.ty);
                        let (x1, y1) = (ts.tx + n.size.x * ts.sx, ts.ty + n.size.y * ts.sy);
                        let r = IRect::new(x0.round() as i32, y0.round() as i32, x1.round() as i32, y1.round() as i32);
                        let cc = self.narrow_clip(clip, r);
                        if !self.visible(cc).is_empty() {
                            self.draw_children(&n.paint_children(), ts, cc);
                        }
                    } else {
                        self.draw_children(&n.paint_children(), ts, clip);
                    }
                }
                return;
            }
        }

        let g = self.geom_of(n, &ts);
        let allow_fx = !self.prog_cap_hit(64) && (!interactive || (bb.2 - bb.0).max(bb.3 - bb.1) >= 4.0);

        // 1. drop shadows + background blur (behind the node)
        if allow_fx {
            for e in &n.effects {
                match e {
                    Effect::DropShadow { color, offset, radius, spread, visible: true, show_behind, blend } => {
                        if !interactive || (*radius * scale >= 1.0 || offset.x.hypot(offset.y) * scale >= 1.0 || *spread * scale >= 1.0) {
                            self.drop_shadow(n, &g, ts, clip, bb, scale, color, *offset, *radius, *spread, *show_behind, *blend, alpha);
                        }
                    }
                    Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => {
                        self.background_blur(n, &g, ts, clip, inner_bb, scale, *radius);
                    }
                    Effect::Glass(gl) if gl.visible => self.glass(n, &g, ts, clip, inner_bb, scale, gl),
                    _ => {}
                }
            }
        }

        // 2. fills
        if matches!(n.kind, NodeKind::Text(_)) {
            self.draw_text(n, &g, ts, clip, alpha);
        } else if let Some(sh) = &g.shape {
            if sh.fillable {
                if let Some(mesh) = &g.fill {
                    for p in visible_fills(n) {
                        self.fill_with(mesh, ts, n.size, p, alpha, clip);
                    }
                }
            }
        }

        // 3. inner shadows
        if allow_fx {
            for e in &n.effects {
                if let Effect::InnerShadow { color, offset, radius, spread, visible: true, blend } = e {
                    self.inner_shadow(n, &g, ts, clip, bb, scale, color, *offset, *radius, *spread, *blend, alpha);
                }
            }

            // 3b. noise grain over the fill
            for e in &n.effects {
                if let Effect::Noise(nz) = e {
                    if nz.visible {
                        self.noise(n, &g, ts, clip, bb, scale, nz, alpha);
                    }
                }
            }
        }

        // 4. strokes
        if g.shape.is_some() {
            self.draw_strokes(n, &g, ts, clip, alpha);
        }

        // 5. children
        if draws_children(n) {
            let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
            if zoom_lod && (sw.max(sh) < 6.0 || ((clips || has_visible_fills(n)) && sw.min(sh) < 2.0)) {
                self.note_stencil_children_skipped(n);
                return;
            }
            if clips {
                let Some(sh) = &g.shape else { return };
                let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
                let max_r_px = radii.iter().copied().fold(0.0f32, f32::max) * scale;
                let rect_like = sh.is_rect && identity_like(&ts) && (max_r_px <= 0.0 || (zoom_lod && (max_r_px <= 2.5 || self.prog_cap_hit(32))));
                let child_clip = if rect_like {
                    // axis-aligned rectangle: a scissor is exact up to the edge pixels
                    let (x0, y0) = (ts.tx, ts.ty);
                    let (x1, y1) = (ts.tx + n.size.x * ts.sx, ts.ty + n.size.y * ts.sy);
                    let r = IRect::new(x0.round() as i32, y0.round() as i32, x1.round() as i32, y1.round() as i32);
                    Some(self.narrow_clip(clip, r))
                } else if self.prog_cap_hit(64) {
                    Some(self.narrow_clip(clip, IRect::outward(inner_bb)))
                } else if let Some(mesh) = &g.fill {
                    let roi = self.visible(clip);
                    self.mask_clip(mesh, ts, clip, false, roi, Degrade::Scissor(IRect::outward(inner_bb)))
                } else {
                    None
                };
                let Some(cc) = child_clip else { return };
                if self.visible(cc).is_empty() {
                    return;
                }
                self.draw_children(&n.paint_children(), ts, cc);
            } else {
                self.draw_children(&n.paint_children(), ts, clip);
            }
        }
    }

    // ----- fills and strokes ---------------------------------------------------

    fn fill_with(&mut self, mesh: &Arc<Mesh>, ts: sk::Transform, size: Vec2, p: &d::Paint, alpha: f32, clip: ClipId) {
        let Some(pi) = self.paints.convert(self.doc, p, size, alpha) else { return };
        if special_blend(p.blend) && !self.prog_cap_hit(64) {
            // a paint blended against what is beneath: isolate it and composite
            let bb = xform_bounds(&ts, mesh.bounds.0, mesh.bounds.1, mesh.bounds.2 - mesh.bounds.0, mesh.bounds.3 - mesh.bounds.1);
            let rect = IRect::outward(bb).grow(1).intersect(self.visible(clip));
            if rect.is_empty() {
                return;
            }
            if self.over_budget(1, self.program_cost(rect)) {
                self.draw_mesh(mesh, ts, pi, DrawBlend::Over, clip);
                return;
            }
            let layer = self.new_prog(rect);
            let lclip = self.narrow_clip(clip, rect);
            self.with_prog(layer, |c| c.draw_mesh(mesh, ts, pi, DrawBlend::Over, lclip));
            let pclip = self.full_clip(rect);
            self.push_op(Op::Composite(Box::new(CompositeOp {
                src: layer,
                rect,
                opacity: 1.0,
                tint: None,
                aux: None,
                mode: CompMode::Blend(p.blend),
                clip: pclip,
            })));
            return;
        }
        self.draw_mesh(mesh, ts, pi, DrawBlend::Over, clip);
    }

    fn draw_text(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId, alpha: f32) {
        let has_strokes = has_visible_strokes(n);
        for (i, t) in g.text.iter().enumerate() {
            for p in &t.fills {
                self.fill_with(&t.mesh, ts, n.size, p, alpha, clip);
            }
            if has_strokes {
                if let Some(sm) = g.text_stroke(i, n) {
                    for p in visible_strokes(n) {
                        self.fill_with(&sm, ts, n.size, p, alpha, clip);
                    }
                }
            }
        }
    }

    fn draw_strokes(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId, alpha: f32) {
        if !has_visible_strokes(n) {
            return;
        }
        let Some((mesh, how)) = g.stroke_mesh(n) else { return };
        let zoom_lod = self.zoom_lod();
        let sclip = match how {
            StrokeClip::None => clip,
            StrokeClip::Inside | StrokeClip::Outside if (zoom_lod && n.stroke.weight * ts_scale(&ts) <= 1.25) || self.prog_cap_hit(48) => clip,
            StrokeClip::Inside | StrokeClip::Outside => {
                let Some(shape) = &g.fill else { return };
                let b = mesh.bounds;
                let roi = IRect::outward(xform_bounds(&ts, b.0, b.1, b.2 - b.0, b.3 - b.1)).grow(1);
                let Some(c) = self.mask_clip(shape, ts, clip, how == StrokeClip::Outside, roi, Degrade::Keep) else { return };
                c
            }
        };
        for p in visible_strokes(n) {
            self.fill_with(&mesh, ts, n.size, p, alpha, sclip);
        }
    }

    // ----- silhouettes ----------------------------------------------------------

    /// A program of `rect` holding the node silhouette (black, with the fill
    /// alpha unless `mode` ignores it), shifted by (`dx`, `dy`) device pixels and grown
    /// (`grow` > 0) or eroded (< 0) by `grow` local units.
    #[allow(clippy::too_many_arguments)]
    fn sil_program(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, rect: IRect, dx: f32, dy: f32, grow: f32, mode: d::SilhouetteMode) -> ProgId {
        let prog = self.new_prog(rect);
        let clip = self.full_clip(rect);
        self.with_prog(prog, |c| c.silhouette(n, g, ts, dx, dy, grow, mode, clip));
        prog
    }

    #[allow(clippy::too_many_arguments)]
    fn silhouette(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, dx: f32, dy: f32, grow: f32, mode: d::SilhouetteMode, clip: ClipId) {
        let opaque = mode.ignores_alpha();
        let ts2 = ts.post_translate(dx, dy);
        let has_fills = has_visible_fills(n);
        let has_strokes = has_visible_strokes(n);
        let black = |a: f32| solid_inst([0.0, 0.0, 0.0, a.clamp(0.0, 1.0)]);
        match d::silhouette_source(n, g.shape.as_ref().map(|sh| sh.fillable), mode) {
            d::SilhouetteSource::Text => {
                for t in g.text.iter() {
                    let a = if opaque { 1.0 } else { t.fills.iter().filter(|p| p.visible).map(d::Paint::silhouette_alpha).fold(0.0, f32::max) };
                    self.draw_mesh(&t.mesh, ts2, black(a), DrawBlend::Over, clip);
                }
            }
            d::SilhouetteSource::OwnShape => {
                let Some(sh) = &g.shape else { return };
                if let Some(fill) = &g.fill {
                    if sh.fillable && opaque && !has_fills {
                        self.draw_mesh(fill, ts2, black(1.0), DrawBlend::Over, clip);
                    } else if sh.fillable {
                        for p in visible_fills(n) {
                            let a = if opaque { 1.0 } else { p.silhouette_alpha() };
                            self.draw_mesh(fill, ts2, black(a), DrawBlend::Over, clip);
                        }
                    }
                }
                // stroke footprint outside the fill
                let outset = match n.stroke.align {
                    d::StrokeAlign::Inside if sh.closed && sh.fillable && has_fills => 0.0,
                    d::StrokeAlign::Outside if sh.closed => n.stroke.weight * 2.0,
                    _ => n.stroke.weight,
                };
                if let (Some(_), true) = (&sh.stroke_paths, has_strokes) {
                    if let Some(m) = g.stroke_paths_mesh() {
                        self.draw_mesh(&m, ts2, black(1.0), DrawBlend::Over, clip);
                    }
                } else if has_strokes && outset > 0.0 {
                    if let Some(m) = g.grow_outline(outset) {
                        self.draw_mesh(&m, ts2, black(1.0), DrawBlend::Over, clip);
                    }
                }
                if grow != 0.0 {
                    if let Some(m) = g.grow_outline(grow.abs() * 2.0) {
                        let blend = if grow < 0.0 { DrawBlend::Erase } else { DrawBlend::Over };
                        self.draw_mesh(&m, ts2, black(1.0), blend, clip);
                    }
                }
            }
            d::SilhouetteSource::Children => {
                let Some(cc) = self.silhouette_clip(n, g, ts2, clip) else { return };
                // silhouettes are drawn offset from the page grid: no index culling
                let was = std::mem::replace(&mut self.cull, false);
                self.draw_children(&n.paint_children(), ts2, cc);
                self.cull = was;
            }
            d::SilhouetteSource::ChildrenOpaque => {
                let Some(cc) = self.silhouette_clip(n, g, ts2, clip) else { return };
                let was = std::mem::replace(&mut self.cull, false);
                self.knockout_children(&n.paint_children(), ts2, cc);
                self.cull = was;
            }
            d::SilhouetteSource::Nothing => {}
        }
    }

    /// The clip the children of `n` draw under in a silhouette: the frame's own shape when it
    /// clips content (the same choice as the body makes). `None`: nothing can show.
    fn silhouette_clip(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId) -> Option<ClipId> {
        if !n.kind.frame_props().is_some_and(|f| f.clips_content) {
            return Some(clip);
        }
        let sh = g.shape.as_ref()?;
        let inner_bb = xform_bounds(&ts, 0.0, 0.0, n.size.x, n.size.y);
        let cc = if sh.is_rect && identity_like(&ts) && n.corner_radii.iter().all(|r| *r <= 0.0) {
            let (x0, y0) = (ts.tx, ts.ty);
            let (x1, y1) = (ts.tx + n.size.x * ts.sx, ts.ty + n.size.y * ts.sy);
            self.narrow_clip(clip, IRect::new(x0.round() as i32, y0.round() as i32, x1.round() as i32, y1.round() as i32))
        } else if self.prog_cap_hit(64) {
            self.narrow_clip(clip, IRect::outward(inner_bb))
        } else {
            let mesh = g.fill.as_ref()?;
            let roi = self.visible(clip);
            self.mask_clip(mesh, ts, clip, false, roi, Degrade::Scissor(IRect::outward(inner_bb)))?
        };
        (!self.visible(cc).is_empty()).then_some(cc)
    }

    /// Draws each visible child's own opaque silhouette (a knockout): paint alpha is ignored,
    /// hidden and fully transparent children add nothing.
    fn knockout_children(&mut self, children: &[NodeId], parent_ts: sk::Transform, clip: ClipId) {
        let doc = self.doc;
        for cid in children {
            let Some(child) = board::get(doc, *cid) else { continue };
            let child = resolve_node(doc, &child);
            let child: &Node = &child;
            if !child.visible || child.is_mask || matches!(child.kind, NodeKind::Page { .. }) || child.opacity <= 0.0 {
                continue;
            }
            let ts = parent_ts.pre_concat(to_sk(&child.transform));
            let g = self.geom_of(child, &ts);
            self.silhouette(child, &g, ts, 0.0, 0.0, 0.0, d::SilhouetteMode::Knockout, clip);
        }
    }

    // ----- effects ----------------------------------------------------------------

    /// The shadow of a plain solid rounded rectangle, drawn analytically in one
    /// quad (the GPU counterpart of the CPU renderer's nine-patch fast path).
    #[allow(clippy::too_many_arguments)]
    fn drop_shadow_analytic(
        &mut self,
        n: &Node,
        g: &Rc<NodeGeom>,
        ts: sk::Transform,
        clip: ClipId,
        scale: f32,
        color: &d::Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        show_behind: bool,
        alpha: f32,
    ) -> bool {
        let Some(sh) = &g.shape else { return false };
        if !sh.is_rect || !sh.fillable || n.fill_geometry.is_some() || has_visible_strokes(n) || !identity_like(&ts) || (ts.sx - ts.sy).abs() > 1e-4 * ts.sx {
            return false;
        }
        let mut fills = visible_fills(n);
        let (Some(fill), None) = (fills.next(), fills.next()) else { return false };
        let d::PaintKind::Solid(c) = &fill.kind else { return false };
        let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
        if radii.iter().any(|r| (r - radii[0]).abs() > 1e-4) || (radii[0] > 0.0 && n.corner_smoothing > 0.0) {
            return false;
        }
        let sigma = radius.max(0.0) * scale / 2.0;
        if sigma < 0.3 || (spread != 0.0 && sigma < 8.0) || n.size.x <= 0.0 || n.size.y <= 0.0 {
            return false;
        }
        let rl = radii[0].max(0.0).min(n.size.x.min(n.size.y) / 2.0);
        let sp = spread * scale;
        let (x0, y0) = (ts.tx, ts.ty);
        let (x1, y1) = (ts.tx + n.size.x * ts.sx, ts.ty + n.size.y * ts.sy);
        let (ox, oy) = (offset.x * scale, offset.y * scale);
        let (a, b, cc, dd) = (x0 + ox - sp, y0 + oy - sp, x1 + ox + sp, y1 + oy + sp);
        let r = if rl > 0.0 { (rl * scale + sp).max(0.0) } else { 0.0 }.min((cc - a) / 2.0).min((dd - b) / 2.0);
        let margin = 3.0 * sigma + 2.0;
        let quad = IRect::outward((a - margin, b - margin, cc + margin, dd + margin)).intersect(self.visible(clip));
        if quad.is_empty() {
            return true;
        }
        let fill_alpha = (c.a * fill.opacity).clamp(0.0, 1.0);
        let col_a = (color.a * alpha).clamp(0.0, 1.0) * fill_alpha;
        let inst = Instance {
            t: [0.0, 0.0, sigma, r],
            c0: [color.r.clamp(0.0, 1.0), color.g.clamp(0.0, 1.0), color.b.clamp(0.0, 1.0), col_a],
            c1: [a, b, cc, dd],
            c2: [x0, y0, x1, y1],
            q: [KIND_SHADOW, 0, (!show_behind) as u32, (rl * scale).to_bits()],
            ..Default::default()
        };
        self.draw_quad(quad, PaintInst { inst, ramp: None, image: None }, DrawBlend::Over, clip);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn drop_shadow(
        &mut self,
        n: &Node,
        g: &Rc<NodeGeom>,
        ts: sk::Transform,
        clip: ClipId,
        bbox: FBox,
        scale: f32,
        color: &d::Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        show_behind: bool,
        blend: d::BlendMode,
        alpha: f32,
    ) {
        // the analytic quad draws straight over the target: a blended shadow needs the layer path
        if !special_blend(blend) && self.drop_shadow_analytic(n, g, ts, clip, scale, color, offset, radius, spread, show_behind, alpha) {
            return;
        }
        let sigma = radius.max(0.0) * scale / 2.0;
        let (ox, oy) = (offset.x * scale, offset.y * scale);
        let margin = 3.0 * sigma + ox.abs() + oy.abs() + (spread * scale).max(0.0) + 2.0;
        let reach = margin.ceil() as i32;
        let vis = self.visible(clip);
        let rect = IRect::outward((bbox.0 - margin, bbox.1 - margin, bbox.2 + margin, bbox.3 + margin)).intersect(vis.grow(reach));
        if rect.is_empty() {
            return;
        }
        let silhouettes = if show_behind { 1 } else { 2 };
        let cost = self.program_cost(rect);
        let blur_cost = if sigma > 0.05 { self.blur_cost(rect) } else { 0 };
        if self.over_budget(silhouettes as usize, silhouettes * cost + blur_cost) {
            return;
        }
        // while the first silhouette draws its children, the blur and the
        // second silhouette are still to come
        let s = self.reserving(silhouettes as usize - 1, (silhouettes - 1) * cost + blur_cost, |c| {
            c.sil_program(n, g, ts, rect, ox, oy, spread, d::SilhouetteMode::Alpha)
        });
        if sigma > 0.05 {
            self.push_post(s, Post::Blur { sigma, clamp: false });
        }
        let aux = (!show_behind).then(|| (self.sil_program(n, g, ts, rect, 0.0, 0.0, 0.0, d::SilhouetteMode::Knockout), AuxMode::MulInv));
        let tint = [color.r.clamp(0.0, 1.0), color.g.clamp(0.0, 1.0), color.b.clamp(0.0, 1.0), (color.a * alpha).clamp(0.0, 1.0)];
        self.push_op(Op::Composite(Box::new(CompositeOp { src: s, rect, opacity: 1.0, tint: Some(tint), aux, mode: shadow_mode(blend), clip })));
    }

    #[allow(clippy::too_many_arguments)]
    fn inner_shadow(
        &mut self,
        n: &Node,
        g: &Rc<NodeGeom>,
        ts: sk::Transform,
        clip: ClipId,
        bbox: FBox,
        scale: f32,
        color: &d::Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        blend: d::BlendMode,
        alpha: f32,
    ) {
        let sigma = radius.max(0.0) * scale / 2.0;
        let (ox, oy) = (offset.x * scale, offset.y * scale);
        let margin = 3.0 * sigma + ox.abs() + oy.abs() + (spread * scale).abs() + 2.0;
        let reach = margin.ceil() as i32;
        let vis = self.visible(clip);
        let rect = IRect::outward((bbox.0 - margin, bbox.1 - margin, bbox.2 + margin, bbox.3 + margin)).intersect(vis.grow(reach));
        if rect.is_empty() {
            return;
        }
        let cost = self.program_cost(rect);
        let blur_cost = if sigma > 0.05 { self.blur_cost(rect) } else { 0 };
        if self.over_budget(3, 3 * cost + blur_cost) {
            return;
        }
        // the grain layer, its blur and the shape silhouette follow the hole
        let hole = self.reserving(2, 2 * cost + blur_cost, |c| c.sil_program(n, g, ts, rect, ox, oy, -spread, d::SilhouetteMode::Alpha));
        let p = self.new_prog(rect);
        let inner = self.full_clip(rect);
        self.with_prog(p, |c| {
            c.draw_quad(rect, solid_inst([0.0, 0.0, 0.0, 1.0]), DrawBlend::Over, inner);
            c.push_op(Op::Composite(Box::new(CompositeOp { src: hole, rect, opacity: 1.0, tint: None, aux: None, mode: CompMode::Erase, clip: inner })));
        });
        if sigma > 0.05 {
            self.push_post(p, Post::Blur { sigma, clamp: false });
        }
        let shape = self.sil_program(n, g, ts, rect, 0.0, 0.0, 0.0, d::SilhouetteMode::Alpha);
        let tint = [color.r.clamp(0.0, 1.0), color.g.clamp(0.0, 1.0), color.b.clamp(0.0, 1.0), (color.a * alpha).clamp(0.0, 1.0)];
        self.push_op(Op::Composite(Box::new(CompositeOp {
            src: p,
            rect,
            opacity: 1.0,
            tint: Some(tint),
            aux: Some((shape, AuxMode::Mul)),
            mode: shadow_mode(blend),
            clip,
        })));
    }

    #[allow(clippy::too_many_arguments)]
    fn background_blur(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId, inner_bb: FBox, scale: f32, radius: f32) {
        let sigma = radius.max(0.0) * scale / 2.0;
        let region = IRect::outward(inner_bb).intersect(self.root_rect);
        if region.is_empty() {
            return;
        }
        let pad = (3.0 * sigma).ceil() as i32 + 1;
        let padr = region.grow(pad).intersect(self.root_rect);
        let backdrop = backdrop_bytes(false, padr);
        if self.over_budget(1, self.program_cost(region) + backdrop) {
            return;
        }
        let mask = self.reserving(0, backdrop, |c| c.sil_program(n, g, ts, region, 0.0, 0.0, 0.0, d::SilhouetteMode::Opaque));
        self.push_backdrop(BackdropOp { kind: BackdropKind::Blur { sigma }, region, pad: padr, mask, clip, grid: self.grid });
    }

    #[allow(clippy::too_many_arguments)]
    fn glass(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId, inner_bb: FBox, scale: f32, e: &d::GlassEffect) {
        let region = IRect::outward(inner_bb).intersect(self.root_rect);
        if region.is_empty() {
            return;
        }
        let depth = e.depth.max(0.0) * scale;
        let sigma_f = e.frost.max(0.0) * scale / 2.0;
        let sigma_m = (depth / 3.0).max(0.6);
        let pad = (3.0 * sigma_f.max(sigma_m)).ceil() as i32 + (depth * 1.5).ceil() as i32 + 2;
        let padr = region.grow(pad).intersect(self.root_rect);
        let backdrop = backdrop_bytes(true, padr);
        if self.over_budget(1, self.program_cost(padr) + backdrop) {
            return;
        }
        let mask = self.reserving(0, backdrop, |c| c.sil_program(n, g, ts, padr, 0.0, 0.0, 0.0, d::SilhouetteMode::Opaque));
        let ang = e.light_angle.to_radians();
        let params = GlassParams {
            light: [ang.cos(), ang.sin()],
            intensity: e.light_intensity.clamp(0.0, 1.0),
            refraction: e.refraction.clamp(0.0, 1.0),
            dispersion: e.dispersion.clamp(0.0, 1.0),
            depth,
            sigma_frost: sigma_f,
            sigma_mask: sigma_m,
        };
        self.push_backdrop(BackdropOp { kind: BackdropKind::Glass(params), region, pad: padr, mask, clip, grid: self.grid });
    }

    #[allow(clippy::too_many_arguments)]
    fn noise(&mut self, n: &Node, g: &Rc<NodeGeom>, ts: sk::Transform, clip: ClipId, bbox: FBox, scale: f32, e: &d::NoiseEffect, alpha: f32) {
        let rect = IRect::outward(bbox).intersect(self.visible(clip));
        if rect.is_empty() {
            return;
        }
        let a = (e.opacity * alpha).clamp(0.0, 1.0);
        let rgba = |c: &d::Color| [c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0), (c.a * a).clamp(0.0, 1.0)];
        let (ca, cb, mode) = match &e.mode {
            d::NoiseMode::Mono { color } => (rgba(color), rgba(color), 0),
            d::NoiseMode::Duo { color, secondary } => (rgba(color), rgba(secondary), 1),
            d::NoiseMode::Multi => ([1.0, 1.0, 1.0, a], [1.0, 1.0, 1.0, a], 2),
        };
        let inst = Instance {
            q: [KIND_NOISE, e.seed, mode, 0],
            q2: [self.grid.0, self.grid.1, self.ss as i32, 0],
            c0: ca,
            c1: cb,
            c2: [(e.size.max(0.05) * scale / self.ss as f32).max(1.0), e.density.clamp(0.0, 1.0), 0.0, 0.0],
            t: [0.0, 0.0, 1.0, 0.0],
            ..Default::default()
        };
        let pi = PaintInst { inst, ramp: None, image: None };
        let normal = !special_blend(e.blend);
        // fast path: grain drawn straight over the shape's own geometry
        let direct = normal && matches!(n.kind, NodeKind::Text(_));
        if direct {
            for t in &g.text {
                self.draw_mesh(&t.mesh, ts, PaintInst { inst: pi.inst, ramp: None, image: None }, DrawBlend::Over, clip);
            }
            return;
        }
        if normal && g.shape.as_ref().is_some_and(|s| s.fillable) && !has_visible_strokes(n) {
            if let Some(m) = &g.fill {
                // opaque silhouette == fill geometry when the node has a fillable shape
                self.draw_mesh(m, ts, pi, DrawBlend::Over, clip);
                return;
            }
        }
        // general case: grain layer masked by the opaque silhouette
        if self.over_budget(2, 2 * self.program_cost(rect)) {
            return;
        }
        let layer = self.new_prog(rect);
        let lclip = self.full_clip(rect);
        self.with_prog(layer, |c| c.draw_quad(rect, pi, DrawBlend::Over, lclip));
        let mask = self.sil_program(n, g, ts, rect, 0.0, 0.0, 0.0, d::SilhouetteMode::Opaque);
        let mode = if normal { CompMode::Over } else { CompMode::Blend(e.blend) };
        self.push_op(Op::Composite(Box::new(CompositeOp { src: layer, rect, opacity: 1.0, tint: None, aux: Some((mask, AuxMode::Mul)), mode, clip })));
    }
}

fn solid_inst(c: [f32; 4]) -> PaintInst {
    let inst = Instance { q: [KIND_SOLID, 0, 0, 0], c0: c, t: [0.0, 0.0, 1.0, 0.0], ..Default::default() };
    PaintInst { inst, ramp: None, image: None }
}
