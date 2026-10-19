//! Self-contained HTML5 + CSS3 code generation (`T7.29`).
//!
//! Emits a standards-compliant HTML5 document (or a `<style>` + root-element fragment when
//! [`HtmlOptions::fragment`] is `true`) for a selected node and its visible subtree. Every class
//! rule in `<style>` builds on [`crate::css_for`] so CSS declarations stay aligned with the CSS
//! codegen target.

use crate::codegen::{first_paint, is_vector_kind, visible_children, web_sizes, Dim, Parent};
use crate::css::{css_for, text_style_css};
use crate::svg::{export_svg, SvgOptions};
use crate::util::{css_color, num};
use base64::Engine;
use designos_doc::*;
use std::collections::{HashMap, HashSet};

/// Options for [`html_with`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlOptions {
    /// Emit only `<style>...</style>` and the root element without `<!doctype html>`, `<html>`,
    /// `<head>`, or `<body>` wrappers.
    pub fragment: bool,
}

/// Complete HTML5 document with an embedded `<style>` block for `node` and its descendants.
pub fn html_for(doc: &Document, node: NodeId) -> String {
    html_with(doc, node, &HtmlOptions::default())
}

/// HTML for `node` configured with [`HtmlOptions`].
pub fn html_with(doc: &Document, node: NodeId, opts: &HtmlOptions) -> String {
    let Some(root) = doc.get(node) else {
        return String::new();
    };
    let mut builder = HtmlBuilder::new(doc, node);
    let body_indent = if opts.fragment { 0 } else { 1 };
    let mut markup_lines = Vec::new();
    builder.emit_node(root, Parent::root(), true, body_indent, &mut markup_lines);

    let style_indent = if opts.fragment { "  " } else { "    " };
    let style_close_indent = if opts.fragment { "" } else { "  " };
    let mut style_block = String::new();
    if opts.fragment {
        style_block.push_str("<style>\n");
    } else {
        style_block.push_str("  <style>\n");
    }
    if !builder.root_vars.is_empty() {
        style_block.push_str(style_indent);
        style_block.push_str(":root {\n");
        for (name, val) in &builder.root_vars {
            style_block.push_str(style_indent);
            style_block.push_str(&format!("  --{name}: {val};\n"));
        }
        style_block.push_str(style_indent);
        style_block.push_str("}\n");
    }
    for (cls, decls) in &builder.rules {
        if decls.is_empty() {
            continue;
        }
        style_block.push_str(style_indent);
        style_block.push_str(&format!(".{cls} {{\n"));
        for d in decls {
            style_block.push_str(style_indent);
            style_block.push_str(&format!("  {d}\n"));
        }
        style_block.push_str(style_indent);
        style_block.push_str("}\n");
    }
    style_block.push_str(style_close_indent);
    style_block.push_str("</style>");

    let markup = markup_lines.join("\n");
    if opts.fragment {
        if markup.is_empty() {
            style_block
        } else {
            format!("{style_block}\n{markup}")
        }
    } else {
        let title = escape_html(&root.name);
        let mut out = String::new();
        out.push_str("<!doctype html>\n");
        out.push_str("<html lang=\"en\">\n");
        out.push_str("<head>\n");
        out.push_str("  <meta charset=\"utf-8\">\n");
        out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        out.push_str(&format!("  <title>{title}</title>\n"));
        out.push_str(&style_block);
        out.push_str("\n</head>\n");
        out.push_str("<body>\n");
        if !markup.is_empty() {
            out.push_str(&markup);
            out.push('\n');
        }
        out.push_str("</body>\n");
        out.push_str("</html>");
        out
    }
}

struct HtmlBuilder<'a> {
    doc: &'a Document,
    root_id: NodeId,
    used_classes: HashSet<String>,
    class_counts: HashMap<String, usize>,
    rules: Vec<(String, Vec<String>)>,
    root_vars: Vec<(String, String)>,
    seen_vars: HashSet<String>,
    run_counter: usize,
}

impl<'a> HtmlBuilder<'a> {
    fn new(doc: &'a Document, root_id: NodeId) -> Self {
        Self {
            doc,
            root_id,
            used_classes: HashSet::new(),
            class_counts: HashMap::new(),
            rules: Vec::new(),
            root_vars: Vec::new(),
            seen_vars: HashSet::new(),
            run_counter: 0,
        }
    }

