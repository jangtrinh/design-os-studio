//! GPU resource management: mesh arena, texture pools, uniform ring, image
//! textures and the gradient ramp texture.

use crate::frame::*;
use crate::mesh::Mesh;
use crate::pipelines::{BlitParams, FORMAT, MSAA};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ---------------------------------------------------------------- mesh arena

const VCAP: u32 = 1 << 20;
const ICAP: u32 = 3 << 20;
/// Bytes of mesh data kept resident before the arena is rebuilt.
const ARENA_LIMIT: usize = 384 << 20;

pub struct Chunk {
    pub v: wgpu::Buffer,
    pub i: wgpu::Buffer,
    vcap: u32,
    icap: u32,
    vused: u32,
    iused: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Loc {
    pub chunk: u32,
    pub base_vertex: i32,
    pub first_index: u32,
    pub count: u32,
}

#[derive(Default)]
pub struct Arena {
    pub chunks: Vec<Chunk>,
    pub locs: HashMap<u64, Loc>,
    bytes: usize,
    pub uploaded: u32,
}

impl Arena {
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.locs.clear();
        self.bytes = 0;
    }

    /// Uploads every mesh in `meshes` that is not resident yet.
    pub fn ensure(&mut self, dev: &wgpu::Device, queue: &wgpu::Queue, meshes: &[Arc<Mesh>]) {
        let mut seen = HashSet::new();
        let mut all: Vec<&Arc<Mesh>> = Vec::new();
        for m in meshes {
            if seen.insert(m.id) {
                all.push(m);
            }
        }
        let mut missing: Vec<&Arc<Mesh>> = all.iter().copied().filter(|m| !self.locs.contains_key(&m.id)).collect();
        let need: usize = missing.iter().map(|m| m.bytes()).sum();
        if self.bytes + need > ARENA_LIMIT {
            self.clear();
            missing = all;
        }
        if missing.is_empty() {
            return;
        }
        // staging per chunk: (first vertex byte, vertex bytes, first index byte, index bytes)
        let mut stage: HashMap<u32, (u64, Vec<u8>, u64, Vec<u8>)> = HashMap::new();
        for m in missing {
            let (nv, ni) = (m.verts.len() as u32, m.idx.len() as u32);
            if nv == 0 || ni == 0 {
                continue;
            }
            let fits = self.chunks.last().is_some_and(|c| c.vused + nv <= c.vcap && c.iused + ni <= c.icap);
            if !fits {
                let (vcap, icap) = (VCAP.max(nv), ICAP.max(ni));
                self.chunks.push(Chunk {
                    v: dev.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("mesh vertices"),
                        size: vcap as u64 * 8,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    i: dev.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("mesh indices"),
                        size: icap as u64 * 4,
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    vcap,
                    icap,
                    vused: 0,
                    iused: 0,
                });
            }
            let ci = self.chunks.len() as u32 - 1;
            let c = self.chunks.last_mut().unwrap();
            let st = stage.entry(ci).or_insert_with(|| (c.vused as u64 * 8, Vec::new(), c.iused as u64 * 4, Vec::new()));
            st.1.extend_from_slice(bytemuck::cast_slice(&m.verts));
            st.3.extend_from_slice(bytemuck::cast_slice(&m.idx));
            self.locs.insert(m.id, Loc { chunk: ci, base_vertex: c.vused as i32, first_index: c.iused, count: ni });
            c.vused += nv;
            c.iused += ni;
            self.bytes += m.bytes();
            self.uploaded += 1;
        }
        for (ci, (vo, vdata, io, idata)) in stage {
            let c = &self.chunks[ci as usize];
            queue.write_buffer(&c.v, vo, &vdata);
            queue.write_buffer(&c.i, io, &idata);
        }
    }
}

// ---------------------------------------------------------------- texture pools

#[derive(Clone)]
pub struct Lease {
    pub idx: usize,
    pub tex: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Allocated size (at least the requested size).
    pub w: u32,
    pub h: u32,
}

struct Entry {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    free: bool,
    last_used: u64,
}

pub struct Pool {
    entries: Vec<Option<Entry>>,
    ms: bool,
    pub created: u32,
}

fn round_up(v: u32) -> u32 {
    if v <= 64 {
        v.next_power_of_two().max(8)
    } else {
        v.div_ceil(64) * 64
    }
}

impl Pool {
    pub fn new(ms: bool) -> Self {
        Self { entries: Vec::new(), ms, created: 0 }
    }

    pub fn acquire(&mut self, dev: &wgpu::Device, w: u32, h: u32, frame: u64) -> Lease {
        let (cw, ch) = (round_up(w.max(1)), round_up(h.max(1)));
        let found = self.entries.iter().position(|e| e.as_ref().is_some_and(|e| e.free && e.w == cw && e.h == ch));
        let idx = match found {
            Some(i) => i,
            None => {
                let usage = if self.ms {
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                } else {
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST
                };
                let tex = dev.create_texture(&wgpu::TextureDescriptor {
                    label: Some(if self.ms { "ms target" } else { "target" }),
                    size: wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: if self.ms { MSAA } else { 1 },
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage,
                    view_formats: &[],
                });
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                self.created += 1;
                let e = Entry { tex, view, w: cw, h: ch, free: true, last_used: frame };
                match self.entries.iter().position(|e| e.is_none()) {
                    Some(i) => {
                        self.entries[i] = Some(e);
                        i
                    }
                    None => {
                        self.entries.push(Some(e));
                        self.entries.len() - 1
                    }
                }
            }
        };
        let e = self.entries[idx].as_mut().unwrap();
        e.free = false;
        e.last_used = frame;
        Lease { idx, tex: e.tex.clone(), view: e.view.clone(), w: e.w, h: e.h }
    }

