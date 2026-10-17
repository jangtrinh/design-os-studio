//! Paragraph flow: splits a text into paragraphs, shapes each with cosmic-text
//! (indent, list marker columns, first-line indent), stacks them with paragraph
//! and list spacing, then applies leading trim and vertical alignment.
//!
//! Both the renderer ([`super::layout`]) and the editor geometry
//! ([`super::text_geometry`]) are produced from the same [`Flow`], so carets and
//! selections line up with the drawn glyphs.

use super::*;
use cosmic_text::CacheKey;
use designos_doc::{bullet_shape, BulletShape, LeadingTrim, ListMarker, TextDirection};

/// A shaped glyph of a line; `x`/`w` are in box coordinates.
#[derive(Clone, Debug)]
pub(super) struct PGlyph {
    /// Byte range in the paragraph's shaped text.
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub w: f32,
    /// Offset from the baseline.
    pub y: f32,
    pub fs: f32,
    pub x_off: f32,
    pub y_off: f32,
    pub style: usize,
    pub key: CacheKey,
    pub rtl: bool,
}

/// An underline / strikethrough bar over part of a line.
#[derive(Clone, Debug)]
pub(super) struct PDeco {
    pub x0: f32,
    pub x1: f32,
    pub style: usize,
    pub fs: f32,
    /// (offset above the baseline, thickness) in em.
    pub underline: Option<(f32, f32)>,
    pub strike: Option<(f32, f32)>,
}

/// One visual line, positioned in the text box.
#[derive(Clone, Debug)]
pub(super) struct PLine {
    pub para: usize,
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    /// Advance width of the line's glyphs.
    pub width: f32,
    pub rtl: bool,
    /// Where a caret sits on a line without glyphs.
    pub empty_x: f32,
    pub glyphs: Vec<PGlyph>,
    pub decos: Vec<PDeco>,
    /// First line of its paragraph.
    pub first: bool,
    /// End of the line's text in the paragraph's shaped text.
    pub cut: usize,
}

/// A list marker drawn left of (or right of, in RTL paragraphs) a paragraph's first line.
#[derive(Clone, Debug)]
pub(super) struct PMarker {
    /// Index into [`Flow::lines`] of the first line of the paragraph.
    pub line: usize,
    pub shape: Option<BulletShape>,
    pub glyphs: Vec<PGlyph>,
    pub x: f32,
    pub fs: f32,
    pub style: usize,
    /// Cap height ratio of the marker's font.
    pub cap: f32,
}

/// A paragraph ready for shaping.
#[derive(Clone, Debug)]
pub(super) struct Para {
    pub segs: Vec<Segment>,
    /// All segments concatenated.
    pub text: String,
    /// Shaped byte -> byte of `TextProps::content` (empty unless requested); one extra
    /// trailing entry for the end.
    pub map: Vec<usize>,
    /// Content range of the paragraph (without the newline).
    pub start: usize,
    pub end: usize,
    /// Bytes at the start of `text` that are an invisible direction mark.
    pub mark_len: usize,
    pub rtl: bool,
    pub marker: Option<ListMarker>,
}

pub(super) struct Flow {
    pub lines: Vec<PLine>,
    pub markers: Vec<PMarker>,
    /// Widest line extent (indent included).
    pub width: f32,
    pub height: f32,
    /// Added to untrimmed, unaligned tops to get the final ones.
    pub shift: f32,
}

fn is_rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Whether the first strong character of `s` is right-to-left.
pub(super) fn first_strong_rtl(s: &str) -> bool {
    for c in s.chars() {
        if c.is_alphabetic() {
            return is_rtl_char(c);
        }
    }
    false
}