    fn alloc_class(&mut self, name: &str, id: NodeId) -> String {
        let base = sanitize_class_name(name, id);
        let count = self.class_counts.entry(base.clone()).or_insert(0);
        *count += 1;
        let mut candidate = if *count == 1 { base.clone() } else { format!("{base}-{}", *count) };
        while self.used_classes.contains(&candidate) {
            *count += 1;
            candidate = format!("{base}-{}", *count);
        }
        self.used_classes.insert(candidate.clone());
        candidate
    }

    fn record_node_vars(&mut self, n: &Node) {
        if n.variables.bindings.is_empty() {
            return;
        }
        let mut keys: Vec<&String> = n.variables.bindings.keys().collect();
        keys.sort();
        for key in keys {
            let Some(var_id) = n.variables.bindings.get(key) else { continue };
            let Some(v) = self.doc.variables.get(var_id) else { continue };
            let prop_name = custom_prop_name(v);
            if self.seen_vars.insert(prop_name.clone()) {
                if let Some(val) = default_mode_css_value(self.doc, v, key) {
                    self.root_vars.push((prop_name, val));
                }
            }
        }
    }

    fn mode_override_style(&self, n: &Node) -> Option<String> {
        if n.variables.modes.is_empty() {
            return None;
        }
        let mut vars_in_subtree = Vec::new();
        let mut seen = HashSet::new();
        collect_subtree_vars(self.doc, n, &mut vars_in_subtree, &mut seen);
        let mut decls = Vec::new();
        for (var_id, key) in vars_in_subtree {
            let Some(v) = self.doc.variables.get(&var_id) else { continue };
            if !n.variables.modes.contains_key(&v.collection) {
                continue;
            }
            if let Some(resolved) = self.doc.resolve_variable(n.id, &v.id) {
                if let Some(css_val) = format_var_value(&resolved, &key) {
                    let prop = custom_prop_name(v);
                    decls.push(format!("--{prop}: {css_val};"));
                }
            }
        }
        if decls.is_empty() {
            None
        } else {
            Some(decls.join(" "))
        }
    }

    fn emit_node(&mut self, n: &Node, parent: Parent, is_root: bool, indent: usize, out: &mut Vec<String>) {
        if !n.visible && !is_root {
            return;
        }
        self.record_node_vars(n);
        let cls = self.alloc_class(&n.name, n.id);
        let pad = "  ".repeat(indent);

        let is_vec = is_vector_kind(&n.kind) || matches!(n.kind, NodeKind::Line);
        let img_fill = image_element_paint(n);

        let decls = self.node_css_decls(n, parent, is_root, is_vec, img_fill);
        let rule_idx = self.rules.len();
        self.rules.push((cls.clone(), decls));

        if let NodeKind::Instance { component, .. } = &n.kind {
            let comp_name = component.and_then(|cid| self.doc.get(cid)).map(|c| c.name.as_str()).unwrap_or(n.name.as_str());
            out.push(format!("{pad}<!-- Component: {} -->", sanitize_comment(comp_name)));
        }

        let style_attr = self.mode_override_style(n).map(|s| format!(" style=\"{}\"", escape_html(&s))).unwrap_or_default();

        if is_vec {
            let svg_raw = export_svg(self.doc, n.id, &SvgOptions::default());
            let svg_trimmed = svg_raw.trim();
            let with_class =
                if let Some(rest) = svg_trimmed.strip_prefix("<svg ") { format!("<svg class=\"{cls}\"{style_attr} {rest}") } else { svg_trimmed.to_string() };
            for line in with_class.lines() {
                out.push(format!("{pad}{line}"));
            }
            return;
        }

        if let Some((image_ref, _scale_mode)) = img_fill {
            let src = data_uri_for_image(self.doc, image_ref);
            let alt = escape_html(&n.name);
            out.push(format!("{pad}<img class=\"{cls}\"{style_attr} src=\"{src}\" alt=\"{alt}\">"));
            return;
        }

        if let NodeKind::Text(t) = &n.kind {
            self.emit_text(t, &cls, &style_attr, indent, out);
            return;
        }

        let kids = visible_children(self.doc, n);
        if kids.is_empty() {
            out.push(format!("{pad}<div class=\"{cls}\"{style_attr}></div>"));
        } else {
            out.push(format!("{pad}<div class=\"{cls}\"{style_attr}>"));
            let me = Parent::of(n);
            for k in kids {
                self.emit_node(k, me, false, indent + 1, out);
            }
            out.push(format!("{pad}</div>"));
        }
        let _ = rule_idx;
    }

