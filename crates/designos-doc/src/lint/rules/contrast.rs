//! WCAG 2.x AA / AAA text contrast rule against the resolved backdrop (E15-10 / #595).
//!
//! Two evaluation paths:
//! 1. **Analytic fast path (`method: "analytic"`)**: When the text has a single solid fill with
//!    normal blending, no layer painted after the text overlaps its box, and every layer painted
//!    under the text is a solid fill with normal blending that covers the text box, the exact
//!    screen colours are composited analytically without invoking [`crate::lint::LintEnv::backdrop`].
//! 2. **Pixel path (`method: "pixels"`)**: When gradients, images, partial covers, rounded-corner
//!    cutouts, blurs/glass, non-normal blend modes, multi-colour text runs, or overlays above the
//!    text are present, [`crate::lint::LintEnv::backdrop`] renders the text's page-space AABB with
//!    and without the text node plus a glyph mask and reports the worst-case interior glyph contrast.

pub mod backdrop;
pub mod behind;
pub mod children;
pub mod extent;
pub mod subtree;
pub mod xpath;

use super::NodeEvalCtx;
use crate::lint::finding::{Finding, FixSuggestion, SamplePoint};
use crate::lint::registry::{Category, Severity};
use crate::{resolved_node, BlendMode, Color, ContrastLevel, Document, Node, NodeId, NodeKind, Paint, PaintKind, TextProps, VariableKind, VariableValue};
pub use backdrop::{backdrop, blends_as_group, Backdrop as AnalyticBackdrop};
pub use behind::{has_layer_behind, layer_painted_after, Behind};
pub use children::ChildIndex;
pub use subtree::SubtreeBounds;

fn linear(v: f32) -> f64 {
    let v = f64::from(v.clamp(0.0, 1.0));
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an opaque sRGB colour (`0.0..=1.0`).
pub fn luminance(c: Color) -> f64 {
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}

/// WCAG contrast ratio (`1.0..=21.0`) between two opaque colours; order does not matter.
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// True when `(font_size, weight)` qualifies as WCAG large text (`>= 24px`, or `>= 18.66px` and `weight >= 700`).
pub fn is_large_text(font_size: f32, weight: u16) -> bool {
    font_size >= 24.0 || (font_size >= 18.66 && weight >= 700)
}

/// Required WCAG contrast ratio for text at `level`:
/// - `AA`: `3.0` for large text (`>= 24px`, or `>= 18.66px` and `weight >= 700`), else `4.5`.
/// - `AAA`: `4.5` for large text, else `7.0`.
pub fn required_ratio(font_size: f32, weight: u16, level: ContrastLevel) -> f64 {
    let large = is_large_text(font_size, weight);
    match (level, large) {
        (ContrastLevel::AA, true) => 3.0,
        (ContrastLevel::AA, false) => 4.5,
        (ContrastLevel::AAA, true) => 4.5,
        (ContrastLevel::AAA, false) => 7.0,
    }
}

/// Effective base fills of a text node: `t.style.fills` normally, or `n.fills` when a caller
/// populated `n.fills` on a text node while leaving `t.style.fills` empty or at default black.
pub fn effective_text_base_fills<'a>(n: &'a Node, t: &'a TextProps) -> &'a [Paint] {
    if !n.fills.is_empty() && (t.style.fills.is_empty() || t.style.fills.as_slice() == [Paint::solid(Color::BLACK)]) {
        &n.fills
    } else {
        &t.style.fills
    }
}

/// True when `n` (or any run of `t`) has at least one visible non-transparent paint.
pub fn has_any_visible_text_paint(n: &Node, t: &TextProps) -> bool {
    effective_text_base_fills(n, t).iter().any(extent::paint_seen)
        || t.style.fills.iter().any(extent::paint_seen)
        || n.fills.iter().any(extent::paint_seen)
        || t.runs.iter().any(|r| r.style.fills.iter().any(extent::paint_seen))
}

/// The text's single visible solid paint: `(paint_index, color, paint_opacity)`.
/// `None` for gradients, images, mixed runs, or non-normal paint blending.
pub fn text_paint(n: &Node) -> Option<(usize, Color, f32)> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let visible = |ps: &[Paint]| -> Vec<(usize, Paint)> { ps.iter().cloned().enumerate().filter(|(_, p)| extent::paint_seen(p)).collect() };
    let base_slice = effective_text_base_fills(n, t);
    let base = visible(base_slice);
    if t.runs.iter().any(|r| visible(&r.style.fills) != base) {
        return None;
    }
    match base.as_slice() {
        [(i, Paint { kind: PaintKind::Solid(c), blend: BlendMode::Normal | BlendMode::PassThrough, opacity, .. })] => Some((*i, *c, *opacity)),
        _ => None,
    }
}

