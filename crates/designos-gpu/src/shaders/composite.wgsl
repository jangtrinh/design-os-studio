// Composites a program result (layer, shadow, mask product ...) into a target.
//
// Modes: 0 over / erase (hardware blending), 1 blend mode against a snapshot of
// the destination, 2 replace under a mask (background blur).

struct Globals {
    origin: vec2<f32>,
    size: vec2<f32>,
};

struct ClipParams {
    origin: vec2<f32>,
    size: vec2<f32>,
    enabled: f32,
    p0: f32,
    p1: f32,
    p2: f32,
};

struct CParams {
    rect: vec4<f32>,       // destination rectangle in root pixels (x0, y0, x1, y1)
    src_origin: vec2<f32>,
    src_size: vec2<f32>,
    aux_origin: vec2<f32>,
    aux_size: vec2<f32>,
    snap_origin: vec2<f32>,
    snap_size: vec2<f32>,
    tint: vec4<f32>,
    opacity: f32,
    has_tint: u32,
    aux_mode: u32,   // 0 none, 1 multiply, 2 multiply by inverse, 3 multiply by Rec. 709 luminance
    blend: u32,      // blend mode id
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<uniform> clip: ClipParams;
@group(1) @binding(1) var clip_tex: texture_2d<f32>;
@group(2) @binding(0) var<uniform> p: CParams;
@group(2) @binding(1) var src_tex: texture_2d<f32>;
@group(2) @binding(2) var aux_tex: texture_2d<f32>;
@group(2) @binding(3) var snap_tex: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[vi];
    let dev = vec2<f32>(mix(p.rect.x, p.rect.z, c.x), mix(p.rect.y, p.rect.w, c.y)) - g.origin;
    var o: VOut;
    o.pos = vec4<f32>(dev.x / g.size.x * 2.0 - 1.0, 1.0 - dev.y / g.size.y * 2.0, 0.0, 1.0);
    return o;
}

fn clip_cov(frag: vec2<f32>) -> f32 {
    if (clip.enabled < 0.5) {
        return 1.0;
    }
    let q = vec2<i32>(floor(frag + g.origin - clip.origin));
    if (q.x < 0 || q.y < 0 || f32(q.x) >= clip.size.x || f32(q.y) >= clip.size.y) {
        return 0.0;
    }
    return textureLoad(clip_tex, q, 0).a;
}

fn load_in(tex: texture_2d<f32>, root: vec2<f32>, origin: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let q = vec2<i32>(root - origin);
    if (q.x < 0 || q.y < 0 || f32(q.x) >= size.x || f32(q.y) >= size.y) {
        return vec4<f32>(0.0);
    }
    return textureLoad(tex, q, 0);
}

// ---- blend functions (W3C compositing, straight colours)

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

fn clip_color(c0: vec3<f32>) -> vec3<f32> {
    var c = c0;
    let l = lum(c);
    let n = min(c.r, min(c.g, c.b));
    let x = max(c.r, max(c.g, c.b));
    if (n < 0.0) {
        c = vec3<f32>(l) + (c - vec3<f32>(l)) * l / (l - n);
    }
    if (x > 1.0) {
        c = vec3<f32>(l) + (c - vec3<f32>(l)) * (1.0 - l) / (x - l);
    }
    return c;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    return clip_color(c + vec3<f32>(l - lum(c)));
}

fn sat(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    if (mx <= mn) {
        return vec3<f32>(0.0);
    }
    return (c - vec3<f32>(mn)) * s / (mx - mn);
}

fn soft_light_ch(cb: f32, cs: f32) -> f32 {
    if (cs <= 0.5) {
        return cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb);
    }
    var d: f32;
    if (cb <= 0.25) {
        d = ((16.0 * cb - 12.0) * cb + 4.0) * cb;
    } else {
        d = sqrt(cb);
    }
    return cb + (2.0 * cs - 1.0) * (d - cb);
}

fn color_dodge_ch(cb: f32, cs: f32) -> f32 {
    if (cb == 0.0) {
        return 0.0;
    }
    if (cs >= 1.0) {
        return 1.0;
    }
    return min(1.0, cb / (1.0 - cs));
}

fn color_burn_ch(cb: f32, cs: f32) -> f32 {
    if (cb >= 1.0) {
        return 1.0;
    }
    if (cs <= 0.0) {
        return 0.0;
    }
    return 1.0 - min(1.0, (1.0 - cb) / cs);
}

