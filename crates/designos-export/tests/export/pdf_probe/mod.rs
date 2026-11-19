//! Reads back the PDF that `export_pdf` writes: page boxes, text runs, embedded page images, and a small
//! content-stream painter built on tiny-skia, so a vector page can be compared pixel for pixel with what
//! the CPU renderer draws for the same frame.
//!
//! This is test support only. It understands the operators the exporter emits (paths, fills, strokes,
//! clips, graphics state, axial and radial shadings) and ignores text operators, whose glyphs are drawn
//! as outlines by the exporter and shown by the renderer from the same outlines.

use std::collections::BTreeMap;
use std::io::Read;
use tiny_skia as sk;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f32),
    Name(String),
    Str(Vec<u8>),
    Arr(Vec<Val>),
    Dict(BTreeMap<String, Val>),
    Ref(usize),
    Bool(bool),
    Op(String),
}

impl Val {
    pub fn num(&self) -> f32 {
        match self {
            Val::Num(n) => *n,
            other => panic!("expected a number, got {other:?}"),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Val::Name(n) => n,
            other => panic!("expected a name, got {other:?}"),
        }
    }
    pub fn arr(&self) -> &[Val] {
        match self {
            Val::Arr(a) => a,
            other => panic!("expected an array, got {other:?}"),
        }
    }
    pub fn dict(&self) -> &BTreeMap<String, Val> {
        match self {
            Val::Dict(d) => d,
            other => panic!("expected a dictionary, got {other:?}"),
        }
    }
    pub fn nums(&self) -> Vec<f32> {
        self.arr().iter().map(Val::num).collect()
    }
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\n' | b'\r' | b'\t' | 0x0C | 0)
}

fn is_delim(c: u8) -> bool {
    matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'/' | b'%')
}

impl<'a> Lexer<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }

    fn skip(&mut self) {
        loop {
            while self.i < self.b.len() && is_ws(self.b[self.i]) {
                self.i += 1;
            }
            if self.b.get(self.i) == Some(&b'%') {
                while self.i < self.b.len() && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn word(&mut self) -> &'a [u8] {
        let start = self.i;
        while self.i < self.b.len() && !is_ws(self.b[self.i]) && !is_delim(self.b[self.i]) {
            self.i += 1;
        }
        &self.b[start..self.i]
    }

    fn string(&mut self) -> Vec<u8> {
        self.i += 1;
        let mut depth = 1;
        let mut out = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.b.get(self.i) {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }

    fn next(&mut self) -> Option<Val> {
        self.skip();
        let c = *self.b.get(self.i)?;
        match c {
            b'/' => {
                self.i += 1;
                Some(Val::Name(String::from_utf8_lossy(self.word()).into_owned()))
            }
            b'(' => Some(Val::Str(self.string())),
            b'<' if self.b.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                let mut d = BTreeMap::new();
                loop {
                    self.skip();
                    if self.b[self.i..].starts_with(b">>") {
                        self.i += 2;
                        break;
                    }
                    let Some(Val::Name(k)) = self.next() else { panic!("dictionary key expected near byte {}", self.i) };
                    let v = self.next().expect("dictionary value");
                    d.insert(k, v);
                }
                Some(Val::Dict(d))
            }
            b'<' => {
                self.i += 1;
                let mut hex = Vec::new();
                while self.b[self.i] != b'>' {
                    if !is_ws(self.b[self.i]) {
                        hex.push(self.b[self.i]);
                    }
                    self.i += 1;
                }
                self.i += 1;
                if hex.len() % 2 == 1 {
                    hex.push(b'0');
                }
                let bytes = hex.chunks(2).map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap()).collect();
                Some(Val::Str(bytes))
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.skip();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        break;
                    }
                    items.push(self.next().expect("array element"));
                }
                Some(Val::Arr(items))
            }
            b']' | b'>' | b')' => None,
            _ => {
                let w = self.word();
                let text = std::str::from_utf8(w).unwrap();
                if let Ok(n) = text.parse::<f32>() {
                    if text.bytes().all(|b| b.is_ascii_digit()) && self.reference_follows() {
                        return Some(Val::Ref(n as usize));
                    }
                    return Some(Val::Num(n));
                }
                match text {
                    "true" => Some(Val::Bool(true)),
                    "false" => Some(Val::Bool(false)),
                    other => Some(Val::Op(other.to_string())),
                }
            }
        }
    }

    /// After an object number: consumes `G R` and returns true when it completes an indirect reference;
    /// leaves the position untouched otherwise.
    fn reference_follows(&mut self) -> bool {
        let save = self.i;
        self.skip();
        let generation = self.word();
        if !generation.is_empty() && generation.iter().all(u8::is_ascii_digit) {
            self.skip();
            if self.word() == b"R" {
                return true;
            }
        }
        self.i = save;
        false
    }
}

