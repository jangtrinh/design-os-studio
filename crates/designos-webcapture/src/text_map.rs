//! CSS text, typography, inline-span flattening, and font substitution mapping
//! into [`designos_doc::TextProps`], [`designos_doc::TextStyle`], and [`designos_doc::TextRun`].

use crate::import::{element_selector_label, FontSubstitution, ImportReport};
use crate::paint_map::{has_own_box_paint, map_effects_and_blend, map_rotation_rad, node_transform_at, parse_css_color, parse_css_px};
use crate::schema::Element;
use designos_doc::{
    Color, LineHeight, Node, NodeId, NodeKind, Paint, ParagraphProps, Rect, TextAlignH, TextAlignV, TextAutoResize, TextCase, TextDecoration, TextRun,
    TextStyle,
};
use std::collections::BTreeMap;

/// Inline tags that can be flattened into styled [`TextRun`]s inside a parent text block
/// when they have no box background, border, or block layout of their own.
const INLINE_TEXT_TAGS: &[&str] = &[
    "span", "strong", "b", "em", "i", "u", "s", "del", "ins", "mark", "small", "sub", "sup", "code", "kbd", "samp", "var", "a", "abbr", "cite", "dfn", "q",
    "time", "label",
];

/// Returns `true` if `el` has any captured SVG or bitmap image payload.
pub fn has_element_image_or_svg(el: &Element) -> bool {
    if el.image.is_some() || !el.images.is_empty() {
        return true;
    }
    false
}

/// Returns `true` if `el` has any rendered `::before` or `::after` pseudo-element.
pub fn has_element_pseudo(el: &Element) -> bool {
    el.pseudo.as_ref().is_some_and(|p| p.before.is_some() || p.after.is_some())
}

/// Returns `true` if `el` is a pure inline text wrapper that should be flattened into
/// its parent text node as a [`TextRun`].
pub fn is_pure_inline_wrapper(el: &Element) -> bool {
    let tag = el.tag.to_ascii_lowercase();
    if tag == "br" {
        return true;
    }
    if !INLINE_TEXT_TAGS.contains(&tag.as_str()) {
        return false;
    }
    let display = el.styles.get("display").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_else(|| "inline".into());
    if !display.starts_with("inline") && display != "contents" {
        return false;
    }
    if has_own_box_paint(&el.styles) {
        return false;
    }
    if has_element_image_or_svg(el) || has_element_pseudo(el) {
        return false;
    }
    el.children.iter().all(is_pure_inline_wrapper)
}

/// Returns `true` if `el` should be represented directly as a single [`NodeKind::Text`]
/// (or a padded/painted Frame wrapping a single [`NodeKind::Text`]): it has non-empty text
/// (either in `el.text_runs` or via pure inline children) and all of its children are pure inline wrappers.
pub fn is_pure_text_element(el: &Element, flatten_inline_wrappers: bool) -> bool {
    if has_element_image_or_svg(el) {
        return false;
    }
    let tag = el.tag.to_ascii_lowercase();
    if matches!(tag.as_str(), "img" | "svg" | "canvas" | "video" | "iframe" | "hr" | "input" | "select" | "textarea") {
        return false;
    }
    if el.children.is_empty() {
        return el.text_runs.iter().any(|r| !r.text.trim().is_empty());
    }
    if !flatten_inline_wrappers {
        return false;
    }
    if !el.children.iter().all(is_pure_inline_wrapper) {
        return false;
    }
    let mut probe = String::new();
    collect_probe_text(el, &mut probe);
    !probe.trim().is_empty()
}

fn collect_probe_text(el: &Element, out: &mut String) {
    if el.tag.eq_ignore_ascii_case("br") {
        out.push('\n');
        return;
    }
    for run in &el.text_runs {
        if !run.text.is_empty() {
            out.push_str(&run.text);
        }
    }
    for child in &el.children {
        collect_probe_text(child, out);
    }
}

