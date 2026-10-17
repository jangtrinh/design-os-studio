//! Frame executor: turns a [`Frame`] into wgpu passes.
//!
//! Programs run children-first: small ones that do not read the backdrop are
//! hoisted so the parent can draw in a single pass; everything else runs in
//! order (the parent's pass is split around it). Multisampled render targets
//! are pooled; results are single-sample textures.

use crate::frame::*;
use crate::pipelines::*;
use crate::resources::*;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

/// Layers up to this many pixels run before their parent's pass starts.
const HOIST_AREA: i64 = 384 * 384;
/// Blurs are done at reduced resolution above this sigma.
const MAX_DIRECT_SIGMA: f32 = 16.0;

/// Counters of the last executed frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecStats {
    pub passes: u32,
    pub draw_calls: u32,
    pub composites: u32,
    pub blur_passes: u32,
    pub programs_run: u32,
    pub meshes_uploaded: u32,
    pub pool_bytes: u64,
}

/// The rendered page.
#[derive(Clone)]
pub struct Output {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

/// All GPU state of the renderer.
pub struct Renderer {
    dev: wgpu::Device,
    queue: wgpu::Queue,
    layouts: Layouts,
    pipes: Pipelines,
    samp_clamp: wgpu::Sampler,
    samp_repeat: wgpu::Sampler,
    ramp_tex: wgpu::Texture,
    ramp_view: wgpu::TextureView,
    ramp_rows: HashMap<u64, u32>,
    ramp_next: u32,
    dummy: wgpu::TextureView,
    noclip_bg: wgpu::BindGroup,
    arena: Arena,
    images: HashMap<u64, ImageTex>,
    ms_pool: Pool,
    plain_pool: Pool,
    ring: Ring,
    inst_buf: Option<wgpu::Buffer>,
    inst_cap: u64,
    output: Option<Output>,
    /// Multisampled twin of `output` (same exact size).
    output_ms: Option<(wgpu::Texture, wgpu::TextureView)>,
    output_ms_size: (u32, u32),
    frame_no: u64,
    max_dim: u32,
    pub stats: ExecStats,
}

const RAMP_ROWS: u32 = 1024;

fn color_of(c: [f32; 4]) -> wgpu::Color {
    wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: c[3] as f64 }
}

impl Renderer {
    /// Creates the renderer on an existing device (eframe's, or a headless one).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, crate::GpuError> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layouts = Layouts::new(device);
        let pipes = Pipelines::new(device, &layouts);
        let samp_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let samp_repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let ramp_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ramps"),
            size: wgpu::Extent3d { width: RAMP_W as u32, height: RAMP_ROWS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let ramp_view = ramp_tex.create_view(&Default::default());
        let dummy_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dummy"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &dummy_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let dummy = dummy_tex.create_view(&Default::default());
        let noclip_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("noclip"),
            size: std::mem::size_of::<ClipParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&noclip_buf, 0, bytemuck::bytes_of(&ClipParams { origin: [0.0; 2], size: [0.0; 2], enabled: 0.0, pad: [0.0; 3] }));
        let noclip_bg = uniform_bg(
            device,
            &layouts.clip,
            &(noclip_buf, 0, std::mem::size_of::<ClipParams>() as u64),
            &[wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&dummy) }],
        );
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(crate::GpuError::Unsupported(e.to_string()));
        }
        Ok(Self {
            dev: device.clone(),
            queue: queue.clone(),
            layouts,
            pipes,
            samp_clamp,
            samp_repeat,
            ramp_tex,
            ramp_view,
            ramp_rows: HashMap::new(),
            ramp_next: 0,
            dummy,
            noclip_bg,
            arena: Arena::default(),
            images: HashMap::new(),
            ms_pool: Pool::new(true),
            plain_pool: Pool::new(false),
            ring: Ring::default(),
            inst_buf: None,
            inst_cap: 0,
            output: None,
            output_ms: None,
            output_ms_size: (0, 0),
            frame_no: 0,
            max_dim: device.limits().max_texture_dimension_2d,
            stats: ExecStats::default(),
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.dev
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn max_texture_dimension(&self) -> u32 {
        self.max_dim
    }

    fn ramp_row(&mut self, r: &RampData) -> u32 {
        if let Some(row) = self.ramp_rows.get(&r.key) {
            return *row;
        }
        if self.ramp_next >= RAMP_ROWS {
            self.ramp_rows.clear();
            self.ramp_next = 0;
        }
        let row = self.ramp_next;
        self.ramp_next += 1;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.ramp_tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: row, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &r.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(RAMP_W as u32 * 4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: RAMP_W as u32, height: 1, depth_or_array_layers: 1 },
        );
        self.ramp_rows.insert(r.key, row);
        row
    }

    fn ensure_output(&mut self, size: (u32, u32), render: (u32, u32)) -> Output {
        if let Some(o) = &self.output {
            if o.size == size && self.output_ms_size == render {
                return o.clone();
            }
        }
        self.output_ms_size = render;
        let texture = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas"),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let ms = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas ms"),
            size: wgpu::Extent3d { width: render.0.max(1), height: render.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: MSAA,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let ms_view = ms.create_view(&Default::default());
        self.output_ms = Some((ms, ms_view));
        let o = Output { texture, view, size };
        self.output = Some(o.clone());
        o
    }

    /// Frees all free pooled offscreen targets unused since `idle_frames` ago.
    pub fn trim_pools(&mut self, idle_frames: u64) {
        self.ms_pool.trim(self.frame_no, idle_frames);
        self.plain_pool.trim(self.frame_no, idle_frames);
        self.stats.pool_bytes = self.ms_pool.bytes() + self.plain_pool.bytes();
    }

    /// Records the passes for `frame` into `enc` and returns the page texture.
    pub fn render(&mut self, frame: &Frame, enc: &mut wgpu::CommandEncoder) -> Output {
        self.frame_no += 1;
        self.stats = ExecStats::default();
        self.ring.reset();
        let output = self.ensure_output(frame.out_size, frame.size);
        let mut run = FrameRun::new(self, frame, enc, output.clone());
        run.run_program(enc, frame.root);
        if frame.ss > 1 {
            run.downsample_root(enc);
        }
        let st = run.stats;
        self.stats = st;
        self.ms_pool.trim(self.frame_no, 0);
        self.plain_pool.trim(self.frame_no, 0);
        self.stats.pool_bytes = self.ms_pool.bytes() + self.plain_pool.bytes();
        self.stats.meshes_uploaded = std::mem::take(&mut self.arena.uploaded);
        let fno = self.frame_no;
        self.images.retain(|_, i| fno.saturating_sub(i.last_used) < 900);
        output
    }

    /// Renders `frame` with an encoder of its own and submits it right away.
    pub fn render_submitted(&mut self, frame: &Frame) -> Output {
        let mut enc = self.dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("designos-gpu frame") });
        let out = self.render(frame, &mut enc);
        self.queue.submit([enc.finish()]);
        out
    }

    /// Renders `frame` and reads the result back as premultiplied RGBA8 rows.
    pub fn render_to_rgba(&mut self, frame: &Frame) -> Vec<u8> {
        let mut enc = self.dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("designos-gpu frame") });
        let out = self.render(frame, &mut enc);
        let (w, h) = out.size;
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &out.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.dev.poll(wgpu::PollType::wait_indefinitely());
        let _ = rx.recv();
        let data = slice.get_mapped_range();
        let mut out_px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            out_px.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]);
        }
        drop(data);
        buf.unmap();
        out_px
    }

    /// Blocks until all submitted GPU work finished (benchmarks).
    pub fn wait_idle(&self) {
        let _ = self.dev.poll(wgpu::PollType::wait_indefinitely());
    }
}

