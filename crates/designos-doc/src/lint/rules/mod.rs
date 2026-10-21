//! Evaluation of all registered design lint rules.

pub mod detached;
pub mod scale;

use super::config::LintConfig;
use super::finding::{Finding, FixSuggestion};
use super::registry::{Category, Severity};
use super::LintEnv;
use crate::openpencil::{infer_role_hint, node_to_xpath};
use crate::{Affine, Document, Node, NodeId, NodeKind, Paint, PaintKind, Rect, Sizing, TextAutoResize, Vec2};
pub use scale::{FONT_SCALE, RADIUS_SCALE, SPACING_SCALE};
use serde_json::json;

pub fn on_scale(v: f32, scale: &[f32]) -> bool {
    scale.iter().any(|&s| (v - s).abs() < 0.05)
}

pub fn nearest_on_scale(v: f32, scale: &[f32]) -> f32 {
    scale.iter().copied().min_by(|a, b| (v - *a).abs().total_cmp(&(v - *b).abs())).unwrap_or(v)
}

pub fn is_grid_multiple(v: f32, step: f32) -> bool {
    if step <= 0.0 {
        return true;
    }
    let rem = (v / step).round() * step - v;
    rem.abs() < 0.05
}

pub fn is_default_layer_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }
    const PREFIXES: [&str; 13] =
        ["Frame", "Group", "Rectangle", "Ellipse", "Line", "Polygon", "Star", "Vector", "Text", "Section", "Component", "Instance", "Layer"];
    for p in PREFIXES {
        if trimmed.eq_ignore_ascii_case(p) {
            return true;
        }
        if let Some(rest) = trimmed.strip_prefix(p) {
            let rest = rest.trim();
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

/// Axis-aligned bounding box of `Rect(0, 0, size.x, size.y)` transformed by `xf`.
#[inline]
pub fn aabb_of_transformed_rect(xf: Affine, size: Vec2) -> Rect {
    if xf.b == 0.0 && xf.c == 0.0 && xf.a >= 0.0 && xf.d >= 0.0 {
        return Rect::new(xf.tx, xf.ty, (xf.a * size.x).max(0.0), (xf.d * size.y).max(0.0));
    }
    let p0 = xf.apply(Vec2::new(0.0, 0.0));
    let p1 = xf.apply(Vec2::new(size.x, 0.0));
    let p2 = xf.apply(Vec2::new(size.x, size.y));
    let p3 = xf.apply(Vec2::new(0.0, size.y));
    let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
    let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
    let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
    let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);
    Rect::new(min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
}

#[inline]
pub fn is_rotated(xf: Affine) -> bool {
    xf.b.abs() > 1e-4 || xf.c.abs() > 1e-4
}

fn paint_is_visible(p: &Paint) -> bool {
    if !p.visible || p.opacity <= 0.0 {
        return false;
    }
    match &p.kind {
        PaintKind::Solid(c) => c.a > 0.0,
        _ => true,
    }
}

fn auto_resize_str(ar: TextAutoResize) -> &'static str {
    match ar {
        TextAutoResize::WidthAndHeight => "WIDTH_AND_HEIGHT",
        TextAutoResize::Height => "HEIGHT",
        TextAutoResize::None => "NONE",
        TextAutoResize::Truncate => "TRUNCATE",
    }
}

fn is_placeholder_text(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.len() < 3 {
        return false;
    }
    const EXACT: [&str; 12] =
        ["text", "label", "button", "title", "heading", "placeholder", "sample text", "your text here", "type something", "todo", "tbd", "xxx"];
    if trimmed.len() <= 15 && EXACT.iter().any(|&w| trimmed.eq_ignore_ascii_case(w)) {
        return true;
    }
    if trimmed.len() >= 11 {
        let b = trimmed.as_bytes();
        return b.windows(11).any(|w| w.eq_ignore_ascii_case(b"lorem ipsum"))
            || (b.len() >= 14 && b.windows(14).any(|w| w.eq_ignore_ascii_case(b"dolor sit amet")));
    }
    false
}

/// Precomputed O(1) active-rule flags resolved once per lint run so per-node evaluation never touches `BTreeMap`.
#[derive(Clone, Copy, Debug)]
pub struct ActiveRules {
    pub overflow_parent: bool,
    pub clipped_partial: bool,
    pub clipped_full: bool,
    pub collapsed_size: bool,
    pub sizing_conflict: bool,
    pub absolute_in_auto_layout: bool,
    pub unbalanced_padding: bool,
    pub missing_auto_layout: bool,
    pub misaligned_siblings: bool,
    pub sibling_overlap: bool,
    pub text_missing_fill: bool,
    pub placeholder_text: bool,
    pub text_truncated: bool,
    pub text_unexpected_wrap: bool,
    pub tiny_text: bool,
    pub detached_text_style: bool,
    pub detached_instance: bool,
    pub default_layer_name: bool,
    pub deeply_nested: bool,
    pub empty_container: bool,
    pub non_4px_grid: bool,
    pub hardcoded_color: bool,
    pub small_touch_target: bool,
}

impl ActiveRules {
    pub fn from_config(config: &LintConfig) -> Self {
        Self {
            overflow_parent: config.is_enabled("overflow-parent"),
            clipped_partial: config.is_enabled("clipped-partial"),
            clipped_full: config.is_enabled("clipped-full"),
            collapsed_size: config.is_enabled("collapsed-size"),
            sizing_conflict: config.is_enabled("sizing-conflict"),
            absolute_in_auto_layout: config.is_enabled("absolute-in-auto-layout"),
            unbalanced_padding: config.is_enabled("unbalanced-padding"),
            missing_auto_layout: config.is_enabled("missing-auto-layout"),
            misaligned_siblings: config.is_enabled("misaligned-siblings"),
            sibling_overlap: config.is_enabled("sibling-overlap"),
            text_missing_fill: config.is_enabled("text-missing-fill"),
            placeholder_text: config.is_enabled("placeholder-text"),
            text_truncated: config.is_enabled("text-truncated"),
            text_unexpected_wrap: config.is_enabled("text-unexpected-wrap"),
            tiny_text: config.is_enabled("tiny-text"),
            detached_text_style: config.is_enabled("detached-text-style"),
            detached_instance: config.is_enabled("detached-instance"),
            default_layer_name: config.is_enabled("default-layer-name"),
            deeply_nested: config.is_enabled("deeply-nested"),
            empty_container: config.is_enabled("empty-container"),
            non_4px_grid: config.is_enabled("non-4px-grid"),
            hardcoded_color: config.is_enabled("hardcoded-color"),
            small_touch_target: config.is_enabled("small-touch-target"),
        }
    }
}

/// Pre-resolved parent node reference and cached mask check passed down from parent to child during DFS.
#[derive(Clone, Copy, Debug)]
pub struct ParentEvalCtx<'a> {
    pub node: &'a Node,
    pub has_mask: bool,
}