/// Parses all values of a content stream or object body.
pub fn tokens(bytes: &[u8]) -> Vec<Val> {
    let mut lx = Lexer::new(bytes);
    let mut out = Vec::new();
    while let Some(v) = lx.next() {
        out.push(v);
    }
    out
}

/// One content-stream operator with its operands.
#[derive(Clone, Debug)]
pub struct Op {
    pub name: String,
    pub args: Vec<Val>,
}

pub fn ops(content: &[u8]) -> Vec<Op> {
    let mut out = Vec::new();
    let mut args = Vec::new();
    for t in tokens(content) {
        match t {
            Val::Op(name) => out.push(Op { name, args: std::mem::take(&mut args) }),
            other => args.push(other),
        }
    }
    out
}

pub struct Pdf {
    bytes: Vec<u8>,
    offsets: BTreeMap<usize, usize>,
    root: usize,
}

pub struct Page {
    /// MediaBox size in PDF points.
    pub width: f32,
    pub height: f32,
    pub resources: BTreeMap<String, Val>,
    pub content: Vec<u8>,
}

/// A page image (the raster fallback), 8-bit RGB.
pub struct RasterPage {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl Pdf {
    pub fn parse(bytes: &[u8]) -> Self {
        let marker = b"startxref\n";
        let at = bytes.windows(marker.len()).rposition(|w| w == marker).expect("startxref") + marker.len();
        let end = bytes[at..].iter().position(|b| *b == b'\n').unwrap() + at;
        let xref: usize = std::str::from_utf8(&bytes[at..end]).unwrap().parse().unwrap();
        let tail = std::str::from_utf8(&bytes[xref..]).expect("xref and trailer are ASCII");
        let mut lines = tail.lines();
        assert_eq!(lines.next(), Some("xref"));
        let size: usize = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        assert_eq!(lines.next(), Some("0000000000 65535 f "));
        let mut offsets = BTreeMap::new();
        for number in 1..size {
            let line = lines.next().unwrap();
            offsets.insert(number, line[..10].parse().unwrap());
        }
        let root = tail.split("/Root ").nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
        Self { bytes: bytes.to_vec(), offsets, root }
    }

    /// The object's value and, for stream objects, its raw stream bytes.
    pub fn object(&self, number: usize) -> (Val, Option<Vec<u8>>) {
        let start = self.offsets[&number];
        let header = format!("{number} 0 obj\n");
        assert!(self.bytes[start..].starts_with(header.as_bytes()), "object {number} is not at its xref offset");
        let body = start + header.len();
        let mut lx = Lexer::new(&self.bytes[body..]);
        let value = lx.next().expect("object value");
        lx.skip();
        let stream = if self.bytes[body + lx.i..].starts_with(b"stream\n") {
            let from = body + lx.i + "stream\n".len();
            let length = value.dict()["Length"].num() as usize;
            Some(self.bytes[from..from + length].to_vec())
        } else {
            None
        };
        (value, stream)
    }