/// Everything the Check designs rule needs about a text layer that fails the active contrast level.
pub struct Failure {
    pub paint: usize,
    pub color: Color,
    /// The text's own alpha: colour alpha x paint opacity x layer opacity.
    pub alpha: f32,
    pub backdrop: AnalyticBackdrop,
    /// The plain backdrop colour on screen.
    pub bg: Color,
    pub ratio: f64,
    pub required: f64,
}

impl Failure {
    /// The contrast the text would have if it were painted in `color`.
    pub fn ratio_with(&self, color: Color) -> f64 {
        contrast_ratio(self.backdrop.pixel(Some((color, self.alpha))), self.bg)
    }
}

/// Evaluates the analytic backdrop model for `id`, returning `(fg_screen, bg_screen, ratio, Failure)`
/// when the backdrop and text are analytically checkable.
pub fn evaluate_analytic(doc: &Document, id: NodeId, bounds: &SubtreeBounds, level: ContrastLevel) -> Option<(Color, Color, f64, Failure)> {
    let n = resolved_node(doc, id)?;
    let NodeKind::Text(t) = &n.kind else { return None };
    if t.content.trim().is_empty() || blends_as_group(n.blend) || n.opacity <= 0.0 {
        return None;
    }
    let (paint, color, paint_opacity) = text_paint(&n)?;
    let alpha = color.a * paint_opacity * n.opacity;
    if alpha <= 0.0 {
        return None;
    }
    let mut bd = backdrop(doc, id)?;
    match has_layer_behind(doc, id, bd.occluder, Some(bounds)) {
        Behind::Nothing => {}
        Behind::Unknown => return None,
        Behind::Cover { parent, fills } => {
            if !bd.covered_by(parent, fills) {
                return None;
            }
        }
    }
    let bg = bd.pixel(None);
    let fg = bd.pixel(Some((color, alpha)));
    let ratio = contrast_ratio(fg, bg);
    let weight = t.style.get_axis("wght").map(|v| v.round().clamp(1.0, 1000.0) as u16).unwrap_or(t.style.font_weight);
    let required = required_ratio(t.style.font_size, weight, level);
    Some((fg, bg, ratio, Failure { paint, color, alpha, backdrop: bd, bg, ratio, required }))
}

pub fn failure(doc: &Document, id: NodeId, bounds: &SubtreeBounds) -> Option<Failure> {
    let (_, _, ratio, f) = evaluate_analytic(doc, id, bounds, doc.check_designs.contrast_level)?;
    (ratio < f.required).then_some(f)
}

/// The mildest recolouring toward black or white that reaches `required`, quantised to 8 bits.
pub fn suggest_recolor(f: &Failure) -> Option<Color> {
    let mut targets = [Color::BLACK, Color::WHITE];
    targets.sort_by(|a, b| contrast_ratio(*b, f.bg).total_cmp(&contrast_ratio(*a, f.bg)));
    let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    targets.iter().find_map(|target| {
        (0..=255u16).find_map(|step| {
            let t = f32::from(step) / 255.0;
            let mix = |a: f32, b: f32| a + (b - a) * t;
            let c = Color { a: f.color.a, ..Color::rgb(q(mix(f.color.r, target.r)), q(mix(f.color.g, target.g)), q(mix(f.color.b, target.b))) };
            (f.ratio_with(c) >= f.required).then_some(c)
        })
    })
}

