//! Sparse XML node metadata builder (`T7.2`) for AI agent context orientation.
//!
//! Emits a compact, valid XML hierarchy containing each node's numeric ID, tag type,
//! name, page-space bounding box, layout mode, component/variant metadata, text preview,
//! in-progress marker, and lint issue count, bounded by `depth` and `max_nodes`.

use designos_doc::{parse_variant_name, variant_name, Affine, Document, LayoutDirection, Node, NodeId, NodeKind, PropValue, Rect, Vec2};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

/// Options controlling the sparse XML metadata walk.
#[derive(Clone, Debug)]
pub struct MetadataOptions {
    /// Maximum descendant depth (`1..=10`, default `3`).
    pub depth: u8,
    /// Maximum total emitted elements (`1..=5_000`, default `1_000`).
    pub max_nodes: usize,
    /// Whether hidden (`visible == false`) nodes are emitted (`hidden="true"`) or omitted.
    pub include_hidden: bool,
    /// Force scoped-canvas shell detection even on small synthetic documents.
    pub scoped: bool,
    /// Optional per-node in-progress markers (`run_id` or `"yes"`).
    pub in_progress: BTreeMap<NodeId, String>,
}

impl Default for MetadataOptions {
    fn default() -> Self {
        Self { depth: 3, max_nodes: 1_000, include_hidden: true, scoped: false, in_progress: BTreeMap::new() }
    }
}

/// Output of [`metadata_xml`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataXml {
    pub root_id: u64,
    pub xml: String,
    pub node_count: usize,
    pub truncated: bool,
    pub remaining_estimate: usize,
    pub hidden_omitted: usize,
    pub scoped: bool,
}

struct XmlElement {
    tag: &'static str,
    attrs: Vec<(&'static str, String)>,
    children_attr: Option<usize>,
    more_attr: Option<&'static str>,
    children: Vec<XmlElement>,
}

impl XmlElement {
    fn write_into(&self, out: &mut String, indent: usize) {
        for _ in 0..indent {
            out.push_str("  ");
        }
        out.push('<');
        out.push_str(self.tag);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(v);
            out.push('"');
        }
        if let Some(c) = self.children_attr {
            out.push_str(" children=\"");
            out.push_str(&c.to_string());
            out.push('"');
        }
        if let Some(m) = self.more_attr {
            out.push_str(" more=\"");
            out.push_str(m);
            out.push('"');
        }
        if self.children.is_empty() {
            out.push_str("/>");
        } else {
            out.push('>');
            for child in &self.children {
                out.push('\n');
                child.write_into(out, indent + 1);
            }
            out.push('\n');
            for _ in 0..indent {
                out.push_str("  ");
            }
            out.push_str("</");
            out.push_str(self.tag);
            out.push('>');
        }
    }
}

/// Lower-case element tag name for a [`NodeKind`].
pub fn xml_tag_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Page { .. } => "page",
        NodeKind::Frame(_) => "frame",
        NodeKind::Group => "group",
        NodeKind::Section => "section",
        NodeKind::Rectangle => "rectangle",
        NodeKind::Ellipse(_) => "ellipse",
        NodeKind::Line => "line",
        NodeKind::Polygon { .. } => "polygon",
        NodeKind::Star { .. } => "star",
        NodeKind::Vector { .. } => "vector",
        NodeKind::Text(_) => "text",
        NodeKind::BooleanOp(_) => "boolean-operation",
        NodeKind::Component(_) => "component",
        NodeKind::ComponentSet(_) => "component-set",
        NodeKind::Instance { .. } => "instance",
        NodeKind::Sticky(_) => "sticky",
        NodeKind::Connector(_) => "connector",
        NodeKind::ShapeWithText(_) => "shape-with-text",
        NodeKind::Stamp { .. } => "stamp",
        NodeKind::Unknown { type_name } => {
            if type_name.eq_ignore_ascii_case("slice") {
                "slice"
            } else {
                "unknown"
            }
        }
    }
}

/// Escapes an arbitrary string for safe inclusion inside a double-quoted XML 1.0 attribute.
/// Control characters `< 0x20` other than `\t`, `\n`, `\r` (plus `\u{FFFE}` and `\u{FFFF}`)
/// are replaced with `\u{FFFD}` so any strict XML 1.0 parser accepts the output.
pub fn escape_xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if (c < '\u{0020}') || c == '\u{FFFE}' || c == '\u{FFFF}' => {
                out.push('\u{FFFD}');
            }
            c => out.push(c),
        }
    }
    out
}