    fn emit_text(&mut self, t: &TextProps, cls: &str, style_attr: &str, indent: usize, out: &mut Vec<String>) {
        let pad = "  ".repeat(indent);
        let parent_style_decls = text_style_css(&t.style);

        let paragraphs = split_paragraphs_with_ranges(&t.content);
        if paragraphs.len() <= 1 {
            let (p_start, p_end, p_text) = paragraphs.first().cloned().unwrap_or((0, 0, ""));
            let dir_attr = paragraph_dir_attr(t.direction, p_text);
            let inner = self.render_runs(&t.content, p_start, p_end, &t.runs, &parent_style_decls);
            out.push(format!("{pad}<p class=\"{cls}\"{dir_attr}{style_attr}>{inner}</p>"));
        } else {
            out.push(format!("{pad}<div class=\"{cls}\"{style_attr}>"));
            let child_pad = "  ".repeat(indent + 1);
            for (p_start, p_end, p_text) in paragraphs {
                let dir_attr = paragraph_dir_attr(t.direction, p_text);
                let inner = self.render_runs(&t.content, p_start, p_end, &t.runs, &parent_style_decls);
                out.push(format!("{child_pad}<p{dir_attr}>{inner}</p>"));
            }
            out.push(format!("{pad}</div>"));
        }
    }

    fn render_runs(&mut self, content: &str, p_start: usize, p_end: usize, runs: &[TextRun], parent_style_decls: &[String]) -> String {
        if runs.is_empty() || p_start >= p_end {
            return escape_html(&content[p_start..p_end]);
        }
        let mut points = vec![p_start, p_end];
        for r in runs {
            let s = r.start.clamp(p_start, p_end);
            let e = r.end.clamp(p_start, p_end);
            if content.is_char_boundary(s) {
                points.push(s);
            }
            if content.is_char_boundary(e) {
                points.push(e);
            }
        }
        points.sort_unstable();
        points.dedup();

        let mut buf = String::new();
        for w in points.windows(2) {
            let (seg_s, seg_e) = (w[0], w[1]);
            if seg_s >= seg_e {
                continue;
            }
            let slice = &content[seg_s..seg_e];
            let active_run = runs.iter().rfind(|r| r.start <= seg_s && r.end >= seg_e);
            if let Some(run) = active_run {
                let diff = diff_run_style(parent_style_decls, &run.style);
                if !diff.is_empty() {
                    self.run_counter += 1;
                    let mut run_cls = format!("run-{}", self.run_counter);
                    while self.used_classes.contains(&run_cls) {
                        self.run_counter += 1;
                        run_cls = format!("run-{}", self.run_counter);
                    }
                    self.used_classes.insert(run_cls.clone());
                    self.rules.push((run_cls.clone(), diff));
                    buf.push_str(&format!("<span class=\"{run_cls}\">{}</span>", escape_html(slice)));
                    continue;
                }
            }
            buf.push_str(&escape_html(slice));
        }
        buf
    }

