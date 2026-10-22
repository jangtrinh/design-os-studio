//! Text layout with cosmic-text. Glyph outlines are converted to tiny-skia
//! paths (no hinting, greyscale AA via the path rasteriser), grouped by the
//! style they belong to so fills (solid / gradient / image) apply uniformly.

use cosmic_text::{
    fontdb, Align, Attrs, Buffer, CacheKeyFlags, Command, Family, FeatureTag, FontFeatures, FontSystem, Hinting, Metrics, Shaping, Stretch, Style, SwashCache,
    TextDecoration as CtDecoration, UnderlineStyle, Weight, Wrap,
};
use designos_doc as d;
use designos_doc::{TextAutoResize, TextProps, Vec2};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia as sk;

/// Set to any value before the first text layout to build the font database from the bundled fonts only.
const BUNDLED_FONTS_ONLY_ENV: &str = "DESIGNOS_BUNDLED_FONTS_ONLY";

struct Fonts {
    fs: FontSystem,
    swash: SwashCache,
    /// Lowercased family names present in the font database.
    families: HashSet<String>,
    /// Faces embedded via `designos-fonts` (preferred over any system copy).
    bundled: HashSet<fontdb::ID>,
}

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
    static LAYOUTS: RefCell<TextCache<TextLayout>> = RefCell::new(TextCache::default());
    /// How many entries of [`EXTRA_FONTS`] this thread's font system has loaded.
    static REPLAYED: Cell<usize> = const { Cell::new(0) };
}

/// Font blobs registered at runtime, in registration order. Every thread's
/// font system replays this list, so tile workers see the same fonts as the
/// thread that called [`register_font`].
static EXTRA_FONTS: Mutex<Vec<Arc<Vec<u8>>>> = Mutex::new(Vec::new());
static FONT_GEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The system + bundled font database, built once and cloned per thread (so
/// scanning system fonts does not repeat for every worker).
static BASE_DB: OnceLock<(fontdb::Database, HashSet<fontdb::ID>)> = OnceLock::new();

/// Number of fonts registered through [`register_font`]; changes whenever the
/// set of available fonts changes (tile caches use it to invalidate).
#[inline]
pub fn font_generation() -> usize {
    FONT_GEN.load(std::sync::atomic::Ordering::Acquire)
}

/// Outlines of laid out text, one path per style (index 0 = base style,
/// i + 1 = `runs[i].style`).
pub struct TextLayout {
    pub styles: Vec<d::TextStyle>,
    pub paths: Vec<Option<sk::Path>>,
}

fn base_db() -> &'static (fontdb::Database, HashSet<fontdb::ID>) {
    BASE_DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        // Bundled fonts go in first and any system face claiming the same family
        // is dropped, so "Inter" always resolves to the embedded copy.
        let mut bundled = HashSet::new();
        let mut owned = HashSet::new();
        for asset in designos_fonts::bundled() {
            owned.insert(asset.family.to_lowercase());
            bundled.extend(db.load_font_source(fontdb::Source::Binary(Arc::new(asset.data))));
        }
        // Hermetic runs (tests, reproducible renders) set this to ignore whatever the host has installed.
        if std::env::var_os(BUNDLED_FONTS_ONLY_ENV).is_none() {
            db.load_system_fonts();
        }
        let shadowed: Vec<fontdb::ID> =
            db.faces().filter(|f| !bundled.contains(&f.id) && f.families.iter().any(|(n, _)| owned.contains(&n.to_lowercase()))).map(|f| f.id).collect();
        for id in shadowed {
            db.remove_face(id);
        }
        db.set_sans_serif_family("Inter");
        (db, bundled)
    })
}

fn init_fonts() -> Fonts {
    let (db, bundled) = base_db();
    let fs = FontSystem::new_with_locale_and_db("en-US".to_string(), db.clone());
    let families = family_names(&fs);
    Fonts { fs, swash: SwashCache::new(), families, bundled: bundled.clone() }
}

fn family_names(fs: &FontSystem) -> HashSet<String> {
    let mut out = HashSet::new();
    for face in fs.db().faces() {
        for (name, _) in &face.families {
            out.insert(name.to_lowercase());
        }
    }
    out
}

fn with_fonts<R>(f: impl FnOnce(&mut Fonts) -> R) -> R {
    FONTS.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(init_fonts());
            REPLAYED.with(|r| r.set(0));
        }
        let fonts = slot.as_mut().unwrap();
        // Load fonts registered (possibly on another thread) since last use.
        let done = REPLAYED.with(|r| r.get());
        if done < font_generation() {
            let pending: Vec<Arc<Vec<u8>>> = {
                let all = EXTRA_FONTS.lock().unwrap_or_else(|e| e.into_inner());
                all[done.min(all.len())..].to_vec()
            };
            if !pending.is_empty() {
                for blob in &pending {
                    fonts.fs.db_mut().load_font_source(fontdb::Source::Binary(blob.clone()));
                }
                fonts.families = family_names(&fonts.fs);
                REPLAYED.with(|r| r.set(done + pending.len()));
                // Shaped text depends on the fonts: neither the outlines nor the measured geometry of the old set stand.
                LAYOUTS.with(|l| l.borrow_mut().clear());
                GEOMS.with(|g| g.borrow_mut().clear());
            }
        }
        f(fonts)
    })
}

/// Loads the fonts registered since this thread last looked, before a cache is read. A cache hit
/// never reaches [`with_fonts`], so a thread that only hits its caches (a tile worker redrawing
/// unchanged text) would otherwise keep the shapes of the old font set for as long as it lives.
#[inline]
fn sync_fonts() {
    if REPLAYED.with(|r| r.get()) < font_generation() {
        with_fonts(|_| ());
    }
}