    pub fn deref(&self, v: &Val) -> Val {
        match v {
            Val::Ref(n) => self.object(*n).0,
            other => other.clone(),
        }
    }

    fn get(&self, dict: &BTreeMap<String, Val>, key: &str) -> Option<Val> {
        dict.get(key).map(|v| self.deref(v))
    }

    pub fn pages(&self) -> Vec<Page> {
        let catalog = self.object(self.root).0;
        let pages = self.get(catalog.dict(), "Pages").unwrap();
        let kids = self.get(pages.dict(), "Kids").unwrap();
        kids.arr()
            .iter()
            .map(|kid| {
                let page = self.deref(kid);
                let d = page.dict();
                let mb = d["MediaBox"].nums();
                let resources = self.get(d, "Resources").map(|r| r.dict().clone()).unwrap_or_default();
                let content = match &d["Contents"] {
                    Val::Ref(n) => self.object(*n).1.expect("content stream"),
                    Val::Arr(refs) => refs.iter().flat_map(|r| if let Val::Ref(n) = r { self.object(*n).1.unwrap() } else { Vec::new() }).collect(),
                    other => panic!("unexpected /Contents {other:?}"),
                };
                Page { width: mb[2] - mb[0], height: mb[3] - mb[1], resources, content }
            })
            .collect()
    }

    /// The page's embedded raster image, if it has one.
    pub fn page_image(&self, page: &Page) -> Option<RasterPage> {
        let xobjects = self.get(&page.resources, "XObject")?;
        for v in xobjects.dict().values() {
            let Val::Ref(n) = v else { continue };
            let (obj, stream) = self.object(*n);
            let d = obj.dict();
            if d.get("Subtype") != Some(&Val::Name("Image".into())) {
                continue;
            }
            let (width, height) = (d["Width"].num() as usize, d["Height"].num() as usize);
            let mut rgb = Vec::new();
            flate2::read::ZlibDecoder::new(&stream?[..]).read_to_end(&mut rgb).expect("image stream inflates");
            assert_eq!(rgb.len(), width * height * 3, "RGB8 image");
            return Some(RasterPage { width, height, rgb });
        }
        None
    }

    fn font_decoder(&self, page: &Page, font: &str) -> Decoder {
        let fonts = self.get(&page.resources, "Font").expect("page has fonts");
        let font = self.get(fonts.dict(), font).expect("font resource");
        let mut map = BTreeMap::new();
        let mut width = 1;
        if let Some(Val::Ref(n)) = font.dict().get("ToUnicode") {
            let cmap = self.object(*n).1.expect("ToUnicode stream");
            let toks = tokens(&cmap);
            let mut i = 0;
            while i < toks.len() {
                match &toks[i] {
                    Val::Op(op) if op == "beginbfchar" => {
                        i += 1;
                        while let (Some(Val::Str(src)), Some(Val::Str(dst))) = (toks.get(i), toks.get(i + 1)) {
                            width = src.len();
                            map.insert(be(src), utf16(dst));
                            i += 2;
                        }
                    }
                    Val::Op(op) if op == "beginbfrange" => {
                        i += 1;
                        while let (Some(Val::Str(lo)), Some(Val::Str(hi)), Some(Val::Str(dst))) = (toks.get(i), toks.get(i + 1), toks.get(i + 2)) {
                            width = lo.len();
                            let first = utf16(dst).chars().next().unwrap() as u32;
                            for k in 0..=(be(hi) - be(lo)) {
                                map.insert(be(lo) + k, char::from_u32(first + k).unwrap().to_string());
                            }
                            i += 3;
                        }
                    }
                    _ => i += 1,
                }
            }
        }
        Decoder { map, width }
    }

