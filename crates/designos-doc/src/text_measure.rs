use crate::{TextProps, Vec2};
use std::sync::OnceLock;

pub trait TextMeasure: Send + Sync {
    /// Intrinsic `(width, height)` for `tp`. When `wrap_width` is `Some(w)`,
    /// wrap at `w` and return `(w, wrapped_height)`.
    fn measure(&self, tp: &TextProps, wrap_width: Option<f32>) -> Vec2;

    /// True when the requested `tp.style.font_family` (or any run's
    /// `font_family`) is not loaded in the renderer's font database and a
    /// fallback face was used. Default `false`.
    fn uses_fallback_font(&self, _tp: &TextProps) -> bool {
        false
    }
}

static MEASURER: OnceLock<&'static dyn TextMeasure> = OnceLock::new();

/// Install the process-wide text measurer. Safe to call more than once with
/// the same static reference; a second call with a different implementation
/// is ignored in release and logs a debug warning.
pub fn install(m: &'static dyn TextMeasure) {
    if let Err(existing) = MEASURER.set(m) {
        #[cfg(debug_assertions)]
        if !std::ptr::eq(existing as *const dyn TextMeasure as *const (), m as *const dyn TextMeasure as *const ()) {
            eprintln!("designos_doc::text_measure::install: a different TextMeasure is already installed; ignoring second install");
        }
        let _ = existing;
    }
}

pub fn is_installed() -> bool {
    MEASURER.get().is_some()
}

/// Measure `tp` with the installed [`TextMeasure`], or fall back to the
/// character-width estimate when no measurer is installed.
pub fn intrinsic_text_size(tp: &TextProps, wrap_width: Option<f32>) -> Vec2 {
    if let Some(m) = MEASURER.get() {
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

/// True when a measurer is installed and reports that `tp` fell back from its
/// requested `font_family`.
pub fn uses_fallback_font(tp: &TextProps) -> bool {
    MEASURER.get().is_some_and(|m| m.uses_fallback_font(tp))
}