/// Context passed to [`evaluate_node`] for a single node during traversal.
pub struct NodeEvalCtx<'a> {
    pub doc: &'a Document,
    pub node: &'a Node,
    pub parent: Option<ParentEvalCtx<'a>>,
    pub children: &'a [&'a Node],
    pub depth: usize,
    pub in_component: bool,
    pub is_instance_mirror: bool,
    pub page_bounds: Rect,
    pub config: &'a LintConfig,
    pub active: &'a ActiveRules,
    pub env: &'a dyn LintEnv,
    pub components: &'a detached::Components,
    pub scales: &'a scale::Scales,
    pub token_index: &'a super::token_match::Index,
}

/// Evaluates all enabled rules on `ctx.node` and appends any findings to `out`.
pub fn evaluate_node(ctx: &NodeEvalCtx<'_>, out: &mut Vec<Finding>) {
    let doc = ctx.doc;
    let n = ctx.node;
    let config = ctx.config;
    let active = ctx.active;
    let tol = config.thresholds.geometry_tolerance;
    let node_xpath = scale::LazyXPath::new(doc, n.id);

    let mut emit = |rule_id: &'static str,
                    category: Category,
                    target: &Node,
                    target_bounds: Rect,
                    message: String,
                    suggested_fix: Option<FixSuggestion>,
                    fixable: bool| {
        let Some(severity) = config.rule_severity(rule_id) else {
            return;
        };
        let xpath = if target.id == n.id { node_xpath.get().to_string() } else { node_to_xpath(doc, target.id) };
        out.push(Finding {
            rule: rule_id,
            category,
            severity,
            node: target.id,
            node_name: target.name.clone(),
            xpath,
            bounds: target_bounds,
            message,
            suggested_fix,
            fixable,
            safe: rule_id == "sizing-conflict" && fixable,
            property: None,
            value: None,
            nearest: None,
            scale_source: None,
            candidates: None,
            ignored: false,
        });
    };

    // ------------------------------------------------------------------
    // 1. Parent-relative geometric rules: `overflow-parent`, `clipped-partial`, `clipped-full`
    // ------------------------------------------------------------------
    if !n.is_mask && (active.overflow_parent || active.clipped_partial || active.clipped_full) {
        if let Some(pctx) = ctx.parent {
            let parent = pctx.node;
            // Skips groups, boolean ops, pages, sections' own children: only frame-like parents qualify.
            if let Some(fp) = parent.kind.frame_props() {
                if !pctx.has_mask {
                    let cbox = aabb_of_transformed_rect(n.transform, n.size);
                    let rotated = is_rotated(n.transform);
                    let rot_prefix = if rotated { "Rotated child" } else { "Child" };
                    let exceeds = cbox.x < -tol || cbox.y < -tol || (cbox.x + cbox.w) > parent.size.x + tol || (cbox.y + cbox.h) > parent.size.y + tol;
                    if exceeds {
                        if !fp.clips_content {
                            if active.overflow_parent {
                                emit(
                                    "overflow-parent",
                                    Category::Layout,
                                    n,
                                    ctx.page_bounds,
                                    format!("{rot_prefix} '{}' overflows parent '{}'", n.name, parent.name),
                                    None,
                                    false,
                                );
                            }
                        } else {
                            let intersects = (cbox.x + cbox.w) > 0.0 && cbox.x < parent.size.x && (cbox.y + cbox.h) > 0.0 && cbox.y < parent.size.y;
                            if intersects {
                                if active.clipped_partial {
                                    emit(
                                        "clipped-partial",
                                        Category::Layout,
                                        n,
                                        ctx.page_bounds,
                                        format!("{rot_prefix} '{}' is partially clipped by parent '{}'", n.name, parent.name),
                                        None,
                                        false,
                                    );
                                }
                            } else if active.clipped_full {
                                emit(
                                    "clipped-full",
                                    Category::Layout,
                                    n,
                                    ctx.page_bounds,
                                    format!("{rot_prefix} '{}' is completely clipped by parent '{}' and invisible", n.name, parent.name),
                                    None,
                                    false,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 2. `collapsed-size` (alias `zero-bounds`)
    // ------------------------------------------------------------------
    if active.collapsed_size {
        let auto_width_text = matches!(
            &n.kind,
            NodeKind::Text(tp) if tp.auto_resize == TextAutoResize::WidthAndHeight && !tp.content.trim().is_empty()
        );
        let thin_exempt = matches!(n.kind, NodeKind::Line | NodeKind::Vector { .. } | NodeKind::Connector(_));
        let zero_dim = if auto_width_text {
            false
        } else if thin_exempt {
            n.size.x < tol && n.size.y < tol
        } else {
            n.size.x < tol || n.size.y < tol
        };
        let empty_hug = n
            .kind
            .frame_props()
            .and_then(|fp| fp.auto_layout.as_ref())
            .is_some_and(|al| (al.primary_sizing == Sizing::Hug || al.counter_sizing == Sizing::Hug) && !ctx.children.iter().any(|c| c.visible));
        if zero_dim {
            emit(
                "collapsed-size",
                Category::Layout,
                n,
                ctx.page_bounds,
                format!("Layer '{}' has collapsed dimensions {}x{}", n.name, n.size.x, n.size.y),
                None,
                false,
            );
        } else if empty_hug {
            emit(
                "collapsed-size",
                Category::Layout,
                n,
                ctx.page_bounds,
                format!("Auto-layout container '{}' hugs content but has no visible children", n.name),
                None,
                false,
            );
        }
    }

    // ------------------------------------------------------------------
    // 3. `sizing-conflict`
    // ------------------------------------------------------------------
    if active.sizing_conflict {
        if let Some(pctx) = ctx.parent {
            let parent = pctx.node;
            if !matches!(parent.kind, NodeKind::Page { .. }) {
                let parent_al = parent.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref());
                match parent_al {
                    None => {
                        if matches!(n.layout_child.horizontal, Sizing::Fill | Sizing::Hug) || matches!(n.layout_child.vertical, Sizing::Fill | Sizing::Hug) {
                            emit(
                                "sizing-conflict",
                                Category::Layout,
                                n,
                                ctx.page_bounds,
                                format!(
                                    "Child '{}' uses {:?}/{:?} sizing inside non-auto-layout parent '{}'",
                                    n.name, n.layout_child.horizontal, n.layout_child.vertical, parent.name
                                ),
                                Some(FixSuggestion {
                                    kind: "set".into(),
                                    property: "sizing".into(),
                                    from: json!(format!("{:?}/{:?}", n.layout_child.horizontal, n.layout_child.vertical)),
                                    to: json!("Fixed"),
                                }),
                                true,
                            );
                        }
                    }
                    Some(al) if !n.layout_child.absolute => {
                        let child_primary = if al.primary_is_horizontal() { n.layout_child.horizontal } else { n.layout_child.vertical };
                        if al.primary_sizing == Sizing::Hug && child_primary == Sizing::Fill {
                            let prop = if al.primary_is_horizontal() { "horizontalSizing" } else { "verticalSizing" };
                            emit(
                                "sizing-conflict",
                                Category::Layout,
                                n,
                                ctx.page_bounds,
                                format!("Child '{}' uses Fill sizing on the primary axis of hugging parent '{}'", n.name, parent.name),
                                Some(FixSuggestion { kind: "set".into(), property: prop.into(), from: json!("Fill"), to: json!("Fixed") }),
                                true,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 4. `absolute-in-auto-layout`
    // ------------------------------------------------------------------
    if active.absolute_in_auto_layout && n.layout_child.absolute {
        if let Some(pctx) = ctx.parent {
            let parent = pctx.node;
            if parent.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some() {
                emit(
                    "absolute-in-auto-layout",
                    Category::Layout,
                    n,
                    ctx.page_bounds,
                    format!("Layer '{}' is absolutely positioned inside auto-layout parent '{}'", n.name, parent.name),
                    None,
                    false,
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // 5. Container rules: `sibling-overlap`, `unbalanced-padding`,
    //    `missing-auto-layout`, `misaligned-siblings`
    // ------------------------------------------------------------------
    if let Some(fp) = n.kind.frame_props() {
        if let Some(al) = &fp.auto_layout {
            if active.unbalanced_padding && ((al.padding[0] - al.padding[2]).abs() > 0.1 || (al.padding[1] - al.padding[3]).abs() > 0.1) {
                let v = nearest_on_scale((al.padding[0] + al.padding[2]) * 0.5, &SPACING_SCALE);
                let h = nearest_on_scale((al.padding[1] + al.padding[3]) * 0.5, &SPACING_SCALE);
                emit(
                    "unbalanced-padding",
                    Category::Layout,
                    n,
                    ctx.page_bounds,
                    format!("Unbalanced padding [{}, {}, {}, {}]", al.padding[0], al.padding[1], al.padding[2], al.padding[3]),
                    Some(FixSuggestion { kind: "set".into(), property: "padding".into(), from: json!(al.padding), to: json!([v, h, v, h]) }),
                    true,
                );
            }
        } else if n.children.len() >= 2 {
            if active.missing_auto_layout {
                emit(
                    "missing-auto-layout",
                    Category::Layout,
                    n,
                    ctx.page_bounds,
                    format!("Container with {} children does not use auto-layout", n.children.len()),
                    None,
                    false,
                );
            }

            if active.misaligned_siblings {
                let child_nodes: Vec<&Node> = ctx.children.iter().copied().filter(|c| c.visible).collect();
                let mut misaligned = false;
                'mis: for i in 0..child_nodes.len() {
                    for j in (i + 1)..child_nodes.len() {
                        let a = child_nodes[i];
                        let b = child_nodes[j];
                        let dx = (a.transform.tx - b.transform.tx).abs();
                        let dy = (a.transform.ty - b.transform.ty).abs();
                        if (dx > 0.1 && dx <= 3.0) || (dy > 0.1 && dy <= 3.0) {
                            misaligned = true;
                            break 'mis;
                        }
                    }
                }
                if misaligned {
                    emit("misaligned-siblings", Category::Layout, n, ctx.page_bounds, "Child layers are off-alignment by 1–3px".into(), None, true);
                }
            }

            if active.sibling_overlap {
                check_sibling_overlap(ctx.children, n, ctx.page_bounds, config, &mut emit);
            }
        }
    } else if matches!(n.kind, NodeKind::Group) && n.children.len() >= 2 && active.sibling_overlap {
        check_sibling_overlap(ctx.children, n, ctx.page_bounds, config, &mut emit);
    }

    // ------------------------------------------------------------------
    // 6. Text rules: `text-missing-fill`, `placeholder-text`, `text-truncated`,
    //    `text-unexpected-wrap`, `tiny-text`, `detached-text-style`,
    //    `low-contrast-text`
    // ------------------------------------------------------------------
    if let Some(tp) = n.kind.any_text() {
        let is_text_node = matches!(n.kind, NodeKind::Text(_));

        if is_text_node && active.text_missing_fill && !tp.content.trim().is_empty() {
            let has_visible_paint = n.opacity > 0.0
                && (n.fills.iter().any(paint_is_visible)
                    || tp.style.fills.iter().any(paint_is_visible)
                    || tp.runs.iter().any(|r| r.style.fills.iter().any(paint_is_visible)));
            if !has_visible_paint {
                emit(
                    "text-missing-fill",
                    Category::Text,
                    n,
                    ctx.page_bounds,
                    format!("Visible text layer '{}' has no visible fill on the node or any text run", n.name),
                    None,
                    false,
                );
            }
        }

        if is_text_node && !ctx.in_component && active.placeholder_text && is_placeholder_text(&tp.content) {
            emit(
                "placeholder-text",
                Category::Content,
                n,
                ctx.page_bounds,
                format!("Text layer '{}' still contains placeholder copy '{}'", n.name, tp.content.trim()),
                None,
                false,
            );
        }

        if is_text_node && !tp.content.is_empty() && tp.auto_resize != TextAutoResize::WidthAndHeight && (active.text_truncated || active.text_unexpected_wrap)
        {
            if let Some(metrics) = ctx.env.text_metrics(n, tp, n.size) {
                let sub_note = metrics.substituted_family.as_ref().map(|sub| format!(" (substituted font '{sub}')")).unwrap_or_default();

                if active.text_truncated && tp.auto_resize != TextAutoResize::WidthAndHeight {
                    let line_cut = metrics.line_count > metrics.visible_lines;
                    let height_cut = matches!(tp.auto_resize, TextAutoResize::None | TextAutoResize::Truncate) && metrics.content_height > n.size.y + tol;
                    let parent_clips_expanded = tp.auto_resize == TextAutoResize::Height
                        && ctx.parent.is_some_and(|pctx| {
                            pctx.node.kind.frame_props().is_some_and(|fp| fp.clips_content)
                                && (n.transform.ty + metrics.content_height > pctx.node.size.y + tol)
                        });
                    let width_cut = metrics.widest_line > n.size.x + tol;
                    if line_cut || height_cut || parent_clips_expanded || width_cut {
                        let snippet: String = tp.content.lines().next().unwrap_or(&tp.content).trim().chars().take(48).collect();
                        let msg = if line_cut {
                            format!("Text needs {} lines but shows {}; '{}' is cut.{sub_note}", metrics.line_count, metrics.visible_lines, snippet)
                        } else {
                            format!(
                                "Text content ({:.0}x{:.0}) exceeds box ({:.0}x{:.0}); '{}' is cut.{sub_note}",
                                metrics.widest_line, metrics.content_height, n.size.x, n.size.y, snippet
                            )
                        };
                        emit(
                            "text-truncated",
                            Category::Text,
                            n,
                            ctx.page_bounds,
                            msg,
                            Some(FixSuggestion {
                                kind: "set".into(),
                                property: "textAutoResize".into(),
                                from: json!(auto_resize_str(tp.auto_resize)),
                                to: json!("HEIGHT"),
                            }),
                            true,
                        );
                    }
                }

                if active.text_unexpected_wrap {
                    let char_count = tp.content.chars().count();
                    let single_line_candidate =
                        !tp.content.contains('\n') && !tp.content.contains('\u{2028}') && char_count <= 40 && tp.auto_resize != TextAutoResize::WidthAndHeight;
                    let wrapped_short = single_line_candidate && metrics.line_count >= 2;
                    let mid_word = !metrics.breaks_inside_word.is_empty();
                    if wrapped_short || mid_word {
                        let msg = if mid_word {
                            format!("Text '{}' breaks inside a word at byte offset {:?}.{sub_note}", tp.content.trim(), metrics.breaks_inside_word)
                        } else {
                            format!("Single-line text '{}' ({} chars) wraps onto {} lines.{sub_note}", tp.content.trim(), char_count, metrics.line_count)
                        };
                        emit(
                            "text-unexpected-wrap",
                            Category::Text,
                            n,
                            ctx.page_bounds,
                            msg,
                            Some(FixSuggestion {
                                kind: "set".into(),
                                property: "textAutoResize".into(),
                                from: json!(auto_resize_str(tp.auto_resize)),
                                to: json!("WIDTH_AND_HEIGHT"),
                            }),
                            true,
                        );
                    }
                }
            }
        }

        if active.tiny_text && tp.style.font_size < config.thresholds.min_font_size {
            emit(
                "tiny-text",
                Category::Accessibility,
                n,
                ctx.page_bounds,
                format!("Text font size {}px is below {}px legibility minimum", tp.style.font_size, config.thresholds.min_font_size),
                Some(FixSuggestion {
                    kind: "set".into(),
                    property: "fontSize".into(),
                    from: json!(tp.style.font_size),
                    to: json!(config.thresholds.min_font_size),
                }),
                true,
            );
        }

        if active.detached_text_style && !n.style_refs.contains_key("text") {
            emit("detached-text-style", Category::Tokens, n, ctx.page_bounds, "Text layer is not linked to a shared text style".into(), None, false);
        }
    }

    // ------------------------------------------------------------------
    // 7. Component rules: `detached-instance`
    // ------------------------------------------------------------------
    if active.detached_instance {
        match detached::plan(doc, ctx.components, n) {
            detached::DetachedPlan::Nothing => {}
            detached::DetachedPlan::Unfixable { message } => {
                emit("detached-instance", Category::Components, n, ctx.page_bounds, message, None, false);
            }
            detached::DetachedPlan::Compare { component } => {
                let equal = detached::matches_component(doc, n.id, component);
                let (message, reconnect, _) = detached::compared_outcome(doc, component, equal);
                let suggested_fix = reconnect.map(|comp| FixSuggestion {
                    kind: "reconnect".into(),
                    property: "component".into(),
                    from: json!(detached::symbol_of(n).unwrap_or("")),
                    to: json!(comp.0),
                });
                let fixable = suggested_fix.is_some();
                emit("detached-instance", Category::Components, n, ctx.page_bounds, message, suggested_fix, fixable);
            }
        }
    }

    // ------------------------------------------------------------------
    // 8. Naming, structure, token, scale, and accessibility rules
    // ------------------------------------------------------------------
    if active.default_layer_name && is_default_layer_name(&n.name) {
        emit("default-layer-name", Category::Naming, n, ctx.page_bounds, format!("Layer '{}' uses a generic default name", n.name), None, true);
    }

    if active.deeply_nested && ctx.depth > config.thresholds.max_nesting_depth {
        emit(
            "deeply-nested",
            Category::Structure,
            n,
            ctx.page_bounds,
            format!("Node is nested at depth {} (> {})", ctx.depth, config.thresholds.max_nesting_depth),
            None,
            false,
        );
    }

    if active.empty_container
        && matches!(n.kind, NodeKind::Frame(_) | NodeKind::Group)
        && n.children.is_empty()
        && n.fills.iter().all(|p| !p.visible)
        && n.stroke.paints.iter().all(|p| !p.visible)
        && n.effects.iter().all(|e| !e.visible())
    {
        emit("empty-container", Category::Structure, n, ctx.page_bounds, "Empty container has no children, fills, strokes, or effects".into(), None, true);
    }

    if active.non_4px_grid
        && !matches!(n.kind, NodeKind::Text(_) | NodeKind::Vector { .. } | NodeKind::Line)
        && (!is_grid_multiple(n.size.x, config.thresholds.grid_step) || !is_grid_multiple(n.size.y, config.thresholds.grid_step))
    {
        let step = config.thresholds.grid_step.max(1.0);
        let sx = (n.size.x / step).round().max(1.0) * step;
        let sy = (n.size.y / step).round().max(1.0) * step;
        emit(
            "non-4px-grid",
            Category::Layout,
            n,
            ctx.page_bounds,
            format!("Size {}x{} is not aligned to the {}px grid", n.size.x, n.size.y, step),
            Some(FixSuggestion { kind: "set".into(), property: "size".into(), from: json!([n.size.x, n.size.y]), to: json!([sx, sy]) }),
            true,
        );
    }

    let mut mark_hardcoded_color_ignored = false;
    if active.hardcoded_color {
        let mut has_unignored_unbound = false;
        let mut has_any_unbound = false;
        let has_bindings = !n.variables.bindings.is_empty();
        let has_ignores = !doc.check_designs.ignored.is_empty();
        if !n.style_refs.contains_key("fill") && (!has_bindings || !n.variables.bindings.contains_key("fill")) {
            let fill_group_ignored = has_ignores && doc.is_lint_property_ignored("hardcoded-color", n.id, "fill");
            for (i, p) in n.fills.iter().enumerate() {
                if p.visible && matches!(p.kind, PaintKind::Solid(_)) {
                    if !has_bindings && !has_ignores {
                        has_any_unbound = true;
                        has_unignored_unbound = true;
                        break;
                    }
                    let key: std::borrow::Cow<'static, str> = if i == 0 { "fills/0".into() } else { format!("fills/{i}").into() };
                    if !has_bindings || !n.variables.bindings.contains_key(key.as_ref()) {
                        has_any_unbound = true;
                        if !fill_group_ignored && (!has_ignores || !doc.is_lint_property_ignored("hardcoded-color", n.id, key.as_ref())) {
                            has_unignored_unbound = true;
                        }
                    }
                }
            }
        }
        if !n.style_refs.contains_key("stroke") && (!has_bindings || !n.variables.bindings.contains_key("stroke")) {
            let stroke_group_ignored = has_ignores && doc.is_lint_property_ignored("hardcoded-color", n.id, "stroke");
            for (i, p) in n.stroke.paints.iter().enumerate() {
                if p.visible && matches!(p.kind, PaintKind::Solid(_)) {
                    if !has_bindings && !has_ignores {
                        has_any_unbound = true;
                        has_unignored_unbound = true;
                        break;
                    }
                    let key: std::borrow::Cow<'static, str> = if i == 0 { "strokes/0".into() } else { format!("strokes/{i}").into() };
                    if !has_bindings || !n.variables.bindings.contains_key(key.as_ref()) {
                        has_any_unbound = true;
                        if !stroke_group_ignored && (!has_ignores || !doc.is_lint_property_ignored("hardcoded-color", n.id, key.as_ref())) {
                            has_unignored_unbound = true;
                        }
                    }
                }
            }
        }
        if has_any_unbound {
            emit(
                "hardcoded-color",
                Category::Tokens,
                n,
                ctx.page_bounds,
                "Solid fill color is hardcoded without a variable or style token".into(),
                None,
                false,
            );
            if !has_unignored_unbound {
                mark_hardcoded_color_ignored = true;
            }
        }
    }

    if active.small_touch_target {
        let role = infer_role_hint(doc, n);
        let min_t = config.thresholds.min_touch_target;
        if matches!(role, "button" | "input") && (n.size.x < min_t || n.size.y < min_t) {
            emit(
                "small-touch-target",
                Category::Accessibility,
                n,
                ctx.page_bounds,
                format!("Interactive {role} is {}x{} (< {}x{} minimum touch target)", n.size.x, n.size.y, min_t, min_t),
                Some(FixSuggestion {
                    kind: "set".into(),
                    property: "size".into(),
                    from: json!([n.size.x, n.size.y]),
                    to: json!([n.size.x.max(min_t), n.size.y.max(min_t)]),
                }),
                true,
            );
        }
    }

    if mark_hardcoded_color_ignored {
        if let Some(f) = out.iter_mut().rev().find(|f| f.rule == "hardcoded-color" && f.node == n.id) {
            f.ignored = true;
        }
    }
    let suppressed_props = scale::evaluate_hardcoded_value(doc, ctx.token_index, config, n, ctx.page_bounds, &node_xpath, out);
    scale::evaluate_off_scale(doc, ctx.scales, config, n, ctx.page_bounds, &node_xpath, &suppressed_props, out);
}

/// Sweep-and-prune sibling overlap detector (`O(n log n + k)`, `k` capped at 1,000 pairs per container).
fn check_sibling_overlap<F>(children: &[&Node], container: &Node, container_bounds: Rect, config: &LintConfig, emit: &mut F)
where
    F: FnMut(&'static str, Category, &Node, Rect, String, Option<FixSuggestion>, bool),
{
    let tol = config.thresholds.geometry_tolerance;
    let min_area = config.thresholds.min_overlap_area;

    // Exclude masks and invisible / zero-opacity layers (§3.2, §8.3).
    let mut boxes: Vec<(&Node, Rect)> =
        children.iter().copied().filter(|c| c.visible && c.opacity > 0.0 && !c.is_mask).map(|c| (c, aabb_of_transformed_rect(c.transform, c.size))).collect();
    if boxes.len() < 2 {
        return;
    }

    boxes.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));

    const MAX_OVERLAP_PAIRS: usize = 1000;
    let mut pairs = 0usize;
    let mut first_names: Option<(&str, &str)> = None;

    'outer: for i in 0..boxes.len() {
        let (na, ra) = boxes[i];
        let ra_right = ra.x + ra.w;
        let ra_bottom = ra.y + ra.h;
        for j in (i + 1)..boxes.len() {
            let (nb, rb) = boxes[j];
            if rb.x >= ra_right {
                break;
            }
            let rb_right = rb.x + rb.w;
            let rb_bottom = rb.y + rb.h;
            let ix0 = ra.x.max(rb.x);
            let iy0 = ra.y.max(rb.y);
            let ix1 = ra_right.min(rb_right);
            let iy1 = ra_bottom.min(rb_bottom);
            let area = (ix1 - ix0).max(0.0) * (iy1 - iy0).max(0.0);
            if area >= min_area {
                let a_contains_b = rb.x >= ra.x - tol && rb.y >= ra.y - tol && rb_right <= ra_right + tol && rb_bottom <= ra_bottom + tol;
                let b_contains_a = ra.x >= rb.x - tol && ra.y >= rb.y - tol && ra_right <= rb_right + tol && ra_bottom <= rb_bottom + tol;
                if !a_contains_b && !b_contains_a {
                    if first_names.is_none() {
                        first_names = Some((&na.name, &nb.name));
                    }
                    pairs += 1;
                    if pairs >= MAX_OVERLAP_PAIRS {
                        break 'outer;
                    }
                }
            }
        }
    }

    if pairs > 0 {
        let (a_name, b_name) = first_names.unwrap_or(("a", "b"));
        let message = if pairs >= MAX_OVERLAP_PAIRS {
            format!(
                "Sibling layers '{}' and '{}' overlap without auto-layout ({}+ overlapping pairs, truncated at {})",
                a_name, b_name, pairs, MAX_OVERLAP_PAIRS
            )
        } else if pairs == 1 {
            format!("Sibling layers '{}' and '{}' overlap without auto-layout", a_name, b_name)
        } else {
            format!("Sibling layers '{}' and '{}' overlap without auto-layout ({} overlapping pairs)", a_name, b_name, pairs)
        };
        emit("sibling-overlap", Category::Layout, container, container_bounds, message, None, false);
    }
}

/// Checks whether a finding on `(rule, node)` is ignored in `doc`.
pub fn is_finding_ignored(doc: &Document, rule: &str, node: NodeId) -> bool {
    doc.is_lint_ignored(rule, node)
}

/// Severity comparison helper.
pub fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}
