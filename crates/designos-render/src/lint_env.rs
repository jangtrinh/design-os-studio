//! `LintEnv` implementation backed by `cosmic-text` shaping and the CPU renderer.

use crate::{render_backdrop, render_backdrop_indexed, render_text_mask, resolve_font, text_geometry, BackdropIndex};
use designos_doc::lint::rules::contrast::contrast_ratio;
use designos_doc::lint::{
    lint_in_slices, lint_in_slices_with_budget, Backdrop, Budget, LintConfig, LintEnv, LintReport, PassCache, SamplePoint, SliceRunStats, TextMetrics,
};
use designos_doc::{resolve_node, Color, Document, Node, NodeId, NodeKind, PaintKind, Rect, TextAutoResize, TextProps, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

thread_local! {
    static FONT_SUBSTITUTION_CACHE: RefCell<HashMap<(String, u16, bool), Option<String>>> = RefCell::new(HashMap::new());
}

fn cached_substituted_family(family: &str, weight: u16, italic: bool) -> Option<String> {
    if family.is_empty() {
        return None;
    }
    FONT_SUBSTITUTION_CACHE.with(|cache| {
        let key = (family.to_string(), weight, italic);
        if let Some(cached) = cache.borrow().get(&key) {
            return cached.clone();
        }
        let substituted = resolve_font(family, weight, italic).and_then(|resolved| (!resolved.family.eq_ignore_ascii_case(family)).then_some(resolved.family));
        cache.borrow_mut().insert(key, substituted.clone());
        substituted
    })
}

/// Real text-measurement and backdrop-sampling environment for [`designos_doc::lint`].
#[derive(Debug, Default, Clone, Copy)]
pub struct RenderLintEnv;

impl LintEnv for RenderLintEnv {
    fn has_text_engine(&self) -> bool {
        true
    }

    fn text_metrics(&self, _node: &Node, props: &TextProps, size: Vec2) -> Option<TextMetrics> {
        let geom = text_geometry(props, size);
        let line_count = geom.lines.len().max(1);
        let content_height = geom.height;
        let widest_line = geom.lines.iter().map(|l| l.width).fold(geom.width, f32::max);

        let mut breaks_inside_word = Vec::new();
        for pair in geom.lines.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);
            if prev.para == next.para && prev.end == next.start && next.start > 0 && next.start < props.content.len() {
                let before_alnum = props.content[..next.start].chars().next_back().is_some_and(char::is_alphanumeric);
                let after_alnum = props.content[next.start..].chars().next().is_some_and(char::is_alphanumeric);
                if before_alnum && after_alnum {
                    breaks_inside_word.push(next.start);
                }
            }
        }

        let max_by_height = if matches!(props.auto_resize, TextAutoResize::None | TextAutoResize::Truncate) && size.y > 0.0 {
            geom.lines.iter().filter(|l| l.top + l.height <= size.y + 0.5).count().max(1)
        } else {
            line_count
        }
        .max(1);

        let visible_lines = match props.max_lines {
            Some(max_l) if props.auto_resize == TextAutoResize::Truncate => line_count.min(max_l.max(1) as usize).min(max_by_height),
            _ => line_count.min(max_by_height),
        };

        let substituted_family = cached_substituted_family(&props.style.font_family, props.style.font_weight, props.style.italic);

        Some(TextMetrics { line_count, visible_lines, content_height, widest_line, breaks_inside_word, substituted_family })
    }

    fn prepare_pass(&self, doc: &Document, roots: &[NodeId], pass: &PassCache) {
        // Without text under the roots there is no backdrop to sample, so nothing to build.
        if !roots.iter().any(|root| has_text(doc, *root)) {
            return;
        }
        let indexes = pass.get_or_insert_with(PageIndexes::default);
        for root in roots {
            if let Some(page) = doc.page_of(*root) {
                indexes.of(doc, page);
            }
        }
    }

    fn backdrop(&self, doc: &Document, node: NodeId, rect: Rect) -> Option<Backdrop> {
        sample_backdrop(doc, node, rect, None)
    }

    fn backdrop_in_pass(&self, doc: &Document, node: NodeId, rect: Rect, pass: &PassCache) -> Option<Backdrop> {
        let indexes = pass.get_or_insert_with(PageIndexes::default);
        sample_backdrop(doc, node, rect, Some(&indexes))
    }
}