/// Paint index and replacement colour for `id` as it is right now, or `None` when it passes, is
/// not checkable, or no recolouring can reach the required level.
pub fn fresh_fix(doc: &Document, id: NodeId) -> Option<(usize, Color)> {
    let f = failure(doc, id, &SubtreeBounds::default())?;
    if owner(doc, doc.get(id)?, f.paint).is_some() {
        return None;
    }
    suggest_recolor(&f).map(|c| (f.paint, c))
}

/// What owns the colour of text paint `paint`: a variable or a style.
pub fn owner(doc: &Document, n: &Node, paint: usize) -> Option<String> {
    let bound = n.variables.bindings.get(&format!("fills/{paint}")).and_then(|v| doc.variables.get(v)).map(|v| format!("variable {}", v.name));
    bound.or_else(|| n.style_refs.get("fill").map(|k| format!("style {}", doc.style_name(k))))
}

pub fn floor2(v: f64) -> f64 {
    (v * 100.0).floor() / 100.0
}

pub fn ratio_label(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

/// Outcome for `Check designs` panel compatibility (`(message, optional_recolor, fix_label)`).
pub fn check_designs_outcome(doc: &Document, bounds: &SubtreeBounds, n: &Node) -> Option<(String, Option<Color>, String)> {
    if !matches!(n.kind, NodeKind::Text(_)) {
        return None;
    }
    let f = failure(doc, n.id, bounds)?;
    let level = doc.check_designs.contrast_level;
    if let Some(src_id) = n.component.source {
        let same_own = match (resolved_node(doc, n.id).as_deref(), resolved_node(doc, src_id).as_deref()) {
            (Some(a), Some(b)) => match (&a.kind, &b.kind) {
                (NodeKind::Text(ta), NodeKind::Text(tb)) => {
                    a.opacity == b.opacity
                        && a.blend == b.blend
                        && ta.style.fills == tb.style.fills
                        && ta.runs == tb.runs
                        && required_ratio(ta.style.font_size, ta.style.font_weight, level) == required_ratio(tb.style.font_size, tb.style.font_weight, level)
                }
                _ => false,
            },
            _ => false,
        };
        if same_own && failure(doc, src_id, bounds).is_some_and(|sf| sf.bg == f.bg && (sf.ratio - f.ratio).abs() < 1e-6 && sf.required == f.required) {
            return None;
        }
    }
    let level_tag = match level {
        ContrastLevel::AA => "WCAG AA",
        ContrastLevel::AAA => "WCAG AAA",
    };
    let mut message = format!("Contrast {:.2}:1 is below {}:1 ({level_tag})", floor2(f.ratio), ratio_label(f.required));
    let (recolor, fix_label) = match (owner(doc, n, f.paint), suggest_recolor(&f)) {
        (Some(own), _) => {
            message.push_str(&format!(" · color comes from {own}"));
            (None, String::new())
        }
        (None, Some(color)) => {
            let new_ratio = f.ratio_with(color);
            (Some(color), format!("Change text to #{} ({:.2}:1)", color.to_hex(), floor2(new_ratio)))
        }
        (None, None) => (None, String::new()),
    };
    Some((message, recolor, fix_label))
}

/// Outcome for `Check designs` panel with pixel-sampler fallback (`env`) for gradients, images,
/// and overlays (`(message, optional_recolor, fix_label, used_pixels)`).
/// One-click recolor (`optional_recolor`) is kept on the analytic path only; pixel-sampled findings
/// include a suggest-only variable bind when available.
pub fn check_designs_outcome_with_env(
    doc: &Document,
    bounds: &SubtreeBounds,
    n: &Node,
    env: &dyn crate::lint::LintEnv,
) -> Option<(String, Option<Color>, String, bool)> {
    if !matches!(n.kind, NodeKind::Text(_)) {
        return None;
    }
    if let Some((message, recolor, fix_label)) = check_designs_outcome(doc, bounds, n) {
        return Some((message, recolor, fix_label, false));
    }
    let res_n = resolved_node(doc, n.id)?;
    let NodeKind::Text(t) = &res_n.kind else {
        return None;
    };
    if !res_n.visible
        || res_n.opacity <= 0.0
        || t.content.trim().is_empty()
        || blends_as_group(res_n.blend)
        || res_n.effects.iter().any(|e| e.visible())
        || ((res_n.stroke.weight > 0.0 || res_n.stroke.side_weights.is_some()) && res_n.stroke.paints.iter().any(extent::paint_seen))
    {
        return None;
    }
    let (paint, color, paint_opacity) = text_paint(&res_n)?;
    if color.a * paint_opacity * res_n.opacity <= 0.0 {
        return None;
    }
    if layer_painted_after(doc, n.id, Some(bounds)).is_some() {
        return None;
    }
    let (occluder, has_non_solid_fill) = backdrop::non_analytic_occluder(doc, n.id)?;
    let eligible = match has_layer_behind(doc, n.id, occluder, Some(bounds)) {
        Behind::Nothing | Behind::Cover { .. } => has_non_solid_fill,
        Behind::Unknown => behind::has_plain_overlay_sibling_in_filled_frame(doc, n.id, occluder, Some(bounds)),
    };
    if !eligible {
        return None;
    }
    let page_bounds = extent::page_box(doc, n.id)?;
    let sampled = env.backdrop_in_pass(doc, n.id, page_bounds, bounds.pass())?;
    let bg = sampled.color;
    let fg = sampled.foreground.unwrap_or(color);
    let ratio = sampled.worst_ratio.map(f64::from).unwrap_or_else(|| contrast_ratio(fg, bg));
    let level = doc.check_designs.contrast_level;
    let weight = t.style.get_axis("wght").map(|v| v.round().clamp(1.0, 1000.0) as u16).unwrap_or(t.style.font_weight);
    let required = required_ratio(t.style.font_size, weight, level);
    if ratio >= required {
        return None;
    }
    let level_tag = match level {
        ContrastLevel::AA => "WCAG AA",
        ContrastLevel::AAA => "WCAG AAA",
    };
    let mut message = format!("Contrast {:.2}:1 is below {}:1 ({level_tag})", floor2(ratio), ratio_label(required));
    if let Some(own) = owner(doc, n, paint) {
        message.push_str(&format!(" · color comes from {own}"));
    }
    if let Some(sug) = suggest_passing_variable(doc, n, &res_n, fg, bg, required) {
        if let Some(to) = sug.to.as_str() {
            message.push_str(&format!(" · suggest {to} on {}", sug.property));
        }
    }
    Some((message, None, String::new(), true))
}

// ---------------------------------------------------------------------------
// CIE76 L*a*b* colour difference and Passing Variable Pair suggestion (§3.4)
// ---------------------------------------------------------------------------

fn srgb_to_lab(c: Color) -> (f64, f64, f64) {
    let r = linear(c.r);
    let g = linear(c.g);
    let b = linear(c.b);
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
    let xn = 0.95047_f64;
    let yn = 1.00000_f64;
    let zn = 1.08883_f64;
    fn f(t: f64) -> f64 {
        let delta = 6.0 / 29.0;
        if t > delta * delta * delta {
            t.cbrt()
        } else {
            t / (3.0 * delta * delta) + 4.0 / 29.0
        }
    }
    let fx = f(x / xn);
    let fy = f(y / yn);
    let fz = f(z / zn);
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// CIE76 `ΔE*ab` distance between two sRGB colours.
pub fn cie76_delta_e(a: Color, b: Color) -> f64 {
    let (l1, a1, b1) = srgb_to_lab(a);
    let (l2, a2, b2) = srgb_to_lab(b);
    ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
}

fn same_rgb(a: Color, b: Color) -> bool {
    let [ar, ag, ab, _] = a.to_u8();
    let [br, bg, bb, _] = b.to_u8();
    ar == br && ag == bg && ab == bb
}

/// Resolves a colour variable `var_id` at `node_id`, falling back to the variable's first mode
/// value if the document does not have an explicit `VariableCollection` entry.
fn resolve_color_variable_at(doc: &Document, node_id: NodeId, var_id: &str) -> Option<Color> {
    if let Some(VariableValue::Color(c)) = doc.resolve_variable(node_id, var_id) {
        return Some(c);
    }
    let mut id = var_id.to_string();
    for _ in 0..16 {
        let v = doc.variables.get(&id)?;
        if v.kind != VariableKind::Color {
            return None;
        }
        let val = if let Some(mode) = doc.variable_mode(node_id, &v.collection) {
            v.values.get(&mode).or_else(|| v.values.values().next())?
        } else {
            v.values.values().next()?
        };
        match val {
            VariableValue::Color(c) => return Some(*c),
            VariableValue::Alias(next) => id = next.clone(),
            _ => return None,
        }
    }
    None
}

/// Finds the `VariableCollection` id associated with a node's fill property `prop` (either via
/// `variables.bindings` when the bound variable resolves to `raw_color`, or when unbound by
/// matching `match_color` against colour variables in `doc.variables`).
fn find_fill_collection<'a>(doc: &'a Document, node: &Node, bound_var_id: Option<&str>, raw_color: Color, match_color: Option<Color>) -> Option<&'a str> {
    if let Some(var_id) = bound_var_id {
        let v = doc.variables.get(var_id)?;
        if v.kind != VariableKind::Color {
            return None;
        }
        let resolved = resolve_color_variable_at(doc, node.id, var_id)?;
        if same_rgb(resolved, raw_color) {
            return Some(&v.collection);
        }
        return None;
    }
    let target = match_color?;
    for v in doc.variables.values() {
        if v.kind == VariableKind::Color {
            if let Some(c) = resolve_color_variable_at(doc, node.id, &v.id) {
                if same_rgb(c, target) {
                    return Some(&v.collection);
                }
            }
        }
    }
    None
}