    /// Text shown by the `Tj` / `TJ` operators of a page, one string per run, decoded the way a text
    /// extractor does (through the font's ToUnicode map, else WinAnsi).
    pub fn text_runs(&self, page: &Page) -> Vec<String> {
        let mut font = String::new();
        let mut out = Vec::new();
        for op in ops(&page.content) {
            match op.name.as_str() {
                "Tf" => font = op.args[0].name().to_string(),
                "Tj" => {
                    if let Val::Str(s) = &op.args[0] {
                        out.push(self.font_decoder(page, &font).decode(s));
                    }
                }
                "TJ" => {
                    let dec = self.font_decoder(page, &font);
                    let mut run = String::new();
                    for item in op.args[0].arr() {
                        if let Val::Str(s) = item {
                            run.push_str(&dec.decode(s));
                        }
                    }
                    out.push(run);
                }
                _ => {}
            }
        }
        out
    }
}

struct Decoder {
    map: BTreeMap<u32, String>,
    width: usize,
}

const WIN_ANSI_80: [char; 32] = [
    '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}',
    '\u{FFFD}', '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}',
    '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
];

impl Decoder {
    fn decode(&self, bytes: &[u8]) -> String {
        if self.map.is_empty() {
            return bytes
                .iter()
                .map(|&b| match b {
                    0x80..=0x9F => WIN_ANSI_80[usize::from(b - 0x80)],
                    other => char::from(other),
                })
                .collect();
        }
        bytes.chunks(self.width).map(|c| self.map.get(&be(c)).cloned().unwrap_or_else(|| "\u{FFFD}".into())).collect()
    }
}

fn be(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0, |acc, b| acc * 256 + u32::from(*b))
}

fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks(2).map(|p| u16::from_be_bytes([p[0], *p.get(1).unwrap_or(&0)])).collect();
    String::from_utf16_lossy(&units)
}

// ---------------------------------------------------------------------------------------------
// Painting

#[derive(Clone)]
struct State {
    ctm: sk::Transform,
    clip: Option<sk::Mask>,
    fill: [f32; 3],
    stroke: [f32; 3],
    fill_alpha: f32,
    stroke_alpha: f32,
    width: f32,
    cap: sk::LineCap,
    join: sk::LineJoin,
    miter: f32,
    dash: Vec<f32>,
}

fn rgb_paint(rgb: [f32; 3], alpha: f32) -> sk::Paint<'static> {
    let mut p = sk::Paint { anti_alias: true, ..Default::default() };
    p.set_color(sk::Color::from_rgba(rgb[0].clamp(0.0, 1.0), rgb[1].clamp(0.0, 1.0), rgb[2].clamp(0.0, 1.0), alpha.clamp(0.0, 1.0)).unwrap());
    p
}

fn intersect(mask: &mut sk::Mask, other: &sk::Mask) {
    for (a, b) in mask.data_mut().iter_mut().zip(other.data()) {
        *a = ((u32::from(*a) * u32::from(*b) + 127) / 255) as u8;
    }
}

/// A PDF function (type 2 exponential interpolation or type 3 stitching), parsed once.
enum Function {
    Exponential { c0: Vec<f32>, c1: Vec<f32>, n: f32 },
    Stitching { domain: [f32; 2], bounds: Vec<f32>, encode: Vec<f32>, funcs: Vec<Function> },
}

impl Function {
    fn parse(f: &Val) -> Self {
        let d = f.dict();
        match d["FunctionType"].num() as u32 {
            2 => Function::Exponential {
                c0: d.get("C0").map(Val::nums).unwrap_or_else(|| vec![0.0]),
                c1: d.get("C1").map(Val::nums).unwrap_or_else(|| vec![1.0]),
                n: d.get("N").map(Val::num).unwrap_or(1.0),
            },
            3 => {
                let domain = d["Domain"].nums();
                Function::Stitching {
                    domain: [domain[0], domain[1]],
                    bounds: d["Bounds"].nums(),
                    encode: d["Encode"].nums(),
                    funcs: d["Functions"].arr().iter().map(Function::parse).collect(),
                }
            }
            other => panic!("unsupported function type {other}"),
        }
    }