/// Formats an `f32` rounded to `0.01` with trailing zeros stripped (`110.0 -> "110"`, `-0.001 -> "0"`).
pub fn fmt_f32_2(v: f32) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let r = (v as f64 * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[inline]
fn affine_rect_aabb(t: &Affine, size: Vec2) -> Rect {
    if t.b == 0.0 && t.c == 0.0 && t.a >= 0.0 && t.d >= 0.0 {
        Rect::new(t.tx, t.ty, t.a * size.x, t.d * size.y)
    } else {
        let p0 = t.apply(Vec2::ZERO);
        let p1 = t.apply(Vec2::new(size.x, 0.0));
        let p2 = t.apply(Vec2::new(size.x, size.y));
        let p3 = t.apply(Vec2::new(0.0, size.y));
        let x0 = p0.x.min(p1.x).min(p2.x).min(p3.x);
        let y0 = p0.y.min(p1.y).min(p2.y).min(p3.y);
        let x1 = p0.x.max(p1.x).max(p2.x).max(p3.x);
        let y1 = p0.y.max(p1.y).max(p2.y).max(p3.y);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

fn parent_world_of(doc: &Document, root: NodeId) -> Affine {
    let mut stack = Vec::new();
    let mut cur = doc.get(root).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let Some(pn) = doc.get(pid) else { break };
        if matches!(pn.kind, NodeKind::Page { .. }) {
            break;
        }
        stack.push(pn.transform);
        cur = pn.parent;
    }
    stack.into_iter().rev().fold(Affine::IDENTITY, |acc, t| acc * t)
}

fn resolve_instance_meta(doc: &Document, node: &Node, comp_id: Option<NodeId>) -> (Option<String>, Option<String>) {
    let mut comp_name = None;
    let mut pairs: Vec<(String, String)> = Vec::new();

    if let Some(cid) = comp_id {
        if let Some(comp_node) = doc.get(cid) {
            if let Some(set_node) = comp_node.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
                comp_name = Some(set_node.name.clone());
                pairs = parse_variant_name(&comp_node.name);
                if pairs.is_empty() {
                    for (k, v) in &comp_node.component.variant_props {
                        pairs.push((k.clone(), v.clone()));
                    }
                }
            } else {
                comp_name = Some(comp_node.name.clone());
                for (k, v) in &comp_node.component.variant_props {
                    pairs.push((k.clone(), v.clone()));
                }
            }
        }
    }

    for (k, v) in &node.component.variant_props {
        if let Some(slot) = pairs.iter_mut().find(|(pk, _)| pk == k) {
            slot.1 = v.clone();
        } else {
            pairs.push((k.clone(), v.clone()));
        }
    }
    for (k, val) in &node.component.values {
        if let PropValue::Variant(v) = val {
            if let Some(slot) = pairs.iter_mut().find(|(pk, _)| pk == k) {
                slot.1 = v.clone();
            } else {
                pairs.push((k.clone(), v.clone()));
            }
        }
    }

    let variant_str = (!pairs.is_empty()).then(|| variant_name(&pairs));
    (comp_name, variant_str)
}

fn is_truthy_flag(v: &str) -> bool {
    matches!(v, "true" | "1")
}

fn node_has_scoped_shell_marker(n: &Node) -> bool {
    n.fig_metadata.get("designos:scoped_shell").is_some_and(|v| is_truthy_flag(v))
        || n.fig_metadata.get("scoped_shell").is_some_and(|v| is_truthy_flag(v))
        || n.plugin_data.get("scoped_shell").is_some_and(|v| is_truthy_flag(v))
}

fn node_has_scoped_page_marker(n: &Node) -> bool {
    n.fig_metadata.get("designos:scoped").is_some_and(|v| is_truthy_flag(v))
        || n.fig_metadata.get("scoped").is_some_and(|v| is_truthy_flag(v))
        || n.plugin_data.get("scoped").is_some_and(|v| is_truthy_flag(v))
}

fn detect_scoped_document(doc: &Document, options: &MetadataOptions) -> (bool, bool) {
    let has_explicit_shell_markers = doc.nodes.values().any(node_has_scoped_shell_marker);
    let scoped = options.scoped || has_explicit_shell_markers || doc.nodes.values().any(node_has_scoped_page_marker);
    (scoped, has_explicit_shell_markers)
}

struct WalkCtx<'a> {
    doc: &'a Document,
    options: &'a MetadataOptions,
    lint_counts: Option<&'a dyn Fn(NodeId) -> Option<u16>>,
    max_depth: usize,
    max_nodes: usize,
    node_count: usize,
    truncated: bool,
    remaining_estimate: usize,
    hidden_omitted: usize,
    scoped: bool,
    has_explicit_shell_markers: bool,
}

