//! `LintEnv` implementation backed by `cosmic-text` shaping and the CPU renderer.

use crate::{resolve_font, text_geometry};
use designos_doc::lint::{lint_in_slices, lint_in_slices_with_budget, Backdrop, Budget, LintConfig, LintEnv, LintReport, SliceRunStats, TextMetrics};
use designos_doc::{Document, Node, NodeId, Rect, TextAutoResize, TextProps, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
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
        };

        let visible_lines = match props.max_lines {
            Some(max_l) if props.auto_resize == TextAutoResize::Truncate => line_count.min(max_l.max(1) as usize).min(max_by_height),
            _ => line_count.min(max_by_height),
        };

        let substituted_family = cached_substituted_family(&props.style.font_family, props.style.font_weight, props.style.italic);

        Some(TextMetrics { line_count, visible_lines, content_height, widest_line, breaks_inside_word, substituted_family })
    }

    fn backdrop(&self, _doc: &Document, _node: NodeId, _rect: Rect) -> Option<Backdrop> {
        // Pixel backdrop sampling is specified by E15-10; E15-9 forbids heuristic fallbacks (§3.1, §9).
        None
    }
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