// ---------------------------------------------------------------- one frame

struct Res {
    /// Pool lease of the result texture (None for the root, which uses the output).
    lease: Option<Lease>,
    view: wgpu::TextureView,
    rect: IRect,
}

struct Live {
    ms: Lease,
    resolve_view: wgpu::TextureView,
    resolve: Option<Lease>,
    resolve_tex: wgpu::Texture,
    started: bool,
}

struct FrameRun<'a> {
    r: &'a mut Renderer,
    frame: &'a Frame,
    done: Vec<bool>,
    res: Vec<Option<Res>>,
    live: HashMap<ProgId, Live>,
    stack: Vec<ProgId>,
    deps: Vec<Vec<ProgId>>,
    consumers: Vec<u32>,
    inst_base: Vec<u32>,
    clip_bgs: HashMap<ProgId, wgpu::BindGroup>,
    output: Output,
    stats: ExecStats,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum G2 {
    None,
    Image(u64),
    Comp,
}

struct CompSpec<'s> {
    params: CParams,
    src: &'s wgpu::TextureView,
    aux: Option<&'s wgpu::TextureView>,
    snap: Option<&'s wgpu::TextureView>,
    kind: CompKind,
    scissor: IRect,
    clip_mask: Option<ProgId>,
}

fn blend_id(b: designos_doc::BlendMode) -> u32 {
    use designos_doc::BlendMode as B;
    match b {
        B::Normal => 0,
        B::PassThrough => 1,
        B::Darken => 2,
        B::Multiply => 3,
        B::ColorBurn => 4,
        B::LinearBurn => 5,
        B::Lighten => 6,
        B::Screen => 7,
        B::ColorDodge => 8,
        B::LinearDodge => 9,
        B::Overlay => 10,
        B::SoftLight => 11,
        B::HardLight => 12,
        B::Difference => 13,
        B::Exclusion => 14,
        B::Hue => 15,
        B::Saturation => 16,
        B::Color => 17,
        B::Luminosity => 18,
    }
}

fn f4(r: IRect) -> [f32; 4] {
    [r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32]
}

