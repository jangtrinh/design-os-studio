//! Contrast rule wrapper for the `Check designs` panel, delegating to [`designos_doc::lint::rules::contrast`].

use super::{Finding, Fix, Rule};
use designos_doc::lint::rules::contrast as lint_contrast;
use designos_doc::{Document, Node};

pub(super) use lint_contrast::fresh_fix;
pub(in crate::check_designs) use lint_contrast::SubtreeBounds;
#[cfg(test)]
pub use lint_contrast::{contrast_ratio, required_ratio};
#[cfg(test)]
pub(super) use lint_contrast::{has_layer_behind, Behind};

/// Checks text layer `n`. `bounds` is shared by all the checks of one audit.
/// Returns `true` when the 3-render pixel sampler was invoked.
pub(super) fn check(doc: &Document, bounds: &SubtreeBounds, n: &Node, allow_pixels: bool, out: &mut Vec<Finding>) -> bool {
    let outcome = if allow_pixels {
        lint_contrast::check_designs_outcome_with_env(doc, bounds, n, &designos_render::RenderLintEnv)
    } else {
        lint_contrast::check_designs_outcome(doc, bounds, n).map(|(m, r, l)| (m, r, l, false))
    };
    let Some((message, recolor, fix_label, used_pixels)) = outcome else {
        return false;
    };
    let fix = recolor.map(|color| Fix::SetTextColor { color });
    out.push(Finding { node: n.id, node_name: n.name.clone(), rule: Rule::Contrast, property: String::new(), message, fix, fix_label });
    used_pixels
}
