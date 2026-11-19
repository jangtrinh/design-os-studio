//! Glyph positions and advances: each glyph Figma stored in `derivedTextData.glyphs` against the cluster Studio's
//! shaping puts at the same character. Figma counts characters in UTF-16 units, so a Studio cluster is found by its
//! start converted from bytes the same way. x position and advance are separate buckets, because one wrong advance
//! moves every glyph after it on the line. Only texts whose line breaks match are scored: after a different wrap every
//! later glyph would fail for that one reason.

use super::board::{broken_comparator, Part};
use super::raw::{f32_of, u64_of, vec2_of};
use super::text::{utf16_at, TextTruth};
use designos_fig::kiwi::KiwiValue;
use designos_render::TextGeometry;
use std::collections::HashMap;

/// Largest difference, in px, between Studio's and Figma's x position or advance of one glyph.
pub const GLYPH_TOL: f32 = 0.05;

pub const X_BUCKET: &str = "check: glyph x positions";
pub const ADVANCE_BUCKET: &str = "check: glyph advances";
pub const FIRST_X_BUCKET: &str = "check: glyph x positions, first glyph of a line";
pub const WHOLE_BUCKET: &str = "check: glyph x and advance, whole text";
pub const X_DELTAS: &str = "glyph x delta, Studio minus Figma (0.05 px bins)";
pub const ADVANCE_DELTAS: &str = "glyph advance delta, Studio minus Figma (0.05 px bins)";

pub struct GlyphTruth {
    /// The glyph's first character, counted in UTF-16 units.
    pub first: u64,
    pub x: f32,
    pub y: f32,
    /// As stored: a fraction of the glyph's font size (`unit_note` measures it on the file; px is not the unit).
    pub advance: Option<f32>,
    pub font_size: f32,
}

impl GlyphTruth {
    /// The stored advance in px.
    pub fn advance_px(&self) -> Option<f32> {
        self.advance.filter(|_| self.font_size > 0.0).map(|a| a * self.font_size)
    }
}

pub fn truth_of(derived: &KiwiValue) -> Vec<GlyphTruth> {
    let Some(glyphs) = derived.get("glyphs").and_then(KiwiValue::as_array) else { return Vec::new() };
    glyphs
        .iter()
        .filter_map(|g| {
            let p = vec2_of(g, "position")?;
            Some(GlyphTruth {
                first: g.get("firstCharacter").and_then(u64_of)?,
                x: p.x,
                y: p.y,
                advance: f32_of(g, "advance"),
                font_size: f32_of(g, "fontSize").unwrap_or(0.0),
            })
        })
        .collect()
}

#[derive(Default)]
pub struct GlyphScore {
    pub x_ok: u64,
    pub x_total: u64,
    /// The x position of the glyphs that start a line alone: a miss there is an offset (alignment, indent), not drift.
    pub first_ok: u64,
    pub first_total: u64,
    pub adv_ok: u64,
    pub adv_total: u64,
    /// Stored glyphs with no Studio cluster at their character (whitespace Studio drops at a wrap, merged clusters).
    pub no_cluster: u64,
    /// Stored glyphs whose Studio cluster is right-to-left (its left edge is not the pen position).
    pub rtl: u64,
    /// Studio minus Figma, for the glyphs that miss.
    pub x_misses: Vec<f32>,
    pub adv_misses: Vec<f32>,
}

/// Scores every stored glyph of one text against Studio's clusters in `g`. `line_starts` are the characters Figma
/// starts a line at.
pub fn score(g: &TextGeometry, content: &str, glyphs: &[GlyphTruth], line_starts: &[u64]) -> GlyphScore {
    let mut at: HashMap<u64, (f32, f32, bool)> = HashMap::new();
    for c in g.lines.iter().flat_map(|l| &l.glyphs) {
        at.entry(utf16_at(content, c.start)).or_insert((c.x, c.w, c.rtl));
    }
    let broken = broken_comparator();
    let mut s = GlyphScore::default();
    for t in glyphs {
        let Some(&(x, w, rtl)) = at.get(&t.first) else {
            s.no_cluster += 1;
            continue;
        };
        if rtl {
            s.rtl += 1;
            continue;
        }
        s.x_total += 1;
        let dx = x - t.x;
        let ok = broken || dx.abs() <= GLYPH_TOL;
        s.x_ok += u64::from(ok);
        if line_starts.contains(&t.first) {
            s.first_total += 1;
            s.first_ok += u64::from(ok);
        }
        if !ok {
            s.x_misses.push(dx);
        }
        if let Some(a) = t.advance_px() {
            s.adv_total += 1;
            let da = w - a;
            if broken || da.abs() <= GLYPH_TOL {
                s.adv_ok += 1;
            } else {
                s.adv_misses.push(da);
            }
        }
    }
    s
}

/// `d` rounded to the nearest 0.05 px, signed, and clamped to two pixels.
fn bin(d: f32) -> String {
    format!("{:+.2}", ((d / GLYPH_TOL).round() * GLYPH_TOL).clamp(-2.0, 2.0) + 0.0)
}

pub fn tally(p: &mut Part, scope: &str, hidden: bool, s: &GlyphScore) {
    p.tally_n(&format!("{scope}{X_BUCKET}"), hidden, s.x_ok, s.x_total);
    p.tally_n(&format!("{scope}{FIRST_X_BUCKET}"), hidden, s.first_ok, s.first_total);
    p.tally_n(&format!("{scope}{ADVANCE_BUCKET}"), hidden, s.adv_ok, s.adv_total);
    if s.x_total + s.adv_total > 0 {
        p.tally(&format!("{scope}{WHOLE_BUCKET}"), hidden, s.x_ok == s.x_total && s.adv_ok == s.adv_total);
    }
    p.unscored_n("glyph with no Studio cluster at its character", "whitespace Studio drops at a wrap, or clusters shaped differently", s.no_cluster);
    p.unscored_n("glyph in a right-to-left cluster", "its left edge is not the pen position", s.rtl);
    if !hidden {
        for d in &s.x_misses {
            p.detail(X_DELTAS, &bin(*d));
        }
        for d in &s.adv_misses {
            p.detail(ADVANCE_DELTAS, &bin(*d));
        }
    }
}

/// What unit the stored advance is in, measured on the file: on consecutive glyphs of one line, how often the next x
/// equals this x plus the advance taken as px, and how often it equals this x plus the advance times the font size.
pub fn unit_note(truth: &[TextTruth]) -> String {
    let (mut pairs, mut px, mut em) = (0u64, 0u64, 0u64);
    for t in truth {
        for w in t.glyphs.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let Some(adv) = a.advance else { continue };
            if (a.y - b.y).abs() > 0.01 || b.x <= a.x {
                continue;
            }
            pairs += 1;
            let dx = b.x - a.x;
            px += u64::from((dx - adv).abs() <= GLYPH_TOL);
            em += u64::from(a.font_size > 1.0 && (dx - adv * a.font_size).abs() <= GLYPH_TOL);
        }
    }
    format!(
        "glyph advance unit: the next glyph's x is this x plus the stored advance in px on {px} of {pairs} consecutive same-line glyph pairs, and plus the advance times the font size on {em} of {pairs}"
    )
}
