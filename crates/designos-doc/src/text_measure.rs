use crate::{TextProps, Vec2};

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