/// Registers extra font data (e.g. a font fetched by `designos-fonts`) with the
/// shared font system. Every thread (including tile-render workers) loads it
/// the next time it lays out text. Cached layouts are discarded.
pub fn register_font(data: Vec<u8>) {
    {
        let mut all = EXTRA_FONTS.lock().unwrap_or_else(|e| e.into_inner());
        all.push(Arc::new(data));
        FONT_GEN.store(all.len(), std::sync::atomic::Ordering::Release);
    }
    with_fonts(|_| ());
}

/// Family names available to text layout (bundled, system and registered fonts),
/// sorted case-insensitively without duplicates. Used by font pickers.
pub fn available_families() -> Vec<String> {
    with_fonts(|f| {
        let mut names: Vec<String> = f.fs.db().faces().filter_map(|face| face.families.first().map(|(n, _)| n.clone())).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names
    })
}

/// Compatible alias of [`register_font`].
pub fn load_font_data(data: Vec<u8>) {
    register_font(data)
}

/// Which font a text style will be shaped with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFont {
    /// Family name in the font database (after substitution).
    pub family: String,
    /// The matched face is one embedded in the binary.
    pub bundled: bool,
}

/// Resolves `family` the way the text layout does: the family itself if it is
/// in the font database, else its open-source substitute, else the default
/// sans-serif (Inter). Returns `None` only if the database has no match at all.
pub fn resolve_font(family: &str, weight: u16, italic: bool) -> Option<ResolvedFont> {
    with_fonts(|f| {
        let name = resolve_family(f, family);
        let fam = match &name {
            Some(n) => fontdb::Family::Name(n),
            None => fontdb::Family::SansSerif,
        };
        let q = fontdb::Query {
            families: &[fam],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        };
        let id = f.fs.db().query(&q)?;
        let face = f.fs.db().face(id)?;
        Some(ResolvedFont { family: face.families.first()?.0.clone(), bundled: f.bundled.contains(&id) })
    })
}

/// Family name to ask the font database for, or `None` to use default sans-serif.
fn resolve_family(fonts: &Fonts, requested: &str) -> Option<String> {
    if fonts.families.contains(&requested.to_lowercase()) {
        return Some(requested.to_string());
    }
    let sub = designos_fonts::substitute(requested)?;
    fonts.families.contains(&sub.to_lowercase()).then(|| sub.to_string())
}

fn auto_line_ratio(fonts: &mut Fonts, st: &d::TextStyle, family: Option<&str>) -> f32 {
    let fam = match family {
        Some(n) => fontdb::Family::Name(n),
        None => fontdb::Family::SansSerif,
    };
    let q = fontdb::Query {
        families: &[fam],
        weight: fontdb::Weight(st.font_weight),
        stretch: fontdb::Stretch::Normal,
        style: if st.italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
    };
    let Some(id) = fonts.fs.db().query(&q) else { return 1.2 };
    let Some(font) = fonts.fs.get_font(id, fontdb::Weight(st.font_weight)) else { return 1.2 };
    let m = font.as_swash().metrics(&[]);
    let upem = m.units_per_em as f32;
    if upem <= 0.0 {
        return 1.2;
    }
    let r = (m.ascent + m.descent + m.leading) / upem;
    if r.is_finite() && r > 0.5 {
        r
    } else {
        1.2
    }
}

/// Cap height / em of the face a style resolves to (0.7 when the font has none).
fn cap_ratio(fonts: &mut Fonts, st: &d::TextStyle, family: Option<&str>) -> f32 {
    let fam = match family {
        Some(n) => fontdb::Family::Name(n),
        None => fontdb::Family::SansSerif,
    };
    let q = fontdb::Query {
        families: &[fam],
        weight: fontdb::Weight(st.font_weight),
        stretch: fontdb::Stretch::Normal,
        style: if st.italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
    };
    let Some(id) = fonts.fs.db().query(&q) else { return 0.7 };
    let Some(font) = fonts.fs.get_font(id, fontdb::Weight(st.font_weight)) else { return 0.7 };
    let m = font.as_swash().metrics(&[]);
    let r = if m.units_per_em > 0 { m.cap_height / m.units_per_em as f32 } else { 0.0 };
    if r.is_finite() && r > 0.2 && r < 1.2 {
        r
    } else {
        0.7
    }
}

/// OpenType layout feature tags (GSUB and GPOS) the face matching the style provides,
/// sorted and deduplicated (e.g. `liga`, `tnum`, `ss01`). Empty when the font cannot be read.
pub fn font_features(family: &str, weight: u16, italic: bool) -> Vec<String> {
    with_fonts(|f| {
        let name = resolve_family(f, family);
        let fam = match &name {
            Some(n) => fontdb::Family::Name(n),
            None => fontdb::Family::SansSerif,
        };
        let q = fontdb::Query {
            families: &[fam],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        };
        let Some(id) = f.fs.db().query(&q) else { return Vec::new() };
        let Some(font) = f.fs.get_font(id, fontdb::Weight(weight)) else { return Vec::new() };
        let index = f.fs.db().face(id).map(|fc| fc.index).unwrap_or(0);
        let Ok(face) = ttf_parser::Face::parse(font.data(), index) else { return Vec::new() };
        let mut tags = std::collections::BTreeSet::new();
        let tables = face.tables();
        for table in [tables.gsub, tables.gpos].into_iter().flatten() {
            for i in 0..table.features.len() {
                if let Some(feat) = table.features.get(i) {
                    tags.insert(feat.tag.to_string());
                }
            }
        }
        tags.into_iter().collect()
    })
}