/// Splits the content into shaping-ready paragraphs (case transforms applied).
pub(super) fn split_paragraphs(props: &TextProps, want_map: bool) -> Vec<Para> {
    let content = &props.content;
    let ranges = d::paragraph_ranges(content);
    let markers = props.list_markers();
    let case_of = |idx: usize| if idx == 0 { props.style.case } else { props.runs[idx - 1].style.case };
    let style_at = |b: usize| -> usize {
        let mut s = 0;
        for (i, r) in props.runs.iter().enumerate() {
            if b >= r.start && b < r.end {
                s = i + 1;
            }
        }
        s
    };
    let mut out = Vec::with_capacity(ranges.len());
    for (pi, r) in ranges.iter().enumerate() {
        let mut segs: Vec<Segment> = Vec::new();
        let mut map: Vec<usize> = Vec::new();
        let mut word_start = true;
        for (i, ch) in content[r.clone()].char_indices() {
            let b = r.start + i;
            let idx = style_at(b);
            let mut small = false;
            let piece: String = match case_of(idx) {
                d::TextCase::Original => ch.to_string(),
                d::TextCase::Upper => ch.to_uppercase().collect(),
                d::TextCase::Lower => ch.to_lowercase().collect(),
                d::TextCase::Title => {
                    if word_start && ch.is_alphabetic() {
                        ch.to_uppercase().collect()
                    } else {
                        ch.to_string()
                    }
                }
                d::TextCase::SmallCaps => {
                    if ch.is_lowercase() {
                        small = true;
                        ch.to_uppercase().collect()
                    } else {
                        ch.to_string()
                    }
                }
            };
            word_start = !(ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}');
            if want_map {
                map.extend(std::iter::repeat_n(b, piece.len()));
            }
            match segs.last_mut() {
                Some(last) if last.style == idx && last.small == small => last.text.push_str(&piece),
                _ => segs.push(Segment { text: piece, style: idx, small }),
            }
        }
        let rtl = match props.direction {
            TextDirection::Rtl => true,
            TextDirection::Ltr => false,
            TextDirection::Auto => first_strong_rtl(&content[r.clone()]),
        };
        // A forced direction is expressed with an invisible strong mark at the start.
        let mark = match props.direction {
            TextDirection::Auto => None,
            TextDirection::Ltr => Some('\u{200E}'),
            TextDirection::Rtl => Some('\u{200F}'),
        };
        let mut mark_len = 0;
        if let (Some(m), Some(first)) = (mark, segs.first_mut()) {
            first.text.insert(0, m);
            mark_len = m.len_utf8();
            if want_map {
                map.splice(0..0, std::iter::repeat_n(r.start, mark_len));
            }
        }
        if want_map {
            map.push(r.end);
        }
        let text: String = segs.iter().map(|s| s.text.as_str()).collect();
        out.push(Para { segs, text, map, start: r.start, end: r.end, mark_len, rtl, marker: markers.get(pi).copied().flatten() });
    }
    out
}

/// Splits segments at byte `at` of their concatenated text.
fn split_segments(segs: &[Segment], mut at: usize) -> (Vec<Segment>, Vec<Segment>) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for s in segs {
        if at >= s.text.len() {
            at -= s.text.len();
            a.push(Segment { text: s.text.clone(), style: s.style, small: s.small });
        } else if at == 0 {
            b.push(Segment { text: s.text.clone(), style: s.style, small: s.small });
        } else {
            a.push(Segment { text: s.text[..at].to_string(), style: s.style, small: s.small });
            b.push(Segment { text: s.text[at..].to_string(), style: s.style, small: s.small });
            at = 0;
        }
    }
    (a, b)
}

/// One laid out line of a block, relative to the block's origin.
struct RawLine {
    top: f32,
    height: f32,
    baseline: f32,
    width: f32,
    rtl: bool,
    glyphs: Vec<PGlyph>,
    decos: Vec<PDeco>,
    cut: usize,
}