    fn eval(&self, t: f32) -> [f32; 3] {
        match self {
            Function::Exponential { c0, c1, n } => {
                let k = t.clamp(0.0, 1.0).powf(*n);
                let mut out = [0.0; 3];
                for (i, o) in out.iter_mut().enumerate() {
                    *o = c0[i] + k * (c1[i] - c0[i]);
                }
                out
            }
            Function::Stitching { domain, bounds, encode, funcs } => {
                let t = t.clamp(domain[0], domain[1]);
                let k = bounds.iter().take_while(|b| t >= **b).count();
                let lo = if k == 0 { domain[0] } else { bounds[k - 1] };
                let hi = if k == bounds.len() { domain[1] } else { bounds[k] };
                let x = if hi > lo { encode[2 * k] + (t - lo) / (hi - lo) * (encode[2 * k + 1] - encode[2 * k]) } else { encode[2 * k] };
                funcs[k].eval(x)
            }
        }
    }
}

struct Painter<'a> {
    pdf: &'a Pdf,
    page: &'a Page,
    pm: sk::Pixmap,
    st: State,
    stack: Vec<State>,
    path: sk::PathBuilder,
    pending_clip: Option<sk::FillRule>,
}

impl Painter<'_> {
    fn fill_rule(star: bool) -> sk::FillRule {
        if star {
            sk::FillRule::EvenOdd
        } else {
            sk::FillRule::Winding
        }
    }

    fn take_path(&mut self) -> Option<sk::Path> {
        std::mem::take(&mut self.path).finish()
    }

    fn apply_clip(&mut self, path: &sk::Path, rule: sk::FillRule) {
        let mut mask = sk::Mask::new(self.pm.width(), self.pm.height()).unwrap();
        mask.fill_path(path, rule, true, self.st.ctm);
        if let Some(cur) = &self.st.clip {
            intersect(&mut mask, cur);
        }
        self.st.clip = Some(mask);
    }

    fn finish_path(&mut self) {
        let clip_rule = self.pending_clip.take();
        let path = self.take_path();
        if let (Some(rule), Some(path)) = (clip_rule, path) {
            self.apply_clip(&path, rule);
        }
    }

    fn paint_fill(&mut self, star: bool) {
        let clip_rule = self.pending_clip.take();
        let Some(path) = self.take_path() else { return };
        let paint = rgb_paint(self.st.fill, self.st.fill_alpha);
        self.pm.fill_path(&path, &paint, Self::fill_rule(star), self.st.ctm, self.st.clip.as_ref());
        if let Some(rule) = clip_rule {
            self.apply_clip(&path, rule);
        }
    }

    fn paint_stroke(&mut self) {
        self.pending_clip = None;
        let Some(path) = self.take_path() else { return };
        let paint = rgb_paint(self.st.stroke, self.st.stroke_alpha);
        let mut stroke = sk::Stroke { width: self.st.width, miter_limit: self.st.miter, line_cap: self.st.cap, line_join: self.st.join, dash: None };
        if !self.st.dash.is_empty() {
            let mut array = self.st.dash.clone();
            if array.len() % 2 == 1 {
                array.extend_from_within(..);
            }
            stroke.dash = sk::StrokeDash::new(array, 0.0);
        }
        self.pm.stroke_path(&path, &paint, &stroke, self.st.ctm, self.st.clip.as_ref());
    }

    fn paint_shading(&mut self, name: &str) {
        let shadings = self.pdf.get(&self.page.resources, "Shading").expect("page has shadings");
        let sh = self.pdf.deref(&shadings.dict()[name]);
        let d = sh.dict();
        let kind = d["ShadingType"].num() as u32;
        let coords = d["Coords"].nums();
        let extend: Vec<bool> = d.get("Extend").map(|e| e.arr().iter().map(|v| *v == Val::Bool(true)).collect()).unwrap_or_else(|| vec![false, false]);
        let func = Function::parse(&self.pdf.deref(&d["Function"]));
        let inv = self.st.ctm.invert().expect("invertible CTM at sh");
        let (w, h) = (self.pm.width(), self.pm.height());
        let mut layer = sk::Pixmap::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                if self.st.clip.as_ref().is_some_and(|m| m.data()[(y * w + x) as usize] == 0) {
                    continue;
                }
                let (dx, dy) = (x as f32 + 0.5, y as f32 + 0.5);
                let (ux, uy) = (inv.sx * dx + inv.kx * dy + inv.tx, inv.ky * dx + inv.sy * dy + inv.ty);
                let t = match kind {
                    2 => {
                        let (ax, ay) = (coords[2] - coords[0], coords[3] - coords[1]);
                        ((ux - coords[0]) * ax + (uy - coords[1]) * ay) / (ax * ax + ay * ay).max(1e-12)
                    }
                    3 => {
                        assert!(
                            (coords[0] - coords[3]).abs() < 1e-4 && (coords[1] - coords[4]).abs() < 1e-4 && coords[2].abs() < 1e-4,
                            "concentric radial shading"
                        );
                        (ux - coords[0]).hypot(uy - coords[1]) / coords[5].max(1e-9)
                    }
                    other => panic!("unsupported shading type {other}"),
                };
                if (t < 0.0 && !extend[0]) || (t > 1.0 && !extend[1]) {
                    continue;
                }
                let c = func.eval(t.clamp(0.0, 1.0));
                let px = [c[0], c[1], c[2]].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
                let i = ((y * w + x) * 4) as usize;
                layer.data_mut()[i..i + 4].copy_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
        let paint = sk::PixmapPaint { opacity: self.st.fill_alpha, ..Default::default() };
        self.pm.draw_pixmap(0, 0, layer.as_ref(), &paint, sk::Transform::identity(), self.st.clip.as_ref());
    }

    fn run(&mut self, op: &Op) {
        let a = &op.args;
        match op.name.as_str() {
            "q" => self.stack.push(self.st.clone()),
            "Q" => self.st = self.stack.pop().expect("balanced q/Q"),
            "cm" => {
                let m = a.iter().map(Val::num).collect::<Vec<_>>();
                self.st.ctm = self.st.ctm.pre_concat(sk::Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]));
            }
            "m" => self.path.move_to(a[0].num(), a[1].num()),
            "l" => self.path.line_to(a[0].num(), a[1].num()),
            "c" => self.path.cubic_to(a[0].num(), a[1].num(), a[2].num(), a[3].num(), a[4].num(), a[5].num()),
            "h" => self.path.close(),
            "re" => {
                let (x, y, w, h) = (a[0].num(), a[1].num(), a[2].num(), a[3].num());
                self.path.move_to(x, y);
                self.path.line_to(x + w, y);
                self.path.line_to(x + w, y + h);
                self.path.line_to(x, y + h);
                self.path.close();
            }
            "W" => self.pending_clip = Some(sk::FillRule::Winding),
            "W*" => self.pending_clip = Some(sk::FillRule::EvenOdd),
            "n" => self.finish_path(),
            "f" | "F" => self.paint_fill(false),
            "f*" => self.paint_fill(true),
            "S" => self.paint_stroke(),
            "rg" => self.st.fill = [a[0].num(), a[1].num(), a[2].num()],
            "RG" => self.st.stroke = [a[0].num(), a[1].num(), a[2].num()],
            "w" => self.st.width = a[0].num(),
            "M" => self.st.miter = a[0].num(),
            "J" => {
                self.st.cap = match a[0].num() as u32 {
                    1 => sk::LineCap::Round,
                    2 => sk::LineCap::Square,
                    _ => sk::LineCap::Butt,
                }
            }
            "j" => {
                self.st.join = match a[0].num() as u32 {
                    1 => sk::LineJoin::Round,
                    2 => sk::LineJoin::Bevel,
                    _ => sk::LineJoin::Miter,
                }
            }
            "d" => self.st.dash = a[0].nums(),
            "gs" => {
                let states = self.pdf.get(&self.page.resources, "ExtGState").expect("page has graphics states");
                let g = self.pdf.deref(&states.dict()[a[0].name()]);
                let g = g.dict();
                if let Some(ca) = g.get("ca") {
                    self.st.fill_alpha = ca.num();
                }
                if let Some(ca) = g.get("CA") {
                    self.st.stroke_alpha = ca.num();
                }
                assert!(!g.contains_key("SMask") && !g.contains_key("BM"), "soft masks and blend modes are not painted by this probe");
            }
            "sh" => self.paint_shading(a[0].name()),
            _ => {}
        }
    }
}