#[derive(Clone, Debug)]
struct Segment {
    text: String,
    style: usize,
    small: bool,
}

mod flow;
mod wrap;
use flow::{flow, glyph_outline, marker_outline, split_paragraphs, Flow, Para};

/// Number of lines kept by `TextAutoResize::Truncate`: `max_lines`, or as many
/// as fit the box height (at least one).
fn truncation_limit(fl: &Flow, props: &TextProps, size: Vec2) -> usize {
    match props.max_lines {
        Some(m) => m.max(1) as usize,
        None if size.y > 0.0 => fl.lines.iter().filter(|l| l.top - fl.shift + l.height <= size.y + 0.5).count().max(1),
        None => usize::MAX,
    }
}

/// The first `cut` bytes of the segments, trailing whitespace removed, followed
/// by an ellipsis in the style of the last kept character.
fn truncated_segments(segs: &[Segment], mut cut: usize) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for s in segs {
        if cut == 0 {
            break;
        }
        let take = s.text.len().min(cut);
        out.push(Segment { text: s.text[..take].to_string(), style: s.style, small: s.small });
        cut -= take;
    }
    while let Some(last) = out.last_mut() {
        let keep = last.text.trim_end().len();
        last.text.truncate(keep);
        if last.text.is_empty() {
            out.pop();
        } else {
            break;
        }
    }
    let style = out.last().map(|s| s.style).unwrap_or(0);
    out.push(Segment { text: "\u{2026}".into(), style, small: false });
    out
}

/// A cached result with the exact inputs it was computed from. The map key is only a cheap bucket
/// hash of the inputs; a hit also needs the inputs to be equal, so two inputs that share a bucket
/// never share a result.
struct Cached<T> {
    props: TextProps,
    size: Vec2,
    value: Rc<T>,
}

impl<T> Cached<T> {
    fn matches(&self, props: &TextProps, size: Vec2) -> bool {
        self.size.x.to_bits() == size.x.to_bits() && self.size.y.to_bits() == size.y.to_bits() && self.props == *props
    }
}

/// Cached results grouped by bucket hash. A bucket keeps every result whose inputs hash to it, so
/// texts that differ only in a field the hash leaves out (fills, letter spacing, ...) never evict
/// each other. The memory bound is on the total entry count: past the limit the cache is emptied.
struct TextCache<T> {
    buckets: HashMap<u64, Vec<Cached<T>>>,
    entries: usize,
}

impl<T> Default for TextCache<T> {
    fn default() -> Self {
        Self { buckets: HashMap::new(), entries: 0 }
    }
}

impl<T> TextCache<T> {
    fn get(&self, key: u64, props: &TextProps, size: Vec2) -> Option<Rc<T>> {
        self.buckets.get(&key)?.iter().find(|c| c.matches(props, size)).map(|c| Rc::clone(&c.value))
    }

    /// Stores `value`, first emptying the cache when it already holds more than `limit` entries.
    fn insert(&mut self, key: u64, props: &TextProps, size: Vec2, value: Rc<T>, limit: usize) {
        if self.entries > limit {
            self.clear();
        }
        self.buckets.entry(key).or_default().push(Cached { props: props.clone(), size, value });
        self.entries += 1;
    }

    fn clear(&mut self) {
        self.buckets.clear();
        self.entries = 0;
    }

    #[cfg(test)]
    fn entries(&self) -> usize {
        self.entries
    }
}

/// Bucket hash of a text's inputs (FNV-1a over the content's ends and the common style fields). It is
/// not a fingerprint of the whole style: fields it leaves out only share a bucket, see [`Cached`].
fn bucket_key(props: &TextProps, size: Vec2) -> u64 {
    const EDGE: usize = 32;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
    let bytes = props.content.as_bytes();
    mix(bytes.len() as u64);
    for &b in bytes.iter().take(EDGE).chain(bytes.iter().skip(EDGE.max(bytes.len().saturating_sub(EDGE)))) {
        mix(u64::from(b));
    }
    mix(u64::from(props.style.font_size.to_bits()));
    mix(u64::from(props.style.font_weight));
    mix(props.style.font_family.len() as u64);
    mix(props.runs.len() as u64);
    mix(u64::from(size.x.to_bits()));
    mix(u64::from(size.y.to_bits()));
    h
}

/// Lays out `props` inside a box of `size`; cached per content/style/size.
pub fn layout(props: &TextProps, size: Vec2) -> Rc<TextLayout> {
    sync_fonts();
    let key = bucket_key(props, size);
    if let Some(hit) = LAYOUTS.with(|l| l.borrow().get(key, props, size)) {
        return hit;
    }
    let lay = Rc::new(with_fonts(|f| compute_layout(f, props, size)));
    LAYOUTS.with(|l| l.borrow_mut().insert(key, props, size, lay.clone(), 8192));
    lay
}

/// Everything needed to turn segments of a [`TextProps`] into a shaped buffer.
/// Shared by the renderer and the editor geometry so both shape identically.
struct Shaper {
    styles: Vec<d::TextStyle>,
    family_names: Vec<Option<String>>,
    ratios: Vec<f32>,
    /// Cap height / em per style.
    caps: Vec<f32>,
}