fn shape_block(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, segs: &[Segment], width: Option<f32>, wrap: Wrap) -> Vec<RawLine> {
    let buffer = shaper.buffer(&mut fonts.fs, props, width, wrap, segs);
    let nstyles = shaper.styles.len();
    let mut out = Vec::new();
    for run in buffer.layout_runs() {
        let glyphs: Vec<PGlyph> = run
            .glyphs
            .iter()
            .map(|g| PGlyph {
                start: g.start,
                end: g.end,
                x: g.x,
                w: g.w,
                y: g.y,
                fs: g.font_size,
                x_off: g.x_offset,
                y_off: g.y_offset,
                style: g.metadata.min(nstyles - 1),
                key: g.physical((0.0, 0.0), 1.0).cache_key,
                rtl: g.level.is_rtl(),
            })
            .collect();
        let mut decos = Vec::new();
        for span in run.decorations {
            let gl = &run.glyphs[span.glyph_range.clone()];
            let Some(first) = gl.first() else { continue };
            let (mut x0, mut x1) = (f32::MAX, f32::MIN);
            for g in gl {
                x0 = x0.min(g.x);
                x1 = x1.max(g.x + g.w);
            }
            if x1 <= x0 {
                continue;
            }
            let td = &span.data.text_decoration;
            decos.push(PDeco {
                x0,
                x1,
                style: first.metadata.min(nstyles - 1),
                fs: span.font_size,
                underline: (!matches!(td.underline, UnderlineStyle::None))
                    .then_some((span.data.underline_metrics.offset, span.data.underline_metrics.thickness)),
                strike: td.strikethrough.then_some((span.data.strikethrough_metrics.offset, span.data.strikethrough_metrics.thickness)),
            });
        }
        let cut = glyphs.iter().map(|g| g.end).max().unwrap_or(0);
        out.push(RawLine { top: run.line_top, height: run.line_height, baseline: run.line_y, width: run.line_w, rtl: run.rtl, glyphs, decos, cut });
    }
    if out.is_empty() {
        let lh = shaper.line_height_for(0, props.style.font_size.max(0.1));
        out.push(RawLine { top: 0.0, height: lh, baseline: lh * 0.8, width: 0.0, rtl: false, glyphs: vec![], decos: vec![], cut: 0 });
    }
    out
}

/// Width and word count of shaped lines, for choosing a `text-wrap` width.
fn line_stats(lines: &[RawLine], text: &str) -> Vec<wrap::LineStat> {
    lines
        .iter()
        .map(|l| {
            let (start, end) = (l.glyphs.iter().map(|g| g.start).min().unwrap_or(0), l.glyphs.iter().map(|g| g.end).max().unwrap_or(0));
            wrap::LineStat { width: l.width, words: text.get(start..end).map_or(0, |t| t.split_whitespace().count()) }
        })
        .collect()
}

/// Shapes a paragraph in `avail` px and, for `text-wrap: balance | pretty`, re-breaks it at the
/// width the style picks. Returns the lines, the shift that keeps the lines aligned inside the
/// full `avail` box, and the width they were broken at.
fn shape_paragraph(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, para: &Para, avail: f32, wrap_mode: Wrap) -> (Vec<RawLine>, f32, f32) {
    let greedy = shape_block(fonts, shaper, props, &para.segs, Some(avail), wrap_mode);
    // es-debt: justified text keeps greedy breaks (the lines would justify to the narrower width), upgrade when justify is measured against the full box
    let applies = props.text_wrap != d::TextWrap::Auto && wrap_mode != Wrap::None && greedy.len() > 1 && props.style.align_h != d::TextAlignH::Justified;
    if !applies {
        return (greedy, 0.0, avail);
    }
    let stats = line_stats(&greedy, &para.text);
    let width =
        wrap::wrap_width(props.text_wrap, avail, &stats, &mut |w| line_stats(&shape_block(fonts, shaper, props, &para.segs, Some(w), wrap_mode), &para.text));
    if avail - width <= 0.01 {
        return (greedy, 0.0, avail);
    }
    let shift = match props.style.align_h {
        d::TextAlignH::Center => (avail - width) / 2.0,
        d::TextAlignH::Right => avail - width,
        _ => 0.0,
    };
    (shape_block(fonts, shaper, props, &para.segs, Some(width), wrap_mode), shift, width)
}

fn is_opening_punct(c: char) -> bool {
    matches!(c, '"' | '\'' | '(' | '[' | '{' | '\u{201C}' | '\u{2018}' | '\u{00AB}' | '\u{201E}' | '\u{201A}' | '\u{2039}' | '\u{00BF}' | '\u{00A1}')
}

fn is_closing_punct(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | ')' | ']' | '}' | '.' | ',' | ';' | ':' | '!' | '?' | '-' | '\u{201D}' | '\u{2019}' | '\u{00BB}' | '\u{2026}' | '\u{2013}' | '\u{2014}'
    )
}

struct Pass {
    lines: Vec<PLine>,
    markers: Vec<PMarker>,
    extent: f32,
    bottom: f32,
}