impl<'a> WalkCtx<'a> {
    fn build_node(&mut self, id: NodeId, cur_depth: usize, parent_world: Affine) -> Option<XmlElement> {
        let node = self.doc.get(id)?;
        self.node_count += 1;

        let tag = xml_tag_name(&node.kind);
        let mut attrs = Vec::with_capacity(10);
        attrs.push(("id", node.id.0.to_string()));
        attrs.push(("name", escape_xml_attr(&node.name)));

        let world = if matches!(node.kind, NodeKind::Page { .. }) {
            Affine::IDENTITY
        } else {
            let w = parent_world * node.transform;
            let bbox = affine_rect_aabb(&w, node.size);
            attrs.push(("x", fmt_f32_2(bbox.x)));
            attrs.push(("y", fmt_f32_2(bbox.y)));
            attrs.push(("width", fmt_f32_2(bbox.w)));
            attrs.push(("height", fmt_f32_2(bbox.h)));
            let rot = fmt_f32_2(w.rotation_rad().to_degrees());
            if rot != "0" {
                attrs.push(("rotation", rot));
            }
            w
        };

        if !node.visible {
            attrs.push(("hidden", "true".to_string()));
        }
        if node.locked {
            attrs.push(("locked", "true".to_string()));
        }
        if let Some(al) = node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
            let dir = match al.direction {
                LayoutDirection::Horizontal => "row",
                LayoutDirection::Vertical => "column",
                LayoutDirection::Grid => "grid",
            };
            attrs.push(("layout", dir.to_string()));
        }
        if let NodeKind::Instance { component, .. } = &node.kind {
            let (comp_name, variant_str) = resolve_instance_meta(self.doc, node, *component);
            if let Some(cname) = comp_name.filter(|s| !s.is_empty()) {
                attrs.push(("component", escape_xml_attr(&cname)));
            }
            if let Some(vstr) = variant_str.filter(|s| !s.is_empty()) {
                attrs.push(("variant", escape_xml_attr(&vstr)));
            }
        }
        if let Some(tp) = node.kind.any_text() {
            if !tp.content.trim().is_empty() {
                let preview: String = tp.content.chars().take(40).collect();
                attrs.push(("text", escape_xml_attr(&preview)));
            }
        }
        if let Some(ip) = self
            .options
            .in_progress
            .get(&node.id)
            .or_else(|| node.plugin_data.get("in_progress"))
            .or_else(|| node.fig_metadata.get("in_progress"))
            .filter(|s| !s.is_empty())
        {
            attrs.push(("in-progress", escape_xml_attr(ip)));
        }
        if let Some(issues) = self.lint_counts.and_then(|f| f(node.id)).filter(|&n| n > 0) {
            attrs.push(("issues", issues.to_string()));
        }

        // Filter direct children according to `include_hidden`
        let mut included_children = Vec::with_capacity(node.children.len());
        for &cid in &node.children {
            let Some(cn) = self.doc.get(cid) else { continue };
            if !cn.visible && !self.options.include_hidden {
                self.hidden_omitted += 1;
                continue;
            }
            included_children.push(cid);
        }

        let mut children_attr = None;
        let mut more_attr = None;
        let mut xml_children = Vec::new();

        if included_children.is_empty() {
            let is_top_shell = if self.has_explicit_shell_markers {
                node_has_scoped_shell_marker(node)
            } else {
                self.scoped
                    && node.kind.is_container()
                    && !matches!(node.kind, NodeKind::Page { .. })
                    && node.parent.and_then(|p| self.doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::Page { .. }))
            };
            if is_top_shell {
                children_attr = Some(0);
            }
        } else if cur_depth >= self.max_depth {
            children_attr = Some(included_children.len());
            more_attr = Some("depth");
        } else {
            for (idx, &cid) in included_children.iter().enumerate() {
                if self.node_count >= self.max_nodes {
                    self.truncated = true;
                    more_attr = Some("budget");
                    if idx == 0 {
                        children_attr = Some(included_children.len());
                    }
                    self.remaining_estimate += included_children.len() - idx;
                    break;
                }
                if let Some(child_el) = self.build_node(cid, cur_depth + 1, world) {
                    xml_children.push(child_el);
                }
                if self.truncated {
                    more_attr = Some("budget");
                    if idx + 1 < included_children.len() {
                        self.remaining_estimate += included_children.len() - (idx + 1);
                    }
                    break;
                }
            }
        }

        Some(XmlElement { tag, attrs, children_attr, more_attr, children: xml_children })
    }
}