impl Shaper {
    fn new(fonts: &mut Fonts, props: &TextProps) -> Self {
        let mut styles = vec![props.style.clone()];
        styles.extend(props.runs.iter().map(|r| r.style.clone()));
        let family_names: Vec<Option<String>> = styles.iter().map(|s| resolve_family(fonts, &s.font_family)).collect();
        let ratios: Vec<f32> = styles
            .iter()
            .enumerate()
            .map(|(i, s)| match s.line_height {
                d::LineHeight::Auto => auto_line_ratio(fonts, s, family_names[i].as_deref()),
                _ => 1.0,
            })
            .collect();
        let caps: Vec<f32> = if props.paragraph.leading_trim == d::LeadingTrim::CapHeight || props.has_lists() {
            styles.iter().enumerate().map(|(i, s)| cap_ratio(fonts, s, family_names[i].as_deref())).collect()
        } else {
            vec![0.7; styles.len()]
        };
        Shaper { styles, family_names, ratios, caps }
    }

    fn line_height_for(&self, idx: usize, font_size: f32) -> f32 {
        match self.styles[idx].line_height {
            d::LineHeight::Auto => self.ratios[idx] * font_size,
            d::LineHeight::Pixels(p) => p,
            d::LineHeight::Percent(p) => font_size * p / 100.0,
        }
    }

    fn attrs_for(&self, seg: &Segment) -> Attrs<'_> {
        let st = &self.styles[seg.style];
        let fsize = (st.font_size * if seg.small { 0.8 } else { 1.0 }).max(0.1);
        let family = match &self.family_names[seg.style] {
            Some(n) => Family::Name(n),
            None => Family::SansSerif,
        };
        let weight = st.get_axis("wght").map(|w| Weight(w.round().clamp(1.0, 1000.0) as u16)).unwrap_or(Weight(st.font_weight));
        let stretch = match st.get_axis("wdth") {
            Some(w) if w <= 56.25 => Stretch::UltraCondensed,
            Some(w) if w <= 68.75 => Stretch::ExtraCondensed,
            Some(w) if w <= 81.25 => Stretch::Condensed,
            Some(w) if w <= 93.75 => Stretch::SemiCondensed,
            Some(w) if w <= 106.25 => Stretch::Normal,
            Some(w) if w <= 118.75 => Stretch::SemiExpanded,
            Some(w) if w <= 137.5 => Stretch::Expanded,
            Some(w) if w <= 175.0 => Stretch::ExtraExpanded,
            Some(_) => Stretch::UltraExpanded,
            None => Stretch::Normal,
        };
        let font_style = if st.italic || st.get_axis("ital").is_some_and(|v| v >= 0.5) {
            Style::Italic
        } else if st.get_axis("slnt").is_some_and(|s| s < -2.0) {
            Style::Oblique
        } else {
            Style::Normal
        };
        let mut a = Attrs::new()
            .family(family)
            .weight(weight)
            .stretch(stretch)
            .style(font_style)
            .metrics(Metrics::new(fsize, self.line_height_for(seg.style, fsize)))
            .metadata(seg.style)
            .cache_key_flags(CacheKeyFlags::DISABLE_HINTING);
        if st.letter_spacing != 0.0 {
            a = a.letter_spacing(st.letter_spacing / fsize);
        }
        if !st.features.is_empty() {
            let mut ff = FontFeatures::new();
            for (tag, value) in &st.features {
                if let Ok(bytes) = <[u8; 4]>::try_from(tag.as_bytes()) {
                    ff.set(FeatureTag::new(&bytes), *value);
                }
            }
            a = a.font_features(ff);
        }
        let mut deco = CtDecoration::new();
        match st.decoration {
            d::TextDecoration::None => {}
            d::TextDecoration::Underline => deco.underline = UnderlineStyle::Single,
            d::TextDecoration::Strikethrough => deco.strikethrough = true,
        }
        if deco.has_decoration() {
            a.text_decoration = deco;
        }
        a
    }

    /// Shapes `segs` (one paragraph, or part of one) as a buffer `width` wide.
    fn buffer(&self, fs: &mut FontSystem, props: &TextProps, width: Option<f32>, wrap: Wrap, segs: &[Segment]) -> Buffer {
        let base_size = props.style.font_size.max(0.1);
        let base_lh = self.line_height_for(0, base_size);
        let align = match props.style.align_h {
            d::TextAlignH::Left => Align::Left,
            d::TextAlignH::Center => Align::Center,
            d::TextAlignH::Right => Align::Right,
            d::TextAlignH::Justified => Align::Justified,
        };
        let default_attrs = self.attrs_for(&Segment { text: String::new(), style: 0, small: false });
        let mut buffer = Buffer::new(fs, Metrics::new(base_size, base_lh));
        buffer.set_hinting(Hinting::Disabled);
        buffer.set_size(width, None);
        buffer.set_wrap(wrap);
        let spans: Vec<(&str, Attrs)> = segs.iter().map(|s| (s.text.as_str(), self.attrs_for(s))).collect();
        buffer.set_rich_text(spans, &default_attrs, Shaping::Advanced, Some(align));
        buffer.shape_until_scroll(fs, false);
        buffer
    }
}

