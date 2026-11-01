//! The hand-over format between the scene compiler (CPU, needs the document)
//! and the executor (GPU, owns the device resources).
//!
//! A [`Frame`] is a list of render *programs*. Program 0..n are targets that
//! get rendered: the page itself, opacity/blend layers, clip masks,
//! silhouettes and effect textures. Each program is a list of [`Op`]s, plus
//! post-processing (blur, displacement) applied to its result. Programs refer
//! to each other (a layer is composited into its parent, a mask clips draws),
//! and all coordinates are root device pixels (page pixels at the frame's
//! viewport), so every target is pixel-aligned with every other.

use crate::mesh::Mesh;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;

pub type ProgId = u32;
pub type ClipId = u32;

/// Half-open integer rectangle in root device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    pub const EMPTY: IRect = IRect { x0: 0, y0: 0, x1: 0, y1: 0 };

    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// Smallest integer rectangle containing the float box.
    pub fn outward(b: (f32, f32, f32, f32)) -> Self {
        let c = |v: f32| v.clamp(-1.0e8, 1.0e8);
        Self { x0: c(b.0).floor() as i32, y0: c(b.1).floor() as i32, x1: c(b.2).ceil() as i32, y1: c(b.3).ceil() as i32 }
    }

    pub fn w(&self) -> i32 {
        (self.x1 - self.x0).max(0)
    }

    pub fn h(&self) -> i32 {
        (self.y1 - self.y0).max(0)
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    pub fn area(&self) -> i64 {
        self.w() as i64 * self.h() as i64
    }

    pub fn intersect(self, o: IRect) -> IRect {
        let r = IRect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) };
        if r.is_empty() {
            IRect::EMPTY
        } else {
            r
        }
    }

    pub fn union(self, o: IRect) -> IRect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        IRect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn grow(self, n: i32) -> IRect {
        IRect { x0: self.x0 - n, y0: self.y0 - n, x1: self.x1 + n, y1: self.y1 + n }
    }

    pub fn contains_rect(&self, o: &IRect) -> bool {
        o.x0 >= self.x0 && o.y0 >= self.y0 && o.x1 <= self.x1 && o.y1 <= self.y1
    }
}

// ---------------------------------------------------------------- draws

pub const KIND_SOLID: u32 = 0;
pub const KIND_LINEAR: u32 = 1;
pub const KIND_RADIAL: u32 = 2;
pub const KIND_ANGULAR: u32 = 3;
pub const KIND_DIAMOND: u32 = 4;
pub const KIND_IMAGE: u32 = 5;
pub const KIND_NOISE: u32 = 6;
/// Analytic blurred rounded-rectangle shadow.
pub const KIND_SHADOW: u32 = 7;

/// Image sampling modes (`Instance::q[2]` for image draws).
pub const IMG_PAD: u32 = 0;
/// Transparent outside the image rectangle (letterbox).
pub const IMG_FIT: u32 = 1;
pub const IMG_TILE: u32 = 2;

