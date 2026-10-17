//! Text measurement and caret geometry for the editor.
//!
//! Everything is derived from `designos_render::text_geometry`, which shapes
//! with exactly the same code as the renderer (same fonts, runs, case, wrap),
//! so carets and selections line up with the drawn glyphs. Positions are
//! UTF-8 byte offsets into `TextProps::content` and node-local pixels.

use designos_doc as d;
use designos_doc::{TextProps, Vec2};
use designos_render::{text_geometry, LineGeom, TextGeometry};
use std::ops::Range;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

/// Natural size of the text: widest line by total height. `width` wraps lines.
pub fn measure(props: &TextProps, width: Option<f32>) -> Vec2 {
    let mut p = props.clone();
    let size = match width {
        Some(w) => {
            if p.auto_resize == d::TextAutoResize::WidthAndHeight {
                p.auto_resize = d::TextAutoResize::Height;
            }
            Vec2::new(w, 0.0)
        }
        None => {
            p.auto_resize = d::TextAutoResize::WidthAndHeight;
            Vec2::ZERO
        }
    };
    let g = text_geometry(&p, size);
    Vec2::new(g.width.ceil().max(1.0), g.height.ceil().max(1.0))
}

/// Geometry for editing: laid out in the node's real box.
pub fn geometry(props: &TextProps, size: Vec2) -> Rc<TextGeometry> {
    text_geometry(props, size)
}

/// Index of the visual line a caret at byte `at` is drawn on. A caret at a soft
/// wrap boundary belongs to the next line (as in Figma).
pub fn line_of(g: &TextGeometry, at: usize) -> usize {
    // Lines drop the whitespace they wrap at, so an offset in that gap stays at the end of its line.
    g.lines.iter().rposition(|l| l.start <= at).unwrap_or(0)
}

// ------------------------------------------------------------------ caret stops (bidi)
/// A place the caret can sit on a line: its x and the byte offset it stands for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub x: f32,
    pub at: usize,
    /// Leading edge of the cluster that starts at `at`.
    pub lead: bool,
    /// Trailing edge of the cluster that ends at `at`.
    pub trail: bool,
}

/// Grapheme boundaries inside a cluster, as offsets from its start (starts with 0, ends with the length).
fn grapheme_bounds(s: &str) -> Vec<usize> {
    let mut b: Vec<usize> = s.grapheme_indices(true).map(|(i, _)| i).collect();
    b.push(s.len());
    b
}

/// The caret stops of a line in visual (left to right, non-decreasing x) order. A right-to-left
/// cluster has its logical start at its right edge, so offsets run backwards along right-to-left
/// runs. Where runs of different directions meet, stops of different offsets share an x (and an
/// offset can appear at two x positions: the end of one run and the start of the next).
pub fn caret_stops(content: &str, l: &LineGeom) -> Vec<Stop> {
    if l.glyphs.is_empty() {
        return vec![Stop { x: l.empty_x, at: l.start, lead: true, trail: true }];
    }
    let mut out: Vec<Stop> = Vec::new();
    for g in &l.glyphs {
        let text = content.get(g.start..g.end).unwrap_or("");
        let b = grapheme_bounds(text);
        let k = (b.len() - 1).max(1);
        let mut edges: Vec<Stop> = (0..=k)
            .map(|j| {
                let frac = j as f32 / k as f32;
                let x = if g.rtl { g.x + g.w * (1.0 - frac) } else { g.x + g.w * frac };
                Stop { x, at: g.start + b.get(j).copied().unwrap_or(g.end - g.start), lead: j == 0, trail: j == k }
            })
            .collect();
        if g.rtl {
            edges.reverse();
        }
        for e in edges {
            match out.last_mut() {
                Some(last) if last.at == e.at && (last.x - e.x).abs() < 0.001 => {
                    last.lead |= e.lead;
                    last.trail |= e.trail;
                }
                _ => out.push(e),
            }
        }
    }
    out
}