    fn node_css_decls(&self, n: &Node, parent: Parent, is_root: bool, is_vec: bool, img_fill: Option<(&str, ImageScaleMode)>) -> Vec<String> {
        let raw = css_for(self.doc, n.id);
        let mut lines: Vec<String> = raw.lines().map(String::from).collect();

        if is_root && self.root_id == n.id {
            lines.retain(|l| !l.starts_with("position: absolute;") && !l.starts_with("left: ") && !l.starts_with("top: "));
        } else if parent.al.is_none() && !parent.root && !lines.iter().any(|l| l.starts_with("position: ")) {
            let top_decl = format!("top: {}px;", num(n.transform.ty));
            let left_decl = format!("left: {}px;", num(n.transform.tx));
            lines.insert(0, top_decl);
            lines.insert(0, left_decl);
            lines.insert(0, "position: absolute;".into());
        }

        let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
        let kids = visible_children(self.doc, n);
        let has_abs_children = !kids.is_empty() && (own_al.is_none() || kids.iter().any(|k| k.layout_child.absolute));
        if has_abs_children && !lines.iter().any(|l| l.starts_with("position: ")) {
            lines.insert(0, "position: relative;".into());
        }

        if let Some(al) = own_al {
            if al.direction == LayoutDirection::Horizontal && !lines.iter().any(|l| l.starts_with("flex-direction: ")) {
                let pos = lines.iter().position(|l| l == "display: flex;").map(|i| i + 1).unwrap_or(0);
                lines.insert(pos, "flex-direction: row;".into());
            }
        }

        if !matches!(n.kind, NodeKind::Text(_)) {
            let sz = web_sizes(n, parent);
            if sz.w == Dim::Hug && !lines.iter().any(|l| l.starts_with("width: ") || l.starts_with("flex: ")) {
                lines.push("width: fit-content;".into());
            }
            if sz.h == Dim::Hug && !lines.iter().any(|l| l.starts_with("height: ") || l.starts_with("flex: ")) {
                lines.push("height: fit-content;".into());
            }
        }

        if let Some(pos) = lines.iter().position(|l| l.starts_with("transform: rotate(")) {
            if !lines.iter().any(|l| l.starts_with("transform-origin: ")) {
                lines.insert(pos + 1, "transform-origin: top left;".into());
            }
        }

        if let NodeKind::Text(t) = &n.kind {
            if !lines.iter().any(|l| l.starts_with("margin: ")) {
                lines.insert(0, "margin: 0;".into());
            }
            match t.auto_resize {
                TextAutoResize::None => {
                    if !lines.iter().any(|l| l.starts_with("overflow: ")) {
                        lines.push("overflow: hidden;".into());
                    }
                }
                TextAutoResize::Truncate => {
                    let max_l = t.max_lines.unwrap_or(1).max(1);
                    lines.retain(|l| {
                        !l.starts_with("overflow: ")
                            && !l.starts_with("text-overflow: ")
                            && !l.starts_with("display: -webkit-box")
                            && !l.starts_with("-webkit-box-orient: ")
                            && !l.starts_with("-webkit-line-clamp: ")
                            && !l.starts_with("white-space: nowrap")
                    });
                    lines.push("display: -webkit-box;".into());
                    lines.push("-webkit-box-orient: vertical;".into());
                    lines.push(format!("-webkit-line-clamp: {max_l};"));
                    lines.push("overflow: hidden;".into());
                    lines.push("text-overflow: ellipsis;".into());
                }
                TextAutoResize::WidthAndHeight | TextAutoResize::Height => {}
            }
        }

        // Upgrade bare `codeSyntax.WEB = var(--name)` references to include the resolved fallback
        // `var(--name, <fallback>)` per T7.29 §3.6 / §10.
        if !n.variables.bindings.is_empty() {
            for (key, var_id) in &n.variables.bindings {
                let Some(v) = self.doc.variables.get(var_id) else { continue };
                let Some(code) = v.code_syntax_for(CodeSyntaxPlatform::Web) else { continue };
                if let Some(ident) = bare_css_var_ident(code) {
                    if let Some(resolved) = self.doc.resolve_variable(n.id, &v.id) {
                        if let Some(fb) = format_var_value(&resolved, key) {
                            let with_fb = format!("var(--{ident}, {fb})");
                            for line in &mut lines {
                                if line.contains(code) {
                                    *line = line.replace(code, &with_fb);
                                }
                            }
                        }
                    }
                }
            }
        }

        if is_vec {
            lines.retain(|l| {
                l.starts_with("position: ")
                    || l.starts_with("left: ")
                    || l.starts_with("top: ")
                    || l.starts_with("width: ")
                    || l.starts_with("height: ")
                    || l.starts_with("flex: ")
                    || l.starts_with("flex-shrink: ")
                    || l.starts_with("align-self: ")
                    || l.starts_with("justify-self: ")
                    || l.starts_with("grid-column: ")
                    || l.starts_with("grid-row: ")
                    || l.starts_with("transform: ")
                    || l.starts_with("transform-origin: ")
                    || l.starts_with("opacity: ")
                    || l.starts_with("mix-blend-mode: ")
            });
            lines.push("display: block;".into());
            return lines;
        }

        if let Some((_, scale_mode)) = img_fill {
            lines.retain(|l| !l.starts_with("background: "));
            lines.push("display: block;".into());
            let fit = match scale_mode {
                ImageScaleMode::Fit => "contain",
                ImageScaleMode::Fill | ImageScaleMode::Crop | ImageScaleMode::Tile => "cover",
            };
            lines.push(format!("object-fit: {fit};"));
            return lines;
        }

        // If a tiled image fill or background image is present on a non-<img> node, expand
        // `url(<image_ref>)` into its embedded `data:` URI.
        if let Some(Paint { kind: PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Tile }, .. }) = first_paint(&n.fills) {
            if n.fills.iter().filter(|p| p.visible && p.opacity > 0.0).count() == 1 {
                let uri = data_uri_for_image(self.doc, image_ref);
                lines.retain(|l| !l.starts_with("background: "));
                lines.push(format!("background-image: url(\"{uri}\");"));
                lines.push("background-repeat: repeat;".into());
            }
        }
        for line in &mut lines {
            while let Some(start) = line.find("url(<") {
                if let Some(end_rel) = line[start + 5..].find(">)") {
                    let img_ref = &line[start + 5..start + 5 + end_rel];
                    let uri = data_uri_for_image(self.doc, img_ref);
                    let replacement = format!("url(\"{uri}\")");
                    line.replace_range(start..start + 5 + end_rel + 2, &replacement);
                } else {
                    break;
                }
            }
        }

