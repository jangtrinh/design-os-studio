//! Glyph positions and steps: each glyph Figma stored in `derivedTextData.glyphs` against the cluster Studio's shaping
//! puts at the same character. Figma counts characters in UTF-16 units, so a Studio cluster is found by its start
//! converted from bytes the same way.
//!
//! The headline is the step: from one glyph to the next on the same line, Studio's x at the next stored glyph's
//! character minus its x here, against Figma's `next.x - x`. Both steps hold kerning and letter spacing. Figma's
//! `advance` is not that step (it leaves out kerning and spacing), so it is used only for the last glyph of a line, with
//! the letter spacing at that glyph taken off Studio's width. x positions stay a separate bucket, and so does the glyph
//! that starts a line, which tells an offset from drift.
//!
//! Only texts whose line breaks match are scored: after a different wrap every later glyph would fail for that one reason.

use super::board::{broken_comparator, Part};
use super::raw::{f32_of, u64_of, vec2_of};
use super::text::{utf16_at, TextTruth};
use designos_doc::TextProps;
use designos_fig::kiwi::KiwiValue;
use designos_render::TextGeometry;
use std::collections::HashMap;

/// Largest difference, in px, between Studio's and Figma's x position or step of one glyph.
pub const GLYPH_TOL: f32 = 0.05;

pub const X_BUCKET: &str = "check: glyph x positions";
pub const FIRST_X_BUCKET: &str = "check: glyph x positions, first glyph of a line";
pub const STEP_BUCKET: &str = "check: glyph steps (advance for a line's last glyph)";
pub const LAST_BUCKET: &str = "check: glyph advance of a line's last glyph";
pub const ADVANCE_ALONE_BUCKET: &str = "check: glyph advance alone, as the first version compared it";
pub const WHOLE_BUCKET: &str = "check: glyph x and step, whole text";
pub const X_DELTAS: &str = "glyph x miss, Studio minus Figma (upper edge of 0.05 px bins)";
pub const STEP_DELTAS: &str = "glyph step miss, Studio minus Figma (upper edge of 0.05 px bins)";
pub const ADVANCE_ALONE_SPLIT: &str = "advance-alone misses by Figma's own step";

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
    /// Steps to the next glyph on the line, plus the last glyph of each line by advance.
    pub step_ok: u64,
    pub step_total: u64,
    /// The last glyph of each line, scored by advance, also counted in the step bucket.
    pub last_ok: u64,
    pub last_total: u64,
    /// How the first version scored every glyph (Studio's width less its spacing against the stored advance), and how
    /// its misses split by Figma's own step: equal to the advance, different from it, or the last glyph of a line.
    pub alone_ok: u64,
    pub alone_total: u64,
    pub alone_split: [u64; 3],
    /// Stored glyphs with no Studio cluster at their character (whitespace Studio drops at a wrap, merged clusters).
    pub no_cluster: u64,
    /// Stored glyphs whose Studio cluster is right-to-left (its left edge is not the pen position).
    pub rtl: u64,
    /// A step whose next glyph has no usable Studio cluster.
    pub step_no_cluster: u64,
    /// Studio minus Figma, for the glyphs that miss.
    pub x_misses: Vec<f32>,
    pub step_misses: Vec<f32>,
}

struct Cluster {
    x: f32,
    w: f32,
    rtl: bool,
    byte: usize,
}

/// The letter spacing, in px, of the style that covers byte `at`: its run's, or the base style's.
fn spacing_at(props: &TextProps, at: usize) -> f32 {
    props.runs.iter().find(|r| (r.start..r.end).contains(&at)).map_or(props.style.letter_spacing, |r| r.style.letter_spacing)
}