/// Paints a vector page at `scale` device pixels per design pixel (1 design pixel = 0.75 point).
pub fn paint_vector_page(pdf: &Pdf, page: &Page, scale: f32) -> sk::Pixmap {
    let k = scale / 0.75;
    let (w, h) = ((page.width * k).round() as u32, (page.height * k).round() as u32);
    let mut pm = sk::Pixmap::new(w, h).unwrap();
    pm.fill(sk::Color::WHITE);
    let base = sk::Transform::from_row(k, 0.0, 0.0, -k, 0.0, page.height * k);
    let st = State {
        ctm: base,
        clip: None,
        fill: [0.0; 3],
        stroke: [0.0; 3],
        fill_alpha: 1.0,
        stroke_alpha: 1.0,
        width: 1.0,
        cap: sk::LineCap::Butt,
        join: sk::LineJoin::Miter,
        miter: 10.0,
        dash: Vec::new(),
    };
    let mut painter = Painter { pdf, page, pm, st, stack: Vec::new(), path: sk::PathBuilder::new(), pending_clip: None };
    for op in ops(&page.content) {
        painter.run(&op);
    }
    painter.pm
}

// ---------------------------------------------------------------------------------------------
// Pixels

/// Opaque RGB pixels of a page, addressed in design pixels.
pub struct Pixels {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
    design_w: f32,
    design_h: f32,
}