        lines
    }
}

/// Returns `Some((image_ref, scale_mode))` when `n` should emit as an `<img>` element:
/// a childless rectangle whose topmost visible fill is a non-tiled image fill.
fn image_element_paint(n: &Node) -> Option<(&str, ImageScaleMode)> {
    if !matches!(n.kind, NodeKind::Rectangle) || !n.children.is_empty() {
        return None;
    }
    let p = first_paint(&n.fills)?;
    match &p.kind {
        PaintKind::Image { image_ref, scale_mode } if *scale_mode != ImageScaleMode::Tile => Some((image_ref.as_str(), *scale_mode)),
        _ => None,
    }
}

fn data_uri_for_image(doc: &Document, image_ref: &str) -> String {
    if let Some(data) = doc.images.get(image_ref) {
        let mime = match image::guess_format(data).ok() {
            Some(image::ImageFormat::Jpeg) => "image/jpeg",
            Some(image::ImageFormat::Gif) => "image/gif",
            Some(image::ImageFormat::WebP) => "image/webp",
            _ => "image/png",
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
        return format!("data:{mime};base64,{b64}");
    }
    // 1x1 transparent GIF fallback when image bytes are not present in `doc.images`.
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".to_string()
}

/// Sanitizes a layer name into a lowercase CSS class identifier (`[a-z0-9-]`), falling back
/// to `n-<id>` when the name contains only emoji/punctuation or prefixing `n-` when starting
/// with a digit.
pub fn sanitize_class_name(name: &str, id: NodeId) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        format!("n-{}", id.0)
    } else if out.as_bytes()[0].is_ascii_digit() {
        format!("n-{out}")
    } else {
        out
    }
}

/// Escapes `<`, `>`, `&`, `"`, and `'` for safe inclusion in HTML text content, attributes,
/// and `<title>`.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn sanitize_comment(s: &str) -> String {
    s.replace("--", "- -").replace('>', "&gt;")
}

fn split_paragraphs_with_ranges(content: &str) -> Vec<(usize, usize, &str)> {
    let mut res = Vec::new();
    let mut start = 0;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            res.push((start, idx, &content[start..idx]));
            start = idx + 1;
        }
    }
    res.push((start, content.len(), &content[start..]));
    res
}

fn paragraph_dir_attr(dir: TextDirection, text: &str) -> &'static str {
    match dir {
        TextDirection::Rtl => " dir=\"rtl\"",
        TextDirection::Ltr => "",
        TextDirection::Auto => {
            if is_rtl_paragraph(text) {
                " dir=\"rtl\""
            } else {
                ""
            }
        }
    }
}