/// The stop a caret at `at` is drawn on: with a `hint` x (where the caret was last placed) the
/// stop of that offset nearest to it; otherwise the trailing edge of the text before the offset
/// (so typing continues where it left off), else the leading edge of the text after it.
fn stop_for(stops: &[Stop], at: usize, hint: Option<f32>) -> Option<usize> {
    let cands = || stops.iter().enumerate().filter(|(_, s)| s.at == at);
    if let Some(h) = hint {
        if let Some((i, _)) = cands().min_by(|a, b| (a.1.x - h).abs().partial_cmp(&(b.1.x - h).abs()).unwrap_or(std::cmp::Ordering::Equal)) {
            return Some(i);
        }
    }
    cands().find(|(_, s)| s.trail).or_else(|| cands().find(|(_, s)| s.lead)).or_else(|| cands().next()).map(|(i, _)| i)
}

fn x_in_line(content: &str, l: &LineGeom, at: usize, hint: Option<f32>) -> f32 {
    let stops = caret_stops(content, l);
    match stop_for(&stops, at, hint) {
        Some(i) => stops[i].x,
        // a gap (dropped wrap whitespace) or outside the line: the nearest offset
        None => stops.iter().min_by_key(|s| (s.at.abs_diff(at), !s.trail)).map(|s| s.x).unwrap_or(l.empty_x),
    }
}

/// Caret rectangle (x, y, height) in node-local space for a byte offset.
pub fn caret_rect(props: &TextProps, size: Vec2, at: usize) -> (f32, f32, f32) {
    caret_rect_hint(props, size, at, None)
}

/// [`caret_rect`] where the offset has two positions on screen (the boundary of a right-to-left
/// and a left-to-right run): the one nearest to `hint`.
pub fn caret_rect_hint(props: &TextProps, size: Vec2, at: usize, hint: Option<f32>) -> (f32, f32, f32) {
    let g = geometry(props, size);
    let l = &g.lines[line_of(&g, at)];
    (x_in_line(&props.content, l, at, hint), l.top, l.height)
}

/// Selection rectangles (x, y, w, h) for the byte range, one per visual line, or several where a
/// line mixes directions and the selection is not contiguous on screen.
pub fn selection_rects(props: &TextProps, size: Vec2, range: Range<usize>) -> Vec<(f32, f32, f32, f32)> {
    selection_rects_in(&geometry(props, size), &props.content, range)
}

/// [`selection_rects`] over precomputed geometry.
pub fn selection_rects_in(g: &TextGeometry, content: &str, range: Range<usize>) -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    if range.is_empty() {
        return out;
    }
    let n = g.lines.len();
    for (i, l) in g.lines.iter().enumerate() {
        let next_same_para = g.lines.get(i + 1).is_some_and(|nl| nl.para == l.para);
        // The newline ending a paragraph is selected too when the range runs past it.
        let nl_selected = !next_same_para && i + 1 < n && range.end > l.end && range.start <= l.end;
        let lo = range.start.max(l.start);
        let hi = range.end.min(l.end);
        if lo >= hi && !nl_selected {
            // empty line fully inside the range: show a small newline marker
            if l.glyphs.is_empty() && range.start <= l.start && range.end > l.start {
                out.push((l.empty_x, l.top, 4.0, l.height));
            }
            continue;
        }
        // x intervals of the selected part of every cluster
        let mut spans: Vec<(f32, f32)> = Vec::new();
        for gl in &l.glyphs {
            let (s, e) = (gl.start.max(lo), gl.end.min(hi));
            if s >= e {
                continue;
            }
            let total = content.get(gl.start..gl.end).map_or(1, |t| t.chars().count()).max(1) as f32;
            let before = content.get(gl.start..s).map_or(0, |t| t.chars().count()) as f32;
            let count = content.get(s..e).map_or(0, |t| t.chars().count()).max(1) as f32;
            let (x0, x1) = if gl.rtl {
                let x1 = gl.x + gl.w * (1.0 - before / total);
                (x1 - gl.w * count / total, x1)
            } else {
                let x0 = gl.x + gl.w * before / total;
                (x0, x0 + gl.w * count / total)
            };
            spans.push((x0, x1));
        }
        spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for s in spans {
            match merged.last_mut() {
                Some(last) if s.0 <= last.1 + 0.01 => last.1 = last.1.max(s.1),
                _ => merged.push(s),
            }
        }
        if nl_selected {
            // the paragraph break is drawn as a small stub at the end of the line (its left end when it runs right to left)
            if merged.is_empty() {
                let end = l.glyphs.iter().map(|g| g.x + g.w).fold(None, |a: Option<f32>, v| Some(a.map_or(v, |a| a.max(v)))).unwrap_or(l.empty_x);
                merged.push((end, end));
            }
            if l.rtl {
                merged[0].0 -= 4.0;
            } else if let Some(last) = merged.last_mut() {
                last.1 += 4.0;
            }
        }
        for (x0, x1) in merged {
            out.push((x0, l.top, (x1 - x0).max(0.0), l.height));
        }
    }
    out
}