/// Resolves a CSS `font-family` stack against bundled fonts (`Inter`, `Roboto Mono`)
/// and [`designos_fonts::substitute`], recording any substitution in `report`.
pub fn resolve_font_family(raw_stack: &str, report: &mut ImportReport) -> String {
    let candidates: Vec<String> =
        raw_stack.split(',').map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string()).filter(|s| !s.is_empty()).collect();

    let first_requested = candidates.first().cloned().unwrap_or_else(|| "Inter".into());

    for cand in &candidates {
        let lower = cand.to_ascii_lowercase();
        if lower == "inter" {
            return "Inter".into();
        }
        if lower == "roboto mono" || lower == "ui-monospace" || lower == "monospace" {
            if !first_requested.eq_ignore_ascii_case("Roboto Mono") {
                record_font_sub(report, &first_requested, "Roboto Mono");
            }
            return "Roboto Mono".into();
        }
        if matches!(
            lower.as_str(),
            "system-ui"
                | "-apple-system"
                | "blinkmacsystemfont"
                | "sans-serif"
                | "ui-sans-serif"
                | "arial"
                | "helvetica"
                | "helvetica neue"
                | "segoe ui"
                | "roboto"
        ) {
            if !first_requested.eq_ignore_ascii_case("Inter") {
                record_font_sub(report, &first_requested, "Inter");
            }
            return "Inter".into();
        }
        if let Some(sub) = designos_fonts::substitute(cand) {
            let resolved = if sub.eq_ignore_ascii_case("Roboto Mono") || sub.to_ascii_lowercase().contains("mono") { "Roboto Mono" } else { "Inter" };
            record_font_sub(report, &first_requested, resolved);
            return resolved.into();
        }
    }

    if !first_requested.eq_ignore_ascii_case("Inter") {
        record_font_sub(report, &first_requested, "Inter");
    }
    "Inter".into()
}

fn record_font_sub(report: &mut ImportReport, requested: &str, substituted: &str) {
    if requested.is_empty() || requested.eq_ignore_ascii_case(substituted) {
        return;
    }
    if !report.font_substitutions.iter().any(|s| s.requested.eq_ignore_ascii_case(requested) && s.substituted.eq_ignore_ascii_case(substituted)) {
        report.font_substitutions.push(FontSubstitution { requested: requested.to_string(), substituted: substituted.to_string() });
    }
}

/// Maps CSS typography properties (`font-family`, `font-size`, `font-weight`, `font-style`,
/// `line-height`, `letter-spacing`, `text-align`, `text-transform`, `text-decoration-line`, `color`)
/// into a [`TextStyle`].
pub fn map_text_style(styles: &BTreeMap<String, String>, report: &mut ImportReport) -> TextStyle {
    let font_family = styles.get("font-family").map(|s| resolve_font_family(s, report)).unwrap_or_else(|| "Inter".into());

    let font_size = styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(16.0).max(1.0);

    let font_weight = styles
        .get("font-weight")
        .map(|s| match s.trim().to_ascii_lowercase().as_str() {
            "normal" => 400,
            "bold" => 700,
            "lighter" => 300,
            "bolder" => 700,
            other => other.parse::<u16>().unwrap_or(400).clamp(100, 900),
        })
        .unwrap_or(400);

    let italic = styles.get("font-style").is_some_and(|s| matches!(s.trim().to_ascii_lowercase().as_str(), "italic" | "oblique"));

    let line_height = match styles.get("line-height").map(|s| s.trim()) {
        None | Some("") | Some("normal") => LineHeight::Auto,
        Some(raw) => {
            if let Some(pct) = raw.strip_suffix('%') {
                pct.trim().parse::<f32>().ok().filter(|v| *v > 0.0).map(LineHeight::Percent).unwrap_or(LineHeight::Auto)
            } else if let Some(px) = parse_css_px(raw, font_size) {
                if raw.ends_with("px") || raw.ends_with("rem") || raw.ends_with("em") || raw.ends_with("pt") {
                    LineHeight::Pixels(px.max(1.0))
                } else {
                    LineHeight::Percent((px * 100.0).max(10.0))
                }
            } else {
                LineHeight::Auto
            }
        }
    };

    let letter_spacing = match styles.get("letter-spacing").map(|s| s.trim()) {
        None | Some("") | Some("normal") => 0.0,
        Some(raw) => {
            if let Some(pct) = raw.strip_suffix('%') {
                pct.trim().parse::<f32>().ok().map(|p| font_size * (p / 100.0)).unwrap_or(0.0)
            } else {
                parse_css_px(raw, font_size).unwrap_or(0.0)
            }
        }
    };

    let align_h = match styles.get("text-align").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("center") => TextAlignH::Center,
        Some("right") | Some("end") => TextAlignH::Right,
        Some("justify") => TextAlignH::Justified,
        _ => TextAlignH::Left,
    };

    let case = match styles.get("text-transform").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("uppercase") => TextCase::Upper,
        Some("lowercase") => TextCase::Lower,
        Some("capitalize") => TextCase::Title,
        _ => TextCase::Original,
    };

    let deco_raw = styles.get("text-decoration-line").or_else(|| styles.get("text-decoration")).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let decoration = if deco_raw.contains("underline") {
        TextDecoration::Underline
    } else if deco_raw.contains("line-through") {
        TextDecoration::Strikethrough
    } else {
        TextDecoration::None
    };

    let color = styles.get("color").and_then(|s| parse_css_color(s)).unwrap_or(Color::BLACK);

    TextStyle {
        font_family,
        font_weight,
        italic,
        font_size,
        line_height,
        letter_spacing,
        paragraph_spacing: 0.0,
        align_h,
        align_v: TextAlignV::Top,
        case,
        decoration,
        fills: vec![Paint::solid(color)],
        features: Vec::new(),
        variation_axes: Vec::new(),
    }
}

