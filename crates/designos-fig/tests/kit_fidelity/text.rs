//! Text layout: Studio's shaping of a text layer against the `derivedTextData` Figma stored for it. Three
//! checks: the same line breaks (each line starts at the same character, counted in UTF-16 units as Figma counts),
//! each baseline within 0.5 px, and the layout size within 0.5 px (width only where the text sizes its own width).
//! Three more buckets sit beside them and do not enter the per-text pass: each glyph's x position and advance within
//! 0.05 px of the stored glyph (`text_glyphs.rs`), and the texts whose every glyph matches.
//! Layers whose font face (family, weight and style) is not available are scored apart: shaping then uses another
//! face. A baseline Figma stored without a position is a mismatch, never a pass.

use super::board::{broken_comparator, Part};
use super::keys::NodeKeys;
use super::raw::{field_str, record_guid, u64_of, vec2_of, Raw};
use super::text_fonts::{font_state, variable_weight_applied, FontState};
use super::text_glyphs::{self, GlyphScore, GlyphTruth};
use designos_doc::*;
use designos_fig::kiwi::KiwiValue;
use std::collections::BTreeSet;

pub const TOL: f32 = 0.5;

pub struct Baseline {
    pub first: u64,
    pub y: f32,
}

pub struct TextTruth {
    pub guid: String,
    pub layout: Option<Vec2>,
    pub baselines: Vec<Baseline>,
    pub line_height_unit: String,
    pub letter_spacing_unit: String,
    pub glyphs: Vec<GlyphTruth>,
}

fn unit(r: &KiwiValue, key: &str) -> String {
    r.get(key).and_then(|v| field_str(v, "units")).unwrap_or("absent").to_string()
}

pub fn truth(raw: &Raw) -> (Vec<TextTruth>, usize) {
    let (mut out, mut without) = (Vec::new(), 0usize);
    for r in raw.live().filter(|r| field_str(r, "type") == Some("TEXT")) {
        let Some(d) = r.get("derivedTextData") else {
            without += 1;
            continue;
        };
        let Some(guid) = record_guid(r) else { continue };
        let baselines = d
            .get("baselines")
            .and_then(KiwiValue::as_array)
            .unwrap_or(&[])
            .iter()
            .map(|b| Baseline { first: b.get("firstCharacter").and_then(u64_of).unwrap_or(0), y: vec2_of(b, "position").map_or(f32::NAN, |p| p.y) })
            .collect();
        out.push(TextTruth {
            guid,
            layout: vec2_of(d, "layoutSize"),
            baselines,
            line_height_unit: unit(r, "lineHeight"),
            letter_spacing_unit: unit(r, "letterSpacing"),
            glyphs: text_glyphs::truth_of(d),
        });
    }
    (out, without)
}

pub(super) fn utf16_at(s: &str, byte: usize) -> u64 {
    s.get(..byte.min(s.len())).map_or(0, |p| p.encode_utf16().count() as u64)
}

/// The three checks for one text layer, and how far off Studio is where a check fails (Studio minus Figma).
///
/// A truncated text (auto-resize Truncate, or a line limit) keeps only the lines Figma shows: Studio's untruncated
/// lines must start with them. Layout size is compared only where the text sizes itself (width and height, or
/// height): for a fixed or truncated box Figma's stored size is the box, not the laid-out text.
pub struct Checks {
    pub breaks: bool,
    pub baselines: bool,
    pub size: Option<bool>,
    pub line_count_delta: i64,
    pub first_baseline_delta: Option<f32>,
    pub size_delta: Option<Vec2>,
    /// `None` when Figma stored no glyphs for the text or its line breaks differ (one wrap difference would fail
    /// every glyph after it).
    pub glyphs: Option<GlyphScore>,
}

pub fn checks(props: &TextProps, size: Vec2, t: &TextTruth) -> Checks {
    let g = designos_render::text_geometry(props, size);
    let truncated = props.auto_resize == TextAutoResize::Truncate || props.max_lines.is_some();
    let starts: Vec<u64> = g.lines.iter().map(|l| utf16_at(&props.content, l.start)).collect();
    let figma: Vec<u64> = t.baselines.iter().map(|b| b.first).collect();
    let n = if truncated { figma.len().min(starts.len()) } else { starts.len() };
    let breaks = if truncated { starts.len() >= figma.len() && starts[..n] == figma[..] } else { starts == figma };
    let off = g.lines.iter().zip(&t.baselines).map(|(l, b)| l.baseline - b.y).find(|d| d.is_nan() || d.abs() > TOL);
    let same_count = if truncated { g.lines.len() >= t.baselines.len() } else { g.lines.len() == t.baselines.len() };
    let size_delta = t.layout.map(|ls| Vec2::new(g.width - ls.x, g.height - ls.y));
    let sizes_itself = !truncated && matches!(props.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height);
    let size_ok = size_delta.is_some_and(|d| d.y.abs() <= TOL && (props.auto_resize != TextAutoResize::WidthAndHeight || d.x.abs() <= TOL));
    let broken = broken_comparator();
    Checks {
        breaks: broken || breaks,
        baselines: broken || (same_count && off.is_none()),
        size: sizes_itself.then_some(broken || size_ok),
        line_count_delta: g.lines.len() as i64 - t.baselines.len() as i64,
        first_baseline_delta: off,
        size_delta,
        glyphs: (breaks && !t.glyphs.is_empty()).then(|| text_glyphs::score(&g, &props.content, &t.glyphs, &figma)),
    }
}