/// Shapes and flows `props` in a box of `size`, truncating with an ellipsis when asked to.
fn laid_out(fonts: &mut Fonts, props: &TextProps, size: Vec2) -> (Shaper, Vec<Para>, Flow) {
    let shaper = Shaper::new(fonts, props);
    let mut paras = split_paragraphs(props, false);
    let mut fl = flow(fonts, &shaper, props, size, &paras);

    // Truncate: keep at most `limit` lines and end the last one with "…".
    if props.auto_resize == TextAutoResize::Truncate {
        let limit = truncation_limit(&fl, props, size);
        if fl.lines.len() > limit {
            let line = &fl.lines[limit - 1];
            let (pi, mut cut) = (line.para, line.cut);
            paras.truncate(pi + 1);
            let original = paras[pi].segs.clone();
            let text = paras[pi].text.clone();
            // the ellipsis itself must still fit on the last line: back off until it does
            for _ in 0..text.len() + 1 {
                cut = cut.min(text.len());
                while !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                paras[pi].segs = truncated_segments(&original, cut);
                paras[pi].text = paras[pi].segs.iter().map(|s| s.text.as_str()).collect();
                fl = flow(fonts, &shaper, props, size, &paras);
                if fl.lines.len() <= limit || cut == 0 {
                    break;
                }
                cut = text[..cut].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            }
        }
    }

    (shaper, paras, fl)
}

fn compute_layout(fonts: &mut Fonts, props: &TextProps, size: Vec2) -> TextLayout {
    let (shaper, _, fl) = laid_out(fonts, props, size);
    let styles = shaper.styles.clone();
    let mut builders: Vec<sk::PathBuilder> = styles.iter().map(|_| sk::PathBuilder::new()).collect();
    for line in &fl.lines {
        for g in &line.glyphs {
            glyph_outline(fonts, &mut builders[g.style], g, g.x, line.baseline);
        }
        for dc in &line.decos {
            let mut rect = |offset: f32, thickness: f32| {
                let t = (thickness * dc.fs).max(1.0);
                let y = line.baseline - offset * dc.fs;
                if let Some(r) = sk::Rect::from_xywh(dc.x0, y, dc.x1 - dc.x0, t) {
                    builders[dc.style].push_rect(r);
                }
            };
            if let Some((o, t)) = dc.underline {
                rect(o, t);
            }
            if let Some((o, t)) = dc.strike {
                rect(o, t);
            }
        }
    }
    for m in &fl.markers {
        let baseline = fl.lines.get(m.line).map(|l| l.baseline).unwrap_or(0.0);
        marker_outline(fonts, &mut builders[m.style], m, baseline);
    }
    TextLayout { styles, paths: builders.into_iter().map(|b| b.finish()).collect() }
}

// ------------------------------------------------------------------ outlines (SVG export)
/// A run of glyphs of one style on one line, as outlines (and the text they show).
#[derive(Clone, Debug)]
pub struct GlyphRun {
    /// Index into [`TextRuns::styles`].
    pub style: usize,
    /// Left edge of the run, node-local px.
    pub x: f32,
    /// The characters shown (list markers included), in shaping order.
    pub text: String,
    /// Glyph outlines (and underline / strikethrough bars) in node-local space.
    pub outline: Vec<d::PathCmd>,
}

#[derive(Clone, Debug)]
pub struct RunLine {
    pub baseline: f32,
    pub spans: Vec<GlyphRun>,
}

/// The laid out text of a node as per-line spans of vector outlines, for exporters.
#[derive(Clone, Debug)]
pub struct TextRuns {
    /// 0 = base style, i + 1 = `runs[i].style`.
    pub styles: Vec<d::TextStyle>,
    pub lines: Vec<RunLine>,
}

fn path_cmds(path: &sk::Path, out: &mut Vec<d::PathCmd>) {
    let v = |p: sk::Point| Vec2::new(p.x, p.y);
    for seg in path.segments() {
        out.push(match seg {
            sk::PathSegment::MoveTo(p) => d::PathCmd::MoveTo(v(p)),
            sk::PathSegment::LineTo(p) => d::PathCmd::LineTo(v(p)),
            sk::PathSegment::QuadTo(c, p) => d::PathCmd::QuadTo(v(c), v(p)),
            sk::PathSegment::CubicTo(c1, c2, p) => d::PathCmd::CubicTo(v(c1), v(c2), v(p)),
            sk::PathSegment::Close => d::PathCmd::Close,
        });
    }
}

/// Lays out `props` in a box of `size` exactly as [`layout`] does, but keeps the glyph runs per
/// line with their outlines as path commands. Right-to-left runs list their text in shaping
/// order (right to left); the outlines are exact either way.
pub fn text_runs(props: &TextProps, size: Vec2) -> TextRuns {
    with_fonts(|f| compute_runs(f, props, size))
}