/// Whether `root` or a layer under it is text.
fn has_text(doc: &Document, root: NodeId) -> bool {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if matches!(n.kind, NodeKind::Text(_)) {
            return true;
        }
        stack.extend(n.children.iter().copied());
    }
    false
}

/// The backdrop index of each page a lint pass has sampled, built the first time the page is needed.
#[derive(Default)]
struct PageIndexes(Mutex<HashMap<NodeId, Arc<BackdropIndex>>>);

impl PageIndexes {
    fn of(&self, doc: &Document, page: NodeId) -> Arc<BackdropIndex> {
        let mut pages = self.0.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(pages.entry(page).or_insert_with(|| Arc::new(BackdropIndex::new(doc, page))))
    }
}

/// Samples the backdrop behind `node` in `rect`. With `indexes`, the page is rendered through its
/// scene index, which gives the same pixels as the full render.
fn sample_backdrop(doc: &Document, node: NodeId, mut rect: Rect, indexes: Option<&PageIndexes>) -> Option<Backdrop> {
    let page_id = doc.page_of(node)?;
    let raw_node = doc.get(node)?;
    let resolved = resolve_node(doc, raw_node);

    if rect.w <= 1.0 || rect.h <= 1.0 {
        if let NodeKind::Text(tp) = &resolved.kind {
            let geom = text_geometry(tp, resolved.size);
            rect.w = rect.w.max(geom.width.ceil().max(4.0));
            rect.h = rect.h.max(geom.height.ceil().max(4.0));
        }
    }

    if rect.w <= 0.0 || rect.h <= 0.0 {
        return ancestor_backdrop(doc, node);
    }

    let scale = (256.0 / rect.w.max(rect.h)).min(1.0);
    let (b_pm, c_pm) = match indexes {
        Some(indexes) => {
            let index = indexes.of(doc, page_id);
            (render_backdrop_indexed(doc, page_id, &[node], rect, scale, &index)?, render_backdrop_indexed(doc, page_id, &[], rect, scale, &index)?)
        }
        None => (render_backdrop(doc, page_id, &[node], rect, scale)?, render_backdrop(doc, page_id, &[], rect, scale)?),
    };
    let m_pm = render_text_mask(doc, node, rect, scale)?;

    let w = m_pm.width() as usize;
    let b_pixels = b_pm.pixels();
    let c_pixels = c_pm.pixels();
    let m_pixels = m_pm.pixels();

    let mut selected: Vec<usize> = m_pixels.iter().enumerate().filter_map(|(i, px)| (px.alpha() >= 192).then_some(i)).collect();

    if selected.len() < 4 {
        selected = m_pixels.iter().enumerate().filter_map(|(i, px)| (px.alpha() > 0).then_some(i)).collect();
    }

    if !selected.is_empty() {
        let mut min_ratio = f64::INFINITY;
        let mut worst_fg = Color::BLACK;
        let mut worst_bg = Color::WHITE;
        let mut worst_sample = SamplePoint { x: (rect.x + rect.w * 0.5).round(), y: (rect.y + rect.h * 0.5).round() };
        let mut all_equal = true;
        let mut first_bg: Option<tiny_skia::ColorU8> = None;
        let mut uniform = true;

        for idx in selected {
            let px = (idx % w) as f32;
            let py = (idx / w) as f32;
            let b_u8 = b_pixels[idx].demultiply();
            let c_u8 = c_pixels[idx].demultiply();
            let m_a = (f32::from(m_pixels[idx].alpha()) / 255.0).clamp(1.0 / 255.0, 1.0);

            if c_u8.red() != b_u8.red() || c_u8.green() != b_u8.green() || c_u8.blue() != b_u8.blue() {
                all_equal = false;
            }
            if let Some(f0) = first_bg {
                if b_u8.red().abs_diff(f0.red()) > 2 || b_u8.green().abs_diff(f0.green()) > 2 || b_u8.blue().abs_diff(f0.blue()) > 2 {
                    uniform = false;
                }
            } else {
                first_bg = Some(b_u8);
            }

            let bg_i = Color { r: f32::from(b_u8.red()) / 255.0, g: f32::from(b_u8.green()) / 255.0, b: f32::from(b_u8.blue()) / 255.0, a: 1.0 };
            let c_col = Color { r: f32::from(c_u8.red()) / 255.0, g: f32::from(c_u8.green()) / 255.0, b: f32::from(c_u8.blue()) / 255.0, a: 1.0 };
            let inv_a = 1.0 - m_a;
            let fg_i = Color {
                r: ((c_col.r - inv_a * bg_i.r) / m_a).clamp(0.0, 1.0),
                g: ((c_col.g - inv_a * bg_i.g) / m_a).clamp(0.0, 1.0),
                b: ((c_col.b - inv_a * bg_i.b) / m_a).clamp(0.0, 1.0),
                a: 1.0,
            };

            let ratio_i = contrast_ratio(fg_i, bg_i);
            if ratio_i < min_ratio {
                min_ratio = ratio_i;
                worst_fg = fg_i;
                worst_bg = bg_i;
                worst_sample = SamplePoint { x: (rect.x + (px + 0.5) / scale).round(), y: (rect.y + (py + 0.5) / scale).round() };
            }
        }

        return Some(Backdrop {
            color: worst_bg,
            uniform,
            foreground: Some(worst_fg),
            worst_ratio: Some(min_ratio as f32),
            worst_sample: Some(worst_sample),
            covered: all_equal,
        });
    }

    // §8.6 Fallback: no glyph outlines produced; sample all pixels of B against solid fill colour.
    let (fg_color, fg_alpha) = fallback_text_fill(&resolved);
    let mut min_ratio = f64::INFINITY;
    let mut worst_fg = fg_color;
    let mut worst_bg = Color::WHITE;
    let mut worst_sample = SamplePoint { x: (rect.x + rect.w * 0.5).round(), y: (rect.y + rect.h * 0.5).round() };
    let mut first_bg: Option<tiny_skia::ColorU8> = None;
    let mut uniform = true;

    for (idx, b_px) in b_pixels.iter().enumerate() {
        let px = (idx % w) as f32;
        let py = (idx / w) as f32;
        let b_u8 = b_px.demultiply();
        if let Some(f0) = first_bg {
            if b_u8.red().abs_diff(f0.red()) > 2 || b_u8.green().abs_diff(f0.green()) > 2 || b_u8.blue().abs_diff(f0.blue()) > 2 {
                uniform = false;
            }
        } else {
            first_bg = Some(b_u8);
        }
        let bg_i = Color { r: f32::from(b_u8.red()) / 255.0, g: f32::from(b_u8.green()) / 255.0, b: f32::from(b_u8.blue()) / 255.0, a: 1.0 };
        let fg_i = Color {
            r: fg_color.r * fg_alpha + bg_i.r * (1.0 - fg_alpha),
            g: fg_color.g * fg_alpha + bg_i.g * (1.0 - fg_alpha),
            b: fg_color.b * fg_alpha + bg_i.b * (1.0 - fg_alpha),
            a: 1.0,
        };
        let ratio_i = contrast_ratio(fg_i, bg_i);
        if ratio_i < min_ratio {
            min_ratio = ratio_i;
            worst_fg = fg_i;
            worst_bg = bg_i;
            worst_sample = SamplePoint { x: (rect.x + (px + 0.5) / scale).round(), y: (rect.y + (py + 0.5) / scale).round() };
        }
    }

    Some(Backdrop {
        color: worst_bg,
        uniform,
        foreground: Some(worst_fg),
        worst_ratio: Some(min_ratio as f32),
        worst_sample: Some(worst_sample),
        covered: false,
    })
}

