// Mesh draws: solid, gradients, images and noise, clipped by a coverage mask.

struct Globals {
    origin: vec2<f32>, // target top-left in root pixels
    size: vec2<f32>,   // target size in pixels
};

struct ClipParams {
    origin: vec2<f32>,
    size: vec2<f32>,
    enabled: f32,
    p0: f32,
    p1: f32,
    p2: f32,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var ramp_tex: texture_2d<f32>;
@group(0) @binding(2) var samp_clamp: sampler;
@group(0) @binding(3) var samp_repeat: sampler;

@group(1) @binding(0) var<uniform> clip: ClipParams;
@group(1) @binding(1) var clip_tex: texture_2d<f32>;

@group(2) @binding(0) var img: texture_2d<f32>;

const RAMP_W: f32 = 512.0;
const RAMP_ROWS: f32 = 1024.0;
const TAU: f32 = 6.283185307179586;

struct VIn {
    @location(0) pos: vec2<f32>,
    @location(1) m: vec4<f32>,
    @location(2) t: vec4<f32>,
    @location(3) c0: vec4<f32>,
    @location(4) c1: vec4<f32>,
    @location(5) c2: vec4<f32>,
    @location(6) q: vec4<u32>,
    @location(7) q2: vec4<i32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) t: vec4<f32>,
    @location(2) @interpolate(flat) c0: vec4<f32>,
    @location(3) @interpolate(flat) c1: vec4<f32>,
    @location(4) @interpolate(flat) c2: vec4<f32>,
    @location(5) @interpolate(flat) q: vec4<u32>,
    @location(6) @interpolate(flat) q2: vec4<i32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let dev = vec2<f32>(v.m.x * v.pos.x + v.m.z * v.pos.y + v.t.x, v.m.y * v.pos.x + v.m.w * v.pos.y + v.t.y) - g.origin;
    o.pos = vec4<f32>(dev.x / g.size.x * 2.0 - 1.0, 1.0 - dev.y / g.size.y * 2.0, 0.0, 1.0);
    o.local = v.pos;
    o.t = v.t;
    o.c0 = v.c0;
    o.c1 = v.c1;
    o.c2 = v.c2;
    o.q = v.q;
    o.q2 = v.q2;
    return o;
}

fn clip_cov(frag: vec2<f32>) -> f32 {
    if (clip.enabled < 0.5) {
        return 1.0;
    }
    let p = vec2<i32>(floor(frag + g.origin - clip.origin));
    if (p.x < 0 || p.y < 0 || f32(p.x) >= clip.size.x || f32(p.y) >= clip.size.y) {
        return 0.0;
    }
    return textureLoad(clip_tex, p, 0).a;
}

fn ramp(t: f32, row: u32) -> vec4<f32> {
    let u = (clamp(t, 0.0, 1.0) * (RAMP_W - 1.0) + 0.5) / RAMP_W;
    return textureSampleLevel(ramp_tex, samp_clamp, vec2<f32>(u, (f32(row) + 0.5) / RAMP_ROWS), 0.0);
}

fn mix32(h0: u32) -> u32 {
    var h = h0;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    return h ^ (h >> 16u);
}

fn hash2(seed: u32, x: i32, y: i32) -> u32 {
    let a = bitcast<u32>(x) * 0x85ebca6bu;
    let b = bitcast<u32>(y) * 0xc2b2ae35u;
    let rb = (b << 13u) | (b >> 19u);
    return mix32((seed * 0x9e3779b1u) ^ mix32(a ^ rb));
}

fn unit(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let h6 = (h - floor(h)) * 6.0;
    let i = i32(floor(h6));
    let f = h6 - floor(h6);
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    switch (i) {
        case 0: { return vec3<f32>(v, t, p); }
        case 1: { return vec3<f32>(q, v, p); }
        case 2: { return vec3<f32>(p, v, t); }
        case 3: { return vec3<f32>(p, q, v); }
        case 4: { return vec3<f32>(t, p, v); }
        default: { return vec3<f32>(v, p, q); }
    }
}