fn compute_runs(fonts: &mut Fonts, props: &TextProps, size: Vec2) -> TextRuns {
    let (shaper, paras, fl) = laid_out(fonts, props, size);
    let mut lines = Vec::new();
    for (li, line) in fl.lines.iter().enumerate() {
        let para: &Para = &paras[line.para];
        let mut spans: Vec<GlyphRun> = Vec::new();
        for m in fl.markers.iter().filter(|m| m.line == li) {
            let mut pb = sk::PathBuilder::new();
            marker_outline(fonts, &mut pb, m, line.baseline);
            let mut outline = Vec::new();
            if let Some(p) = pb.finish() {
                path_cmds(&p, &mut outline);
            }
            let text = match (m.shape, para.marker) {
                (Some(d::BulletShape::Disc), _) => "\u{2022}".to_string(),
                (Some(d::BulletShape::Ring), _) => "\u{25E6}".to_string(),
                (Some(d::BulletShape::Square), _) => "\u{25AA}".to_string(),
                (None, Some(mk)) => mk.label().unwrap_or_default(),
                (None, None) => String::new(),
            };
            spans.push(GlyphRun { style: m.style, x: m.x, text, outline });
        }
        let first_glyph = spans.len();
        for g in &line.glyphs {
            if g.end <= para.mark_len {
                continue;
            }
            if spans.len() == first_glyph || spans.last().map(|s| s.style) != Some(g.style) {
                spans.push(GlyphRun { style: g.style, x: g.x, text: String::new(), outline: Vec::new() });
            }
            let span = spans.last_mut().unwrap();
            span.x = span.x.min(g.x);
            span.text.push_str(para.text.get(g.start..g.end).unwrap_or(""));
            let mut pb = sk::PathBuilder::new();
            glyph_outline(fonts, &mut pb, g, g.x, line.baseline);
            if let Some(p) = pb.finish() {
                path_cmds(&p, &mut span.outline);
            }
        }
        for dc in &line.decos {
            let mut bar = |offset: f32, thickness: f32| {
                let t = (thickness * dc.fs).max(1.0);
                let y = line.baseline - offset * dc.fs;
                if let Some(span) = spans.iter_mut().find(|s| s.style == dc.style) {
                    span.outline.extend([
                        d::PathCmd::MoveTo(Vec2::new(dc.x0, y)),
                        d::PathCmd::LineTo(Vec2::new(dc.x1, y)),
                        d::PathCmd::LineTo(Vec2::new(dc.x1, y + t)),
                        d::PathCmd::LineTo(Vec2::new(dc.x0, y + t)),
                        d::PathCmd::Close,
                    ]);
                }
            };
            if let Some((o, t)) = dc.underline {
                bar(o, t);
            }
            if let Some((o, t)) = dc.strike {
                bar(o, t);
            }
        }
        if !spans.is_empty() {
            lines.push(RunLine { baseline: line.baseline, spans });
        }
    }
    TextRuns { styles: shaper.styles.clone(), lines }
}

// ------------------------------------------------------------------ editor geometry
/// One shaped glyph (cluster) of a line, in byte offsets of `TextProps::content`.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphSpan {
    pub start: usize,
    pub end: usize,
    /// Left edge, in node-local px (alignment applied).
    pub x: f32,
    pub w: f32,
    /// The cluster is right-to-left (odd bidi level).
    pub rtl: bool,
}

/// One visual line of laid out text, in node-local px.
#[derive(Clone, Debug, PartialEq)]
pub struct LineGeom {
    /// Index of the paragraph (`\n`-separated) this line belongs to.
    pub para: usize,
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    pub width: f32,
    /// Byte range of `content` shown on this line.
    pub start: usize,
    pub end: usize,
    /// Left edge of an empty line (alignment, indent).
    pub empty_x: f32,
    /// The paragraph of this line is right-to-left.
    pub rtl: bool,
    /// Glyph clusters sorted left to right (visual order); for right-to-left text the
    /// byte offsets run backwards along the line.
    pub glyphs: Vec<GlyphSpan>,
}

/// Shaped line/glyph positions of a text node, produced by the same shaping as
/// [`layout`] (without truncation), for caret placement and hit testing.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TextGeometry {
    pub lines: Vec<LineGeom>,
    pub width: f32,
    pub height: f32,
}

thread_local! {
    static GEOMS: RefCell<TextCache<TextGeometry>> = RefCell::new(TextCache::default());
}

/// Lines and glyph positions of `props` laid out in a box of `size`. Positions
/// match the glyph outlines drawn by the renderer exactly.
pub fn text_geometry(props: &TextProps, size: Vec2) -> Rc<TextGeometry> {
    sync_fonts();
    let key = bucket_key(props, size);
    if let Some(hit) = GEOMS.with(|g| g.borrow().get(key, props, size)) {
        return hit;
    }
    let geom = Rc::new(with_fonts(|f| compute_geometry(f, props, size)));
    GEOMS.with(|g| g.borrow_mut().insert(key, props, size, geom.clone(), 256));
    geom
}

