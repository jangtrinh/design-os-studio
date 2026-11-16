// Bilinear resample (down/up scaling, mip generation). `src_extent` is the
// logical size of the source content; pooled textures are larger than that.

struct FsIn {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FsIn {
    // one oversized triangle covering the viewport
    var o: FsIn;
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return o;
}

struct BlitParams {
    src_extent: vec2<f32>, // logical size of the source content
    dst_extent: vec2<f32>, // logical size of the destination
    src_tex: vec2<f32>,    // allocated size of the source texture
    edge_mode: u32,
    p0: u32,
};

@group(0) @binding(0) var<uniform> tp: BlitParams;
@group(0) @binding(1) var tsrc: texture_2d<f32>;
@group(0) @binding(2) var tsamp: sampler;

fn valid(p: vec2<i32>) -> bool {
    return p.x >= 0 && p.y >= 0 && p.x < i32(tp.src_extent.x) && p.y < i32(tp.src_extent.y);
}

fn edge_sample(p: vec2<i32>) -> vec4<f32> {
    // Clamp the load itself before masking the sample. WGSL may evaluate both
    // branches of select, so an out-of-range textureLoad is not safe here.
    let max_coord = vec2<i32>(i32(tp.src_extent.x) - 1, i32(tp.src_extent.y) - 1);
    let safe = clamp(p, vec2<i32>(0), max_coord);
    return select(vec4<f32>(0.0), textureLoad(tsrc, safe, 0), valid(p));
}

@fragment
fn fs_blit(f: FsIn) -> @location(0) vec4<f32> {
    var s = (f.pos.xy) * (tp.src_extent / tp.dst_extent);
    let hi = tp.src_extent - vec2<f32>(0.5);
    if tp.edge_mode != 0u && (any(s < vec2<f32>(0.5)) || any(s > hi)) {
        let p = s - vec2<f32>(0.5);
        let base = vec2<i32>(floor(p));
        let frac = fract(p);
        let a = edge_sample(base);
        let b = edge_sample(base + vec2<i32>(1, 0));
        let c = edge_sample(base + vec2<i32>(0, 1));
        let d = edge_sample(base + vec2<i32>(1, 1));
        return mix(mix(a, b, frac.x), mix(c, d, frac.x), frac.y);
    }
    s = clamp(s, vec2<f32>(0.5), hi);
    return textureSampleLevel(tsrc, tsamp, s / tp.src_tex, 0.0);
}