fn is_rtl_paragraph(text: &str) -> bool {
    for ch in text.chars() {
        let u = ch as u32;
        if (0x0590..=0x08FF).contains(&u) || (0xFB1D..=0xFDFF).contains(&u) || (0xFE70..=0xFEFF).contains(&u) {
            return true;
        }
        if ch.is_alphabetic() {
            return false;
        }
    }
    false
}

fn diff_run_style(parent_decls: &[String], run_style: &TextStyle) -> Vec<String> {
    let mut run_decls = Vec::new();
    if let Some(p) = run_style.fills.iter().find(|p| p.visible && p.opacity > 0.0) {
        if let PaintKind::Solid(c) = &p.kind {
            run_decls.push(format!("color: {};", css_color(c, p.opacity)));
        }
    }
    run_decls.extend(text_style_css(run_style));
    run_decls.into_iter().filter(|d| !parent_decls.contains(d)).collect()
}

fn bare_css_var_ident(code: &str) -> Option<&str> {
    let trimmed = code.trim();
    if let Some(inner) = trimmed.strip_prefix("var(--").and_then(|s| s.strip_suffix(')')) {
        let ident = inner.trim();
        if !ident.is_empty() && !ident.contains(',') && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Some(ident);
        }
    }
    None
}

fn custom_prop_name(v: &Variable) -> String {
    if let Some(code) = v.code_syntax_for(CodeSyntaxPlatform::Web) {
        let trimmed = code.trim();
        if let Some(rest) = trimmed.strip_prefix("var(--") {
            let end = rest.find([',', ')']).unwrap_or(rest.len());
            let ident = rest[..end].trim();
            if !ident.is_empty() && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return ident.to_string();
            }
        } else if let Some(ident) = trimmed.strip_prefix("--") {
            let ident = ident.trim();
            if !ident.is_empty() && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return ident.to_string();
            }
        }
    }
    let var_ref = crate::css_variable_reference(v);
    var_ref.strip_prefix("var(--").and_then(|s| s.strip_suffix(')')).unwrap_or("unnamed").to_string()
}

fn format_var_value(value: &VariableValue, prop_key: &str) -> Option<String> {
    match value {
        VariableValue::Color(c) => Some(css_color(c, 1.0)),
        VariableValue::Number(x) => {
            if prop_key == "fontWeight" || prop_key == "opacity" {
                Some(num(*x))
            } else {
                Some(format!("{}px", num(*x)))
            }
        }
        VariableValue::String(_) | VariableValue::Boolean(_) | VariableValue::Alias(_) => None,
    }
}

fn default_mode_css_value(doc: &Document, v: &Variable, prop_key: &str) -> Option<String> {
    let resolved = resolve_in_default_mode(doc, v, 0)?;
    format_var_value(&resolved, prop_key)
}

fn resolve_in_default_mode(doc: &Document, v: &Variable, depth: usize) -> Option<VariableValue> {
    if depth > 16 {
        return None;
    }
    let col = doc.variable_collections.get(&v.collection)?;
    let val = v.values.get(&col.default_mode).or_else(|| v.values.values().next())?;
    match val {
        VariableValue::Alias(target_id) => {
            let target = doc.variables.get(target_id)?;
            resolve_in_default_mode(doc, target, depth + 1)
        }
        other => Some(other.clone()),
    }
}