/// Byte offset nearest to the node-local point (x, y).
pub fn hit_caret(props: &TextProps, size: Vec2, x: f32, y: f32) -> usize {
    hit_in(&geometry(props, size), &props.content, x, y).0
}

/// Like [`hit_caret`] but also returns the x of the caret stop that was hit, to pass as the hint
/// of [`caret_rect_hint`].
pub fn hit_stop(props: &TextProps, size: Vec2, x: f32, y: f32) -> (usize, f32) {
    hit_in(&geometry(props, size), &props.content, x, y)
}

/// [`hit_stop`] over precomputed geometry.
pub fn hit_in(g: &TextGeometry, content: &str, x: f32, y: f32) -> (usize, f32) {
    let li = g.lines.iter().rposition(|l| l.top <= y).unwrap_or(0);
    let l = &g.lines[li];
    let wrapped = g.lines.get(li + 1).is_some_and(|n| n.para == l.para);
    if l.glyphs.is_empty() {
        return (l.start, l.empty_x);
    }
    let stops = caret_stops(content, l);
    // the nearest stop; on ties the trailing edge of the text before it
    let mut best = stops[0];
    let mut dist = f32::MAX;
    for s in &stops {
        let d = (s.x - x).abs();
        if d < dist - 1e-4 || ((d - dist).abs() <= 1e-4 && s.trail && !best.trail) {
            best = *s;
            dist = d;
        }
    }
    // a caret after a wrap-trailing space would render on the next line
    if wrapped && best.at == l.end {
        if let Some(last) = l.glyphs.iter().max_by_key(|g| g.end) {
            if content.get(last.start..last.end).is_some_and(|t| t.chars().all(char::is_whitespace)) {
                return (last.start, stops.iter().find(|s| s.at == last.start).map_or(best.x, |s| s.x));
            }
        }
    }
    (best.at, best.x)
}

/// Start and end byte offsets of the visual line holding `at` (Home / End).
pub fn line_bounds(props: &TextProps, size: Vec2, at: usize) -> (usize, usize) {
    let g = geometry(props, size);
    let l = &g.lines[line_of(&g, at)];
    let wrapped = g.lines.get(line_of(&g, at) + 1).is_some_and(|n| n.para == l.para);
    let mut end = l.end;
    if wrapped {
        // keep the caret on this line: before a trailing space
        if let Some(last) = l.glyphs.iter().max_by_key(|g| g.end) {
            if props.content[last.start..last.end].chars().all(char::is_whitespace) {
                end = last.start;
            }
        }
    }
    (l.start, end)
}

/// Whether the run holding the caret at `at` reads right to left (arrow keys and word jumps
/// follow the screen, not the text order).
pub fn run_rtl(props: &TextProps, size: Vec2, at: usize) -> bool {
    run_rtl_in(&geometry(props, size), at)
}

/// [`run_rtl`] over precomputed geometry. A caret between runs takes the direction of the
/// cluster that ends at `at` (what is being typed), else of the one that starts there; empty
/// lines use the paragraph direction.
pub fn run_rtl_in(g: &TextGeometry, at: usize) -> bool {
    let l = &g.lines[line_of(g, at)];
    l.glyphs.iter().find(|gl| gl.start < at && at <= gl.end).or_else(|| l.glyphs.iter().find(|gl| gl.start <= at && at < gl.end)).map_or(l.rtl, |gl| gl.rtl)
}