impl<'a> FrameRun<'a> {
    fn new(r: &'a mut Renderer, frame: &'a Frame, enc: &mut wgpu::CommandEncoder, output: Output) -> Self {
        let n = frame.programs.len();
        // ---- dependencies
        let mut deps: Vec<Vec<ProgId>> = vec![Vec::new(); n];
        let mut consumers = vec![0u32; n];
        for (p, prog) in frame.programs.iter().enumerate() {
            let mut d: Vec<ProgId> = Vec::new();
            let clip_dep = |c: ClipId, d: &mut Vec<ProgId>| {
                if let Some(m) = frame.clips[c as usize].mask {
                    d.push(m);
                }
            };
            for op in &prog.ops {
                match op {
                    Op::Draw(x) => clip_dep(x.clip, &mut d),
                    Op::Composite(c) => {
                        d.push(c.src);
                        if let Some((a, _)) = c.aux {
                            d.push(a);
                        }
                        clip_dep(c.clip, &mut d);
                    }
                    Op::Backdrop(b) => {
                        d.push(b.mask);
                        clip_dep(b.clip, &mut d);
                    }
                }
            }
            for post in &prog.post {
                if let Post::Displace { clip_to: Some(m), .. } = post {
                    d.push(*m);
                }
            }
            d.sort_unstable();
            d.dedup();
            for x in &d {
                consumers[*x as usize] += 1;
            }
            deps[p] = d;
        }

        // ---- ramps: make room for the gradients of this frame in the ramp texture
        {
            let mut fresh = std::collections::HashSet::new();
            for prog in &frame.programs {
                for op in &prog.ops {
                    if let Op::Draw(d) = op {
                        if let Some(rp) = &d.ramp {
                            if !r.ramp_rows.contains_key(&rp.key) {
                                fresh.insert(rp.key);
                            }
                        }
                    }
                }
            }
            if r.ramp_next as usize + fresh.len() > RAMP_ROWS as usize {
                r.ramp_rows.clear();
                r.ramp_next = 0;
            }
        }
        // ---- instances, meshes, images
        let mut instances: Vec<Instance> = Vec::new();
        let mut inst_base = Vec::with_capacity(n);
        let mut meshes: Vec<Arc<crate::mesh::Mesh>> = Vec::new();
        let mut images: Vec<Arc<ImageData>> = Vec::new();
        for prog in &frame.programs {
            inst_base.push(instances.len() as u32);
            for op in &prog.ops {
                match op {
                    Op::Draw(d) => {
                        let mut inst = d.inst;
                        if let Some(rp) = &d.ramp {
                            inst.q[3] = r.ramp_row(rp).min(RAMP_ROWS - 1);
                        }
                        instances.push(inst);
                        meshes.push(d.mesh.clone());
                        if let Some(img) = &d.image {
                            images.push(img.clone());
                        }
                    }
                    _ => instances.push(Instance::default()),
                }
            }
        }
        let (dev, queue) = (r.dev.clone(), r.queue.clone());
        r.arena.ensure(&dev, &queue, &meshes);
        {
            let mut seen = std::collections::HashSet::new();
            for img in images {
                if !seen.insert(img.id) {
                    continue;
                }
                if let Some(t) = r.images.get_mut(&img.id) {
                    t.last_used = r.frame_no;
                    continue;
                }
                let mut up = ImageUpload {
                    dev: &dev,
                    queue: &queue,
                    enc,
                    blit: &r.pipes.blit,
                    blit_layout: &r.layouts.blit,
                    image_layout: &r.layouts.image,
                    sampler: &r.samp_clamp,
                    ring: &mut r.ring,
                    max_dim: r.max_dim,
                    frame: r.frame_no,
                };
                let t = up.upload(&img);
                r.images.insert(img.id, t);
            }
        }
        // instance buffer
        let bytes: &[u8] = bytemuck::cast_slice(&instances);
        let need = (bytes.len() as u64).max(256);
        if r.inst_buf.is_none() || r.inst_cap < need {
            r.inst_cap = need.next_power_of_two();
            r.inst_buf = Some(dev.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: r.inst_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if !bytes.is_empty() {
            queue.write_buffer(r.inst_buf.as_ref().unwrap(), 0, bytes);
        }
        Self {
            r,
            frame,
            done: vec![false; n],
            res: (0..n).map(|_| None).collect(),
            live: HashMap::new(),
            stack: Vec::new(),
            deps,
            consumers,
            inst_base,
            clip_bgs: HashMap::new(),
            output,
            stats: ExecStats::default(),
        }
    }

    /// Box-filters the supersampled root into the output texture.
    fn downsample_root(&mut self, enc: &mut wgpu::CommandEncoder) {
        let root = self.frame.root as usize;
        let Some(src) = self.res[root].as_ref().and_then(|r| r.lease.clone()) else { return };
        let ub = self.r.ring.alloc(
            &self.r.dev,
            &self.r.queue,
            &BlitParams {
                src_extent: [self.frame.size.0 as f32, self.frame.size.1 as f32],
                dst_extent: [self.frame.out_size.0 as f32, self.frame.out_size.1 as f32],
                src_tex: [src.w as f32, src.h as f32],
                pad: [0.0; 2],
            },
        );
        let bg = uniform_bg(
            &self.r.dev,
            &self.r.layouts.blit,
            &ub,
            &[
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.r.samp_clamp) },
            ],
        );
        let pipe = self.r.pipes.blit.clone();
        let out = self.output.view.clone();
        self.full_pass(enc, &out, self.frame.out_size, &pipe, &bg);
        self.r.plain_pool.release(&src);
    }

    fn hoistable(&self, p: ProgId) -> bool {
        let prog = &self.frame.programs[p as usize];
        !prog.needs_backdrop && prog.rect.area() <= HOIST_AREA
    }

    fn ensure_done(&mut self, enc: &mut wgpu::CommandEncoder, p: ProgId) {
        if !self.done[p as usize] {
            self.run_program(enc, p);
        }
    }

    // ----- program lifecycle ---------------------------------------------------