// ---- analytic blurred rounded rectangle (Evan Wallace's "fast rounded rectangle shadows")

fn erf2(x: vec2<f32>) -> vec2<f32> {
    let s = sign(x);
    let a = abs(x);
    var y = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    y = y * y;
    return s - s / (y * y);
}

fn gauss(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (2.5066282746310002 * sigma);
}

fn shadow_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2<f32>) -> f32 {
    let delta = min(half_size.y - corner - abs(y), 0.0);
    let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf2((x + vec2<f32>(-curved, curved)) * (0.7071067811865476 / sigma));
    return integral.y - integral.x;
}

fn rbox_shadow(lower: vec2<f32>, upper: vec2<f32>, pt: vec2<f32>, sigma: f32, corner: f32) -> f32 {
    let center = (lower + upper) * 0.5;
    let half_size = (upper - lower) * 0.5;
    let p = pt - center;
    let low = p.y - half_size.y;
    let high = p.y + half_size.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) / 8.0;
    var y = start + step * 0.5;
    var value = 0.0;
    for (var k = 0; k < 8; k = k + 1) {
        value = value + shadow_x(p.x, p.y - y, sigma, corner, half_size) * gauss(y, sigma) * step;
        y = y + step;
    }
    return value;
}

// exact-ish pixel coverage of a rounded rectangle
fn rrect_cov(pt: vec2<f32>, lower: vec2<f32>, upper: vec2<f32>, r: f32) -> f32 {
    let center = (lower + upper) * 0.5;
    let half_size = (upper - lower) * 0.5;
    let q = abs(pt - center) - (half_size - vec2<f32>(r));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    return clamp(0.5 - d, 0.0, 1.0);
}

// straight-colour paint result for kinds that need no texture
fn paint(i: VOut, frag: vec2<f32>) -> vec4<f32> {
    let kind = i.q.x;
    if (kind == 7u) {
        let pt = frag + g.origin;
        var a = rbox_shadow(i.c1.xy, i.c1.zw, pt, i.t.z, i.t.w) * i.c0.a;
        if (i.q.z != 0u) {
            a = a * (1.0 - rrect_cov(pt, i.c2.xy, i.c2.zw, bitcast<f32>(i.q.w)));
        }
        return vec4<f32>(i.c0.rgb * a, a);
    }
    if (kind == 0u) {
        return vec4<f32>(i.c0.rgb * i.c0.a, i.c0.a);
    }
    if (kind == 6u) {
        let px = vec2<i32>(floor((frag + g.origin) / f32(max(i.q2.z, 1))));
        let gx = px.x + i.q2.x;
        let gy = px.y + i.q2.y;
        let cell = i.c2.x;
        let cx = i32(floor(f32(gx) / cell));
        let cy = i32(floor(f32(gy) / cell));
        let h = hash2(i.q.y, cx, cy);
        if (unit(h) >= i.c2.y) {
            return vec4<f32>(0.0);
        }
        let h2 = mix32(h ^ 0x6a09e667u);
        var c = i.c0;
        if (i.q.z == 1u && (h2 & 1u) != 0u) {
            c = i.c1;
        }
        if (i.q.z == 2u) {
            c = vec4<f32>(hsv_to_rgb(unit(h2), 1.0, 1.0), i.c0.a);
        }
        return vec4<f32>(c.rgb * c.a, c.a);
    }
    let u = vec2<f32>(i.c1.x * i.local.x + i.c1.z * i.local.y + i.c2.x, i.c1.y * i.local.x + i.c1.w * i.local.y + i.c2.y);
    var t = u.x;
    if (kind == 2u) {
        t = length(u);
    } else if (kind == 3u) {
        t = atan2(u.y, u.x) / TAU;
        if (t < 0.0) {
            t = t + 1.0;
        }
    } else if (kind == 4u) {
        t = min(abs(u.x) + abs(u.y), 1.0);
    }
    return ramp(t, i.q.w) * i.t.z;
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let cov = clip_cov(i.pos.xy);
    return paint(i, i.pos.xy) * cov;
}