/// Whether the line holding `at` is a right-to-left paragraph.
pub fn line_rtl(props: &TextProps, size: Vec2, at: usize) -> bool {
    let g = geometry(props, size);
    g.lines[line_of(&g, at)].rtl
}

/// Caret one step to the left (`dir` < 0) or right (> 0) on screen within its line, over the
/// caret stops, with the x it lands on (pass it on as the hint). Stops at the same x as the caret
/// are skipped, so every key press moves visibly. `None` when the caret is already at that end
/// of the line.
pub fn move_visual(props: &TextProps, size: Vec2, at: usize, hint: Option<f32>, dir: i32) -> Option<(usize, f32)> {
    move_visual_in(&geometry(props, size), &props.content, at, hint, dir)
}

/// [`move_visual`] over precomputed geometry.
pub fn move_visual_in(g: &TextGeometry, content: &str, at: usize, hint: Option<f32>, dir: i32) -> Option<(usize, f32)> {
    let l = &g.lines[line_of(g, at)];
    let stops = caret_stops(content, l);
    let cur = stop_for(&stops, at, hint)?;
    let step = dir.signum();
    if step == 0 {
        return None;
    }
    let mut i = cur as i32 + step;
    while stops.get(usize::try_from(i).ok()?).is_some_and(|s| (s.x - stops[cur].x).abs() < 0.001) {
        i += step;
    }
    stops.get(usize::try_from(i).ok()?).map(|s| (s.at, s.x))
}