/// Searches `collection` for a colour variable (resolved at `node_id`) that passes `required`
/// when applied to the text foreground role over `worst_bg`.
#[allow(clippy::too_many_arguments)]
fn best_passing_fg_in_collection<'a>(
    doc: &'a Document,
    node_id: NodeId,
    collection: &str,
    bound_var_id: Option<&str>,
    paint_opacity: f32,
    node_opacity: f32,
    current_fg: Color,
    worst_bg: Color,
    required: f64,
) -> Option<(f64, &'a str)> {
    let mut best: Option<(f64, &'a str)> = None;
    for v in doc.variables.values() {
        if v.kind != VariableKind::Color || v.collection != collection || bound_var_id == Some(v.id.as_str()) {
            continue;
        }
        let Some(cand_color) = resolve_color_variable_at(doc, node_id, &v.id) else {
            continue;
        };
        let eff_alpha = (cand_color.a * paint_opacity * node_opacity).clamp(0.0, 1.0);
        let eff_cand_fg = Color {
            r: cand_color.r * eff_alpha + worst_bg.r * (1.0 - eff_alpha),
            g: cand_color.g * eff_alpha + worst_bg.g * (1.0 - eff_alpha),
            b: cand_color.b * eff_alpha + worst_bg.b * (1.0 - eff_alpha),
            a: 1.0,
        };
        if contrast_ratio(eff_cand_fg, worst_bg) >= required {
            let de = cie76_delta_e(current_fg, eff_cand_fg);
            if best.is_none_or(|(best_de, _)| de < best_de) {
                best = Some((de, &v.name));
            }
        }
    }
    best
}

/// Suggests binding the failing text fill to the closest passing variable in the same collection,
/// scored in the role that would actually change (§3.4). Backdrop variables are not suggested:
/// a finding has no separate parent target, so a parent `fills/*` path would be applied to text.
pub fn suggest_passing_variable(doc: &Document, raw_node: &Node, resolved_n: &Node, fg: Color, bg: Color, required: f64) -> Option<FixSuggestion> {
    if doc.variables.is_empty() {
        return None;
    }
    let (paint_idx, raw_fg, paint_opacity) = text_paint(resolved_n).unwrap_or((0, fg, 1.0));
    let text_alpha = (raw_fg.a * paint_opacity * resolved_n.opacity).clamp(0.0, 1.0);
    let indexed_fg_key = format!("fills/{paint_idx}");
    let (fg_prop, bound_fg_var) = if let Some(v) = raw_node.variables.bindings.get(&indexed_fg_key) {
        (indexed_fg_key, Some(v.as_str()))
    } else if paint_idx == 0 {
        if let Some(v) = raw_node.variables.bindings.get("fill") {
            ("fill".to_string(), Some(v.as_str()))
        } else {
            (indexed_fg_key, None)
        }
    } else {
        (indexed_fg_key, None)
    };
    let fg_match_color = (text_alpha >= 1.0 - 1e-4).then_some(raw_fg);
    let fg_candidate = find_fill_collection(doc, raw_node, bound_fg_var, raw_fg, fg_match_color)
        .and_then(|coll| best_passing_fg_in_collection(doc, raw_node.id, coll, bound_fg_var, paint_opacity, resolved_n.opacity, fg, bg, required))
        .map(|(de, name)| (de, fg_prop.clone(), format!("#{}", fg.to_hex()), name));

    let (_, property, from, name) = fg_candidate?;

    Some(FixSuggestion { kind: "bind".into(), property, from: serde_json::Value::String(from), to: serde_json::Value::String(format!("var:{name}")) })
}

/// Evaluates the unified `"contrast"` rule on `ctx.node`, appending any finding to `out`.
/// Returns `true` if the pixel-sampling path (`LintEnv::backdrop`) was invoked.
pub fn evaluate_contrast(ctx: &NodeEvalCtx<'_>, bounds: &SubtreeBounds, out: &mut Vec<Finding>) -> bool {
    if !ctx.config.is_enabled("contrast") {
        return false;
    }
    let doc = ctx.doc;
    let raw_n = ctx.node;
    let Some(resolved_n) = resolved_node(doc, raw_n.id) else {
        return false;
    };
    let NodeKind::Text(t) = &resolved_n.kind else {
        return false;
    };
    // §8.2 & §8.3: Skip empty/whitespace-only text and invisible or missing-fill text.
    if !resolved_n.visible || resolved_n.opacity <= 0.0 || t.content.trim().is_empty() || !has_any_visible_text_paint(&resolved_n, t) {
        return false;
    }
    // Check if any ancestor has zero opacity or is invisible.
    let mut cur_anc = resolved_n.parent;
    while let Some(pid) = cur_anc {
        let Some(pn) = doc.get(pid) else { break };
        if matches!(pn.kind, NodeKind::Page { .. }) {
            break;
        }
        if !pn.visible || pn.opacity <= 0.0 {
            return false;
        }
        cur_anc = pn.parent;
    }

    let weight = t.style.get_axis("wght").map(|v| v.round().clamp(1.0, 1000.0) as u16).unwrap_or(t.style.font_weight);
    let font_size = t.style.font_size;
    let large = is_large_text(font_size, weight);
    let level = ctx.config.effective_contrast_level(doc);
    let aa_req = required_ratio(font_size, weight, ContrastLevel::AA);
    let required = required_ratio(font_size, weight, level);

    let after_layer = layer_painted_after(doc, raw_n.id, Some(bounds));
    let has_own_effects_or_stroke = resolved_n.effects.iter().any(|e| e.visible())
        || ((resolved_n.stroke.weight > 0.0 || resolved_n.stroke.side_weights.is_some()) && resolved_n.stroke.paints.iter().any(extent::paint_seen));

    let mut used_pixel_path = false;
    let eval_result = if after_layer.is_none() && !has_own_effects_or_stroke {
        if let Some((fg, bg, ratio, _)) = evaluate_analytic(doc, raw_n.id, bounds, level) {
            let pt = SamplePoint { x: (ctx.page_bounds.x + ctx.page_bounds.w * 0.5).round(), y: (ctx.page_bounds.y + ctx.page_bounds.h * 0.5).round() };
            Some((fg, bg, ratio, "analytic", pt, None))
        } else {
            None
        }
    } else {
        None
    };

    let (fg, bg, ratio, method, worst_sample, covered_by) = match eval_result {
        Some(res) => res,
        None => {
            let Some(sampled) = ctx.env.backdrop_in_pass(doc, raw_n.id, ctx.page_bounds, bounds.pass()) else {
                return false;
            };
            used_pixel_path = true;
            let bg = sampled.color;
            let fg = sampled.foreground.unwrap_or_else(|| text_paint(&resolved_n).map(|(_, c, _)| c).unwrap_or(Color::BLACK));
            let ratio = sampled.worst_ratio.map(f64::from).unwrap_or_else(|| contrast_ratio(fg, bg));
            let pt = sampled
                .worst_sample
                .unwrap_or(SamplePoint { x: (ctx.page_bounds.x + ctx.page_bounds.w * 0.5).round(), y: (ctx.page_bounds.y + ctx.page_bounds.h * 0.5).round() });
            let cov = if sampled.covered { behind::covering_layer_painted_after(doc, raw_n.id, pt, Some(bounds)) } else { None };
            (fg, bg, ratio, "pixels", pt, cov)
        }
    };

    if ratio < required {
        let Some(configured_sev) = ctx.config.rule_severity("contrast") else {
            return used_pixel_path;
        };
        let severity = if configured_sev != Severity::Error {
            configured_sev
        } else if level == ContrastLevel::AAA && ratio >= aa_req {
            Severity::Warning
        } else {
            Severity::Error
        };
        let measured = (ratio * 100.0).round() / 100.0;
        let fg_hex = format!("#{}", fg.to_hex());
        let bg_hex = format!("#{}", bg.to_hex());
        let level_str = match level {
            ContrastLevel::AA => "AA",
            ContrastLevel::AAA => "AAA",
        };
        let message = if let Some(cov_name) = covered_by {
            format!("text is covered by {cov_name}")
        } else {
            format!(
                "Text '{}' has {:.2}:1 contrast ({} on {}) at ({:.0}, {:.0}); WCAG {} requires {:.1}:1.",
                raw_n.name, measured, fg_hex, bg_hex, worst_sample.x, worst_sample.y, level_str, required
            )
        };
        let suggested_fix = suggest_passing_variable(doc, raw_n, &resolved_n, fg, bg, required);
        out.push(Finding {
            rule: "contrast",
            category: Category::Accessibility,
            severity,
            node: raw_n.id,
            node_name: raw_n.name.clone(),
            xpath: xpath::xpath_in_pass(doc, raw_n.id, bounds.pass()),
            bounds: ctx.page_bounds,
            message,
            measured_ratio: Some(measured),
            required_ratio: Some(required),
            level: Some(level_str),
            large: Some(large),
            method: Some(method),
            worst_sample: Some(worst_sample),
            foreground_hex: Some(fg_hex),
            backdrop_hex: Some(bg_hex),
            suggested_fix,
            fixable: false,
            safe: false,
            property: None,
            value: None,
            nearest: None,
            scale_source: None,
            candidates: None,
            ignored: false,
        });
    }

    used_pixel_path
}