/// Scores every stored glyph of one text against Studio's clusters in `g`. `line_starts` are the characters Figma
/// starts a line at.
pub fn score(g: &TextGeometry, props: &TextProps, glyphs: &[GlyphTruth], line_starts: &[u64]) -> GlyphScore {
    let mut at: HashMap<u64, Cluster> = HashMap::new();
    for c in g.lines.iter().flat_map(|l| &l.glyphs) {
        at.entry(utf16_at(&props.content, c.start)).or_insert(Cluster { x: c.x, w: c.w, rtl: c.rtl, byte: c.start });
    }
    let broken = broken_comparator();
    let mut s = GlyphScore::default();
    for (i, t) in glyphs.iter().enumerate() {
        let Some(c) = at.get(&t.first) else {
            s.no_cluster += 1;
            continue;
        };
        if c.rtl {
            s.rtl += 1;
            continue;
        }
        s.x_total += 1;
        let dx = c.x - t.x;
        let ok = broken || dx.abs() <= GLYPH_TOL;
        s.x_ok += u64::from(ok);
        if line_starts.contains(&t.first) {
            s.first_total += 1;
            s.first_ok += u64::from(ok);
        }
        if !ok {
            s.x_misses.push(dx);
        }
        let next = glyphs.get(i + 1).filter(|b| (t.y - b.y).abs() <= 0.01 && b.x > t.x);
        let adv = t.advance_px();
        match next {
            Some(b) => match at.get(&b.first).filter(|cb| !cb.rtl) {
                Some(cb) => {
                    s.step_total += 1;
                    let d = (cb.x - c.x) - (b.x - t.x);
                    if broken || d.abs() <= GLYPH_TOL {
                        s.step_ok += 1;
                    } else {
                        s.step_misses.push(d);
                    }
                }
                None => s.step_no_cluster += 1,
            },
            None => {
                if let Some(a) = adv {
                    let d = c.w - spacing_at(props, c.byte) - a;
                    let ok = broken || d.abs() <= GLYPH_TOL;
                    s.step_total += 1;
                    s.last_total += 1;
                    s.step_ok += u64::from(ok);
                    s.last_ok += u64::from(ok);
                    if !ok {
                        s.step_misses.push(d);
                    }
                }
            }
        }
        if let Some(a) = adv {
            s.alone_total += 1;
            let ok = broken || (c.w - spacing_at(props, c.byte) - a).abs() <= GLYPH_TOL;
            s.alone_ok += u64::from(ok);
            if !ok {
                let split = match next {
                    None => 2,
                    Some(b) if ((b.x - t.x) - a).abs() <= GLYPH_TOL => 0,
                    Some(_) => 1,
                };
                s.alone_split[split] += 1;
            }
        }
    }
    s
}

/// The miss `d`, binned by the upper edge of its 0.05 px bin (so 0.06 reads +0.10, never "+0.05"), clamped to two pixels.
fn bin(d: f32) -> String {
    // a hair under the edge, so 0.20 computed as 0.20000002 stays in the 0.20 bin
    let edge = (((d.abs() / GLYPH_TOL) - 1e-3).ceil() * GLYPH_TOL).min(2.0);
    format!("{}{edge:.2}", if d < 0.0 { "-" } else { "+" })
}

pub fn tally(p: &mut Part, scope: &str, hidden: bool, s: &GlyphScore) {
    p.tally_n(&format!("{scope}{X_BUCKET}"), hidden, s.x_ok, s.x_total);
    p.tally_n(&format!("{scope}{FIRST_X_BUCKET}"), hidden, s.first_ok, s.first_total);
    p.tally_n(&format!("{scope}{STEP_BUCKET}"), hidden, s.step_ok, s.step_total);
    p.tally_n(&format!("{scope}{LAST_BUCKET}"), hidden, s.last_ok, s.last_total);
    p.tally_n(&format!("{scope}{ADVANCE_ALONE_BUCKET}"), hidden, s.alone_ok, s.alone_total);
    if s.x_total + s.step_total > 0 {
        p.tally(&format!("{scope}{WHOLE_BUCKET}"), hidden, s.x_ok == s.x_total && s.step_ok == s.step_total);
    }
    p.unscored_n("glyph with no Studio cluster at its character", "whitespace Studio drops at a wrap, or clusters shaped differently", s.no_cluster);
    p.unscored_n("glyph in a right-to-left cluster", "its left edge is not the pen position", s.rtl);
    p.unscored_n("glyph step to a glyph with no usable Studio cluster", "the next stored glyph has no left-to-right cluster to step to", s.step_no_cluster);
    if !hidden {
        for d in &s.x_misses {
            p.detail(X_DELTAS, &bin(*d));
        }
        for d in &s.step_misses {
            p.detail(STEP_DELTAS, &bin(*d));
        }
        for (n, why) in s.alone_split.iter().zip(["Figma's step equals its advance", "Figma's step differs from its advance", "last glyph of a line"]) {
            for _ in 0..*n {
                p.detail(ADVANCE_ALONE_SPLIT, why);
            }
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