impl Pixels {
    pub fn at(&self, x: f32, y: f32) -> [u8; 3] {
        let ix = ((x / self.design_w * self.width as f32).floor().max(0.0) as usize).min(self.width - 1);
        let iy = ((y / self.design_h * self.height as f32).floor().max(0.0) as usize).min(self.height - 1);
        let i = (iy * self.width + ix) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }

    /// A renderer pixmap (premultiplied RGBA) composited over white.
    pub fn from_pixmap_over_white(pm: &sk::Pixmap, design_w: f32, design_h: f32) -> Self {
        let rgb = pm
            .data()
            .chunks_exact(4)
            .flat_map(|p| {
                let white = 255 - p[3];
                [p[0].saturating_add(white), p[1].saturating_add(white), p[2].saturating_add(white)]
            })
            .collect();
        Self { width: pm.width() as usize, height: pm.height() as usize, rgb, design_w, design_h }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// Drawn with PDF paths and shadings.
    Vector,
    /// A flattened page image.
    Raster,
}

/// What a viewer shows for page `index`: the page image, or the painted vector content.
pub fn page_pixels(pdf_bytes: &[u8], index: usize, scale: f32) -> (PageKind, Pixels) {
    let pdf = Pdf::parse(pdf_bytes);
    let pages = pdf.pages();
    let page = &pages[index];
    let (design_w, design_h) = (page.width / 0.75, page.height / 0.75);
    if let Some(img) = pdf.page_image(page) {
        return (PageKind::Raster, Pixels { width: img.width, height: img.height, rgb: img.rgb, design_w, design_h });
    }
    let pm = paint_vector_page(&pdf, page, scale);
    let rgb = pm.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    (PageKind::Vector, Pixels { width: pm.width() as usize, height: pm.height() as usize, rgb, design_w, design_h })
}