/// Builds the sparse XML hierarchy rooted at `root`.
pub fn metadata_xml(doc: &Document, root: NodeId, options: &MetadataOptions, lint_counts: Option<&dyn Fn(NodeId) -> Option<u16>>) -> MetadataXml {
    let max_depth = options.depth.clamp(1, 10) as usize;
    let max_nodes = options.max_nodes.clamp(1, 5_000);
    let (scoped, has_explicit_shell_markers) = detect_scoped_document(doc, options);
    let mut ctx = WalkCtx {
        doc,
        options,
        lint_counts,
        max_depth,
        max_nodes,
        node_count: 0,
        truncated: false,
        remaining_estimate: 0,
        hidden_omitted: 0,
        scoped,
        has_explicit_shell_markers,
    };

    let parent_world = parent_world_of(doc, root);
    let mut xml = String::new();
    if let Some(root_el) = ctx.build_node(root, 0, parent_world) {
        root_el.write_into(&mut xml, 0);
    }

    MetadataXml {
        root_id: root.0,
        xml,
        node_count: ctx.node_count,
        truncated: ctx.truncated,
        remaining_estimate: ctx.remaining_estimate,
        hidden_omitted: ctx.hidden_omitted,
        scoped: ctx.scoped,
    }
}

/// Returns the ordered list of [`NodeId`]s that [`metadata_xml`] will emit for `(doc, root, options)`.
pub fn emitted_metadata_nodes(doc: &Document, root: NodeId, options: &MetadataOptions) -> Vec<NodeId> {
    let max_depth = options.depth.clamp(1, 10) as usize;
    let max_nodes = options.max_nodes.clamp(1, 5_000);
    let mut out = Vec::with_capacity(max_nodes.min(256));

    fn visit(doc: &Document, id: NodeId, cur_depth: usize, max_depth: usize, max_nodes: usize, include_hidden: bool, out: &mut Vec<NodeId>) {
        if out.len() >= max_nodes {
            return;
        }
        let Some(node) = doc.get(id) else { return };
        out.push(id);
        if cur_depth >= max_depth || out.len() >= max_nodes {
            return;
        }
        for &cid in &node.children {
            if out.len() >= max_nodes {
                break;
            }
            let Some(cn) = doc.get(cid) else { continue };
            if !cn.visible && !include_hidden {
                continue;
            }
            visit(doc, cid, cur_depth + 1, max_depth, max_nodes, include_hidden, out);
        }
    }

    visit(doc, root, 0, max_depth, max_nodes, options.include_hidden, &mut out);
    out
}

/// Checks whether a lint rule (`rule`) is ignored on `n` via `plugin_data` or `fig_metadata`.
pub fn is_lint_rule_ignored(n: &Node, rule: &str) -> bool {
    let key = format!("lint_ignore:{rule}");
    if n.plugin_data.get(&key).is_some_and(|v| is_truthy_flag(v)) || n.fig_metadata.get(&key).is_some_and(|v| is_truthy_flag(v)) {
        return true;
    }
    for list in [n.plugin_data.get("lint_ignore"), n.fig_metadata.get("lint_ignore")].into_iter().flatten() {
        if list.split(',').map(str::trim).any(|item| item == "*" || item == rule) {
            return true;
        }
    }
    false
}