    fn run_program(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId) {
        if self.done[id as usize] {
            return;
        }
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        self.stats.programs_run += 1;
        if prog.rect.is_empty() {
            self.done[id as usize] = true;
            return;
        }
        if let Some(px) = &prog.pixels {
            self.upload_pixels(id, prog, px);
            self.done[id as usize] = true;
            self.finish_deps(id);
            return;
        }
        // hoist small independent children so this program can draw in one pass
        for d in self.deps[id as usize].clone().into_iter().take(24) {
            if !self.done[d as usize] && self.hoistable(d) {
                self.run_program(enc, d);
            }
        }
        let (w, h) = (prog.rect.w() as u32, prog.rect.h() as u32);
        let frame_no = self.r.frame_no;
        let direct_root = id == frame.root && frame.ss == 1;
        let ms = if direct_root {
            let (tex, view) = self.r.output_ms.clone().expect("root target");
            Lease { idx: usize::MAX, tex, view, w, h }
        } else {
            self.r.ms_pool.acquire(&self.r.dev, w, h, frame_no)
        };
        let (resolve, resolve_view, resolve_tex) = if direct_root {
            (None, self.output.view.clone(), self.output.texture.clone())
        } else {
            let l = self.r.plain_pool.acquire(&self.r.dev, w, h, frame_no);
            (Some(l.clone()), l.view.clone(), l.tex.clone())
        };
        self.live.insert(id, Live { ms, resolve_view, resolve, resolve_tex, started: false });
        self.stack.push(id);

        let mut i = 0;
        let nops = prog.ops.len();
        while i < nops {
            let mut j = i;
            while j < nops && self.passable(&prog.ops[j]) {
                j += 1;
            }
            if j > i {
                self.encode_segment(enc, id, i..j);
            }
            if j < nops {
                self.special(enc, id, j);
                // `special` either consumed the op or made it passable
                if self.consumed(&prog.ops[j]) {
                    j += 1;
                }
            }
            i = j;
        }
        if !self.live[&id].started {
            self.encode_segment(enc, id, 0..0);
        }
        self.stack.pop();
        let live = self.live.remove(&id).unwrap();
        self.r.ms_pool.release(&live.ms);
        let mut view = live.resolve_view.clone();
        let mut lease = live.resolve.clone();
        // post processing
        for post in &prog.post {
            match post {
                Post::Blur { sigma, clamp } => {
                    let src = lease.clone().expect("blur of the root target");
                    let out = self.blur(enc, &src, w, h, *sigma, *clamp);
                    self.r.plain_pool.release(&src);
                    view = out.view.clone();
                    lease = Some(out);
                }
                Post::Displace { amp, cell, seed, grid, clip_to, ss } => {
                    if let Some(m) = *clip_to {
                        self.ensure_done(enc, m);
                    }
                    let src = lease.clone().expect("displace of the root target");
                    let mask = clip_to.and_then(|m| self.res[m as usize].as_ref().map(|r| (r.view.clone(), r.rect)));
                    let dst = self.r.plain_pool.acquire(&self.r.dev, w, h, self.r.frame_no);
                    let params = FxParams {
                        origin: [prog.rect.x0, prog.rect.y0],
                        size: [w as i32, h as i32],
                        mask_origin: mask.as_ref().map_or([0; 2], |(_, r)| [r.x0, r.y0]),
                        mask_size: mask.as_ref().map_or([0; 2], |(_, r)| [r.w(), r.h()]),
                        grid: [grid.0, grid.1],
                        seed: *seed,
                        has_mask: mask.is_some() as u32,
                        amp: *amp,
                        cell: *cell,
                        ss: *ss as f32,
                        depth: 0.0,
                        light: [0.0; 2],
                        intensity: 0.0,
                        refraction: 0.0,
                        dispersion: 0.0,
                        out_w: w as f32,
                        out_h: h as f32,
                        pad: 0.0,
                    };
                    let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &params);
                    let bg = uniform_bg(
                        &self.r.dev,
                        &self.r.layouts.fx,
                        &ub,
                        &[
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.view) },
                            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(mask.as_ref().map_or(&self.r.dummy, |(v, _)| v)) },
                            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.r.dummy) },
                        ],
                    );
                    let pipe = self.r.pipes.displace.clone();
                    self.full_pass(enc, &dst.view, (w, h), &pipe, &bg);
                    self.r.plain_pool.release(&src);
                    view = dst.view.clone();
                    lease = Some(dst);
                }
            }
        }
        if let Some(l) = &lease {
            self.res[id as usize] = Some(Res { lease: Some(l.clone()), view, rect: prog.rect });
        } else {
            self.res[id as usize] = Some(Res { lease: None, view, rect: prog.rect });
        }
        let _ = live.resolve_tex;
        self.done[id as usize] = true;
        self.finish_deps(id);
    }

    fn finish_deps(&mut self, id: ProgId) {
        for d in self.deps[id as usize].clone() {
            let c = &mut self.consumers[d as usize];
            *c = c.saturating_sub(1);
            if *c == 0 && d != self.frame.root {
                if let Some(Some(res)) = self.res.get_mut(d as usize).map(|r| r.take()) {
                    if let Some(l) = &res.lease {
                        self.r.plain_pool.release(l);
                    }
                }
            }
        }
    }

    fn upload_pixels(&mut self, id: ProgId, prog: &Program, px: &PixelData) {
        let l = self.r.plain_pool.acquire(&self.r.dev, px.w, px.h, self.r.frame_no);
        self.r.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &l.tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &px.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(px.w * 4), rows_per_image: Some(px.h) },
            wgpu::Extent3d { width: px.w, height: px.h, depth_or_array_layers: 1 },
        );
        self.res[id as usize] = Some(Res { view: l.view.clone(), lease: Some(l), rect: prog.rect });
    }

    // ----- segments --------------------------------------------------------------

    /// Whether `op` can be encoded right now inside a pass.
    fn passable(&self, op: &Op) -> bool {
        let ready = |p: ProgId| self.done[p as usize];
        let clip_ready = |c: ClipId| self.frame.clips[c as usize].mask.is_none_or(ready);
        match op {
            Op::Draw(d) => clip_ready(d.clip),
            Op::Composite(c) => !matches!(c.mode, CompMode::Blend(_)) && ready(c.src) && c.aux.is_none_or(|(a, _)| ready(a)) && clip_ready(c.clip),
            Op::Backdrop(_) => false,
        }
    }

    /// Whether `special` fully handled the op (otherwise it only prepared it).
    fn consumed(&self, op: &Op) -> bool {
        match op {
            Op::Backdrop(_) => true,
            Op::Composite(c) => {
                matches!(c.mode, CompMode::Blend(_)) && self.done[c.src as usize] && c.aux.is_none_or(|(a, _)| self.done[a as usize]) && self.clip_done(c.clip)
            }
            Op::Draw(_) => false,
        }
    }

    fn clip_done(&self, c: ClipId) -> bool {
        self.frame.clips[c as usize].mask.is_none_or(|m| self.done[m as usize])
    }

    fn special(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, k: usize) {
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        // run whatever the op depends on first
        let mut need: Vec<ProgId> = Vec::new();
        let clip_mask = |c: ClipId, need: &mut Vec<ProgId>| {
            if let Some(m) = frame.clips[c as usize].mask {
                need.push(m);
            }
        };
        match &prog.ops[k] {
            Op::Draw(d) => clip_mask(d.clip, &mut need),
            Op::Composite(c) => {
                need.push(c.src);
                if let Some((a, _)) = c.aux {
                    need.push(a);
                }
                clip_mask(c.clip, &mut need);
            }
            Op::Backdrop(b) => {
                need.push(b.mask);
                clip_mask(b.clip, &mut need);
            }
        }
        let mut ran = false;
        for p in need {
            if !self.done[p as usize] {
                self.run_program(enc, p);
                ran = true;
            }
        }
        match &prog.ops[k] {
            Op::Composite(c) => {
                if let CompMode::Blend(mode) = c.mode {
                    let _ = ran;
                    self.blend_composite(enc, id, c, mode);
                }
            }
            Op::Backdrop(b) => self.backdrop(enc, id, b),
            Op::Draw(_) => {}
        }
    }

    fn ensure_started(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId) {
        if !self.live[&id].started {
            self.encode_segment(enc, id, 0..0);
        }
    }

    fn globals_bg(&mut self, prog: &Program) -> wgpu::BindGroup {
        let g = Globals { origin: [prog.rect.x0 as f32, prog.rect.y0 as f32], size: [prog.rect.w() as f32, prog.rect.h() as f32] };
        self.globals_for(g)
    }

    fn globals_for(&mut self, g: Globals) -> wgpu::BindGroup {
        let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &g);
        uniform_bg(
            &self.r.dev,
            &self.r.layouts.globals,
            &ub,
            &[
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.r.ramp_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.r.samp_clamp) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.r.samp_repeat) },
            ],
        )
    }

    fn clip_bg(&mut self, mask: Option<ProgId>) -> wgpu::BindGroup {
        let Some(m) = mask else { return self.r.noclip_bg.clone() };
        if let Some(bg) = self.clip_bgs.get(&m) {
            return bg.clone();
        }
        let res = self.res[m as usize].as_ref();
        let bg = match res {
            Some(res) => {
                let cp = ClipParams {
                    origin: [res.rect.x0 as f32, res.rect.y0 as f32],
                    size: [res.rect.w() as f32, res.rect.h() as f32],
                    enabled: 1.0,
                    pad: [0.0; 3],
                };
                let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &cp);
                uniform_bg(
                    &self.r.dev,
                    &self.r.layouts.clip,
                    &ub,
                    &[wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&res.view) }],
                )
            }
            // the mask never rendered (empty): everything it clips is invisible
            None => {
                let cp = ClipParams { origin: [0.0; 2], size: [0.0; 2], enabled: 1.0, pad: [0.0; 3] };
                let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &cp);
                uniform_bg(
                    &self.r.dev,
                    &self.r.layouts.clip,
                    &ub,
                    &[wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.r.dummy) }],
                )
            }
        };
        self.clip_bgs.insert(m, bg.clone());
        bg
    }

    /// Encodes `ops[range]` of program `id` in one render pass.
    fn encode_segment(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, range: Range<usize>) {
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        let globals = self.globals_bg(prog);
        let (ms_view, res_view, started) = {
            let l = &self.live[&id];
            (l.ms.view.clone(), l.resolve_view.clone(), l.started)
        };
        let load = if started { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(color_of(prog.clear)) };
        self.live.get_mut(&id).unwrap().started = true;
        self.stats.passes += 1;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("program"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ms_view,
                depth_slice: None,
                resolve_target: Some(&res_view),
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0.0, 0.0, prog.rect.w() as f32, prog.rect.h() as f32, 0.0, 1.0);
        pass.set_bind_group(0, &globals, &[]);
        let inst_buf = self.r.inst_buf.clone().expect("instance buffer");
        pass.set_vertex_buffer(1, inst_buf.slice(..));
        // state caches
        let mut cur_pipe: u8 = 255;
        let mut cur_mask: Option<Option<ProgId>> = None;
        let mut cur_g2 = G2::None;
        let mut cur_scissor = IRect::EMPTY;
        let mut cur_chunk = u32::MAX;
        for k in range {
            match &prog.ops[k] {
                Op::Draw(d) => {
                    let cd = frame.clips[d.clip as usize];
                    let sc = cd.scissor.intersect(prog.rect);
                    if sc.is_empty() {
                        continue;
                    }
                    let Some(loc) = self.r.arena.locs.get(&d.mesh.id).copied() else { continue };
                    let want: u8 = match (d.blend, d.image.is_some()) {
                        (DrawBlend::Over, false) => 0,
                        (DrawBlend::Erase, false) => 1,
                        (DrawBlend::Over, true) => 2,
                        (DrawBlend::Erase, true) => 3,
                    };
                    if cur_pipe != want {
                        let p = &self.r.pipes;
                        pass.set_pipeline(match want {
                            0 => &p.draw_over,
                            1 => &p.draw_erase,
                            2 => &p.image_over,
                            _ => &p.image_erase,
                        });
                        if cur_pipe == 255 || (cur_pipe >= 2) != (want >= 2) || cur_g2 == G2::Comp {
                            cur_g2 = G2::None;
                        }
                        cur_pipe = want;
                    }
                    if cur_mask != Some(cd.mask) {
                        let bg = self.clip_bg(cd.mask);
                        pass.set_bind_group(1, &bg, &[]);
                        cur_mask = Some(cd.mask);
                    }
                    if let Some(img) = &d.image {
                        if cur_g2 != G2::Image(img.id) {
                            let Some(t) = self.r.images.get(&img.id) else { continue };
                            pass.set_bind_group(2, &t.bind, &[]);
                            cur_g2 = G2::Image(img.id);
                        }
                    }
                    if cur_scissor != sc {
                        pass.set_scissor_rect((sc.x0 - prog.rect.x0) as u32, (sc.y0 - prog.rect.y0) as u32, sc.w() as u32, sc.h() as u32);
                        cur_scissor = sc;
                    }
                    if cur_chunk != loc.chunk {
                        let c = &self.r.arena.chunks[loc.chunk as usize];
                        pass.set_vertex_buffer(0, c.v.slice(..));
                        pass.set_index_buffer(c.i.slice(..), wgpu::IndexFormat::Uint32);
                        cur_chunk = loc.chunk;
                    }
                    let inst = self.inst_base[id as usize] + k as u32;
                    pass.draw_indexed(loc.first_index..loc.first_index + loc.count, loc.base_vertex, inst..inst + 1);
                    self.stats.draw_calls += 1;
                }
                Op::Composite(c) => {
                    let Some((src_view, src_rect)) = self.res[c.src as usize].as_ref().map(|r| (r.view.clone(), r.rect)) else { continue };
                    let aux = match c.aux {
                        Some((a, m)) => match self.res[a as usize].as_ref() {
                            Some(r) => Some((r.view.clone(), r.rect, m)),
                            None => continue,
                        },
                        None => None,
                    };
                    let cd = frame.clips[c.clip as usize];
                    let sc = cd.scissor.intersect(prog.rect).intersect(c.rect);
                    if sc.is_empty() {
                        continue;
                    }
                    let kind = match c.mode {
                        CompMode::Erase => CompKind::Erase,
                        _ => CompKind::Over,
                    };
                    let params = CParams {
                        rect: f4(c.rect),
                        src_origin: [src_rect.x0 as f32, src_rect.y0 as f32],
                        src_size: [src_rect.w() as f32, src_rect.h() as f32],
                        aux_origin: aux.as_ref().map_or([0.0; 2], |(_, r, _)| [r.x0 as f32, r.y0 as f32]),
                        aux_size: aux.as_ref().map_or([0.0; 2], |(_, r, _)| [r.w() as f32, r.h() as f32]),
                        snap_origin: [0.0; 2],
                        snap_size: [0.0; 2],
                        tint: c.tint.unwrap_or([0.0; 4]),
                        opacity: c.opacity,
                        has_tint: c.tint.is_some() as u32,
                        aux_mode: aux.as_ref().map_or(0, |(_, _, m)| if *m == AuxMode::Mul { 1 } else { 2 }),
                        blend: 0,
                    };
                    let spec = CompSpec { params, src: &src_view, aux: aux.as_ref().map(|(v, _, _)| v), snap: None, kind, scissor: sc, clip_mask: cd.mask };
                    self.write_composite(&mut pass, prog.rect, &spec, true);
                    // the composite pipeline has its own layout: reset the cached state
                    cur_pipe = 254;
                    cur_g2 = G2::Comp;
                    cur_mask = Some(cd.mask);
                    cur_scissor = sc;
                    self.stats.composites += 1;
                }
                Op::Backdrop(_) => {}
            }
        }
    }

    /// Records one composite quad into an open pass (`ms`: multisampled target).
    fn write_composite(&mut self, pass: &mut wgpu::RenderPass<'_>, target: IRect, spec: &CompSpec, ms: bool) {
        let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &spec.params);
        let bg = uniform_bg(
            &self.r.dev,
            &self.r.layouts.comp,
            &ub,
            &[
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(spec.src) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(spec.aux.unwrap_or(&self.r.dummy)) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(spec.snap.unwrap_or(&self.r.dummy)) },
            ],
        );
        let clip = self.clip_bg(spec.clip_mask);
        pass.set_pipeline(self.r.pipes.comp(spec.kind, ms));
        pass.set_bind_group(1, &clip, &[]);
        pass.set_bind_group(2, &bg, &[]);
        let sc = spec.scissor.intersect(target);
        pass.set_scissor_rect((sc.x0 - target.x0) as u32, (sc.y0 - target.y0) as u32, sc.w() as u32, sc.h() as u32);
        pass.draw(0..6, 0..1);
    }

    /// Copies `rect` of the program's current content into a fresh texture.
    fn snapshot(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, rect: IRect) -> Lease {
        let prog_rect = self.frame.programs[id as usize].rect;
        let l = self.r.plain_pool.acquire(&self.r.dev, rect.w() as u32, rect.h() as u32, self.r.frame_no);
        let src = self.live[&id].resolve_tex.clone();
        enc.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &src,
                mip_level: 0,
                origin: wgpu::Origin3d { x: (rect.x0 - prog_rect.x0) as u32, y: (rect.y0 - prog_rect.y0) as u32, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo { texture: &l.tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: rect.w() as u32, height: rect.h() as u32, depth_or_array_layers: 1 },
        );
        l
    }

    /// One composite quad in its own pass over the program's target.
    fn composite_pass(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, spec: CompSpec<'_>) {
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        let globals = self.globals_bg(prog);
        let (ms_view, res_view) = {
            let l = &self.live[&id];
            (l.ms.view.clone(), l.resolve_view.clone())
        };
        self.stats.passes += 1;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ms_view,
                depth_slice: None,
                resolve_target: Some(&res_view),
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0.0, 0.0, prog.rect.w() as f32, prog.rect.h() as f32, 0.0, 1.0);
        pass.set_bind_group(0, &globals, &[]);
        self.write_composite(&mut pass, prog.rect, &spec, true);
        self.stats.composites += 1;
    }

    fn blend_composite(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, c: &CompositeOp, mode: designos_doc::BlendMode) {
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        let cd = frame.clips[c.clip as usize];
        let sc = cd.scissor.intersect(prog.rect).intersect(c.rect);
        if sc.is_empty() {
            return;
        }
        let (Some(src), aux) = (self.res[c.src as usize].as_ref(), c.aux.and_then(|(a, m)| self.res[a as usize].as_ref().map(|r| (r, m)))) else { return };
        let (src_view, src_rect) = (src.view.clone(), src.rect);
        let aux = aux.map(|(r, m)| (r.view.clone(), r.rect, m));
        self.ensure_started(enc, id);
        let snap = self.snapshot(enc, id, sc);
        let tint = c.tint.unwrap_or([0.0; 4]);
        let params = CParams {
            rect: f4(c.rect),
            src_origin: [src_rect.x0 as f32, src_rect.y0 as f32],
            src_size: [src_rect.w() as f32, src_rect.h() as f32],
            aux_origin: aux.as_ref().map_or([0.0; 2], |(_, r, _)| [r.x0 as f32, r.y0 as f32]),
            aux_size: aux.as_ref().map_or([0.0; 2], |(_, r, _)| [r.w() as f32, r.h() as f32]),
            snap_origin: [sc.x0 as f32, sc.y0 as f32],
            snap_size: [sc.w() as f32, sc.h() as f32],
            tint,
            opacity: c.opacity,
            has_tint: c.tint.is_some() as u32,
            aux_mode: aux.as_ref().map_or(0, |(_, _, m)| if *m == AuxMode::Mul { 1 } else { 2 }),
            blend: blend_id(mode),
        };
        let spec = CompSpec {
            params,
            src: &src_view,
            aux: aux.as_ref().map(|(v, _, _)| v),
            snap: Some(&snap.view),
            kind: CompKind::Blend,
            scissor: sc,
            clip_mask: cd.mask,
        };
        self.composite_pass(enc, id, spec);
        self.r.plain_pool.release(&snap);
    }

    // ----- backdrop effects --------------------------------------------------------

    /// What lies beneath `pad` for the program on top of the execution stack:
    /// the chain of programs that read their parent's backdrop, composited.
    fn backdrop_texture(&mut self, enc: &mut wgpu::CommandEncoder, pad: IRect) -> Lease {
        let frame = self.frame;
        let mut start = self.stack.len() - 1;
        while start > 0 && frame.programs[self.stack[start] as usize].needs_backdrop {
            start -= 1;
        }
        let chain: Vec<ProgId> = self.stack[start..].to_vec();
        let dst = self.r.plain_pool.acquire(&self.r.dev, pad.w() as u32, pad.h() as u32, self.r.frame_no);
        let globals = self.globals_for(Globals { origin: [pad.x0 as f32, pad.y0 as f32], size: [pad.w() as f32, pad.h() as f32] });
        let mut first = true;
        for p in chain {
            let prog_rect = frame.programs[p as usize].rect;
            let r = pad.intersect(prog_rect);
            let (view, load) = (self.live[&p].resolve_view.clone(), if first { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load });
            first = false;
            self.stats.passes += 1;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if r.is_empty() {
                continue;
            }
            pass.set_viewport(0.0, 0.0, pad.w() as f32, pad.h() as f32, 0.0, 1.0);
            pass.set_bind_group(0, &globals, &[]);
            let params = CParams {
                rect: f4(r),
                src_origin: [prog_rect.x0 as f32, prog_rect.y0 as f32],
                src_size: [prog_rect.w() as f32, prog_rect.h() as f32],
                aux_origin: [0.0; 2],
                aux_size: [0.0; 2],
                snap_origin: [0.0; 2],
                snap_size: [0.0; 2],
                tint: [0.0; 4],
                opacity: 1.0,
                has_tint: 0,
                aux_mode: 0,
                blend: 0,
            };
            let spec = CompSpec { params, src: &view, aux: None, snap: None, kind: CompKind::Over, scissor: r, clip_mask: None };
            self.write_composite(&mut pass, pad, &spec, false);
        }
        dst
    }

    fn backdrop(&mut self, enc: &mut wgpu::CommandEncoder, id: ProgId, b: &BackdropOp) {
        let frame = self.frame;
        let prog = &frame.programs[id as usize];
        let region = b.region.intersect(prog.rect);
        let pad = b.pad;
        if region.is_empty() || pad.is_empty() {
            return;
        }
        let Some(mask) = self.res[b.mask as usize].as_ref().map(|r| (r.view.clone(), r.rect)) else { return };
        self.ensure_started(enc, id);
        let cd = frame.clips[b.clip as usize];
        let sc = cd.scissor.intersect(prog.rect).intersect(region);
        if sc.is_empty() {
            return;
        }
        let glass = matches!(b.kind, BackdropKind::Glass(_));
        let bd = self.backdrop_texture(enc, pad);
        let (pw, ph) = (pad.w() as u32, pad.h() as u32);
        let processed = match b.kind {
            BackdropKind::Blur { sigma } => {
                let out = self.blur(enc, &bd, pw, ph, sigma, true);
                self.r.plain_pool.release(&bd);
                out
            }
            BackdropKind::Glass(gp) => {
                let mres = self.res[b.mask as usize].as_ref().and_then(|r| r.lease.clone().map(|l| (l, r.rect)));
                let Some((mlease, mrect)) = mres else {
                    self.r.plain_pool.release(&bd);
                    return;
                };
                let frost = self.blur(enc, &bd, pw, ph, gp.sigma_frost, true);
                self.r.plain_pool.release(&bd);
                let soft = self.blur(enc, &mlease, mrect.w() as u32, mrect.h() as u32, gp.sigma_mask, false);
                let out = self.r.plain_pool.acquire(&self.r.dev, sc.w() as u32, sc.h() as u32, self.r.frame_no);
                let params = FxParams {
                    origin: [sc.x0, sc.y0],
                    size: [pw as i32, ph as i32],
                    mask_origin: [mrect.x0, mrect.y0],
                    mask_size: [mrect.w(), mrect.h()],
                    grid: [0; 2],
                    seed: 0,
                    has_mask: 1,
                    amp: 0.0,
                    cell: 1.0,
                    ss: 1.0,
                    depth: gp.depth,
                    light: gp.light,
                    intensity: gp.intensity,
                    refraction: gp.refraction,
                    dispersion: gp.dispersion,
                    out_w: sc.w() as f32,
                    out_h: sc.h() as f32,
                    pad: 0.0,
                };
                let ub = self.r.ring.alloc(&self.r.dev, &self.r.queue, &params);
                let bgf = uniform_bg(
                    &self.r.dev,
                    &self.r.layouts.fx,
                    &ub,
                    &[
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frost.view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mask.0) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&soft.view) },
                    ],
                );
                let pipe = self.r.pipes.glass.clone();
                self.full_pass(enc, &out.view, (sc.w() as u32, sc.h() as u32), &pipe, &bgf);
                self.r.plain_pool.release(&frost);
                self.r.plain_pool.release(&soft);
                out
            }
        };
        let snap = self.snapshot(enc, id, sc);
        let params = CParams {
            rect: f4(sc),
            src_origin: if glass { [sc.x0 as f32, sc.y0 as f32] } else { [pad.x0 as f32, pad.y0 as f32] },
            src_size: if glass { [sc.w() as f32, sc.h() as f32] } else { [pw as f32, ph as f32] },
            aux_origin: [mask.1.x0 as f32, mask.1.y0 as f32],
            aux_size: [mask.1.w() as f32, mask.1.h() as f32],
            snap_origin: [sc.x0 as f32, sc.y0 as f32],
            snap_size: [sc.w() as f32, sc.h() as f32],
            tint: [0.0; 4],
            opacity: 1.0,
            has_tint: 0,
            aux_mode: 1,
            blend: 0,
        };
        let spec =
            CompSpec { params, src: &processed.view, aux: Some(&mask.0), snap: Some(&snap.view), kind: CompKind::Replace, scissor: sc, clip_mask: cd.mask };
        self.composite_pass(enc, id, spec);
        self.r.plain_pool.release(&snap);
        self.r.plain_pool.release(&processed);
    }

    // ----- blur ----------------------------------------------------------------------

    fn full_pass(&mut self, enc: &mut wgpu::CommandEncoder, dst: &wgpu::TextureView, extent: (u32, u32), pipe: &wgpu::RenderPipeline, bg: &wgpu::BindGroup) {
        self.stats.passes += 1;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0.0, 0.0, extent.0 as f32, extent.1 as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, extent.0, extent.1);
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }

    fn resample(&mut self, enc: &mut wgpu::CommandEncoder, src: &Lease, src_extent: (u32, u32), dst_extent: (u32, u32)) -> Lease {
        let dst = self.r.plain_pool.acquire(&self.r.dev, dst_extent.0, dst_extent.1, self.r.frame_no);
        let ub = self.r.ring.alloc(
            &self.r.dev,
            &self.r.queue,
            &BlitParams {
                src_extent: [src_extent.0 as f32, src_extent.1 as f32],
                dst_extent: [dst_extent.0 as f32, dst_extent.1 as f32],
                src_tex: [src.w as f32, src.h as f32],
                pad: [0.0; 2],
            },
        );
        let bg = uniform_bg(
            &self.r.dev,
            &self.r.layouts.blit,
            &ub,
            &[
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.r.samp_clamp) },
            ],
        );
        let pipe = self.r.pipes.blit.clone();
        self.full_pass(enc, &dst.view, dst_extent, &pipe, &bg);
        dst
    }

    fn blur_pass(&mut self, enc: &mut wgpu::CommandEncoder, src: &Lease, extent: (u32, u32), sigma: f32, dir: [i32; 2], clamp: bool) -> Lease {
        let dst = self.r.plain_pool.acquire(&self.r.dev, extent.0, extent.1, self.r.frame_no);
        let radius = (3.0 * sigma).ceil().max(1.0) as i32;
        let ub = self.r.ring.alloc(
            &self.r.dev,
            &self.r.queue,
            &BlurParams { dir, size: [extent.0 as i32, extent.1 as i32], radius, clamp_edges: clamp as u32, sigma, pad: 0.0 },
        );
        let bg =
            uniform_bg(&self.r.dev, &self.r.layouts.blur, &ub, &[wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.view) }]);
        let pipe = self.r.pipes.blur.clone();
        self.full_pass(enc, &dst.view, extent, &pipe, &bg);
        self.stats.blur_passes += 1;
        dst
    }

    /// Gaussian blur of the `w` x `h` content of `src` (not released here).
    /// Large sigmas are blurred at reduced resolution.
    fn blur(&mut self, enc: &mut wgpu::CommandEncoder, src: &Lease, w: u32, h: u32, sigma: f32, clamp: bool) -> Lease {
        if sigma < 0.05 {
            // nothing to do: hand back a copy so the caller can release both
            return self.resample(enc, src, (w, h), (w, h));
        }
        let mut k = 1u32;
        while sigma / k as f32 > MAX_DIRECT_SIGMA && k < 32 {
            k *= 2;
        }
        let (sw, sh) = (w.div_ceil(k).max(1), h.div_ceil(k).max(1));
        let (small, owned) = if k > 1 { (self.resample(enc, src, (w, h), (sw, sh)), true) } else { (src.clone(), false) };
        let s = sigma / k as f32;
        let tmp = self.blur_pass(enc, &small, (sw, sh), s, [1, 0], clamp);
        if owned {
            self.r.plain_pool.release(&small);
        }
        let done = self.blur_pass(enc, &tmp, (sw, sh), s, [0, 1], clamp);
        self.r.plain_pool.release(&tmp);
        if k > 1 {
            let up = self.resample(enc, &done, (sw, sh), (w, h));
            self.r.plain_pool.release(&done);
            up
        } else {
            done
        }
    }
}