/// Collects text content and per-span [`TextRun`] style overrides from `el` and its inline children.
pub fn collect_flattened_text_runs(
    el: &Element,
    base_style: &TextStyle,
    flatten_inline_wrappers: bool,
    report: &mut ImportReport,
) -> (String, Vec<TextRun>, usize) {
    let mut content = String::new();
    let mut runs = Vec::new();
    let mut total_lines = 0usize;

    append_element_own_runs(el, base_style, &mut content, &mut runs, &mut total_lines, report);

    if flatten_inline_wrappers {
        for child in &el.children {
            append_inline_child(child, base_style, &mut content, &mut runs, &mut total_lines, report);
        }
    }

    (content, runs, total_lines)
}

fn append_element_own_runs(
    el: &Element,
    base_style: &TextStyle,
    content: &mut String,
    runs: &mut Vec<TextRun>,
    total_lines: &mut usize,
    report: &mut ImportReport,
) {
    for tr in &el.text_runs {
        *total_lines += tr.lines.len();
        let style_map = if tr.styles.is_empty() { &el.styles } else { &tr.styles };
        let t = normalize_html_whitespace(&tr.text, style_map);
        if t.is_empty() {
            continue;
        }
        if !content.is_empty() && !content.ends_with(char::is_whitespace) && !t.starts_with(char::is_whitespace) {
            content.push(' ');
        }
        let start = content.len();
        content.push_str(&t);
        let end = content.len();

        let mut run_style = map_text_style(style_map, report);
        apply_semantic_tag_style(&el.tag, &mut run_style);
        if &run_style != base_style {
            runs.push(TextRun { start, end, style: run_style });
        }
    }
}

fn append_inline_child(
    el: &Element,
    base_style: &TextStyle,
    content: &mut String,
    runs: &mut Vec<TextRun>,
    total_lines: &mut usize,
    report: &mut ImportReport,
) {
    report.flattened_wrappers += 1;
    if el.tag.eq_ignore_ascii_case("br") {
        content.push('\n');
        return;
    }
    if el.tag.eq_ignore_ascii_case("a") {
        let label = element_selector_label(el);
        report.notes.push(format!("inline link flattened: {label}"));
    }

    append_element_own_runs(el, base_style, content, runs, total_lines, report);
    for grand in &el.children {
        append_inline_child(grand, base_style, content, runs, total_lines, report);
    }
}