    pub fn release(&mut self, l: &Lease) {
        if let Some(Some(e)) = self.entries.get_mut(l.idx) {
            e.free = true;
        }
    }

    /// Frees textures unused for `idle` frames.
    pub fn trim(&mut self, frame: u64, idle: u64) {
        for e in &mut self.entries {
            if e.as_ref().is_some_and(|x| x.free && frame.saturating_sub(x.last_used) > idle) {
                *e = None;
            }
        }
    }

    pub fn bytes(&self) -> u64 {
        let px = if self.ms { MSAA as u64 } else { 1 };
        self.entries.iter().flatten().map(|e| e.w as u64 * e.h as u64 * 4 * px).sum()
    }
}

// ---------------------------------------------------------------- uniform ring

const RING_CHUNK: u64 = 1 << 20;
const SLOT: u64 = 256;

#[derive(Default)]
pub struct Ring {
    chunks: Vec<wgpu::Buffer>,
    cur: usize,
    off: u64,
}

impl Ring {
    pub fn reset(&mut self) {
        self.cur = 0;
        self.off = 0;
    }

    /// Writes `v` into a fresh 256-byte slot; returns the buffer, offset and size.
    pub fn alloc<T: bytemuck::Pod>(&mut self, dev: &wgpu::Device, queue: &wgpu::Queue, v: &T) -> (wgpu::Buffer, u64, u64) {
        let size = std::mem::size_of::<T>() as u64;
        if self.chunks.is_empty() || self.off + SLOT > RING_CHUNK {
            if !self.chunks.is_empty() {
                self.cur += 1;
            }
            self.off = 0;
            if self.cur >= self.chunks.len() {
                self.chunks.push(dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("uniform ring"),
                    size: RING_CHUNK,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
        }
        let buf = self.chunks[self.cur].clone();
        let off = self.off;
        queue.write_buffer(&buf, off, bytemuck::bytes_of(v));
        self.off += SLOT;
        (buf, off, size)
    }
}

pub fn uniform_bg(dev: &wgpu::Device, layout: &wgpu::BindGroupLayout, ub: &(wgpu::Buffer, u64, u64), extra: &[wgpu::BindGroupEntry]) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &ub.0, offset: ub.1, size: wgpu::BufferSize::new(ub.2) }),
    }];
    entries.extend_from_slice(extra);
    dev.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
}

// ---------------------------------------------------------------- images

pub struct ImageTex {
    pub bind: wgpu::BindGroup,
    pub last_used: u64,
}

fn mip_count(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

/// Halves an RGBA8 image with a box filter until it fits `max` pixels per side.
fn shrink_to_fit(img: &ImageData, max: u32) -> (u32, u32, std::borrow::Cow<'_, [u8]>) {
    let (mut w, mut h) = (img.w, img.h);
    let mut data = std::borrow::Cow::Borrowed(&img.rgba[..]);
    while w > max || h > max {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut s = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                        s += data[((sy * w + sx) * 4 + c) as usize] as u32;
                    }
                    out[((y * nw + x) * 4 + c) as usize] = (s / 4) as u8;
                }
            }
        }
        data = std::borrow::Cow::Owned(out);
        w = nw;
        h = nh;
    }
    (w, h, data)
}

pub struct ImageUpload<'a> {
    pub dev: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub enc: &'a mut wgpu::CommandEncoder,
    pub blit: &'a wgpu::RenderPipeline,
    pub blit_layout: &'a wgpu::BindGroupLayout,
    pub image_layout: &'a wgpu::BindGroupLayout,
    pub sampler: &'a wgpu::Sampler,
    pub ring: &'a mut Ring,
    pub max_dim: u32,
    pub frame: u64,
    pub split: &'a mut crate::submit::Splitter,
}

impl ImageUpload<'_> {
    pub fn upload(&mut self, img: &ImageData) -> ImageTex {
        let (w, h, data) = shrink_to_fit(img, self.max_dim.max(64));
        let mips = mip_count(w, h);
        let tex = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let level = |m: u32| tex.create_view(&wgpu::TextureViewDescriptor { base_mip_level: m, mip_level_count: Some(1), ..Default::default() });
        for m in 1..mips {
            let (sw, sh) = ((w >> (m - 1)).max(1), (h >> (m - 1)).max(1));
            let (dw, dh) = ((w >> m).max(1), (h >> m).max(1));
            let src = level(m - 1);
            let dst = level(m);
            let ub = self.ring.alloc(
                self.dev,
                self.queue,
                &BlitParams { src_extent: [sw as f32, sh as f32], dst_extent: [dw as f32, dh as f32], src_tex: [sw as f32, sh as f32], pad: [0.0; 2] },
            );
            let bg = uniform_bg(
                self.dev,
                self.blit_layout,
                &ub,
                &[
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(self.sampler) },
                ],
            );
            self.split.reserve(self.enc, crate::submit::HAL_BUFFERS_PER_PASS);
            let mut pass = self.enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mip"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(self.blit);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..3, 0..1);
        }
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: self.image_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        ImageTex { bind, last_used: self.frame }
    }
}