fn compute_geometry(fonts: &mut Fonts, props: &TextProps, size: Vec2) -> TextGeometry {
    let shaper = Shaper::new(fonts, props);
    let paras = split_paragraphs(props, true);
    let fl = flow(fonts, &shaper, props, size, &paras);
    let mut lines = Vec::with_capacity(fl.lines.len());
    for pl in &fl.lines {
        let para: &Para = &paras[pl.para];
        let at = |shaped: usize| para.map.get(shaped).copied().unwrap_or(para.end);
        let mut glyphs: Vec<GlyphSpan> =
            pl.glyphs.iter().filter(|g| g.end > para.mark_len).map(|g| GlyphSpan { start: at(g.start), end: at(g.end), x: g.x, w: g.w, rtl: g.rtl }).collect();
        // right-to-left lines come out of the shaper right to left
        glyphs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        let (start, end) = match (glyphs.iter().map(|g| g.start).min(), glyphs.iter().map(|g| g.end).max()) {
            (Some(a), Some(b)) => (a, b),
            // a line without glyphs (empty paragraph, blank line after a soft break) sits where its text would start
            _ => {
                let a = at(pl.cut).max(para.start);
                (a, a)
            }
        };
        lines.push(LineGeom {
            para: pl.para,
            top: pl.top,
            height: pl.height,
            baseline: pl.baseline,
            width: pl.width,
            start,
            end,
            empty_x: pl.empty_x,
            rtl: para.rtl,
            glyphs,
        });
    }
    TextGeometry { lines, width: fl.width, height: fl.height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_glyphs_match_drawn_ink() {
        // Glyph positions from `text_geometry` bracket the ink the renderer draws:
        // lay out "   X" (spaces keep X at its natural x) and compare per letter.
        let size = Vec2::new(0.0, 0.0);
        let text = "HIEx";
        let p = props(text, TextAutoResize::WidthAndHeight, None);
        let geom = text_geometry(&p, size);
        assert_eq!(geom.lines.len(), 1);
        let gl = &geom.lines[0].glyphs;
        assert_eq!(gl.len(), 4);
        for (i, g) in gl.iter().enumerate() {
            assert_eq!((g.start, g.end), (i, i + 1));
            // the ink of the first i+1 letters ends inside glyph i
            let prefix = props(&text[..i + 1], TextAutoResize::WidthAndHeight, None);
            let b = ink(&layout(&prefix, size)).bounds();
            assert!(b.right() >= g.x && b.right() <= g.x + g.w + 0.5, "glyph {i}: ink right {} vs {}..{}", b.right(), g.x, g.x + g.w);
        }
        // glyphs tile the line without gaps
        for w in gl.windows(2) {
            assert!((w[0].x + w[0].w - w[1].x).abs() < 0.01);
        }
    }

    #[test]
    fn geometry_maps_runs_cases_and_paragraphs() {
        let mut p = props("ab\n\ncd", TextAutoResize::WidthAndHeight, None);
        p.style.case = d::TextCase::Upper;
        let g = text_geometry(&p, Vec2::ZERO);
        assert_eq!(g.lines.len(), 3, "empty paragraph keeps its own line");
        assert_eq!((g.lines[1].start, g.lines[1].end), (3, 3));
        assert_eq!((g.lines[2].start, g.lines[2].end), (4, 6));
        assert!(g.lines[2].top > g.lines[1].top && g.lines[1].top > g.lines[0].top);
        // bigger run makes its line taller and glyph wider
        let mut q = props("abcd", TextAutoResize::WidthAndHeight, None);
        let mut big = q.style.clone();
        big.font_size = 40.0;
        big.line_height = d::LineHeight::Pixels(48.0);
        q.runs = vec![d::TextRun { start: 2, end: 4, style: big }];
        let g = text_geometry(&q, Vec2::ZERO);
        let gl = &g.lines[0].glyphs;
        assert!(gl[2].w > gl[0].w * 2.0, "run style is shaped");
        assert!(g.lines[0].height >= 48.0);
    }

    #[test]
    fn cached_layout_needs_equal_inputs_not_just_an_equal_bucket() {
        let size = Vec2::ZERO;
        let base = props("Design system", TextAutoResize::WidthAndHeight, None);
        // Each variant leaves every field of the bucket hash alone, so it shares the base text's bucket.
        let mut spaced = base.clone();
        spaced.style.letter_spacing = 6.0;
        let mut upper = base.clone();
        upper.style.case = d::TextCase::Upper;
        let mut tall = base.clone();
        tall.style.line_height = d::LineHeight::Pixels(44.0);
        for other in [&spaced, &upper, &tall] {
            assert_eq!(bucket_key(&base, size), bucket_key(other, size));
        }
        let first = layout(&base, size);
        for other in [&spaced, &upper, &tall] {
            assert!(!Rc::ptr_eq(&first, &layout(other, size)), "a style that differs must not reuse the cached layout");
        }
        assert!(ink(&layout(&spaced, size)).bounds().right() > ink(&first).bounds().right() + 20.0, "letter spacing is laid out");
        assert!(ink(&layout(&tall, size)).bounds().bottom() > ink(&first).bounds().bottom() + 10.0, "line height is laid out");
        assert!(Rc::ptr_eq(&first, &layout(&base.clone(), size)), "equal inputs hit the cache");
        let geometry = text_geometry(&base, size);
        assert!(text_geometry(&spaced, size).width > geometry.width + 20.0, "geometry is keyed by the same exact inputs");
        assert!(Rc::ptr_eq(&geometry, &text_geometry(&base.clone(), size)));
    }

    #[test]
    fn many_variants_sharing_a_bucket_all_stay_cached() {
        let size = Vec2::ZERO;
        let base = props("Button label", TextAutoResize::WidthAndHeight, None);
        // Letter spacing is not in the bucket hash, so every variant lands in one bucket.
        let variants: Vec<TextProps> = (0..12)
            .map(|i| {
                let mut p = base.clone();
                p.style.letter_spacing = i as f32 * 0.5;
                p
            })
            .collect();
        assert!(variants.windows(2).all(|w| bucket_key(&w[0], size) == bucket_key(&w[1], size)));
        let first: Vec<_> = variants.iter().map(|p| layout(p, size)).collect();
        let second: Vec<_> = variants.iter().map(|p| layout(p, size)).collect();
        let third: Vec<_> = variants.iter().map(|p| layout(p, size)).collect();
        for (i, (b, c)) in second.iter().zip(&third).enumerate() {
            assert!(Rc::ptr_eq(b, c), "layout variant {i} is re-shaped on every pass");
        }
        assert_eq!(first.len(), variants.len());
        let geoms: Vec<_> = variants.iter().map(|p| text_geometry(p, size)).collect();
        let again: Vec<_> = variants.iter().map(|p| text_geometry(p, size)).collect();
        for (i, (a, b)) in geoms.iter().zip(&again).enumerate() {
            assert!(Rc::ptr_eq(a, b), "geometry variant {i} is recomputed on every pass");
        }
    }

    #[test]
    fn cache_clears_by_total_entries_not_by_bucket() {
        let mut c: TextCache<u32> = TextCache::default();
        let p = props("x", TextAutoResize::WidthAndHeight, None);
        for i in 0..6u32 {
            c.insert(7, &p, Vec2::new(i as f32, 0.0), Rc::new(i), 4);
        }
        assert_eq!(c.entries(), 1, "past the limit the whole cache is dropped, then the new entry stays");
        c.clear();
        assert_eq!(c.entries(), 0, "clear resets the entry count");
        c.insert(7, &p, Vec2::ZERO, Rc::new(0), 4);
        assert_eq!(c.entries(), 1);
    }

    const LONG: &str = "The quick brown fox jumps over the lazy dog and keeps running far away";

    fn props(content: &str, auto_resize: TextAutoResize, max_lines: Option<u32>) -> TextProps {
        TextProps {
            content: content.into(),
            style: d::TextStyle { font_size: 16.0, line_height: d::LineHeight::Pixels(20.0), ..Default::default() },
            runs: Vec::new(),
            auto_resize,
            max_lines,
            ..Default::default()
        }
    }

    fn ink(l: &TextLayout) -> sk::Path {
        l.paths[0].clone().expect("text has ink")
    }

    #[test]
    fn truncate_stops_at_max_lines() {
        let size = Vec2::new(120.0, 300.0);
        let full = ink(&layout(&props(LONG, TextAutoResize::Height, None), size));
        let cut = ink(&layout(&props(LONG, TextAutoResize::Truncate, Some(2)), size));
        assert!(full.bounds().bottom() > 60.0, "untruncated text spans 3+ lines");
        assert!(cut.bounds().bottom() <= 40.0, "two lines of 20px: {}", cut.bounds().bottom());
        assert!(cut.bounds().right() <= 120.5, "ellipsis stays inside the box");
    }

    #[test]
    fn truncate_ends_last_line_with_ellipsis() {
        let size = Vec2::new(120.0, 300.0);
        let cut = ink(&layout(&props(LONG, TextAutoResize::Truncate, Some(2)), size));
        // some prefix + "…" laid out normally must be identical
        let found = (1..LONG.len()).filter(|k| LONG.is_char_boundary(*k)).any(|k| {
            let s = format!("{}\u{2026}", LONG[..k].trim_end());
            let p = ink(&layout(&props(&s, TextAutoResize::Height, None), size));
            p.points() == cut.points()
        });
        assert!(found, "truncated layout is a text prefix followed by an ellipsis");
    }

    #[test]
    fn truncate_without_max_lines_uses_box_height() {
        let by_height = ink(&layout(&props(LONG, TextAutoResize::Truncate, None), Vec2::new(120.0, 45.0)));
        let by_lines = ink(&layout(&props(LONG, TextAutoResize::Truncate, Some(2)), Vec2::new(120.0, 45.0)));
        assert!(by_height.bounds().bottom() <= 45.0);
        assert_eq!(by_height.points(), by_lines.points(), "45px box holds two 20px lines");
        let one = ink(&layout(&props(LONG, TextAutoResize::Truncate, None), Vec2::new(120.0, 25.0)));
        assert!(one.bounds().bottom() <= 25.0);
    }

    #[test]
    fn truncate_leaves_fitting_text_alone() {
        let size = Vec2::new(300.0, 100.0);
        let a = ink(&layout(&props("short", TextAutoResize::Truncate, Some(3)), size));
        let b = ink(&layout(&props("short", TextAutoResize::Height, None), size));
        assert_eq!(a.points(), b.points(), "no ellipsis when everything fits");
    }

    #[test]
    fn soft_line_break_u2028_breaks_line_without_paragraph_spacing() {
        let mut p = props("Still thinking\u{2028}about it?", TextAutoResize::WidthAndHeight, None);
        p.style.paragraph_spacing = 30.0;
        let g = text_geometry(&p, Vec2::ZERO);
        assert_eq!(g.lines.len(), 2, "U+2028 must break into two lines");
        assert_eq!(g.lines[0].para, 0);
        assert_eq!(g.lines[1].para, 0);
        assert_eq!(&p.content[g.lines[0].start..g.lines[0].end], "Still thinking");
        assert_eq!(&p.content[g.lines[1].start..g.lines[1].end], "about it?");
        assert!((g.lines[1].top - g.lines[0].top - 20.0).abs() < 0.5, "no paragraph spacing added at U+2028: {}", g.lines[1].top - g.lines[0].top);
        let bounds = ink(&layout(&p, Vec2::ZERO)).bounds();
        assert!(bounds.bottom() > 25.0 && bounds.bottom() <= 40.5, "two 20px lines without 30px paragraph gap: {:?}", bounds);
    }

    #[test]
    fn empty_soft_break_lines_start_after_their_separator() {
        const LS: usize = '\u{2028}'.len_utf8();
        // trailing soft break, and a blank line between two breaks
        for (content, starts) in [("Hello\u{2028}", vec![0, 5 + LS]), ("Hello\u{2028}\u{2028}World", vec![0, 5 + LS, 5 + 2 * LS])] {
            let p = props(content, TextAutoResize::WidthAndHeight, None);
            let g = text_geometry(&p, Vec2::ZERO);
            assert_eq!(g.lines.len(), starts.len(), "{content:?}: one visual line per piece");
            let got: Vec<usize> = g.lines.iter().map(|l| l.start).collect();
            assert_eq!(got, starts, "{content:?}: line starts follow the separators");
            assert_eq!((g.lines[0].start, g.lines[0].end), (0, 5), "{content:?}: first line owns Hello");
            let blank = &g.lines[1];
            assert!(blank.glyphs.is_empty() && blank.start == blank.end, "{content:?}: blank line has no extent");
            assert!(g.lines.windows(2).all(|w| w[0].start < w[1].start), "{content:?}: starts strictly increase");
        }
    }
}