fn flow_pass(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, paras: &[Para], box_w: Option<f32>) -> Pass {
    let wrap = match props.auto_resize {
        TextAutoResize::WidthAndHeight => Wrap::None,
        _ => Wrap::WordOrGlyph,
    };
    let base_fs = props.style.font_size.max(0.1);
    let unit = base_fs * 1.5;
    let hang = if props.paragraph.hanging_list { -unit } else { 0.0 };
    let align_h = props.style.align_h;
    let mut lines: Vec<PLine> = Vec::new();
    let mut markers: Vec<PMarker> = Vec::new();
    let mut y = 0.0f32;
    let mut bottom = 0.0f32;
    let mut extent = 0.0f32;
    for (pi, para) in paras.iter().enumerate() {
        let style0 = para.segs.first().map(|s| s.style).unwrap_or(0);
        let fs0 = shaper.styles[style0].font_size.max(0.1);
        let rtl_para = para.rtl && box_w.is_some() && para.marker.is_some();

        // list column
        let mut text_x = 0.0f32;
        let mut marker: Option<(Option<BulletShape>, Vec<PGlyph>, f32, f32)> = None; // shape, glyphs, width, x (ltr)
        if let Some(m) = para.marker {
            let mx = hang + m.level as f32 * unit;
            let (shape, glyphs, lw) = match m.label() {
                Some(label) => {
                    let (g, w) = label_glyphs(fonts, shaper, props, style0, &label);
                    (None, g, w)
                }
                None => (Some(bullet_shape(m.level)), Vec::new(), fs0 * 0.3),
            };
            let ref_w = if m.kind == d::ListKind::Numbered && m.level % 3 == 0 { label_glyphs(fonts, shaper, props, style0, "88.").1.max(lw) } else { lw };
            text_x = (hang + (m.level as f32 + 1.0) * unit).max(mx + ref_w + fs0 * 0.25);
            marker = Some((shape, glyphs, lw, mx));
        }
        let region_left = if rtl_para { 0.0 } else { text_x };
        let avail = box_w.map(|w| (w - text_x.max(0.0)).max(1.0));

        // first-line indent of ordinary paragraphs
        let fi = if para.marker.is_none() && !para.text.is_empty() { props.paragraph.indent.max(0.0) } else { 0.0 };
        let first_avail = avail.map(|a| (a - fi).max(1.0));
        let first_dx = region_left + if para.rtl && box_w.is_some() { 0.0 } else { fi };
        let mut blocks: Vec<(Vec<RawLine>, f32, usize, Option<f32>)> = Vec::new();
        // text-wrap balance / pretty applies to paragraphs without a first-line indent
        let (first, wrap_shift, wrap_w) = match avail {
            Some(a) if fi == 0.0 => {
                let (lines, shift, w) = shape_paragraph(fonts, shaper, props, para, a, wrap);
                (lines, shift, Some(w))
            }
            _ => (shape_block(fonts, shaper, props, &para.segs, if fi > 0.0 { first_avail } else { avail }, wrap), 0.0, avail),
        };
        if fi > 0.0 && first.len() > 1 && avail.is_some() && wrap != Wrap::None {
            let cut = first[0].cut.min(para.text.len());
            let (_, mut rest) = split_segments(&para.segs, cut);
            // the break swallowed the space at the wrap
            let mut skipped = 0;
            while let Some(s) = rest.first_mut() {
                let trimmed = s.text.trim_start_matches(' ').len();
                skipped += s.text.len() - trimmed;
                if trimmed == 0 {
                    rest.remove(0);
                } else {
                    s.text = s.text[s.text.len() - trimmed..].to_string();
                    break;
                }
            }
            let rest_lines = shape_block(fonts, shaper, props, &rest, avail, wrap);
            let first_line = first.into_iter().next().unwrap();
            blocks.push((vec![first_line], first_dx, 0, first_avail));
            blocks.push((rest_lines, region_left, cut + skipped, avail));
        } else {
            blocks.push((first, if fi > 0.0 { first_dx } else { region_left + wrap_shift }, 0, if fi > 0.0 { first_avail } else { wrap_w }));
        }

        let para_first_line = lines.len();
        let mut block_y = y;
        for (bi, (raw, dx, off, bavail)) in blocks.into_iter().enumerate() {
            let block_h = raw.last().map(|l| l.top + l.height).unwrap_or(0.0);
            for (li, rl) in raw.into_iter().enumerate() {
                let empty_x = match align_h {
                    d::TextAlignH::Center => dx + bavail.unwrap_or(0.0) / 2.0,
                    d::TextAlignH::Right => dx + bavail.unwrap_or(0.0),
                    _ => dx,
                };
                let mut glyphs = rl.glyphs;
                for g in &mut glyphs {
                    g.x += dx;
                    g.start += off;
                    g.end += off;
                }
                let mut decos = rl.decos;
                for dc in &mut decos {
                    dc.x0 += dx;
                    dc.x1 += dx;
                }
                if !glyphs.is_empty() {
                    extent = extent.max(dx + rl.width);
                }
                lines.push(PLine {
                    para: pi,
                    top: block_y + rl.top,
                    height: rl.height,
                    baseline: block_y + rl.baseline,
                    width: rl.width,
                    rtl: rl.rtl,
                    empty_x,
                    glyphs,
                    decos,
                    first: bi == 0 && li == 0,
                    cut: rl.cut + off,
                });
            }
            block_y += block_h;
        }
        let para_bottom = block_y;
        bottom = para_bottom;

        // hanging punctuation
        if props.paragraph.hanging_punctuation && box_w.is_some() {
            hang_punctuation(&mut lines[para_first_line..], &para.text, align_h);
        }

        if let Some((shape, glyphs, lw, mx)) = marker {
            let x = if rtl_para { box_w.unwrap_or(0.0) - mx - lw } else { mx };
            markers.push(PMarker { line: para_first_line, shape, glyphs, x, fs: fs0, style: style0, cap: shaper.caps[style0] });
        }

        let next_list = paras.get(pi + 1).is_some_and(|n| n.marker.is_some());
        let gap = if para.marker.is_some() && next_list { props.paragraph.list_spacing } else { props.style.paragraph_spacing };
        y = para_bottom + gap;
    }
    Pass { lines, markers, extent, bottom }
}

