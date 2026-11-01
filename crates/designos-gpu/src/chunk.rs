//! Cutting a compiled frame into pieces the executor renders one per UI frame.
//!
//! The idle refine of a large page is thousands of offscreen programs and tens of
//! thousands of draws. Recording that into one submission takes the UI thread
//! half a second or more (about 75 microseconds a program, plus the upload of
//! every mesh the frame holds). [`Frame::plan_chunks`] cuts the root program's
//! op list into ranges whose estimated cost stays under a budget;
//! [`Renderer::render_chunk`](crate::Renderer::render_chunk) renders one range per
//! frame into a page texture of its own, so the interactive frame stays on
//! screen until the last range is in.
//!
//! A range carries the programs its ops need (layers, masks, silhouettes) with
//! it: they are rendered in the chunk that first reaches them, and again in a
//! later chunk that needs them (a clip mask shared across a range boundary is
//! drawn twice, which costs a pass and changes nothing in the pixels).
//!
//! The cut is only as fine as the root's own ops: a layer subtree that is one
//! op of the root cannot be split.

use crate::frame::*;
use std::ops::Range;

/// Estimated microseconds to render one offscreen program: its pass, its pooled
/// targets and the command buffers wgpu-core records for it.
pub const PROGRAM_COST_US: f32 = 90.0;
/// Estimated microseconds to record one draw call.
pub const DRAW_COST_US: f32 = 1.0;
/// Estimated microseconds per byte of mesh data (the arena uploads every mesh the
/// first time a frame uses it).
pub const MESH_BYTE_COST_US: f32 = 0.0015;
/// Estimated microseconds to composite one layer into its parent.
pub const COMPOSITE_COST_US: f32 = 15.0;
/// Estimated microseconds for a backdrop op (copy of the backdrop, blur, replace).
pub const BACKDROP_COST_US: f32 = 600.0;
/// Estimated microseconds for a blur or displacement of a whole program result.
pub const POST_COST_US: f32 = 400.0;
/// Estimated microseconds per byte of CPU-rendered pixels uploaded.
pub const PIXEL_BYTE_COST_US: f32 = 0.001;

fn op_cost(op: &Op) -> f32 {
    match op {
        Op::Draw(d) => DRAW_COST_US + d.mesh.bytes() as f32 * MESH_BYTE_COST_US,
        Op::Composite(_) => COMPOSITE_COST_US,
        Op::Backdrop(_) => BACKDROP_COST_US,
    }
}

struct Coster<'f> {
    frame: &'f Frame,
    /// Cost of each program on its own (its pass, ops and post-processing), once known.
    own: Vec<Option<f32>>,
    /// Chunk number in which a program was last counted.
    counted: Vec<u32>,
    /// Op number in which a program was last visited (the dry run of one op).
    visited: Vec<u32>,
    /// Scratch for dependency lists.
    deps: Vec<ProgId>,
}

impl<'f> Coster<'f> {
    fn new(frame: &'f Frame) -> Self {
        let n = frame.programs.len();
        Self { frame, own: vec![None; n], counted: vec![0; n], visited: vec![0; n], deps: Vec::new() }
    }

    fn own_cost(&mut self, p: usize) -> f32 {
        if let Some(c) = self.own[p] {
            return c;
        }
        let prog = &self.frame.programs[p];
        let mut c = PROGRAM_COST_US;
        if let Some(px) = &prog.pixels {
            c += px.rgba.len() as f32 * PIXEL_BYTE_COST_US;
        }
        c += prog.ops.iter().map(op_cost).sum::<f32>();
        c += prog.post.len() as f32 * POST_COST_US;
        self.own[p] = Some(c);
        c
    }

    /// Cost of root op `k` including the programs it needs that chunk `chunk` has
    /// not rendered yet; those programs are appended to `fresh`.
    fn root_op_cost(&mut self, k: usize, chunk: u32, op_no: u32, fresh: &mut Vec<usize>) -> f32 {
        let root = self.frame.root;
        let mut cost = op_cost(&self.frame.programs[root as usize].ops[k]);
        let mut stack: Vec<usize> = Vec::new();
        self.deps.clear();
        self.frame.deps_of(root, k..k + 1, &mut self.deps);
        stack.extend(self.deps.iter().map(|d| *d as usize));
        while let Some(p) = stack.pop() {
            if self.counted[p] == chunk || self.visited[p] == op_no {
                continue;
            }
            self.visited[p] = op_no;
            cost += self.own_cost(p);
            fresh.push(p);
            self.deps.clear();
            self.frame.deps_of(p as ProgId, 0..self.frame.programs[p].ops.len(), &mut self.deps);
            stack.extend(self.deps.iter().map(|d| *d as usize));
        }
        cost
    }
}

impl Frame {
    /// Cuts the root program's ops into ranges that each cost an estimated `budget_us`
    /// microseconds or less to render (a single op that costs more is a range of its own).
    /// `None` when the frame cannot be rendered in pieces: it is supersampled, or its root
    /// has post-processing or is a pixel layer.
    pub fn plan_chunks(&self, budget_us: f32) -> Option<Vec<Range<usize>>> {
        let root = &self.programs[self.root as usize];
        if self.ss != 1 || !root.post.is_empty() || root.pixels.is_some() {
            return None;
        }
        let n = root.ops.len();
        let mut coster = Coster::new(self);
        let mut ranges = Vec::new();
        let (mut start, mut acc, mut chunk) = (0usize, 0.0f32, 1u32);
        let mut op_no = 0u32;
        let mut fresh = Vec::new();
        for k in 0..n {
            op_no += 1;
            fresh.clear();
            let mut c = coster.root_op_cost(k, chunk, op_no, &mut fresh);
            if acc > 0.0 && acc + c > budget_us {
                ranges.push(start..k);
                start = k;
                acc = 0.0;
                chunk += 1;
                op_no += 1;
                fresh.clear();
                c = coster.root_op_cost(k, chunk, op_no, &mut fresh);
            }
            for p in &fresh {
                coster.counted[*p] = chunk;
            }
            acc += c;
        }
        ranges.push(start..n);
        Some(ranges)
    }

    /// Estimated microseconds to render ops `ops` of the root program as one chunk.
    pub fn estimate_chunk_us(&self, ops: Range<usize>) -> f32 {
        let mut coster = Coster::new(self);
        let mut total = 0.0;
        let mut fresh = Vec::new();
        for (i, k) in ops.enumerate() {
            fresh.clear();
            total += coster.root_op_cost(k, 1, i as u32 + 1, &mut fresh);
            for p in &fresh {
                coster.counted[*p] = 1;
            }
        }
        total
    }
}