fn apply_semantic_tag_style(tag: &str, style: &mut TextStyle) {
    let lower = tag.to_ascii_lowercase();
    if matches!(lower.as_str(), "strong" | "b") && style.font_weight < 600 {
        style.font_weight = 700;
    }
    if matches!(lower.as_str(), "em" | "i") {
        style.italic = true;
    }
    if lower == "u" {
        style.decoration = TextDecoration::Underline;
    }
    if matches!(lower.as_str(), "s" | "del") {
        style.decoration = TextDecoration::Strikethrough;
    }
    if matches!(lower.as_str(), "code" | "kbd" | "samp") {
        style.font_family = "Roboto Mono".into();
    }
}

fn normalize_html_whitespace(text: &str, styles: &BTreeMap<String, String>) -> String {
    let ws = styles.get("white-space").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_else(|| "normal".into());
    if ws.starts_with("pre") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Builds a [`NodeKind::Text`] node for `el` inside `target_rect` (in parent coordinates).
pub fn build_text_node_in_rect(
    id: NodeId,
    el: &Element,
    target_rect: Rect,
    flatten_inline_wrappers: bool,
    apply_element_effects: bool,
    report: &mut ImportReport,
) -> Option<Node> {
    let base_styles = el.text_runs.first().filter(|r| !r.styles.is_empty()).map(|r| &r.styles).unwrap_or(&el.styles);
    let mut base_style = map_text_style(base_styles, report);
    apply_semantic_tag_style(&el.tag, &mut base_style);

    let (content, runs, total_lines) = collect_flattened_text_runs(el, &base_style, flatten_inline_wrappers, report);
    if content.is_empty() {
        return None;
    }

    let line_h = match base_style.line_height {
        LineHeight::Pixels(px) => px,
        LineHeight::Percent(pct) => base_style.font_size * (pct / 100.0),
        LineHeight::Auto => base_style.font_size * 1.2,
    };

    let white_space = el.styles.get("white-space").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let text_overflow = el.styles.get("text-overflow").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let line_clamp = el.styles.get("-webkit-line-clamp").and_then(|s| s.trim().parse::<u32>().ok()).filter(|&n| n > 0);

    let is_single_line = if total_lines > 0 { total_lines == 1 && !content.contains('\n') } else { target_rect.h <= line_h * 1.65 && !content.contains('\n') };

    let (auto_resize, max_lines) = if text_overflow == "ellipsis" || line_clamp.is_some() {
        (TextAutoResize::Truncate, line_clamp.or_else(|| (total_lines > 0).then_some(total_lines as u32)).or(Some(1)))
    } else if white_space == "nowrap" || is_single_line {
        (TextAutoResize::WidthAndHeight, None)
    } else {
        (TextAutoResize::Height, None)
    };

    let w =
        if auto_resize == TextAutoResize::Height && base_style.align_h == TextAlignH::Left { (target_rect.w + 2.0).max(1.0) } else { target_rect.w.max(1.0) };
    let h = target_rect.h.max(1.0);

    let indent = el.styles.get("text-indent").and_then(|s| parse_css_px(s, base_style.font_size)).unwrap_or(0.0);

    let mut node = Node::text(id, Rect::new(target_rect.x, target_rect.y, w, h), content.clone(), base_style);
    node.name = make_text_layer_name(el, &content);
    if let NodeKind::Text(ref mut tp) = node.kind {
        tp.runs = runs;
        tp.auto_resize = auto_resize;
        tp.max_lines = max_lines;
        tp.paragraph = ParagraphProps { indent, ..Default::default() };
    }

    if apply_element_effects {
        let sel = element_selector_label(el);
        let (effects, blend, opacity) = map_effects_and_blend(&sel, &el.styles, 16.0, report);
        let rad = map_rotation_rad(&sel, &el.styles, report);
        node.effects = effects;
        node.blend = blend;
        node.opacity = opacity;
        node.transform = node_transform_at(target_rect.x, target_rect.y, w, h, rad);
    }

    Some(node)
}

fn make_text_layer_name(el: &Element, content: &str) -> String {
    let snippet: String = content.chars().take(28).collect();
    if snippet.is_empty() {
        element_selector_label(el)
    } else {
        snippet
    }
}