fn fallback_text_fill(n: &Node) -> (Color, f32) {
    if let NodeKind::Text(t) = &n.kind {
        if let Some(p) = t.style.fills.iter().rev().find(|p| p.visible) {
            if let PaintKind::Solid(c) = p.kind {
                let a = (c.a * p.opacity * n.opacity).clamp(0.0, 1.0);
                return (Color { r: c.r, g: c.g, b: c.b, a: 1.0 }, a);
            }
        }
    }
    (Color::BLACK, n.opacity.clamp(0.0, 1.0))
}

fn ancestor_backdrop(doc: &Document, id: NodeId) -> Option<Backdrop> {
    let mut cur = doc.get(id).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let Some(p) = doc.get(pid) else { break };
        if let NodeKind::Page { background } = p.kind {
            return Some(Backdrop { color: background, uniform: true, foreground: None, worst_ratio: None, worst_sample: None, covered: false });
        }
        if let Some(fill) = p.fills.iter().rev().find(|f| f.visible && f.opacity >= 0.5) {
            if let PaintKind::Solid(c) = fill.kind {
                if c.a >= 0.5 {
                    return Some(Backdrop { color: c, uniform: true, foreground: None, worst_ratio: None, worst_sample: None, covered: false });
                }
            }
        }
        cur = p.parent;
    }
    Some(Backdrop { color: Color::WHITE, uniform: true, foreground: None, worst_ratio: None, worst_sample: None, covered: false })
}