/// Runs the `agent` structural lint pass (`zero-bounds`, `clipped-text`, `off-page`,
/// `overlapping-siblings`) ONLY on `emitted` nodes, stopping once `budget` (50 ms) elapses.
/// Returns `(per_node_rule_hits, checked_nodes)`.
pub fn structural_lint_rules_for_nodes(doc: &Document, emitted: &[NodeId], budget: Duration) -> (HashMap<NodeId, BTreeSet<&'static str>>, Vec<NodeId>) {
    let start = Instant::now();
    let mut rules: HashMap<NodeId, BTreeSet<&'static str>> = HashMap::new();
    let mut checked = Vec::with_capacity(emitted.len());

    'nodes: for &id in emitted {
        if start.elapsed() >= budget {
            break 'nodes;
        }
        let Some(n) = doc.get(id) else { continue };
        if matches!(n.kind, NodeKind::Page { .. }) || !n.visible {
            continue;
        }

        let mut node_rules: BTreeSet<&'static str> = BTreeSet::new();

        // 1. zero-bounds
        if !matches!(n.kind, NodeKind::Line) && (n.size.x <= 0.0 || n.size.y <= 0.0) && !is_lint_rule_ignored(n, "zero-bounds") {
            node_rules.insert("zero-bounds");
        }

        // 2. clipped text (text overflowing its own bounds)
        if let NodeKind::Text(tp) = &n.kind {
            if !tp.content.trim().is_empty() && n.size.x > 0.0 && n.size.y > 0.0 && !is_lint_rule_ignored(n, "clipped-text") {
                if start.elapsed() >= budget {
                    break 'nodes;
                }
                let geom = designos_render::text_geometry(tp, n.size);
                let max_lines_exceeded = tp.max_lines.is_some_and(|m| m > 0 && geom.lines.len() > m as usize);
                if geom.width > n.size.x + 0.5 || geom.height > n.size.y + 0.5 || max_lines_exceeded {
                    node_rules.insert("clipped-text");
                }
            }
        }

        // 3. off-page / overflowing parent frame (checked on `n` itself so only emitted nodes are recorded)
        if let Some(parent_frame) = n.parent.and_then(|pid| doc.get(pid)).filter(|p| p.kind.frame_props().is_some()) {
            if !is_lint_rule_ignored(n, "off-page")
                && (n.transform.tx < -0.5
                    || n.transform.ty < -0.5
                    || n.transform.tx + n.size.x > parent_frame.size.x + 0.5
                    || n.transform.ty + n.size.y > parent_frame.size.y + 0.5)
            {
                node_rules.insert("off-page");
            }
        }

        // 4. overlapping siblings on frame `n`
        if let Some(fp) = n.kind.frame_props() {
            if fp.auto_layout.is_none() && n.children.len() >= 2 && !is_lint_rule_ignored(n, "overlapping-siblings") {
                let visible_children: Vec<&Node> =
                    n.children.iter().filter_map(|&cid| doc.get(cid)).filter(|c| c.visible && c.opacity > 0.0 && !c.is_mask).collect();
                let mut has_overlap = false;
                'outer: for i in 0..visible_children.len() {
                    if start.elapsed() >= budget {
                        break 'nodes;
                    }
                    for j in (i + 1)..visible_children.len() {
                        let a = visible_children[i];
                        let b = visible_children[j];
                        let ra = Rect::new(a.transform.tx, a.transform.ty, a.size.x, a.size.y);
                        let rb = Rect::new(b.transform.tx, b.transform.ty, b.size.x, b.size.y);
                        let ix0 = ra.x.max(rb.x);
                        let iy0 = ra.y.max(rb.y);
                        let ix1 = (ra.x + ra.w).min(rb.x + rb.w);
                        let iy1 = (ra.y + ra.h).min(rb.y + rb.h);
                        if ix1 > ix0 && iy1 > iy0 && (ix1 - ix0) * (iy1 - iy0) >= 1.0 {
                            let a_contains_b = ra.x <= rb.x && ra.y <= rb.y && ra.x + ra.w >= rb.x + rb.w && ra.y + ra.h >= rb.y + rb.h;
                            let b_contains_a = rb.x <= ra.x && rb.y <= ra.y && rb.x + rb.w >= ra.x + ra.w && rb.y + rb.h >= ra.y + rb.h;
                            if !a_contains_b && !b_contains_a {
                                has_overlap = true;
                                break 'outer;
                            }
                        }
                    }
                }
                if has_overlap {
                    node_rules.insert("overlapping-siblings");
                }
            }
        }

        if !node_rules.is_empty() {
            rules.insert(id, node_rules);
        }
        checked.push(id);
    }

    (rules, checked)
}

/// Fast structural lint pass over `root` (`clipped-text`, `zero-bounds`, `off-page`,
/// `overlapping-siblings`) bounded by the default metadata options and a 50 ms budget.
pub fn structural_lint_counts(doc: &Document, root: NodeId) -> HashMap<NodeId, u16> {
    let emitted = emitted_metadata_nodes(doc, root, &MetadataOptions::default());
    let (rules, _) = structural_lint_rules_for_nodes(doc, &emitted, Duration::from_millis(50));
    rules.into_iter().filter_map(|(id, set)| (!set.is_empty()).then_some((id, set.len() as u16))).collect()
}