const ADJUST_GAMMA: f32 = 2.2;
const INV_ADJUST_GAMMA: f32 = 0.45454547;
const LUMA_WEIGHTS: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const HIGHLIGHT_SHADOW_THRESHOLD: f32 = 0.5;
const CONTRAST_LIMIT: f32 = 0.3;
const NEG_EXPOSURE_RESAT_PIVOT: f32 = 0.2;
const POS_SAT_MAX_SCALE: f32 = 2.0;

fn img_smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

fn img_shadows_highlights_offset(lin: vec3<f32>, shadows: f32, highlights: f32) -> f32 {
    let h = dot(lin, LUMA_WEIGHTS) - pow(HIGHLIGHT_SHADOW_THRESHOLD, ADJUST_GAMMA);
    let weight = select(-0.999 * shadows, highlights, h > 0.0);
    return 0.75 * h / sqrt(1.0 + 4.0 * h * h) * weight;
}

fn img_lab_tint_temp_contrast(lin: vec3<f32>, temperature: f32, tint: f32, contrast: f32) -> vec3<f32> {
    let x_xyz = (0.4124564 * lin.r + 0.3575761 * lin.g + 0.1804375 * lin.b) / 0.95047;
    let y_xyz = 0.2126729 * lin.r + 0.7151522 * lin.g + 0.0721750 * lin.b;
    let z_xyz = (0.0193339 * lin.r + 0.1191920 * lin.g + 0.9503041 * lin.b) / 1.08883;

    let f = pow(max(vec3<f32>(x_xyz, y_xyz, z_xyz), vec3<f32>(0.0)), vec3<f32>(1.0 / 3.0));
    var l_lab = 116.0 * f.y - 16.0;
    let a_lab = 500.0 * (f.x - f.y) + 30.0 * tint;
    let b_lab = 200.0 * (f.y - f.z) + 30.0 * temperature;

    let c = clamp(contrast, -CONTRAST_LIMIT, CONTRAST_LIMIT);
    let l_c = HIGHLIGHT_SHADOW_THRESHOLD * 100.0;
    if (c < 0.0) {
        let low = 50.0 + 0.2 * (l_lab - l_c);
        l_lab = mix(low, l_lab, c + 1.0);
    } else {
        let s = img_smootherstep(l_c - 30.0, l_c + 30.0, l_lab) * 100.0;
        l_lab = mix(l_lab, s, c);
    }

    let fy2 = (l_lab + 16.0) / 116.0;
    let fx2 = a_lab / 500.0 + fy2;
    let fz2 = fy2 - b_lab / 200.0;

    let f3 = vec3<f32>(fx2, fy2, fz2) * vec3<f32>(fx2, fy2, fz2) * vec3<f32>(fx2, fy2, fz2);
    let x2 = f3.x * 0.95047;
    let y2 = f3.y;
    let z2 = f3.z * 1.08883;

    let out_lin = vec3<f32>(
        3.2404542 * x2 - 1.5371385 * y2 - 0.4985314 * z2,
        -0.9692660 * x2 + 1.8760108 * y2 + 0.0415560 * z2,
        0.0556434 * x2 - 0.2040259 * y2 + 1.0572252 * z2,
    );
    return clamp(out_lin, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn img_exposure(color: vec3<f32>, j: f32) -> vec3<f32> {
    let k = pow(10.0, 2.0 * j) - 1.0;
    let l = 0.75;
    var m = 1.0 / (1.0 - l) - 1.0;
    var n = dot(color, LUMA_WEIGHTS);
    let j_neg = min(0.0, j);
    m = m * (1.0 + n * n * j_neg / (NEG_EXPOSURE_RESAT_PIVOT - j_neg));
    var c = color + (vec3<f32>(n) - color) * l;
    if (j < 0.0) {
        let s = 1.0 + j / 16.0;
        c = c * s;
        n = n * s;
    }
    c = c * (k + 1.0) / (c * k + vec3<f32>(1.0));
    n = n * (k + 1.0) / (n * k + 1.0);
    c = c + (c - vec3<f32>(n)) * m;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn img_saturation(color: vec3<f32>, p: f32) -> vec3<f32> {
    let n = dot(color, LUMA_WEIGHTS);
    if (p <= 0.0) {
        return clamp(color + (color - vec3<f32>(n)) * p, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let min_c = min(min(color.r, color.g), color.b);
    let max_c = max(max(color.r, color.g), color.b);
    let q = n / (n - min_c + 1e-6);
    let r = (1.0 - n) / (max_c - n + 1e-6);
    let scale = min(mix(1.0, min(q, r), p), POS_SAT_MAX_SCALE);
    return clamp(vec3<f32>(n) + (color - vec3<f32>(n)) * scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn adjust_image(rgb: vec3<f32>, c0: vec4<f32>, tint_high: vec2<f32>, shadows: f32) -> vec3<f32> {
    let exposure = select(0.0, clamp(c0.x, -1.0, 1.0), abs(c0.x) > 1e-6);
    let contrast = select(0.0, clamp(c0.y, -CONTRAST_LIMIT, CONTRAST_LIMIT), abs(c0.y) > 1e-6);
    let vibrance = select(0.0, clamp(c0.z, -1.0, 1.0), abs(c0.z) > 1e-6);
    let temperature = select(0.0, clamp(c0.w, -1.0, 1.0), abs(c0.w) > 1e-6);
    let tint = select(0.0, clamp(tint_high.x, -1.0, 1.0), abs(tint_high.x) > 1e-6);
    let highlights = select(0.0, clamp(tint_high.y, -1.0, 1.0), abs(tint_high.y) > 1e-6);
    let sh = select(0.0, clamp(shadows, -1.0, 1.0), abs(shadows) > 1e-6);

    var lin = pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(ADJUST_GAMMA));
    let ai = img_shadows_highlights_offset(lin, sh, highlights);
    if (tint != 0.0 || temperature != 0.0 || contrast != 0.0) {
        lin = img_lab_tint_temp_contrast(lin, temperature, tint, contrast);
    }
    let j = exposure + ai;
    if (abs(j) > 1e-6) {
        lin = img_exposure(lin, j);
    }
    if (vibrance != 0.0) {
        lin = img_saturation(lin, vibrance);
    }
    return clamp(pow(max(lin, vec3<f32>(0.0)), vec3<f32>(INV_ADJUST_GAMMA)), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_image(i: VOut) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(i.c1.x * i.local.x + i.c1.z * i.local.y + i.c2.x, i.c1.y * i.local.x + i.c1.w * i.local.y + i.c2.y);
    // One texture may only be paired with one sampler on the GL backend, so tile
    // mode wraps the coordinates here instead of using a second, repeating sampler.
    // Derivatives come from the unwrapped uv so mip selection has no seams at tile edges.
    let dx = dpdx(uv);
    let dy = dpdy(uv);
    var suv = uv;
    if (i.q.z == 2u) {
        suv = fract(uv);
    }
    var c = textureSampleGrad(img, samp_clamp, suv, dx, dy);
    if (i.q.z == 1u && (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0)) {
        c = vec4<f32>(0.0);
    }
    if (i.q.y != 0u && c.a > 0.0) {
        let straight = clamp(c.rgb / c.a, vec3<f32>(0.0), vec3<f32>(1.0));
        let adj = adjust_image(straight, i.c0, i.c2.zw, i.t.w);
        c = vec4<f32>(adj * c.a, c.a);
    }
    return c * i.t.z * clip_cov(i.pos.xy);
}