/// `x` rounded to the nearest half pixel, signed.
fn bin(x: f32) -> String {
    format!("{:+.1}", (x * 2.0).round() / 2.0)
}

pub fn part(truth: &[TextTruth], without: usize, doc: &Document, keys: &NodeKeys) -> Part {
    let mut p = Part::new("text_layout", "Text layout", "same line breaks, every baseline and the layout size within 0.5 px of derivedTextData");
    p.print_detail(text_glyphs::X_DELTAS);
    p.print_detail(text_glyphs::ADVANCE_DELTAS);
    p.note(format!("text records without derivedTextData: {without}"));
    p.note(text_glyphs::unit_note(truth));
    p.note(format!("embedded variable Inter drawn wider at weight 700 than 400 (weight applied by shaping): {}", variable_weight_applied()));
    let mut fonts: BTreeSet<(String, u16, bool, FontState)> = BTreeSet::new();
    for t in truth {
        let Some(&id) = keys.by_source.get(&t.guid) else {
            p.unscored("text record with no layer", "the record did not become a layer on a page");
            continue;
        };
        let Some(n) = doc.get(id) else { continue };
        let resolved = resolve_node(doc, n);
        let NodeKind::Text(props) = &resolved.kind else {
            p.unscored("text record that is not a text layer", "imported as another kind");
            continue;
        };
        let styles: Vec<&TextStyle> = std::iter::once(&props.style).chain(props.runs.iter().map(|r| &r.style)).collect();
        let state = styles.iter().map(|s| font_state(s)).max().unwrap_or(FontState::Missing);
        let present = state == FontState::Present;
        for s in &styles {
            fonts.insert((s.font_family.clone(), s.font_weight, s.italic, font_state(s)));
        }
        let c = checks(props, n.size, t);
        let (breaks, baselines, size_ok) = (c.breaks, c.baselines, c.size.unwrap_or(true));
        let hidden = keys.is_hidden(id);
        let scope = state.scope();
        let mut b = vec![format!("{scope}total")];
        if present {
            b.push(format!("auto-resize: {:?}", props.auto_resize));
            b.push(format!("line-height unit: {}", t.line_height_unit));
            b.push(format!("letter-spacing unit: {}", t.letter_spacing_unit));
            b.push(format!("lists: {}", if props.lists.is_empty() { "no" } else { "yes" }));
            b.push(format!("truncation: {}", if props.auto_resize == TextAutoResize::Truncate || props.max_lines.is_some() { "yes" } else { "no" }));
            b.push(format!("mixed runs: {}", if props.runs.is_empty() { "no" } else { "yes" }));
        }
        p.tally(&format!("{scope}check: line breaks"), hidden, breaks);
        p.tally(&format!("{scope}check: baselines"), hidden, baselines);
        match c.size {
            Some(ok) => p.tally(&format!("{scope}check: layout size"), hidden, ok),
            None => p.unscored(&format!("{scope}check: layout size of a fixed or truncated box"), "Figma stores the box there, not the laid-out text"),
        }
        if present {
            match &c.glyphs {
                Some(gs) => text_glyphs::tally(&mut p, scope, hidden, gs),
                None if !t.glyphs.is_empty() => {
                    p.unscored("glyph positions of a text whose line breaks differ", "one wrap difference would fail every later glyph")
                }
                None => {}
            }
        }
        if present && !hidden {
            let mode = format!("{:?}", props.auto_resize);
            if !breaks {
                p.detail(&format!("line count delta when breaks differ: {mode}"), &format!("{:+}", c.line_count_delta));
            }
            if let (false, Some(d)) = (baselines, c.first_baseline_delta) {
                p.detail(&format!("first baseline delta: {mode}, line height {}", t.line_height_unit), &bin(d));
            }
            if let (false, Some(d)) = (size_ok, c.size_delta) {
                p.detail(&format!("layout size delta: {mode}, line height {}", t.line_height_unit), &format!("w {} h {}", bin(d.x), bin(d.y)));
            }
        }
        p.record(t.guid.clone(), hidden, breaks && baselines && size_ok, &b);
    }
    for (f, w, italic, st) in fonts {
        let what = match st {
            FontState::Present => "embedded face covers it",
            FontState::WeightUnverified => "installed family, weight and style not verified",
            FontState::Missing => "missing",
        };
        p.detail("fonts used", &format!("{f} {w}{}: {what}", if italic { " italic" } else { "" }));
    }
    p
}