/// Caret one visual line up (-1) or down (+1) keeping the column `goal_x`.
/// At the first/last line it goes to the start/end of the text.
pub fn move_vertical(props: &TextProps, size: Vec2, at: usize, dir: i32, goal_x: f32) -> usize {
    let g = geometry(props, size);
    let li = line_of(&g, at) as i32 + dir;
    if li < 0 {
        return 0;
    }
    if li as usize >= g.lines.len() {
        return props.content.len();
    }
    let l = &g.lines[li as usize];
    hit_caret(props, size, goal_x, l.top + l.height / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(s: &str, size: f32) -> TextProps {
        TextProps {
            content: s.into(),
            style: d::TextStyle { font_size: size, ..Default::default() },
            runs: vec![],
            auto_resize: d::TextAutoResize::WidthAndHeight,
            max_lines: None,
            ..Default::default()
        }
    }

    const Z: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[test]
    fn width_grows_with_content_and_size() {
        let a = measure(&props("Hi", 16.0), None);
        let b = measure(&props("Hello world", 16.0), None);
        let c = measure(&props("Hello world", 32.0), None);
        assert!(b.x > a.x && c.x > b.x * 1.5);
        assert!(a.y >= 16.0 && a.y < 30.0);
        // Proportional font: 'i' is narrower than 'W'.
        assert!(measure(&props("iiii", 16.0), None).x < measure(&props("WWWW", 16.0), None).x);
    }

    #[test]
    fn lines_and_wrapping() {
        let one = measure(&props("abc", 16.0), None);
        let two = measure(&props("abc\ndef", 16.0), None);
        assert!(two.y > one.y * 1.8);
        let wrapped = measure(&props("one two three four five six", 16.0), Some(60.0));
        assert!(wrapped.y > one.y * 2.0 && wrapped.x <= 60.5);
        assert!(measure(&props("", 16.0), None).y >= 16.0);
    }

    #[test]
    fn text_wrap_changes_the_measured_lines() {
        const HEADLINE: &str = "The quick brown fox jumps over the lazy dog";
        let wrapped = |wrap| {
            let mut p = props(HEADLINE, 16.0);
            p.text_wrap = wrap;
            p.auto_resize = d::TextAutoResize::Height;
            p
        };
        let box_w = Vec2::new(230.0, 200.0);
        // Inter 16px: greedy breaks after "jumps" (204.6px wide first line); balance after "fox" (180.8px widest line)
        let auto = measure(&wrapped(d::TextWrap::Auto), Some(230.0));
        let balanced = measure(&wrapped(d::TextWrap::Balance), Some(230.0));
        assert!(auto.x > 204.0 && balanced.x < 182.0, "{auto:?} vs {balanced:?}");
        assert_eq!(auto.y, balanced.y, "same line count, same height");
        // editor geometry follows the balanced break: line 2 starts at "jumps" (byte 20), greedy at "over" (byte 26)
        let lines = |wrap| geometry(&wrapped(wrap), box_w).lines.iter().map(|l| l.start).collect::<Vec<_>>();
        assert_eq!(lines(d::TextWrap::Auto), vec![0, 26]);
        assert_eq!(lines(d::TextWrap::Balance), vec![0, 20]);
    }

    #[test]
    fn measure_follows_run_styles() {
        let mut p = props("ab cd", 12.0);
        let small = measure(&p, None);
        let mut big = p.style.clone();
        big.font_size = 48.0;
        p.runs = vec![d::TextRun { start: 3, end: 5, style: big }];
        let m = measure(&p, None);
        assert!(m.x > small.x * 1.8 && m.y > small.y * 2.0, "{small:?} -> {m:?}");
    }

    #[test]
    fn caret_follows_text() {
        let p = props("Hello\nWorld", 16.0);
        let (x0, y0, h) = caret_rect(&p, Z, 0);
        let (x3, _, _) = caret_rect(&p, Z, 3);
        let (xe, _, _) = caret_rect(&p, Z, 5);
        let (x6, y6, _) = caret_rect(&p, Z, 6);
        assert_eq!(x0, 0.0);
        assert!(x3 > x0 && xe > x3);
        assert_eq!(x6, 0.0);
        assert!(y6 >= y0 + h - 0.5);
    }

    #[test]
    fn caret_x_matches_glyph_positions() {
        // Caret x for every offset equals the shaped glyph edge: the sum of the
        // advances of the preceding glyphs, i.e. where the renderer draws them.
        let p = props("Hamburgefonstiv ijl", 20.0);
        let g = geometry(&p, Z);
        let glyphs = &g.lines[0].glyphs;
        let mut last = -1.0;
        for (i, gl) in glyphs.iter().enumerate() {
            let (x, _, _) = caret_rect(&p, Z, gl.start);
            assert_eq!(x, gl.x, "glyph {i}");
            assert!(x > last);
            last = x;
        }
        let end = p.content.len();
        let (xe, _, _) = caret_rect(&p, Z, end);
        let lastg = glyphs.last().unwrap();
        assert_eq!(xe, lastg.x + lastg.w);
        // and a measured width agrees with the caret at the end
        assert!((measure(&p, None).x - xe).abs() <= 1.0);
    }

    #[test]
    fn caret_x_matches_rendered_ink() {
        // The renderer's ink for the first n letters never extends past the caret after n.
        let p = props("Hello", 24.0);
        for n in 1..=5usize {
            let mut q = p.clone();
            q.content = p.content[..n].to_string();
            let ink = designos_render::text_geometry(&q, Z);
            let (cx, _, _) = caret_rect(&p, Z, n);
            // (kerning with the following letter may shift the edge by a fraction of a px)
            assert!((ink.width - cx).abs() < 1.0, "n={n}: width {} caret {cx}", ink.width);
        }
    }

    #[test]
    fn hit_roundtrips_caret() {
        let p = props("Hello\nWorld", 16.0);
        for caret in [0usize, 2, 5, 6, 9, 11] {
            let (x, y, h) = caret_rect(&p, Z, caret);
            assert_eq!(hit_caret(&p, Z, x + 0.5, y + h / 2.0), caret, "caret {caret}");
        }
    }

    #[test]
    fn multibyte_offsets_roundtrip() {
        let p = props("aé😀中b", 18.0);
        for (caret, _) in p.content.char_indices().chain(std::iter::once((p.content.len(), ' '))) {
            let (x, y, h) = caret_rect(&p, Z, caret);
            assert_eq!(hit_caret(&p, Z, x + 0.01, y + h / 2.0), caret, "caret {caret}");
        }
    }

    #[test]
    fn wrapped_caret_moves_to_next_line() {
        let mut p = props("one two three four", 16.0);
        p.auto_resize = d::TextAutoResize::Height;
        let size = Vec2::new(70.0, 100.0);
        let g = geometry(&p, size);
        assert!(g.lines.len() >= 3);
        let l0 = &g.lines[0];
        // wrapping drops the space between the lines: the offset before it ends line 0,
        // the offset after it starts line 1
        let (_, y_end, _) = caret_rect(&p, size, l0.end);
        assert_eq!(y_end, g.lines[0].top);
        let (_, y_next, _) = caret_rect(&p, size, g.lines[1].start);
        assert_eq!(y_next, g.lines[1].top);
        let (hs, he) = line_bounds(&p, size, 2);
        assert_eq!((hs, he), (0, l0.end));
        // clicking far right of line 0 stays on line 0
        let hit = hit_caret(&p, size, 500.0, g.lines[0].top + 2.0);
        assert!(hit <= l0.end && line_of(&g, hit) == 0, "hit {hit}");
    }

    #[test]
    fn vertical_moves_keep_column() {
        let p = props("abcdef\nab\nabcdef", 16.0);
        let (gx, _, _) = caret_rect(&p, Z, 5);
        let down = move_vertical(&p, Z, 5, 1, gx);
        assert_eq!(down, 9, "short line clamps to its end");
        let down2 = move_vertical(&p, Z, down, 1, gx);
        assert_eq!(down2, 10 + 5);
        assert_eq!(move_vertical(&p, Z, 2, -1, 0.0), 0);
        assert_eq!(move_vertical(&p, Z, 12, 1, 0.0), p.content.len());
    }

    #[test]
    fn selection_rects_cover_lines() {
        let p = props("abc\ndef", 16.0);
        let r = selection_rects(&p, Z, 1..6);
        assert_eq!(r.len(), 2);
        assert!(r[0].0 > 0.0 && r[0].2 > 0.0);
        assert_eq!(r[1].0, 0.0);
        assert!(selection_rects(&p, Z, 3..3).is_empty());
    }

    #[test]
    fn empty_text_has_a_caret_line() {
        let p = props("", 16.0);
        let (x, y, h) = caret_rect(&p, Z, 0);
        assert_eq!((x, y), (0.0, 0.0));
        assert!(h >= 16.0);
        assert_eq!(hit_caret(&p, Z, 5.0, 5.0), 0);
    }

    #[test]
    fn centered_empty_line_sits_in_the_middle() {
        let mut p = props("a\n\nb", 16.0);
        p.style.align_h = d::TextAlignH::Center;
        p.auto_resize = d::TextAutoResize::Height;
        let size = Vec2::new(100.0, 80.0);
        let (x, _, _) = caret_rect(&p, size, 2);
        assert!((x - 50.0).abs() < 0.01, "{x}");
    }

    // ---------------------------------------------------------------- bidi
    use designos_render::GlyphSpan;

    fn bidi_line(glyphs: &[(usize, usize, f32, bool)], start: usize, end: usize) -> LineGeom {
        LineGeom {
            para: 0,
            top: 0.0,
            height: 20.0,
            baseline: 15.0,
            width: glyphs.len() as f32 * 10.0,
            start,
            end,
            empty_x: 0.0,
            rtl: false,
            glyphs: glyphs.iter().map(|&(start, end, x, rtl)| GlyphSpan { start, end, x, w: 10.0, rtl }).collect(),
        }
    }

    /// "ab \u{5D0}\u{5D1}\u{5D2} cd" in a left-to-right paragraph: the Hebrew run is reversed on screen.
    /// Visual order: a b _ (ג ב א) _ c d, ten pixels per character.
    fn mixed() -> (String, TextGeometry) {
        let content = "ab \u{5D0}\u{5D1}\u{5D2} cd".to_string();
        assert_eq!(content.len(), 12);
        let glyphs = [
            (0, 1, 0.0, false),
            (1, 2, 10.0, false),
            (2, 3, 20.0, false),
            (7, 9, 30.0, true),
            (5, 7, 40.0, true),
            (3, 5, 50.0, true),
            (9, 10, 60.0, false),
            (10, 11, 70.0, false),
            (11, 12, 80.0, false),
        ];
        (content, TextGeometry { lines: vec![bidi_line(&glyphs, 0, 12)], width: 90.0, height: 20.0 })
    }

    fn caret_x(g: &TextGeometry, content: &str, at: usize, hint: Option<f32>) -> f32 {
        x_in_line(content, &g.lines[line_of(g, at)], at, hint)
    }

    #[test]
    fn mixed_line_carets_follow_the_screen_positions() {
        let (c, g) = mixed();
        // inside the Latin text and at the ends nothing special
        assert_eq!(caret_x(&g, &c, 0, None), 0.0);
        assert_eq!(caret_x(&g, &c, 2, None), 20.0);
        assert_eq!(caret_x(&g, &c, 12, None), 90.0);
        // inside the Hebrew run offsets run right to left: the start of the run is its right edge
        assert_eq!(caret_x(&g, &c, 5, None), 50.0, "between alef and bet");
        assert_eq!(caret_x(&g, &c, 7, None), 40.0, "between bet and gimel");
        // the boundaries have two places; without a hint the caret trails the text before it
        assert_eq!(caret_x(&g, &c, 3, None), 30.0, "after the space, i.e. before the Hebrew starts: end of the preceding text");
        assert_eq!(caret_x(&g, &c, 3, Some(58.0)), 60.0, "...or the right edge of the alef, as placed by a click");
        assert_eq!(caret_x(&g, &c, 9, None), 30.0);
        assert_eq!(caret_x(&g, &c, 9, Some(61.0)), 60.0);
        // pointing: the nearest stop
        assert_eq!(hit_in(&g, &c, 41.0, 5.0), (7, 40.0));
        assert_eq!(hit_in(&g, &c, 58.0, 5.0).1, 60.0, "next to the right edge of the Hebrew run");
        assert_eq!(hit_in(&g, &c, -30.0, 5.0), (0, 0.0));
        assert_eq!(hit_in(&g, &c, 300.0, 5.0), (12, 90.0));
        // the hint of a click reproduces the click position
        let (at, x) = hit_in(&g, &c, 59.0, 5.0);
        assert_eq!(caret_x(&g, &c, at, Some(x)), x);
    }

    #[test]
    fn mixed_line_selection_highlights_every_visual_piece() {
        let (c, g) = mixed();
        // a..b plus the space and the alef: Latin piece, then the alef at the right end of the Hebrew run
        let r = selection_rects_in(&g, &c, 0..5);
        let xs: Vec<(f32, f32)> = r.iter().map(|r| (r.0, r.0 + r.2)).collect();
        assert_eq!(xs, vec![(0.0, 30.0), (50.0, 60.0)], "{r:?}");
        // the middle of the Hebrew run is one block
        let r = selection_rects_in(&g, &c, 5..9);
        assert_eq!(r.iter().map(|r| (r.0, r.0 + r.2)).collect::<Vec<_>>(), vec![(30.0, 50.0)]);
        // across the run: contiguous again
        let r = selection_rects_in(&g, &c, 1..11);
        assert_eq!(r.iter().map(|r| (r.0, r.0 + r.2)).collect::<Vec<_>>(), vec![(10.0, 80.0)]);
        // whole line
        assert_eq!(selection_rects_in(&g, &c, 0..12).len(), 1);
        assert!(selection_rects_in(&g, &c, 4..4).is_empty());
    }

    #[test]
    fn arrows_walk_the_screen_left_to_right_and_back() {
        let (c, g) = mixed();
        // from the start, Right visits every distinct x once, moving by one cluster
        let mut at = 0;
        let mut hint = None;
        let mut xs = vec![0.0];
        let mut path = vec![0usize];
        while let Some((a, x)) = move_visual_in(&g, &c, at, hint, 1) {
            at = a;
            hint = Some(x);
            xs.push(x);
            path.push(a);
        }
        assert_eq!(xs, vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0], "{path:?}");
        assert_eq!(at, 12);
        // walking back retraces the same positions
        let mut back = vec![xs[xs.len() - 1]];
        while let Some((a, x)) = move_visual_in(&g, &c, at, hint, -1) {
            at = a;
            hint = Some(x);
            back.push(x);
        }
        back.reverse();
        assert_eq!(back, xs);
        assert_eq!(at, 0);
        // inside the Hebrew run Right moves backwards through the text
        let (a, _) = move_visual_in(&g, &c, 9, Some(30.0), 1).unwrap();
        assert_eq!(a, 7, "from the end of the run, one step right is before the last letter");
    }

    #[test]
    fn pure_rtl_line() {
        // gimel bet alef on screen, alef at the right
        let c = "\u{5D0}\u{5D1}\u{5D2}".to_string();
        let mut l = bidi_line(&[(4, 6, 0.0, true), (2, 4, 10.0, true), (0, 2, 20.0, true)], 0, 6);
        l.rtl = true;
        let g = TextGeometry { lines: vec![l], width: 30.0, height: 20.0 };
        assert_eq!(caret_x(&g, &c, 0, None), 30.0, "the start of the text is at the right edge");
        assert_eq!(caret_x(&g, &c, 2, None), 20.0);
        assert_eq!(caret_x(&g, &c, 6, None), 0.0);
        assert!(run_rtl_in(&g, 2) && run_rtl_in(&g, 0) && run_rtl_in(&g, 6));
        // Right on screen goes back in the text
        assert_eq!(move_visual_in(&g, &c, 6, None, 1).map(|m| m.0), Some(4));
        assert_eq!(move_visual_in(&g, &c, 0, None, 1), None, "right end of the line");
        assert_eq!(move_visual_in(&g, &c, 0, None, -1).map(|m| m.0), Some(2));
        // selecting the first letter highlights the rightmost cell
        let r = selection_rects_in(&g, &c, 0..2);
        assert_eq!((r[0].0, r[0].2), (20.0, 10.0));
        // pointing right of the text selects the start, left of it the end
        assert_eq!(hit_in(&g, &c, 100.0, 5.0).0, 0);
        assert_eq!(hit_in(&g, &c, -5.0, 5.0).0, 6);
        assert_eq!(hit_in(&g, &c, 24.0, 5.0).0, 2, "the nearer edge of the alef");
        assert_eq!(hit_in(&g, &c, 26.0, 5.0).0, 0);
    }

    #[test]
    fn rtl_text_is_laid_out_and_measured_right_to_left() {
        // shaping and bidi do not need a Hebrew font to decide positions
        let p = props("\u{5D0}\u{5D1}\u{5D2} abc", 20.0);
        let g = geometry(&p, Z);
        let l = &g.lines[0];
        assert!(l.rtl, "the first strong character is Hebrew");
        let gl: Vec<&GlyphSpan> = l.glyphs.iter().collect();
        assert!(gl.windows(2).all(|w| w[0].x <= w[1].x), "sorted left to right");
        let alef = gl.iter().find(|g| g.start == 0).unwrap();
        let abc = gl.iter().find(|g| g.start == 7).unwrap();
        assert!(alef.rtl && !abc.rtl);
        assert!(abc.x < alef.x, "in a right-to-left paragraph the Latin word sits left of the Hebrew");
        // the caret at the start of the text is on the right edge of the Hebrew run; the end of the
        // text is the right edge of the last Latin letter (the Latin run is left to right)
        let (x0, _, _) = caret_rect(&p, Z, 0);
        let (xe, _, _) = caret_rect(&p, Z, p.content.len());
        let c = gl.iter().find(|g| g.start == 9).unwrap();
        assert!((x0 - (alef.x + alef.w)).abs() < 0.01);
        assert!((xe - (c.x + c.w)).abs() < 0.01);
        // a click at the right edge of the line places the caret at the start
        assert_eq!(hit_caret(&p, Z, x0 + 3.0, 5.0), 0);
        // selecting the first letter paints exactly its cell
        let r = selection_rects(&p, Z, 0..2);
        assert_eq!(r.len(), 1);
        assert!((r[0].0 - alef.x).abs() < 0.01 && (r[0].2 - alef.w).abs() < 0.01);
    }
}