fn collect_subtree_vars(doc: &Document, n: &Node, out: &mut Vec<(String, String)>, seen: &mut HashSet<String>) {
    let mut keys: Vec<&String> = n.variables.bindings.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(var_id) = n.variables.bindings.get(k) {
            if seen.insert(var_id.clone()) {
                out.push((var_id.clone(), k.clone()));
            }
        }
    }
    for c in visible_children(doc, n) {
        collect_subtree_vars(doc, c, out, seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fixture;
    use crate::CodeLanguage;

    #[test]
    fn card_snapshot() {
        let (d, id) = fixture::card();
        let expected = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Card</title>
  <style>
    :root {
      --Color-Text-Muted: #666;
      --Color-Primary: #0D99FF;
      --Spacing-Md: 12px;
    }
    .card {
      display: flex;
      width: 312px;
      height: 200px;
      padding: 16px;
      flex-direction: column;
      align-items: flex-start;
      gap: 8px;
      background: #FFF;
      border: 1px solid #E5E5E5;
      border-radius: 8px;
      overflow: hidden;
      box-shadow: 0px 4px 8px 0px rgba(0, 0, 0, 0.25);
    }
    .title {
      margin: 0;
      color: #111;
      font-family: Inter;
      font-size: 20px;
      font-style: normal;
      font-weight: 600;
      line-height: normal;
    }
    .body {
      margin: 0;
      align-self: stretch;
      color: var(--Color-Text-Muted, #666);
      font-family: Inter;
      font-size: 14px;
      font-style: normal;
      font-weight: 400;
      line-height: 20px;
      text-align: center;
    }
    .actions {
      display: flex;
      flex-direction: row;
      align-self: stretch;
      padding: 8px 12px;
      justify-content: center;
      align-items: center;
      gap: var(--Spacing-Md, 12px);
      background: var(--Color-Primary, #0D99FF);
      border-radius: 4px;
      height: fit-content;
    }
    .button {
      width: 80px;
      height: 32px;
      flex-shrink: 0;
    }
    .dot {
      width: 24px;
      height: 24px;
      flex-shrink: 0;
      background: #F00;
      border-radius: 50%;
    }
  </style>
</head>
<body>
  <div class="card">
    <p class="title">Hello world</p>
    <p class="body">Body copy</p>
    <div class="actions">
      <!-- Component: Button -->
      <div class="button"></div>
      <div class="dot"></div>
    </div>
  </div>
</body>
</html>"#;
        assert_eq!(html_for(&d, id), expected);
    }

    #[test]
    fn card_passes_html_validate() {
        let (d, id) = fixture::card();
        let html = html_for(&d, id);
        assert!(html.starts_with("<!doctype html>\n<html lang=\"en\">\n"));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("<title>Card</title>"));
        assert!(html.ends_with("</body>\n</html>"));

        // Verify balanced non-void HTML tags and absence of XML-style self-closing void elements.
        assert!(!html.contains("/>"), "HTML5 output should not use XML self-closing syntax: {html}");
        assert_eq!(html.matches("<div ").count(), html.matches("</div>").count());
        assert_eq!(html.matches("<p ").count(), html.matches("</p>").count());

        // When `html-validate` is installed on PATH (e.g. in the `codegen-validators` CI job),
        // pipe the generated document to `html-validate --stdin` and assert zero errors.
        if let Ok(status) =
            std::process::Command::new("html-validate").arg("--version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
        {
            if status.success() {
                use std::io::Write;
                let mut child = std::process::Command::new("html-validate")
                    .arg("--stdin")
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("spawn html-validate");
                child.stdin.as_mut().unwrap().write_all(html.as_bytes()).expect("write html to stdin");
                let out = child.wait_with_output().expect("wait html-validate");
                assert!(
                    out.status.success(),
                    "html-validate failed:\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    }

    #[test]
    fn bound_variables_print_code_syntax() {
        let (mut d, card) = fixture::card();
        let without = html_for(&d, card);
        assert!(without.contains("background: var(--Color-Primary, #0D99FF);"), "{without}");
        assert!(without.contains("--Color-Primary: #0D99FF;"), "{without}");

        d.variables.get_mut("primary").unwrap().code_syntax.insert(CodeSyntaxPlatform::Web, "var(--brand-primary)".into());
        let with_syntax = html_for(&d, card);
        assert!(with_syntax.contains("background: var(--brand-primary, #0D99FF);"), "{with_syntax}");
        assert!(with_syntax.contains("--brand-primary: #0D99FF;"), "{with_syntax}");
    }

    #[test]
    fn auto_layout_to_flex() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 240.0, 64.0));
        row.name = "Row".into();
        row.fills.clear();
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                item_spacing: 12.0,
                padding: [16.0; 4],
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            });
        }
        let fid = d.insert(page, row).unwrap();
        let html = html_for(&d, fid);
        for want in ["display: flex;", "flex-direction: row;", "gap: 12px;", "padding: 16px;"] {
            assert!(html.contains(want), "missing `{want}` in:\n{html}");
        }
    }

    #[test]
    fn escaping() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let tid = d.alloc_id();
        let payload = "<script>alert(\"x\")</script> & 'quotes'";
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 200.0, 24.0), payload, TextStyle::default());
        t.name = payload.into();
        let tid = d.insert(page, t).unwrap();
        let html = html_for(&d, tid);
        let escaped = "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;quotes&#39;";
        assert!(html.contains(&format!("<title>{escaped}</title>")), "title not escaped:\n{html}");
        assert!(html.contains(&format!(">{escaped}</p>")), "body not escaped:\n{html}");
        assert!(!html.contains("<script>"), "raw script tag leaked:\n{html}");
    }

    #[test]
    fn language_listed_once() {
        assert!(CodeLanguage::ALL.contains(&CodeLanguage::Html));
        let ids: Vec<&str> = CodeLanguage::ALL.iter().map(|l| l.id()).collect();
        assert!(ids.contains(&"html"));
        for &lang in &CodeLanguage::ALL {
            assert_eq!(CodeLanguage::from_id(lang.id()), Some(lang));
        }

        // Verify that `designos-mcp` derives its `generate_code` language list from
        // `CodeLanguage::ALL` rather than hardcoding a separate language array.
        let mcp_engine_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("designos-mcp/src/engine.rs");
        let mcp_src = std::fs::read_to_string(&mcp_engine_path).expect("read engine.rs");
        assert!(mcp_src.contains("CodeLanguage::ALL"), "designos-mcp/src/engine.rs must derive language enum from CodeLanguage::ALL");
    }

    #[test]
    fn edge_cases_emoji_runs_rtl_rotation_images_hidden_and_fragment() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let root_id = d.alloc_id();
        let mut root = Node::frame(root_id, Rect::new(0.0, 0.0, 400.0, 300.0));
        root.name = "🔥".into();
        let root_id = d.insert(page, root).unwrap();

        // Multi-paragraph text with RTL paragraph and styled run
        let tid = d.alloc_id();
        let mut txt = Node::text(tid, Rect::new(10.0, 20.0, 200.0, 60.0), "Hello bold\nשלום", TextStyle::default());
        txt.name = "123 Copy".into();
        if let NodeKind::Text(tp) = &mut txt.kind {
            let bold = TextStyle { font_weight: 700, ..Default::default() };
            tp.runs.push(TextRun { start: 6, end: 10, style: bold });
        }
        d.insert(root_id, txt).unwrap();

        // Rotated image rectangle (Fill)
        let img_id = d.alloc_id();
        let mut img = Node::rectangle(img_id, Rect::new(20.0, 90.0, 100.0, 80.0));
        img.name = "Hero Photo".into();
        img.transform = Affine::rotate(15.0_f32.to_radians());
        img.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "missing".into(), scale_mode: ImageScaleMode::Fill },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        d.insert(root_id, img).unwrap();

        // Tiled image rectangle
        let tile_id = d.alloc_id();
        let mut tile = Node::rectangle(tile_id, Rect::new(140.0, 90.0, 60.0, 60.0));
        tile.name = "Hero Photo".into(); // duplicate name -> hero-photo-2
        tile.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "missing".into(), scale_mode: ImageScaleMode::Tile },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        d.insert(root_id, tile).unwrap();

        // Hidden layer (must be omitted)
        let hid_id = d.alloc_id();
        let mut hid = Node::rectangle(hid_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        hid.name = "SecretGhost".into();
        hid.visible = false;
        d.insert(root_id, hid).unwrap();

        let frag = html_with(&d, root_id, &HtmlOptions { fragment: true });
        assert!(frag.starts_with("<style>\n"), "{frag}");
        assert!(!frag.contains("<!doctype html>"), "{frag}");
        assert!(frag.contains(&format!(".n-{} {{", root_id.0)), "{frag}");
        assert!(frag.contains("position: relative;"), "{frag}");
        assert!(frag.contains(".n-123-copy {"), "{frag}");
        assert!(frag.contains("<span class=\"run-1\">bold</span>"), "{frag}");
        assert!(frag.contains("<p dir=\"rtl\">שלום</p>"), "{frag}");
        assert!(frag.contains("transform-origin: top left;"), "{frag}");
        assert!(frag.contains("<img class=\"hero-photo\""), "{frag}");
        assert!(frag.contains("object-fit: cover;"), "{frag}");
        assert!(frag.contains(".hero-photo-2 {"), "{frag}");
        assert!(frag.contains("background-repeat: repeat;"), "{frag}");
        assert!(!frag.contains("secret-ghost") && !frag.contains("SecretGhost"), "{frag}");
    }
}