/// Runs a full lint scan on a dedicated worker thread holding an `Arc<Document>` snapshot,
/// creating its own [`RenderLintEnv`] on the worker thread and driving repeated `lint_subtree`
/// slices bounded by `(max_nodes_per_slice, max_slice_duration)`.
pub fn lint_on_worker(
    doc: Arc<Document>,
    roots: Vec<NodeId>,
    config: LintConfig,
    max_nodes_per_slice: usize,
    max_slice_duration: Duration,
) -> std::thread::JoinHandle<(LintReport, usize)> {
    std::thread::spawn(move || {
        let env = RenderLintEnv;
        lint_in_slices(&doc, &roots, &config, &env, max_nodes_per_slice, max_slice_duration)
    })
}

/// Runs a budgeted lint scan on a dedicated worker thread holding an `Arc<Document>` snapshot,
/// returning detailed [`SliceRunStats`] (including the slice durations and resumable cursor when
/// `total_budget` expires).
pub fn lint_on_worker_with_budget(
    doc: Arc<Document>,
    roots: Vec<NodeId>,
    config: LintConfig,
    max_nodes_per_slice: usize,
    max_slice_duration: Duration,
    mut total_budget: Budget,
) -> std::thread::JoinHandle<SliceRunStats> {
    std::thread::spawn(move || {
        let env = RenderLintEnv;
        lint_in_slices_with_budget(&doc, &roots, &config, &env, max_nodes_per_slice, max_slice_duration, &mut total_budget)
    })
}

/// Runs a budgeted lint scan on a scoped worker thread borrowing `doc`, creating its own
/// [`RenderLintEnv`] on the worker thread and driving repeated `lint_subtree` slices bounded by
/// `(max_nodes_per_slice, max_slice_duration)` and `total_budget`.
pub fn lint_scoped_on_worker(
    doc: &Document,
    roots: &[NodeId],
    config: &LintConfig,
    max_nodes_per_slice: usize,
    max_slice_duration: Duration,
    total_budget: &mut Budget,
) -> SliceRunStats {
    std::thread::scope(|s| {
        s.spawn(|| {
            let env = RenderLintEnv;
            lint_in_slices_with_budget(doc, roots, config, &env, max_nodes_per_slice, max_slice_duration, total_budget)
        })
        .join()
        .expect("lint worker thread panicked")
    })
}