fn hard_light_ch(cb: f32, cs: f32) -> f32 {
    if (cs <= 0.5) {
        return cb * 2.0 * cs;
    }
    let s2 = 2.0 * cs - 1.0;
    return cb + s2 - cb * s2;
}

fn blend_fn(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    switch (mode) {
        case 2u: { return min(cb, cs); }
        case 3u: { return cb * cs; }
        case 4u: { return vec3<f32>(color_burn_ch(cb.r, cs.r), color_burn_ch(cb.g, cs.g), color_burn_ch(cb.b, cs.b)); }
        case 5u: { return max(cb + cs - vec3<f32>(1.0), vec3<f32>(0.0)); }
        case 6u: { return max(cb, cs); }
        case 7u: { return cb + cs - cb * cs; }
        case 8u: { return vec3<f32>(color_dodge_ch(cb.r, cs.r), color_dodge_ch(cb.g, cs.g), color_dodge_ch(cb.b, cs.b)); }
        case 9u: { return min(cb + cs, vec3<f32>(1.0)); }
        case 10u: { return vec3<f32>(hard_light_ch(cs.r, cb.r), hard_light_ch(cs.g, cb.g), hard_light_ch(cs.b, cb.b)); }
        case 11u: { return vec3<f32>(soft_light_ch(cb.r, cs.r), soft_light_ch(cb.g, cs.g), soft_light_ch(cb.b, cs.b)); }
        case 12u: { return vec3<f32>(hard_light_ch(cb.r, cs.r), hard_light_ch(cb.g, cs.g), hard_light_ch(cb.b, cs.b)); }
        case 13u: { return abs(cb - cs); }
        case 14u: { return cb + cs - 2.0 * cb * cs; }
        case 15u: { return set_lum(set_sat(cs, sat(cb)), lum(cb)); }
        case 16u: { return set_lum(set_sat(cb, sat(cs)), lum(cb)); }
        case 17u: { return set_lum(cs, lum(cb)); }
        case 18u: { return set_lum(cb, lum(cs)); }
        default: { return cs; }
    }
}

fn source(root: vec2<f32>) -> vec4<f32> {
    var s = load_in(src_tex, root, p.src_origin, p.src_size);
    if (p.has_tint != 0u) {
        let a = s.a * p.tint.a;
        s = vec4<f32>(p.tint.rgb * a, a);
    }
    if (p.aux_mode != 0u) {
        let aux = load_in(aux_tex, root, p.aux_origin, p.aux_size);
        if (p.aux_mode == 1u) {
            s = s * aux.a;
        } else if (p.aux_mode == 2u) {
            s = s * (1.0 - aux.a);
        } else {
            let m = dot(aux.rgb, LUMINANCE_MASK_COEFFS);
            s = s * m;
        }
    }
    return s * p.opacity;
}

// over / erase: plain premultiplied output, blended by the pipeline
@fragment
fn fs_plain(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let root = floor(frag.xy + g.origin);
    return source(root) * clip_cov(frag.xy);
}

// blend mode against the destination snapshot (pipeline blending disabled)
@fragment
fn fs_blend(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let root = floor(frag.xy + g.origin);
    let s = source(root);
    let dst = load_in(snap_tex, root, p.snap_origin, p.snap_size);
    let cov = clip_cov(frag.xy);
    if (s.a <= 0.0 || cov <= 0.0) {
        return dst;
    }
    let ab = dst.a;
    let sa = s.a;
    var cb = vec3<f32>(0.0);
    if (ab > 0.0) {
        cb = min(dst.rgb / ab, vec3<f32>(1.0));
    }
    let cs = min(s.rgb / sa, vec3<f32>(1.0));
    let b = blend_fn(p.blend, cb, cs);
    var co = s.rgb * (1.0 - ab) + dst.rgb * (1.0 - sa) + sa * ab * b;
    var ao = sa + ab - sa * ab;
    co = min(co, vec3<f32>(ao));
    let res = vec4<f32>(co, ao);
    return mix(dst, res, cov);
}

// replace the destination by the source where the mask (aux) covers it
@fragment
fn fs_replace(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let root = floor(frag.xy + g.origin);
    var s = load_in(src_tex, root, p.src_origin, p.src_size);
    let dst = load_in(snap_tex, root, p.snap_origin, p.snap_size);
    let m = load_in(aux_tex, root, p.aux_origin, p.aux_size).a * clip_cov(frag.xy);
    return dst * (1.0 - m) + s * m;
}