fn hang_punctuation(lines: &mut [PLine], text: &str, align: d::TextAlignH) {
    for line in lines.iter_mut() {
        if line.rtl {
            continue;
        }
        match align {
            d::TextAlignH::Left | d::TextAlignH::Justified if line.first => {
                if let Some(g) = line.glyphs.first().cloned() {
                    if text.get(g.start..g.end).and_then(|s| s.chars().next()).is_some_and(is_opening_punct) {
                        let w = g.w;
                        for g in &mut line.glyphs {
                            g.x -= w;
                        }
                        for dc in &mut line.decos {
                            dc.x0 -= w;
                            dc.x1 -= w;
                        }
                        line.empty_x -= w;
                    }
                }
            }
            d::TextAlignH::Right => {
                if let Some(g) = line.glyphs.last().cloned() {
                    if text.get(g.start..g.end).and_then(|s| s.chars().next_back()).is_some_and(is_closing_punct) {
                        let w = g.w;
                        for g in &mut line.glyphs {
                            g.x += w;
                        }
                        for dc in &mut line.decos {
                            dc.x0 += w;
                            dc.x1 += w;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Shaped glyphs of a list label in the style of a paragraph, and their width.
fn label_glyphs(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, style: usize, label: &str) -> (Vec<PGlyph>, f32) {
    let seg = Segment { text: label.to_string(), style, small: false };
    let mut lines = shape_block(fonts, shaper, props, std::slice::from_ref(&seg), None, Wrap::None);
    let l = lines.remove(0);
    (l.glyphs, l.width)
}

/// Lays out all paragraphs in a box of `size`, with leading trim and vertical alignment applied.
pub(super) fn flow(fonts: &mut Fonts, shaper: &Shaper, props: &TextProps, size: Vec2, paras: &[Para]) -> Flow {
    let box_w = if size.x > 0.0 { Some(size.x) } else { None };
    let mut pass = flow_pass(fonts, shaper, props, paras, box_w);
    // Without a fixed width, multi-line alignment is relative to the widest line.
    if box_w.is_none() && props.style.align_h != d::TextAlignH::Left && pass.lines.len() > 1 {
        pass = flow_pass(fonts, shaper, props, paras, Some(pass.extent.max(1.0)));
    }
    let Pass { mut lines, markers, extent, bottom } = pass;
    let (mut top_trim, mut bottom_trim) = (0.0f32, 0.0f32);
    if props.paragraph.leading_trim == LeadingTrim::CapHeight {
        if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
            let (fs, st) = first.glyphs.first().map(|g| (g.fs, g.style)).unwrap_or((shaper.styles[0].font_size.max(0.1), 0));
            top_trim = (first.baseline - shaper.caps[st] * fs - first.top).max(0.0);
            bottom_trim = (last.top + last.height - last.baseline).max(0.0);
        }
    }
    let total_h = (bottom - top_trim - bottom_trim).max(0.0);
    let oy = if size.y > 0.0 {
        match props.style.align_v {
            d::TextAlignV::Top => 0.0,
            d::TextAlignV::Center => (size.y - total_h) / 2.0,
            d::TextAlignV::Bottom => size.y - total_h,
        }
    } else {
        0.0
    };
    let shift = oy - top_trim;
    for l in &mut lines {
        l.top += shift;
        l.baseline += shift;
    }
    Flow { lines, markers, width: extent, height: total_h, shift }
}

// ------------------------------------------------------------------ drawing

fn circle(pb: &mut sk::PathBuilder, cx: f32, cy: f32, r: f32, ccw: bool) {
    const K: f32 = 0.552_284_8;
    let k = r * K;
    let s = if ccw { -1.0 } else { 1.0 };
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + s * k, cx + k, cy + s * r, cx, cy + s * r);
    pb.cubic_to(cx - k, cy + s * r, cx - r, cy + s * k, cx - r, cy);
    pb.cubic_to(cx - r, cy - s * k, cx - k, cy - s * r, cx, cy - s * r);
    pb.cubic_to(cx + k, cy - s * r, cx + r, cy - s * k, cx + r, cy);
    pb.close();
}

/// Adds the marker's outline to `pb` (the builder of its style).
pub(super) fn marker_outline(fonts: &mut Fonts, pb: &mut sk::PathBuilder, m: &PMarker, baseline: f32) {
    if let Some(shape) = m.shape {
        let cy = baseline - 0.5 * m.cap * m.fs;
        let d = m.fs * 0.3;
        let (cx, r) = (m.x + d / 2.0, d / 2.0);
        match shape {
            BulletShape::Disc => circle(pb, cx, cy, r, false),
            BulletShape::Ring => {
                let t = m.fs * 0.07;
                circle(pb, cx, cy, r, false);
                circle(pb, cx, cy, (r - t).max(0.1), true);
            }
            BulletShape::Square => {
                let s = d * 0.85;
                if let Some(rc) = sk::Rect::from_xywh(cx - s / 2.0, cy - s / 2.0, s, s) {
                    pb.push_rect(rc);
                }
            }
        }
    } else {
        for g in &m.glyphs {
            glyph_outline(fonts, pb, g, m.x + g.x, baseline);
        }
    }
}

/// Appends the outline of `g` placed with its origin at (`x`, `baseline` + glyph offset).
pub(super) fn glyph_outline(fonts: &mut Fonts, pb: &mut sk::PathBuilder, g: &PGlyph, x: f32, baseline: f32) {
    let gx = x + g.fs * g.x_off;
    let gy = baseline + g.y - g.fs * g.y_off;
    let Some(cmds) = fonts.swash.get_outline_commands(&mut fonts.fs, g.key) else { return };
    for c in cmds {
        match c {
            Command::MoveTo(p) => pb.move_to(gx + p.x, gy - p.y),
            Command::LineTo(p) => pb.line_to(gx + p.x, gy - p.y),
            Command::QuadTo(c1, p) => pb.quad_to(gx + c1.x, gy - c1.y, gx + p.x, gy - p.y),
            Command::CurveTo(c1, c2, p) => pb.cubic_to(gx + c1.x, gy - c1.y, gx + c2.x, gy - c2.y, gx + p.x, gy - p.y),
            Command::Close => pb.close(),
        }
    }
}
