use crate::{Node, NodeKind, TextAutoResize, TextProps, Vec2};
use std::cell::RefCell;
use std::sync::{Arc, OnceLock, RwLock};

pub trait TextMeasure: Send + Sync {
    /// Shaped `(width, height)` of `tp`. `wrap_width` limits the line width when `Some`.
    fn measure(&self, tp: &TextProps, wrap_width: Option<f32>) -> Vec2;

    /// True when the requested `tp.style.font_family` (or any run's
    /// `font_family`) is not loaded in the renderer's font database and a
    /// fallback face was used. Default `false`.
    fn uses_fallback_font(&self, _tp: &TextProps) -> bool {
        false
    }
}

static LAYOUT_MEASURER: OnceLock<RwLock<Option<Arc<dyn TextMeasure>>>> = OnceLock::new();

thread_local! {
    static THREAD_LAYOUT_MEASURER: RefCell<Option<Arc<dyn TextMeasure>>> = const { RefCell::new(None) };
}

fn global_slot() -> &'static RwLock<Option<Arc<dyn TextMeasure>>> {
    LAYOUT_MEASURER.get_or_init(|| RwLock::new(None))
}

/// Process-wide measurer used by [`crate::Document::relayout_subtree`]. `None` keeps stored heights.
/// MCP and the app install the renderer measurer here.
pub fn set_layout_measurer(m: Option<Arc<dyn TextMeasure>>) {
    *global_slot().write().unwrap_or_else(|p| p.into_inner()) = m;
}

/// Restores the previous per-thread measurer when dropped.
pub struct ThreadLayoutMeasurerGuard {
    prev: Option<Arc<dyn TextMeasure>>,
}

impl ThreadLayoutMeasurerGuard {
    pub fn install(m: Arc<dyn TextMeasure>) -> Self {
        let prev = THREAD_LAYOUT_MEASURER.with(|c| c.replace(Some(m)));
        Self { prev }
    }
}

impl Drop for ThreadLayoutMeasurerGuard {
    fn drop(&mut self) {
        THREAD_LAYOUT_MEASURER.with(|c| c.replace(self.prev.take()));
    }
}

/// Measurer [`crate::Document::relayout_subtree`] uses: the thread override, else the process hook.
pub fn layout_measurer() -> Option<Arc<dyn TextMeasure>> {
    let thread = THREAD_LAYOUT_MEASURER.with(|c| c.borrow().clone());
    if thread.is_some() {
        thread
    } else {
        global_slot().read().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

/// Measures a hug-width (`TextAutoResize::WidthAndHeight`) text layer that may carry a
/// `max_width` cap (`layout_child.max_w`).
///
/// The text keeps `TextAutoResize::WidthAndHeight`; its width is `min(natural_width, max_w)`
/// and when `natural_width > max_w` the text wraps at `max_w`.
pub fn measure_hug_text_with(tp: &TextProps, max_w: Option<f32>, mut measure: impl FnMut(&TextProps, Option<f32>) -> Vec2) -> Vec2 {
    let natural = measure(tp, None);
    if let Some(limit) = max_w.map(|w| w.max(1.0)).filter(|&limit| natural.x > limit) {
        let wrapped = measure(tp, Some(limit));
        Vec2::new(limit, wrapped.y)
    } else {
        natural
    }
}

/// Height of wrapping `TextAutoResize::Height` text (or `TextAutoResize::WidthAndHeight` text
/// clamped by `max_w`) at `width`, or `None` when `n` does not wrap its height. A text turned off
/// the page axes (quarter turn or tilt) is never refit: the solved box width is not its text
/// width, so it keeps the stored height. Every solver measures through here.
pub fn height_resize_text_height(n: &Node, width: f32, measure: &dyn TextMeasure) -> Option<f32> {
    let NodeKind::Text(t) = &n.kind else { return None };
    if !crate::layout_rotated::keeps_axes(&n.transform) {
        return None;
    }
    match t.auto_resize {
        TextAutoResize::Height => Some(measure.measure(t, Some(width.max(1.0))).y),
        TextAutoResize::WidthAndHeight if n.layout_child.max_w.is_some() => {
            Some(measure_hug_text_with(t, Some(width.max(1.0)), |tp, w| measure.measure(tp, w)).y)
        }
        _ => None,
    }
}

/// Measure `tp` with `measure` when the caller has a renderer, otherwise the
/// character-width estimate.
pub fn intrinsic_text_size(tp: &TextProps, wrap_width: Option<f32>, measure: Option<&dyn TextMeasure>) -> Vec2 {
    if let Some(m) = measure {
        return m.measure(tp, wrap_width);
    }
    let est = crate::spec::estimate_intrinsic_text_size(tp);
    match wrap_width {
        Some(w) => {
            let lines = (est.x / w.max(20.0)).ceil().max(1.0);
            let h = (lines * tp.style.font_size.max(8.0) * 1.45).ceil();
            Vec2::new(w, h)
        }
        None => est,
    }
}

/// True when `measure` reports that `tp` fell back from its requested `font_family`.
pub fn uses_fallback_font(tp: &TextProps, measure: Option<&dyn TextMeasure>) -> bool {
    measure.is_some_and(|m| m.uses_fallback_font(tp))
}