/// Per-draw data, passed as a per-instance vertex buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Instance {
    /// Local -> root pixels: x' = m0*x + m2*y + t0, y' = m1*x + m3*y + t1.
    pub m: [f32; 4],
    /// `t0, t1` translation, `t2` alpha multiplier, `t3` unused.
    pub t: [f32; 4],
    /// Solid: straight rgba. Noise: color A.
    pub c0: [f32; 4],
    /// Gradient/image: local -> unit space matrix (a, b, c, d). Noise: color B.
    pub c1: [f32; 4],
    /// Gradient/image: that matrix's translation in `xy`. Noise: cell size, density.
    pub c2: [f32; 4],
    /// kind, seed, mode/extra, ramp row (filled in by the executor).
    pub q: [u32; 4],
    /// Noise grid origin.
    pub q2: [i32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawBlend {
    /// Premultiplied source-over.
    Over,
    /// `dst *= 1 - src.a` (DestinationOut).
    Erase,
}

pub const RAMP_W: usize = 512;

/// A gradient colour ramp (premultiplied RGBA8, `RAMP_W` texels).
#[derive(Debug)]
pub struct RampData {
    pub key: u64,
    pub rgba: Vec<u8>,
}

/// A decoded image (premultiplied RGBA8).
#[derive(Debug)]
pub struct ImageData {
    pub id: u64,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

pub struct DrawOp {
    pub mesh: Arc<Mesh>,
    pub inst: Instance,
    pub blend: DrawBlend,
    pub clip: ClipId,
    pub image: Option<Arc<ImageData>>,
    pub ramp: Option<Arc<RampData>>,
}

// ---------------------------------------------------------------- composites

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxMode {
    /// Multiply the source by the aux alpha.
    Mul,
    /// Multiply the source by `1 - aux alpha`.
    MulInv,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompMode {
    /// Source-over.
    Over,
    /// DestinationOut: the source alpha knocks the destination out.
    Erase,
    /// Separable/non-separable blend mode against what the destination holds
    /// (reads a snapshot of the destination).
    Blend(designos_doc::BlendMode),
}

pub struct CompositeOp {
    pub src: ProgId,
    /// Destination area covered (root pixels); normally the source's rect.
    pub rect: IRect,
    pub opacity: f32,
    /// Replace the colour by this straight colour, keeping `src.a * tint.a`.
    pub tint: Option<[f32; 4]>,
    pub aux: Option<(ProgId, AuxMode)>,
    pub mode: CompMode,
    pub clip: ClipId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassParams {
    /// Unit vector towards the light, in pixels space.
    pub light: [f32; 2],
    pub intensity: f32,
    pub refraction: f32,
    pub dispersion: f32,
    /// Bevel depth in device px.
    pub depth: f32,
    pub sigma_frost: f32,
    pub sigma_mask: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackdropKind {
    Blur { sigma: f32 },
    Glass(GlassParams),
}

/// Replaces the pixels under a silhouette by a processed copy of what lies
/// beneath (background blur, glass).
pub struct BackdropOp {
    pub kind: BackdropKind,
    /// Pixels replaced (clipped to the shape by `mask`).
    pub region: IRect,
    /// Pixels read (region plus filter reach).
    pub pad: IRect,
    /// Opaque silhouette of the node, covering `pad`.
    pub mask: ProgId,
    pub clip: ClipId,
    /// Absolute-pixel grid origin (anchors blur grids; unused by the GPU blur).
    pub grid: (i32, i32),
}

pub enum Op {
    Draw(Box<DrawOp>),
    Composite(Box<CompositeOp>),
    Backdrop(Box<BackdropOp>),
}

// ---------------------------------------------------------------- post processing

pub enum Post {
    /// Gaussian blur of the whole result. `clamp`: edge pixels extend outwards
    /// instead of fading to transparent.
    Blur { sigma: f32, clamp: bool },
    /// Texture effect: displace pixels with seeded value noise.
    Displace { amp: f32, cell: f32, seed: u32, grid: (i32, i32), clip_to: Option<ProgId>, ss: u32 },
}

/// Pixels rendered by the CPU renderer (fallback layers).
#[derive(Debug)]
pub struct PixelData {
    pub w: u32,
    pub h: u32,
    /// Premultiplied RGBA8.
    pub rgba: Vec<u8>,
}

pub struct Program {
    pub rect: IRect,
    pub clear: [f32; 4],
    pub ops: Vec<Op>,
    pub post: Vec<Post>,
    /// Reads what lies beneath it (background blur / glass inside): must be run
    /// in order, after everything painted before it in its parent.
    pub needs_backdrop: bool,
    /// Set for fallback layers: the result is these pixels, `ops` is empty.
    pub pixels: Option<Arc<PixelData>>,
}

impl Program {
    pub fn new(rect: IRect) -> Self {
        Self { rect, clear: [0.0; 4], ops: Vec::new(), post: Vec::new(), needs_backdrop: false, pixels: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClipDesc {
    pub scissor: IRect,
    pub mask: Option<ProgId>,
}

pub struct Frame {
    /// Render target size in pixels (`out_size * ss`).
    pub size: (u32, u32),
    /// Size of the final image.
    pub out_size: (u32, u32),
    /// Supersampling factor: the page is rendered `ss` times larger and box-filtered down.
    pub ss: u32,
    pub root: ProgId,
    pub programs: Vec<Program>,
    pub clips: Vec<ClipDesc>,
}

/// Counters describing a compiled frame (diagnostics and tests).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    pub programs: usize,
    pub draws: usize,
    pub composites: usize,
    pub backdrops: usize,
    pub fallback_layers: usize,
    pub triangles: usize,
}

impl Frame {
    /// The programs that must have run before ops `ops` of program `p` (and its
    /// post-processing) can: composited and aux sources, clip masks, backdrop
    /// masks and displacement masks. Not sorted, may repeat.
    pub(crate) fn deps_of(&self, p: ProgId, ops: std::ops::Range<usize>, out: &mut Vec<ProgId>) {
        let prog = &self.programs[p as usize];
        let clip_dep = |c: ClipId, out: &mut Vec<ProgId>| {
            if let Some(m) = self.clips[c as usize].mask {
                out.push(m);
            }
        };
        for op in &prog.ops[ops] {
            match op {
                Op::Draw(x) => clip_dep(x.clip, out),
                Op::Composite(c) => {
                    out.push(c.src);
                    if let Some((a, _)) = c.aux {
                        out.push(a);
                    }
                    clip_dep(c.clip, out);
                }
                Op::Backdrop(b) => {
                    out.push(b.mask);
                    clip_dep(b.clip, out);
                }
            }
        }
        for post in &prog.post {
            if let Post::Displace { clip_to: Some(m), .. } = post {
                out.push(*m);
            }
        }
    }

    pub fn stats(&self) -> FrameStats {
        let mut s = FrameStats { programs: self.programs.len(), ..Default::default() };
        for p in &self.programs {
            if p.pixels.is_some() {
                s.fallback_layers += 1;
            }
            for op in &p.ops {
                match op {
                    Op::Draw(d) => {
                        s.draws += 1;
                        s.triangles += d.mesh.idx.len() / 3;
                    }
                    Op::Composite(_) => s.composites += 1,
                    Op::Backdrop(_) => s.backdrops += 1,
                }
            }
        }
        s
    }
}
