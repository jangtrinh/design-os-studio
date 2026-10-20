//! Open-Pencil XPath selector engine, semantic node description, 17-rule design
//! linter + auto-fixer, and 4 design token/pattern analyzers.

use crate::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Canonical XPath upper-case node type name for a [`NodeKind`].
pub fn xpath_type_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Page { .. } => "PAGE",
        NodeKind::Frame(_) => "FRAME",
        NodeKind::Group => "GROUP",
        NodeKind::Section => "SECTION",
        NodeKind::Rectangle => "RECTANGLE",
        NodeKind::Ellipse(_) => "ELLIPSE",
        NodeKind::Line => "LINE",
        NodeKind::Polygon { .. } => "POLYGON",
        NodeKind::Star { .. } => "STAR",
        NodeKind::Vector { .. } => "VECTOR",
        NodeKind::Text(_) => "TEXT",
        NodeKind::BooleanOp(_) => "BOOLEAN_OPERATION",
        NodeKind::Component(_) => "COMPONENT",
        NodeKind::ComponentSet(_) => "COMPONENT_SET",
        NodeKind::Instance { .. } => "INSTANCE",
        NodeKind::Sticky(_) => "STICKY",
        NodeKind::Connector(_) => "CONNECTOR",
        NodeKind::ShapeWithText(_) => "SHAPE_WITH_TEXT",
        NodeKind::Stamp { .. } => "STAMP",
        NodeKind::Unknown { .. } => "UNKNOWN",
    }
}

fn matches_node_test(kind: &NodeKind, test: &str) -> bool {
    if matches!(kind, NodeKind::Page { .. }) {
        return test.eq_ignore_ascii_case("PAGE");
    }
    if test == "*" || test.eq_ignore_ascii_case("NODE") {
        return true;
    }
    let canonical = xpath_type_name(kind);
    if canonical.eq_ignore_ascii_case(test) {
        return true;
    }
    matches!(
        (canonical, test.to_ascii_uppercase().as_str()),
        ("BOOLEAN_OPERATION", "BOOLEAN_OP" | "BOOLEAN") | ("COMPONENT_SET", "COMPONENTSET") | ("VECTOR", "PATH") | ("INSTANCE", "REF")
    )
}

fn layout_mode_str(n: &Node) -> &'static str {
    match n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
        Some(al) => match al.direction {
            LayoutDirection::Horizontal => "HORIZONTAL",
            LayoutDirection::Vertical => "VERTICAL",
            LayoutDirection::Grid => "GRID",
        },
        None => "NONE",
    }
}

/// Computes the canonical XPath of `id` relative to its page (e.g. `/FRAME[1]/FRAME[2]/TEXT[1]`).
pub fn node_to_xpath(doc: &Document, id: NodeId) -> String {
    let Some(node) = doc.get(id) else {
        return String::new();
    };
    if matches!(node.kind, NodeKind::Page { .. }) {
        let idx = doc.pages.iter().position(|&p| p == id).map_or(1, |i| i + 1);
        return format!("/PAGE[{idx}]");
    }
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(cid) = cur {
        let Some(cn) = doc.get(cid) else { break };
        if matches!(cn.kind, NodeKind::Page { .. }) {
            break;
        }
        let ty = xpath_type_name(&cn.kind);
        let idx = if let Some(pid) = cn.parent.and_then(|p| doc.get(p)) {
            let mut count = 0usize;
            let mut found = 1usize;
            for &sib_id in &pid.children {
                if let Some(sib) = doc.get(sib_id) {
                    if xpath_type_name(&sib.kind) == ty {
                        count += 1;
                        if sib_id == cid {
                            found = count;
                            break;
                        }
                    }
                }
            }
            found
        } else {
            1
        };
        chain.push(format!("/{ty}[{idx}]"));
        cur = cn.parent;
    }
    chain.reverse();
    if chain.is_empty() {
        format!("/{}[1]", xpath_type_name(&node.kind))
    } else {
        chain.concat()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XPathAxis {
    Child,
    Descendant,
}

#[derive(Clone, Debug)]
struct XPathStep {
    axis: XPathAxis,
    node_test: String,
    predicates: Vec<String>,
}

fn parse_xpath(expr: &str) -> Result<Vec<XPathStep>, String> {
    let s = expr.trim();
    if s.is_empty() {
        return Err("empty XPath expression".into());
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut steps = Vec::new();

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let axis = if s[i..].starts_with("//") {
            i += 2;
            XPathAxis::Descendant
        } else if bytes[i] == b'/' {
            i += 1;
            XPathAxis::Child
        } else if steps.is_empty() {
            XPathAxis::Descendant
        } else {
            return Err(format!("unexpected token in XPath at byte {i}: {}", &s[i..]));
        };

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start_name = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' && bytes[i] != b'[' {
            i += 1;
        }
        let node_test = s[start_name..i].trim().to_string();
        if node_test.is_empty() {
            return Err(format!("missing node test in XPath near byte {i}"));
        }

        let mut predicates = Vec::new();
        loop {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                let pred_start = i;
                let mut depth = 1usize;
                let mut quote: Option<u8> = None;
                while i < bytes.len() && depth > 0 {
                    let b = bytes[i];
                    if let Some(q) = quote {
                        if b == b'\\' && i + 1 < bytes.len() {
                            i += 2;
                            continue;
                        }
                        if b == q {
                            quote = None;
                        }
                    } else if b == b'\'' || b == b'"' {
                        quote = Some(b);
                    } else if b == b'[' {
                        depth += 1;
                    } else if b == b']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    i += 1;
                }
                if depth != 0 {
                    return Err("unclosed '[' in XPath predicate".into());
                }
                predicates.push(s[pred_start..i].trim().to_string());
                i += 1; // consume ']'
            } else {
                break;
            }
        }

        steps.push(XPathStep { axis, node_test, predicates });
    }

    if steps.is_empty() {
        return Err("XPath produced no steps".into());
    }
    Ok(steps)
}

#[derive(Clone, Debug)]
enum AttrValue {
    Num(f32),
    Str(String),
    Bool(bool),
    None,
}

fn node_attr(n: &Node, raw_attr: &str) -> AttrValue {
    let attr = raw_attr.trim().trim_start_matches('@');
    match attr {
        "width" | "w" => AttrValue::Num(n.size.x),
        "height" | "h" => AttrValue::Num(n.size.y),
        "x" => AttrValue::Num(n.transform.tx),
        "y" => AttrValue::Num(n.transform.ty),
        "cornerRadius" | "radius" => AttrValue::Num(n.corner_radii[0]),
        "opacity" => AttrValue::Num(n.opacity),
        "fontSize" => n.kind.any_text().map(|t| AttrValue::Num(t.style.font_size)).unwrap_or(AttrValue::None),
        "fontWeight" => n.kind.any_text().map(|t| AttrValue::Num(t.style.font_weight as f32)).unwrap_or(AttrValue::None),
        "fontFamily" => n.kind.any_text().map(|t| AttrValue::Str(t.style.font_family.clone())).unwrap_or(AttrValue::None),
        "characters" | "text" | "text()" => n.kind.any_text().map(|t| AttrValue::Str(t.content.clone())).unwrap_or(AttrValue::None),
        "itemSpacing" | "gap" => {
            n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| AttrValue::Num(al.item_spacing)).unwrap_or(AttrValue::Num(0.0))
        }
        "paddingTop" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| AttrValue::Num(al.padding[0])).unwrap_or(AttrValue::Num(0.0)),
        "paddingRight" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| AttrValue::Num(al.padding[1])).unwrap_or(AttrValue::Num(0.0)),
        "paddingBottom" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| AttrValue::Num(al.padding[2])).unwrap_or(AttrValue::Num(0.0)),
        "paddingLeft" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| AttrValue::Num(al.padding[3])).unwrap_or(AttrValue::Num(0.0)),
        "childCount" | "count(*)" => AttrValue::Num(n.children.len() as f32),
        "name" => AttrValue::Str(n.name.clone()),
        "type" => AttrValue::Str(xpath_type_name(&n.kind).to_string()),
        "layoutMode" => AttrValue::Str(layout_mode_str(n).to_string()),
        "visible" => AttrValue::Bool(n.visible),
        "locked" => AttrValue::Bool(n.locked),
        "clipsContent" | "clip" => AttrValue::Bool(n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false)),
        "id" => AttrValue::Num(n.id.0 as f32),
        _ => AttrValue::None,
    }
}

fn split_logical_top_level<'a>(s: &'a str, kw: &str) -> Vec<&'a str> {
    let bytes = s.as_bytes();
    let kw_bytes = kw.as_bytes();
    let mut parts = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut paren = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'\'' || b == b'"' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'(' {
            paren += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            paren = paren.saturating_sub(1);
            i += 1;
            continue;
        }
        if paren == 0 && i + kw_bytes.len() <= bytes.len() {
            let prev_ws = i > 0 && bytes[i - 1].is_ascii_whitespace();
            let next_ws = i + kw_bytes.len() < bytes.len() && bytes[i + kw_bytes.len()].is_ascii_whitespace();
            if prev_ws && next_ws && s[i..i + kw_bytes.len()].eq_ignore_ascii_case(kw) {
                parts.push(s[last..i].trim());
                i += kw_bytes.len();
                last = i;
                continue;
            }
        }
        i += 1;
    }
    parts.push(s[last..].trim());
    parts
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')) {
        if t.len() >= 2 {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn parse_fn_two_args(inner: &str) -> Option<(String, String)> {
    let bytes = inner.as_bytes();
    let mut quote: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'\'' || b == b'"' {
            quote = Some(b);
        } else if b == b',' {
            let a = inner[..i].trim().to_string();
            let b = unquote(inner[i + 1..].trim());
            return Some((a, b));
        }
    }
    None
}

fn eval_predicate_expr(doc: &Document, n: &Node, expr: &str) -> Result<bool, String> {
    let s = expr.trim();
    if s.is_empty() {
        return Ok(true);
    }
    // Posix / XPath 1-based positional index `[1]`, `[2]`
    if let Ok(pos) = s.parse::<usize>() {
        if pos == 0 {
            return Ok(false);
        }
        let ty = xpath_type_name(&n.kind);
        if let Some(parent) = n.parent.and_then(|p| doc.get(p)) {
            let mut count = 0usize;
            for &sib_id in &parent.children {
                if let Some(sib) = doc.get(sib_id) {
                    if xpath_type_name(&sib.kind) == ty {
                        count += 1;
                        if sib_id == n.id {
                            return Ok(count == pos);
                        }
                    }
                }
            }
            return Ok(false);
        }
        return Ok(pos == 1);
    }

    let or_parts = split_logical_top_level(s, "or");
    if or_parts.len() > 1 {
        for part in or_parts {
            if eval_predicate_expr(doc, n, part)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let and_parts = split_logical_top_level(s, "and");
    if and_parts.len() > 1 {
        for part in and_parts {
            if !eval_predicate_expr(doc, n, part)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    if s.starts_with('(') && s.ends_with(')') {
        return eval_predicate_expr(doc, n, &s[1..s.len() - 1]);
    }

    if let Some(rest) = s.strip_prefix("not(").and_then(|r| r.strip_suffix(')')) {
        return Ok(!eval_predicate_expr(doc, n, rest)?);
    }

    for (fn_name, mode) in [("contains(", "contains"), ("starts-with(", "starts"), ("ends-with(", "ends")] {
        if let Some(inner) = s.strip_prefix(fn_name).and_then(|r| r.strip_suffix(')')) {
            let (attr_expr, needle) = parse_fn_two_args(inner).ok_or_else(|| format!("invalid {fn_name}...) args"))?;
            let hay = match node_attr(n, &attr_expr) {
                AttrValue::Str(v) => v,
                AttrValue::Num(v) => v.to_string(),
                AttrValue::Bool(v) => v.to_string(),
                AttrValue::None => String::new(),
            };
            let matched = match mode {
                "contains" => hay.contains(&needle),
                "starts" => hay.starts_with(&needle),
                "ends" => hay.ends_with(&needle),
                _ => false,
            };
            return Ok(matched);
        }
    }

    // Comparison operators outside quotes
    let ops = ["<=", ">=", "!=", "=", "<", ">"];
    let bytes = s.as_bytes();
    let mut quote: Option<u8> = None;
    for i in 0..bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            continue;
        }
        if b == b'\'' || b == b'"' {
            quote = Some(b);
            continue;
        }
        for op in ops {
            if s[i..].starts_with(op) {
                let lhs = s[..i].trim();
                let rhs = s[i + op.len()..].trim();
                let lv = node_attr(n, lhs);
                return Ok(compare_attr(&lv, op, rhs));
            }
        }
    }

    // Bare attribute truthiness, e.g. `@visible`
    match node_attr(n, s) {
        AttrValue::Bool(b) => Ok(b),
        AttrValue::Num(v) => Ok(v != 0.0),
        AttrValue::Str(v) => Ok(!v.is_empty()),
        AttrValue::None => Err(format!("unsupported XPath predicate: {s}")),
    }
}

fn compare_attr(lv: &AttrValue, op: &str, rhs_raw: &str) -> bool {
    let rhs_unquoted = unquote(rhs_raw);
    match lv {
        AttrValue::Num(a) => {
            let Ok(b) = rhs_unquoted.parse::<f32>() else {
                return false;
            };
            match op {
                "=" => (*a - b).abs() < 1e-4,
                "!=" => (*a - b).abs() >= 1e-4,
                "<" => *a < b,
                "<=" => *a <= b + 1e-4,
                ">" => *a > b,
                ">=" => *a >= b - 1e-4,
                _ => false,
            }
        }
        AttrValue::Str(a) => match op {
            "=" => a == &rhs_unquoted,
            "!=" => a != &rhs_unquoted,
            _ => false,
        },
        AttrValue::Bool(a) => {
            let b = match rhs_unquoted.to_ascii_lowercase().as_str() {
                "true" | "1" | "true()" => true,
                "false" | "0" | "false()" => false,
                _ => return false,
            };
            match op {
                "=" => *a == b,
                "!=" => *a != b,
                _ => false,
            }
        }
        AttrValue::None => false,
    }
}

fn node_summary_json(doc: &Document, n: &Node) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), json!(n.id.0));
    obj.insert("name".into(), json!(n.name));
    obj.insert("type".into(), json!(xpath_type_name(&n.kind)));
    obj.insert("xpath".into(), json!(node_to_xpath(doc, n.id)));
    obj.insert("x".into(), json!(n.transform.tx));
    obj.insert("y".into(), json!(n.transform.ty));
    obj.insert("width".into(), json!(n.size.x));
    obj.insert("height".into(), json!(n.size.y));
    obj.insert("visible".into(), json!(n.visible));
    obj.insert("opacity".into(), json!(n.opacity));
    obj.insert("layoutMode".into(), json!(layout_mode_str(n)));
    obj.insert("childCount".into(), json!(n.children.len()));
    obj.insert("cornerRadius".into(), json!(n.corner_radii[0]));
    if let Some(t) = n.kind.any_text() {
        obj.insert("characters".into(), json!(t.content));
        obj.insert("fontSize".into(), json!(t.style.font_size));
        obj.insert("fontWeight".into(), json!(t.style.font_weight));
    }
    Value::Object(obj)
}

/// Evaluates an Open-Pencil XPath selector query on `doc` (scoped to `page` if provided).
pub fn query_xpath(doc: &Document, page: Option<NodeId>, expr: &str, limit: usize) -> Result<Vec<Value>, String> {
    let steps = parse_xpath(expr)?;
    let mut current: Vec<NodeId> = match page {
        Some(p) => {
            if !doc.nodes.contains_key(&p) {
                return Err(format!("page {p:?} not found"));
            }
            vec![p]
        }
        None => doc.pages.clone(),
    };

    for step in &steps {
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();
        for &root_id in &current {
            match step.axis {
                XPathAxis::Child => {
                    if let Some(rn) = doc.get(root_id) {
                        for &cid in &rn.children {
                            if seen.insert(cid) {
                                candidates.push(cid);
                            }
                        }
                    }
                }
                XPathAxis::Descendant => {
                    doc.walk(root_id, |node, depth| {
                        if depth > 0 && seen.insert(node.id) {
                            candidates.push(node.id);
                        }
                    });
                }
            }
        }

        let mut next = Vec::new();
        for cid in candidates {
            let Some(cn) = doc.get(cid) else { continue };
            if !matches_node_test(&cn.kind, &step.node_test) {
                continue;
            }
            let mut ok = true;
            for pred in &step.predicates {
                if !eval_predicate_expr(doc, cn, pred)? {
                    ok = false;
                    break;
                }
            }
            if ok {
                next.push(cid);
            }
        }
        current = next;
    }

    let max = if limit == 0 { usize::MAX } else { limit };
    Ok(current.into_iter().take(max).filter_map(|id| doc.get(id).map(|n| node_summary_json(doc, n))).collect())
}

/// Infers a semantic UI role hint (`"button"`, `"input"`, `"card"`, `"navbar"`, `"heading"`, `"badge"`, `"icon"`, `"container"`).
pub fn infer_role_hint(doc: &Document, n: &Node) -> &'static str {
    let lname = n.name.to_ascii_lowercase();
    if lname.contains("btn") || lname.contains("button") || lname.contains("cta") {
        return "button";
    }
    if lname.contains("input") || lname.contains("field") || lname.contains("search") || lname.contains("textbox") {
        return "input";
    }
    if lname.contains("nav") || lname.contains("header") || lname.contains("toolbar") || lname.contains("topbar") {
        return "navbar";
    }
    if lname.contains("badge") || lname.contains("tag") || lname.contains("chip") || lname.contains("pill") {
        return "badge";
    }
    if lname.contains("card") || lname.contains("tile") || lname.contains("panel") {
        return "card";
    }
    if lname.contains("icon") || lname.contains("ico") {
        return "icon";
    }
    match &n.kind {
        NodeKind::Text(t) => {
            if t.style.font_size >= 20.0 || t.style.font_weight >= 600 || lname.contains("title") || lname.contains("heading") {
                "heading"
            } else {
                "text"
            }
        }
        NodeKind::Vector { .. } | NodeKind::Star { .. } | NodeKind::Polygon { .. } => {
            if n.size.x <= 32.0 && n.size.y <= 32.0 {
                "icon"
            } else {
                "shape"
            }
        }
        NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::Instance { .. } => {
            let has_text_child = n.children.iter().any(|&c| doc.get(c).is_some_and(|cn| matches!(cn.kind, NodeKind::Text(_))));
            if has_text_child && n.children.len() <= 3 && n.size.y >= 24.0 && n.size.y <= 64.0 && n.size.x <= 320.0 && !n.fills.is_empty() {
                "button"
            } else if n.size.y <= 28.0 && n.size.x <= 120.0 && has_text_child {
                "badge"
            } else if n.children.len() >= 2 && (!n.fills.is_empty() || !n.effects.is_empty()) && n.size.y > 80.0 {
                "card"
            } else {
                "container"
            }
        }
        _ => "container",
    }
}

/// Returns a structured semantic summary of `id` up to `depth`.
pub fn describe_node(doc: &Document, id: NodeId, depth: usize) -> Result<Value, String> {
    let n = doc.get(id).ok_or_else(|| format!("node {id:?} not found"))?;
    let fills: Vec<String> = n
        .fills
        .iter()
        .filter(|p| p.visible)
        .map(|p| match &p.kind {
            PaintKind::Solid(c) => format!("#{}", c.to_hex()),
            PaintKind::LinearGradient { .. } => "linear-gradient".into(),
            PaintKind::RadialGradient { .. } => "radial-gradient".into(),
            PaintKind::AngularGradient { .. } => "angular-gradient".into(),
            PaintKind::DiamondGradient { .. } => "diamond-gradient".into(),
            PaintKind::Image { image_ref, .. } => format!("image:{image_ref}"),
        })
        .collect();

    let typography = n.kind.any_text().map(|t| {
        json!({
            "content": t.content,
            "font_family": t.style.font_family,
            "font_size": t.style.font_size,
            "font_weight": t.style.font_weight,
            "line_height": match t.style.line_height {
                LineHeight::Auto => json!("auto"),
                LineHeight::Pixels(px) => json!(px),
                LineHeight::Percent(pct) => json!(format!("{pct}%")),
            },
        })
    });

    let children: Vec<Value> = if depth > 0 { n.children.iter().filter_map(|&cid| describe_node(doc, cid, depth - 1).ok()).collect() } else { Vec::new() };

    Ok(json!({
        "id": n.id.0,
        "xpath": node_to_xpath(doc, n.id),
        "type": xpath_type_name(&n.kind),
        "name": n.name,
        "bounds": {
            "x": n.transform.tx,
            "y": n.transform.ty,
            "width": n.size.x,
            "height": n.size.y,
        },
        "layout_mode": layout_mode_str(n),
        "fills": fills,
        "typography": typography,
        "role_hint": infer_role_hint(doc, n),
        "child_count": n.children.len(),
        "children": children,
    }))
}

const SPACING_SCALE: [f32; 15] = [0.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0, 80.0, 96.0];
const FONT_SCALE: [f32; 22] =
    [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0, 28.0, 30.0, 32.0, 36.0, 40.0, 48.0, 56.0, 60.0, 64.0, 72.0, 80.0, 96.0];
const RADIUS_SCALE: [f32; 14] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0, 999.0, 1000.0, 9999.0];

fn on_scale(v: f32, scale: &[f32]) -> bool {
    scale.iter().any(|&s| (v - s).abs() < 0.05)
}

fn nearest_on_scale(v: f32, scale: &[f32]) -> f32 {
    scale.iter().copied().min_by(|a, b| (v - *a).abs().total_cmp(&(v - *b).abs())).unwrap_or(v)
}

fn is_grid_multiple(v: f32, step: f32) -> bool {
    let rem = (v / step).round() * step - v;
    rem.abs() < 0.05
}

fn is_default_layer_name(name: &str) -> bool {
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

fn relative_luminance(c: Color) -> f32 {
    fn chan(v: f32) -> f32 {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
}

fn contrast_ratio(a: Color, b: Color) -> f32 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (bright, dark) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (bright + 0.05) / (dark + 0.05)
}

fn effective_background_color(doc: &Document, id: NodeId) -> Color {
    let mut cur = doc.get(id).and_then(|n| n.parent);
    while let Some(pid) = cur {
        let Some(pn) = doc.get(pid) else { break };
        for fill in pn.fills.iter().rev() {
            if fill.visible {
                if let PaintKind::Solid(c) = fill.kind {
                    if c.a * fill.opacity >= 0.5 {
                        return c;
                    }
                }
            }
        }
        if let NodeKind::Page { background } = pn.kind {
            return background;
        }
        cur = pn.parent;
    }
    Color::WHITE
}

fn rule_enabled(preset: &str, rule: &str, category: &str) -> bool {
    match preset.to_ascii_lowercase().as_str() {
        "" | "all" | "strict" | "recommended" | "default" => true,
        "layout" => category.eq_ignore_ascii_case("layout"),
        "tokens" => category.eq_ignore_ascii_case("tokens"),
        "naming" => category.eq_ignore_ascii_case("Naming"),
        "accessibility" | "a11y" => category.eq_ignore_ascii_case("accessibility"),
        "structure" => category.eq_ignore_ascii_case("structure"),
        other => other.split(',').any(|r| r.trim().eq_ignore_ascii_case(rule)),
    }
}

/// Runs the 17 Open-Pencil design lint rules across `layout`, `tokens`, `Naming`, `accessibility`, and `structure`.
pub fn lint_document(doc: &Document, page: Option<NodeId>, preset: &str) -> Value {
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };
    let mut issues = Vec::new();
    let mut by_category: BTreeMap<String, usize> =
        BTreeMap::from([("layout".into(), 0), ("tokens".into(), 0), ("Naming".into(), 0), ("accessibility".into(), 0), ("structure".into(), 0)]);
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();

    let mut push_issue = |rule: &str, category: &str, severity: &str, n: &Node, msg: String, fixable: bool| {
        if !rule_enabled(preset, rule, category) {
            return;
        }
        *by_category.entry(category.to_string()).or_insert(0) += 1;
        *by_rule.entry(rule.to_string()).or_insert(0) += 1;
        issues.push(json!({
            "rule": rule,
            "category": category,
            "severity": severity,
            "node_id": n.id.0,
            "node_name": n.name,
            "xpath": node_to_xpath(doc, n.id),
            "message": msg,
            "fixable": fixable,
        }));
    };

    for root in roots {
        doc.walk(root, |n, depth| {
            if matches!(n.kind, NodeKind::Page { .. }) || !n.visible {
                return;
            }

            // 10. default-layer-name (Naming)
            if is_default_layer_name(&n.name) {
                push_issue("default-layer-name", "Naming", "warning", n, format!("Layer '{}' uses a generic default name", n.name), true);
            }

            // 14. deeply-nested (structure)
            if depth > 6 {
                push_issue("deeply-nested", "structure", "warning", n, format!("Node is nested at depth {depth} (> 6)"), false);
            }

            // 15. empty-container (structure)
            if matches!(n.kind, NodeKind::Frame(_) | NodeKind::Group)
                && n.children.is_empty()
                && n.fills.iter().all(|p| !p.visible)
                && n.stroke.paints.iter().all(|p| !p.visible)
                && n.effects.iter().all(|e| !e.visible())
            {
                push_issue("empty-container", "structure", "info", n, "Empty container has no children, fills, strokes, or effects".into(), true);
            }

            // 6. non-4px-grid (layout)
            if !matches!(n.kind, NodeKind::Text(_) | NodeKind::Vector { .. } | NodeKind::Line)
                && (!is_grid_multiple(n.size.x, 4.0) || !is_grid_multiple(n.size.y, 4.0))
            {
                push_issue("non-4px-grid", "layout", "info", n, format!("Size {}x{} is not aligned to the 4px grid", n.size.x, n.size.y), true);
            }

            // Frame / Auto-layout rules
            if let Some(fp) = n.kind.frame_props() {
                if let Some(al) = &fp.auto_layout {
                    // 1. inconsistent-spacing (layout)
                    if !on_scale(al.item_spacing, &SPACING_SCALE) {
                        push_issue("inconsistent-spacing", "layout", "warning", n, format!("item_spacing {} is off the spacing scale", al.item_spacing), true);
                    }
                    // 5. unbalanced-padding (layout)
                    if (al.padding[0] - al.padding[2]).abs() > 0.1 || (al.padding[1] - al.padding[3]).abs() > 0.1 {
                        push_issue(
                            "unbalanced-padding",
                            "layout",
                            "info",
                            n,
                            format!("Unbalanced padding [{}, {}, {}, {}]", al.padding[0], al.padding[1], al.padding[2], al.padding[3]),
                            true,
                        );
                    }
                } else if n.children.len() >= 2 {
                    // 3. missing-auto-layout (layout)
                    push_issue(
                        "missing-auto-layout",
                        "layout",
                        "warning",
                        n,
                        format!("Container with {} children does not use auto-layout", n.children.len()),
                        false,
                    );

                    // Check sibling alignment & overlaps
                    let child_nodes: Vec<&Node> = n.children.iter().filter_map(|&cid| doc.get(cid)).filter(|c| c.visible).collect();
                    let mut misaligned = false;
                    let mut overlapping = false;
                    for i in 0..child_nodes.len() {
                        for j in (i + 1)..child_nodes.len() {
                            let a = child_nodes[i];
                            let b = child_nodes[j];
                            let dx = (a.transform.tx - b.transform.tx).abs();
                            let dy = (a.transform.ty - b.transform.ty).abs();
                            if (dx > 0.1 && dx <= 3.0) || (dy > 0.1 && dy <= 3.0) {
                                misaligned = true;
                            }
                            let ra = Rect::new(a.transform.tx, a.transform.ty, a.size.x, a.size.y);
                            let rb = Rect::new(b.transform.tx, b.transform.ty, b.size.x, b.size.y);
                            if ra.intersects(&rb) {
                                overlapping = true;
                            }
                        }
                    }
                    // 2. misaligned-siblings (layout)
                    if misaligned {
                        push_issue("misaligned-siblings", "layout", "warning", n, "Child layers are off-alignment by 1–3px".into(), true);
                    }
                    // 16. overlapping-siblings (structure)
                    if overlapping {
                        push_issue("overlapping-siblings", "structure", "warning", n, "Sibling layers overlap without auto-layout".into(), false);
                    }
                }

                // 4. overflowing-children (layout)
                for &cid in &n.children {
                    if let Some(cn) = doc.get(cid) {
                        if cn.visible
                            && (cn.transform.tx < -0.5
                                || cn.transform.ty < -0.5
                                || cn.transform.tx + cn.size.x > n.size.x + 0.5
                                || cn.transform.ty + cn.size.y > n.size.y + 0.5)
                        {
                            push_issue("overflowing-children", "layout", "warning", cn, format!("Child '{}' overflows parent '{}'", cn.name, n.name), false);
                        }
                    }
                }
            }

            // 7. hardcoded-color (tokens)
            let has_solid_fill = n.fills.iter().any(|p| p.visible && matches!(p.kind, PaintKind::Solid(_)));
            let has_bound_fill = n.variables.bindings.keys().any(|k| k.starts_with("fills") || k == "fill") || n.style_refs.contains_key("fill");
            if has_solid_fill && !has_bound_fill {
                push_issue("hardcoded-color", "tokens", "info", n, "Solid fill color is hardcoded without a variable or style token".into(), false);
            }

            // 9. inconsistent-radius (tokens)
            if n.corner_radii != [0.0; 4] {
                let r0 = n.corner_radii[0];
                let uniform = n.corner_radii.iter().all(|&r| (r - r0).abs() < 0.05);
                if !uniform || !on_scale(r0, &RADIUS_SCALE) {
                    push_issue(
                        "inconsistent-radius",
                        "tokens",
                        "info",
                        n,
                        format!("Corner radii {:?} are non-uniform or off the radius scale", n.corner_radii),
                        true,
                    );
                }
            }

            // 12. small-touch-target (accessibility)
            let role = infer_role_hint(doc, n);
            if matches!(role, "button" | "input") && (n.size.x < 44.0 || n.size.y < 44.0) {
                push_issue(
                    "small-touch-target",
                    "accessibility",
                    "warning",
                    n,
                    format!("Interactive {role} is {}x{} (< 44x44 minimum touch target)", n.size.x, n.size.y),
                    true,
                );
            }

            // Text-specific rules: 8, 11, 13, 17
            if let Some(t) = n.kind.any_text() {
                // 8. non-scale-font-size (tokens)
                if !on_scale(t.style.font_size, &FONT_SCALE) {
                    push_issue("non-scale-font-size", "tokens", "info", n, format!("Font size {}px is not on the typographic scale", t.style.font_size), true);
                }

                // 13. tiny-text (accessibility)
                if t.style.font_size < 12.0 {
                    push_issue(
                        "tiny-text",
                        "accessibility",
                        "warning",
                        n,
                        format!("Text font size {}px is below 12px legibility minimum", t.style.font_size),
                        true,
                    );
                }

                // 17. detached-text-style (tokens)
                if !n.style_refs.contains_key("text") {
                    push_issue("detached-text-style", "tokens", "info", n, "Text layer is not linked to a shared text style".into(), false);
                }

                // 11. low-contrast-text (accessibility)
                if let Some(fg) = t.style.fills.iter().rev().find(|p| p.visible).and_then(|p| match p.kind {
                    PaintKind::Solid(c) => Some(c),
                    _ => None,
                }) {
                    let bg = effective_background_color(doc, n.id);
                    let ratio = contrast_ratio(fg, bg);
                    let large = t.style.font_size >= 24.0 || (t.style.font_size >= 18.0 && t.style.font_weight >= 700);
                    let min_ratio = if large { 3.0 } else { 4.5 };
                    if ratio < min_ratio {
                        push_issue(
                            "low-contrast-text",
                            "accessibility",
                            "error",
                            n,
                            format!("Text contrast ratio {:.2}:1 is below WCAG AA {:.1}:1 (#{} on #{})", ratio, min_ratio, fg.to_hex(), bg.to_hex()),
                            false,
                        );
                    }
                }
            }
        });
    }

    for (style_key, meta) in &doc.style_meta {
        for (prop, vid) in &meta.bindings {
            if !doc.variables.contains_key(vid) && rule_enabled(preset, "broken-style-binding", "tokens") {
                *by_category.entry("tokens".to_string()).or_insert(0) += 1;
                *by_rule.entry("broken-style-binding".to_string()).or_insert(0) += 1;
                issues.push(json!({
                    "rule": "broken-style-binding",
                    "category": "tokens",
                    "severity": "warning",
                    "style_key": style_key,
                    "style_name": meta.name,
                    "property": prop,
                    "variable_id": vid,
                    "message": format!("Style '{}' binds '{}' to missing variable '{}'", meta.name, prop, vid),
                    "fixable": false,
                }));
            }
        }
    }

    json!({
        "preset": if preset.is_empty() { "recommended" } else { preset },
        "issue_count": issues.len(),
        "summary_by_category": by_category,
        "summary_by_rule": by_rule,
        "issues": issues,
    })
}

/// Applies deterministic auto-fixes for enabled lint rules and returns `{ "fixed_count", "changes" }`.
pub fn lint_fix_document(doc: &mut Document, page: Option<NodeId>, rules: Option<&[String]>) -> Value {
    let allow = |rule: &str| -> bool {
        match rules {
            None => true,
            Some(list) if list.is_empty() => true,
            Some(list) => list.iter().any(|r| r.eq_ignore_ascii_case(rule)),
        }
    };

    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };
    let mut visit_ids = Vec::new();
    for r in roots {
        doc.walk(r, |n, depth| {
            if depth > 0 {
                visit_ids.push(n.id);
            }
        });
    }

    let mut changes = Vec::new();
    let mut to_remove = Vec::new();

    for id in visit_ids {
        let role = doc.get(id).map(|n| infer_role_hint(doc, n)).unwrap_or("container");
        let is_empty_leaf = doc.get(id).is_some_and(|n| {
            matches!(n.kind, NodeKind::Frame(_) | NodeKind::Group)
                && n.children.is_empty()
                && n.fills.iter().all(|p| !p.visible)
                && n.stroke.paints.iter().all(|p| !p.visible)
                && n.effects.iter().all(|e| !e.visible())
        });
        if allow("empty-container") && is_empty_leaf {
            to_remove.push(id);
            changes.push(json!({
                "rule": "empty-container",
                "node_id": id.0,
                "action": "removed empty container",
            }));
            continue;
        }

        // Fix misaligned siblings inside non-auto-layout containers
        if allow("misaligned-siblings") {
            let children =
                doc.get(id).filter(|n| n.kind.frame_props().is_some_and(|f| f.auto_layout.is_none())).map(|n| n.children.clone()).unwrap_or_default();
            for i in 0..children.len() {
                for j in (i + 1)..children.len() {
                    let (Some(a), Some(b)) = (doc.get(children[i]).cloned(), doc.get(children[j]).cloned()) else {
                        continue;
                    };
                    let dx = (a.transform.tx - b.transform.tx).abs();
                    let dy = (a.transform.ty - b.transform.ty).abs();
                    if dx > 0.1 && dx <= 3.0 {
                        if let Some(bm) = doc.get_mut(children[j]) {
                            bm.transform.tx = a.transform.tx;
                            changes.push(json!({
                                "rule": "misaligned-siblings",
                                "node_id": children[j].0,
                                "action": format!("snapped x to {}", a.transform.tx),
                            }));
                        }
                    }
                    if dy > 0.1 && dy <= 3.0 {
                        if let Some(bm) = doc.get_mut(children[j]) {
                            bm.transform.ty = a.transform.ty;
                            changes.push(json!({
                                "rule": "misaligned-siblings",
                                "node_id": children[j].0,
                                "action": format!("snapped y to {}", a.transform.ty),
                            }));
                        }
                    }
                }
            }
        }

        let Some(n) = doc.get_mut(id) else { continue };

        if allow("default-layer-name") && is_default_layer_name(&n.name) {
            let new_name = match &n.kind {
                NodeKind::Text(t) => {
                    let snippet: String = t.content.trim().chars().take(24).collect();
                    if snippet.is_empty() {
                        "Text Label".to_string()
                    } else {
                        snippet
                    }
                }
                _ => match role {
                    "button" => "Button".to_string(),
                    "input" => "Input Field".to_string(),
                    "card" => "Card".to_string(),
                    "navbar" => "Navigation Bar".to_string(),
                    "badge" => "Badge".to_string(),
                    "icon" => "Icon".to_string(),
                    _ => match n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
                        Some(al) => match al.direction {
                            LayoutDirection::Horizontal => "Row".to_string(),
                            LayoutDirection::Vertical => "Column".to_string(),
                            LayoutDirection::Grid => "Grid".to_string(),
                        },
                        None => format!("{} Layer", xpath_type_name(&n.kind).to_ascii_lowercase()),
                    },
                },
            };
            n.name = new_name.clone();
            changes.push(json!({
                "rule": "default-layer-name",
                "node_id": id.0,
                "action": format!("renamed to '{new_name}'"),
            }));
        }

        if allow("tiny-text") {
            if let Some(t) = n.kind.any_text_mut() {
                if t.style.font_size < 12.0 {
                    t.style.font_size = 12.0;
                    changes.push(json!({
                        "rule": "tiny-text",
                        "node_id": id.0,
                        "action": "bumped font_size to 12px",
                    }));
                }
            }
        }

        if allow("non-scale-font-size") {
            if let Some(t) = n.kind.any_text_mut() {
                if !on_scale(t.style.font_size, &FONT_SCALE) {
                    let snapped = nearest_on_scale(t.style.font_size, &FONT_SCALE);
                    t.style.font_size = snapped;
                    changes.push(json!({
                        "rule": "non-scale-font-size",
                        "node_id": id.0,
                        "action": format!("snapped font_size to {snapped}px"),
                    }));
                }
            }
        }

        if allow("small-touch-target") && matches!(role, "button" | "input") {
            if n.size.x < 44.0 || n.size.y < 44.0 {
                n.size.x = n.size.x.max(44.0);
                n.size.y = n.size.y.max(44.0);
                changes.push(json!({
                    "rule": "small-touch-target",
                    "node_id": id.0,
                    "action": format!("expanded touch target to {}x{}", n.size.x, n.size.y),
                }));
            }
        }

        if allow("non-4px-grid") && !matches!(n.kind, NodeKind::Text(_) | NodeKind::Vector { .. } | NodeKind::Line) {
            let sx = (n.size.x / 4.0).round().max(1.0) * 4.0;
            let sy = (n.size.y / 4.0).round().max(1.0) * 4.0;
            if (sx - n.size.x).abs() >= 0.05 || (sy - n.size.y).abs() >= 0.05 {
                n.size.x = sx;
                n.size.y = sy;
                changes.push(json!({
                    "rule": "non-4px-grid",
                    "node_id": id.0,
                    "action": format!("snapped size to {sx}x{sy}"),
                }));
            }
        }

        if allow("inconsistent-radius") && n.corner_radii != [0.0; 4] {
            let r0 = nearest_on_scale(n.corner_radii[0], &RADIUS_SCALE);
            if n.corner_radii != [r0; 4] {
                n.corner_radii = [r0; 4];
                changes.push(json!({
                    "rule": "inconsistent-radius",
                    "node_id": id.0,
                    "action": format!("normalized corner_radii to {r0}px"),
                }));
            }
        }

        if let Some(fp) = n.kind.frame_props_mut() {
            if let Some(al) = &mut fp.auto_layout {
                if allow("inconsistent-spacing") && !on_scale(al.item_spacing, &SPACING_SCALE) {
                    let snapped = nearest_on_scale(al.item_spacing, &SPACING_SCALE);
                    al.item_spacing = snapped;
                    changes.push(json!({
                        "rule": "inconsistent-spacing",
                        "node_id": id.0,
                        "action": format!("snapped item_spacing to {snapped}px"),
                    }));
                }
                if allow("unbalanced-padding") && ((al.padding[0] - al.padding[2]).abs() > 0.1 || (al.padding[1] - al.padding[3]).abs() > 0.1) {
                    let v = nearest_on_scale((al.padding[0] + al.padding[2]) * 0.5, &SPACING_SCALE);
                    let h = nearest_on_scale((al.padding[1] + al.padding[3]) * 0.5, &SPACING_SCALE);
                    al.padding = [v, h, v, h];
                    changes.push(json!({
                        "rule": "unbalanced-padding",
                        "node_id": id.0,
                        "action": format!("balanced padding to [{v}, {h}, {v}, {h}]"),
                    }));
                }
            }
        }
    }

    for rem_id in to_remove {
        let _ = doc.remove(rem_id);
    }
    if !changes.is_empty() {
        doc.mark_full_dirty();
    }

    json!({
        "fixed_count": changes.len(),
        "changes": changes,
    })
}

fn srgb_to_lab(c: Color) -> [f32; 3] {
    let lin = |v: f32| -> f32 {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = lin(c.r);
    let g = lin(c.g);
    let b = lin(c.b);
    let x = (r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.95047;
    let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
    let z = (r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.08883;
    let f = |t: f32| -> f32 {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let fx = f(x);
    let fy = f(y);
    let fz = f(z);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn delta_e(a: Color, b: Color) -> f32 {
    let la = srgb_to_lab(a);
    let lb = srgb_to_lab(b);
    let dl = la[0] - lb[0];
    let da = la[1] - lb[1];
    let db = la[2] - lb[2];
    (dl * dl + da * da + db * db).sqrt()
}

/// Clusters fill & stroke colors by usage count, bound variable status, and CIE76 Delta-E similarity.
pub fn analyze_colors(doc: &Document, page: Option<NodeId>) -> Value {
    #[derive(Default)]
    struct ColorEntry {
        color: Option<Color>,
        count: usize,
        bound_count: usize,
        node_ids: Vec<u64>,
    }

    let mut map: BTreeMap<String, ColorEntry> = BTreeMap::new();
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };

    for root in roots {
        doc.walk(root, |n, depth| {
            if depth == 0 {
                return;
            }
            let is_bound = !n.variables.bindings.is_empty() || !n.style_refs.is_empty();
            let mut paints: Vec<&Paint> = n.fills.iter().chain(n.stroke.paints.iter()).collect();
            if let Some(t) = n.kind.any_text() {
                paints.extend(t.style.fills.iter());
            }
            for p in paints {
                if !p.visible {
                    continue;
                }
                if let PaintKind::Solid(c) = p.kind {
                    let hex = format!("#{}", c.to_hex());
                    let entry = map.entry(hex).or_default();
                    entry.color = Some(c);
                    entry.count += 1;
                    if is_bound {
                        entry.bound_count += 1;
                    }
                    if entry.node_ids.len() < 16 && !entry.node_ids.contains(&n.id.0) {
                        entry.node_ids.push(n.id.0);
                    }
                }
            }
        });
    }

    let mut list: Vec<(String, ColorEntry)> = map.into_iter().collect();
    list.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));

    // Cluster similar colors with Delta-E < 10.0
    let mut visited = vec![false; list.len()];
    let mut clusters = Vec::new();
    for i in 0..list.len() {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        let ci = list[i].1.color.unwrap_or(Color::BLACK);
        let mut members = vec![list[i].0.clone()];
        for j in (i + 1)..list.len() {
            if visited[j] {
                continue;
            }
            let cj = list[j].1.color.unwrap_or(Color::BLACK);
            if delta_e(ci, cj) < 10.0 {
                visited[j] = true;
                members.push(list[j].0.clone());
            }
        }
        clusters.push(json!({
            "primary": list[i].0,
            "variants": members,
        }));
    }

    let total_uses: usize = list.iter().map(|(_, e)| e.count).sum();
    let colors_json: Vec<Value> = list
        .iter()
        .map(|(hex, e)| {
            json!({
                "hex": hex,
                "count": e.count,
                "bound_count": e.bound_count,
                "hardcoded_count": e.count.saturating_sub(e.bound_count),
                "node_ids": e.node_ids,
            })
        })
        .collect();

    json!({
        "total_uses": total_uses,
        "unique_colors": colors_json.len(),
        "colors": colors_json,
        "clusters": clusters,
    })
}

/// Groups `(font_family, font_size, font_weight, line_height)` styles across text nodes.
pub fn analyze_typography(doc: &Document, page: Option<NodeId>) -> Value {
    #[derive(Default)]
    struct TypoEntry {
        font_family: String,
        font_size: f32,
        font_weight: u16,
        line_height: String,
        count: usize,
        sample: String,
        node_ids: Vec<u64>,
    }

    let mut map: BTreeMap<String, TypoEntry> = BTreeMap::new();
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };
    let mut total_text_nodes = 0usize;

    for root in roots {
        doc.walk(root, |n, _| {
            let Some(t) = n.kind.any_text() else { return };
            total_text_nodes += 1;
            let lh = match t.style.line_height {
                LineHeight::Auto => "auto".to_string(),
                LineHeight::Pixels(px) => format!("{px}px"),
                LineHeight::Percent(pct) => format!("{pct}%"),
            };
            let key = format!("{}|{}|{}|{}", t.style.font_family, t.style.font_size, t.style.font_weight, lh);
            let e = map.entry(key).or_insert_with(|| TypoEntry {
                font_family: t.style.font_family.clone(),
                font_size: t.style.font_size,
                font_weight: t.style.font_weight,
                line_height: lh,
                count: 0,
                sample: t.content.chars().take(32).collect(),
                node_ids: Vec::new(),
            });
            e.count += 1;
            if e.node_ids.len() < 16 {
                e.node_ids.push(n.id.0);
            }
        });
    }

    let mut styles: Vec<TypoEntry> = map.into_values().collect();
    styles.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| b.font_size.total_cmp(&a.font_size)));

    let styles_json: Vec<Value> = styles
        .into_iter()
        .map(|s| {
            json!({
                "font_family": s.font_family,
                "font_size": s.font_size,
                "font_weight": s.font_weight,
                "line_height": s.line_height,
                "count": s.count,
                "sample": s.sample,
                "node_ids": s.node_ids,
            })
        })
        .collect();

    json!({
        "total_text_nodes": total_text_nodes,
        "unique_styles": styles_json.len(),
        "styles": styles_json,
    })
}

/// Computes a histogram of spacing/padding values and 4px/8px grid compliance percentages.
pub fn analyze_spacing(doc: &Document, page: Option<NodeId>) -> Value {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };
    let mut total = 0usize;
    let mut on_4px = 0usize;
    let mut on_8px = 0usize;

    let mut record = |v: f32| {
        if v <= 0.0 {
            return;
        }
        total += 1;
        if is_grid_multiple(v, 4.0) {
            on_4px += 1;
        }
        if is_grid_multiple(v, 8.0) {
            on_8px += 1;
        }
        *hist.entry(v.round() as i32).or_insert(0) += 1;
    };

    for root in roots {
        doc.walk(root, |n, _| {
            if let Some(al) = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
                record(al.item_spacing);
                record(al.counter_spacing);
                for &p in &al.padding {
                    record(p);
                }
            }
        });
    }

    let values: Vec<Value> = hist
        .into_iter()
        .map(|(px, count)| {
            json!({
                "value": px,
                "count": count,
                "on_4px_grid": px % 4 == 0,
                "on_8px_grid": px % 8 == 0,
            })
        })
        .collect();

    let pct_4 = if total == 0 { 100.0 } else { (on_4px as f64 / total as f64) * 100.0 };
    let pct_8 = if total == 0 { 100.0 } else { (on_8px as f64 / total as f64) * 100.0 };

    json!({
        "total_samples": total,
        "grid_4px_compliance_pct": (pct_4 * 10.0).round() / 10.0,
        "grid_8px_compliance_pct": (pct_8 * 10.0).round() / 10.0,
        "values": values,
    })
}

/// Groups structurally similar subtrees by `(kind, child_count, layout_mode, approx_size, child_kinds)` to identify reusable component candidates.
pub fn analyze_clusters(doc: &Document, page: Option<NodeId>) -> Value {
    struct ClusterGroup {
        signature: String,
        kind: String,
        child_count: usize,
        layout_mode: String,
        approx_size: (i32, i32),
        node_ids: Vec<u64>,
        names: Vec<String>,
    }

    let mut groups: BTreeMap<String, ClusterGroup> = BTreeMap::new();
    let roots: Vec<NodeId> = match page {
        Some(p) => vec![p],
        None => doc.pages.clone(),
    };

    for root in roots {
        doc.walk(root, |n, depth| {
            if depth == 0 || n.children.is_empty() {
                return;
            }
            let kind = xpath_type_name(&n.kind).to_string();
            let lm = layout_mode_str(n).to_string();
            let approx_w = ((n.size.x / 16.0).round() as i32) * 16;
            let approx_h = ((n.size.y / 16.0).round() as i32) * 16;
            let child_kinds: Vec<&str> = n.children.iter().filter_map(|&cid| doc.get(cid).map(|cn| xpath_type_name(&cn.kind))).collect();
            let sig = format!("{}|{}|{}|{}x{}|{}", kind, n.children.len(), lm, approx_w, approx_h, child_kinds.join(","));
            let g = groups.entry(sig.clone()).or_insert_with(|| ClusterGroup {
                signature: sig,
                kind,
                child_count: n.children.len(),
                layout_mode: lm,
                approx_size: (approx_w, approx_h),
                node_ids: Vec::new(),
                names: Vec::new(),
            });
            g.node_ids.push(n.id.0);
            if g.names.len() < 8 {
                g.names.push(n.name.clone());
            }
        });
    }

    let mut clusters: Vec<Value> = groups
        .into_values()
        .filter(|g| g.node_ids.len() >= 2)
        .map(|g| {
            json!({
                "signature": g.signature,
                "kind": g.kind,
                "child_count": g.child_count,
                "layout_mode": g.layout_mode,
                "approx_width": g.approx_size.0,
                "approx_height": g.approx_size.1,
                "instance_count": g.node_ids.len(),
                "node_ids": g.node_ids,
                "sample_names": g.names,
            })
        })
        .collect();
    clusters.sort_by(|a, b| b["instance_count"].as_u64().unwrap_or(0).cmp(&a["instance_count"].as_u64().unwrap_or(0)));

    json!({
        "cluster_count": clusters.len(),
        "clusters": clusters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_page_default_background_is_5a5a5a() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let Some(NodeKind::Page { background }) = doc.get(page).map(|n| &n.kind) else {
            panic!("expected Page node");
        };
        assert_eq!(background.to_hex(), "5A5A5A");
        assert_eq!(*background, Color::rgb(0x5A, 0x5A, 0x5A));
    }

    #[test]
    fn xpath_queries_and_node_to_xpath() {
        let mut doc = Document::new("XPath Test");
        let page = doc.add_page("Page 1");

        let f1 = doc.alloc_id();
        let mut frame1 = Node::frame(f1, Rect::new(0.0, 0.0, 240.0, 120.0));
        frame1.name = "Card".into();
        doc.insert(page, frame1).unwrap();

        let f2 = doc.alloc_id();
        let mut frame2 = Node::frame(f2, Rect::new(300.0, 0.0, 480.0, 200.0));
        frame2.name = "Hero".into();
        doc.insert(page, frame2).unwrap();

        let f_inner = doc.alloc_id();
        let inner_id = f_inner;
        let mut inner = Node::frame(f_inner, Rect::new(16.0, 16.0, 200.0, 80.0));
        inner.name = "Header Wrap".into();
        doc.insert(f1, inner).unwrap();

        let t1 = doc.alloc_id();
        let mut text1 = Node::text(t1, Rect::new(0.0, 0.0, 180.0, 24.0), "Welcome Title", TextStyle::default());
        text1.name = "Main Title".into();
        doc.insert(inner_id, text1).unwrap();

        assert_eq!(node_to_xpath(&doc, t1), "/FRAME[1]/FRAME[1]/TEXT[1]");

        let small_frames = query_xpath(&doc, Some(page), "//FRAME[@width < 300]", 50).unwrap();
        let ids: Vec<u64> = small_frames.iter().map(|v| v["id"].as_u64().unwrap()).collect();
        assert_eq!(ids, vec![f1.0, inner_id.0]);

        let titles = query_xpath(&doc, Some(page), "//TEXT[contains(@name, 'Title')]", 50).unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0]["id"].as_u64().unwrap(), t1.0);

        let exact = query_xpath(&doc, Some(page), "/FRAME[1]/FRAME[1]/TEXT[1]", 10).unwrap();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0]["id"].as_u64().unwrap(), t1.0);

        let desc = describe_node(&doc, f1, 2).unwrap();
        assert_eq!(desc["type"], "FRAME");
        assert_eq!(desc["children"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn lint_and_fix_and_analyzers_work() {
        let mut doc = Document::new("Lint Test");
        let page = doc.add_page("Page 1");

        // Frame with default name, non-4px size, off-scale spacing, unbalanced padding
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 123.0, 77.0));
        frame.name = "Frame 1".into();
        if let NodeKind::Frame(fp) = &mut frame.kind {
            fp.auto_layout =
                Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 11.0, padding: [10.0, 16.0, 14.0, 8.0], ..Default::default() });
        }
        doc.insert(page, frame).unwrap();

        // Tiny low-contrast text child
        let tid = doc.alloc_id();
        let mut style = TextStyle::default();
        style.font_size = 9.0;
        style.fills = vec![Paint::solid(Color::rgb(0xEE, 0xEE, 0xEE))]; // low contrast on white frame
        let mut txt = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 16.0), "Tiny Label", style);
        txt.name = "Text 1".into();
        doc.insert(fid, txt).unwrap();

        // Empty container
        let empty_id = doc.alloc_id();
        let mut empty = Node::frame(empty_id, Rect::new(200.0, 0.0, 40.0, 40.0));
        empty.fills.clear();
        doc.insert(page, empty).unwrap();

        // Duplicate card structures for cluster detection
        for i in 0..2 {
            let cid = doc.alloc_id();
            let mut card = Node::frame(cid, Rect::new(i as f32 * 220.0, 200.0, 200.0, 120.0));
            card.name = format!("Card {}", i + 1);
            doc.insert(page, card).unwrap();
            let r = doc.alloc_id();
            doc.insert(cid, Node::rectangle(r, Rect::new(8.0, 8.0, 40.0, 40.0))).unwrap();
        }

        let report = lint_document(&doc, Some(page), "recommended");
        assert!(report["issue_count"].as_u64().unwrap() >= 6);
        let rules_hit = report["summary_by_rule"].as_object().unwrap();
        assert!(rules_hit.contains_key("default-layer-name"));
        assert!(rules_hit.contains_key("tiny-text"));
        assert!(rules_hit.contains_key("low-contrast-text"));
        assert!(rules_hit.contains_key("empty-container"));

        let fix_res = lint_fix_document(&mut doc, Some(page), None);
        assert!(fix_res["fixed_count"].as_u64().unwrap() >= 4);
        assert!(doc.get(empty_id).is_none(), "empty container should be removed");
        assert!(doc.get(tid).unwrap().kind.any_text().unwrap().style.font_size >= 12.0);
        assert_ne!(doc.get(fid).unwrap().name, "Frame 1");

        let colors = analyze_colors(&doc, Some(page));
        assert!(colors["unique_colors"].as_u64().unwrap() >= 1);

        let typo = analyze_typography(&doc, Some(page));
        assert_eq!(typo["total_text_nodes"].as_u64().unwrap(), 1);

        let spacing = analyze_spacing(&doc, Some(page));
        assert!(spacing["total_samples"].as_u64().unwrap() >= 1);

        let clusters = analyze_clusters(&doc, Some(page));
        assert!(clusters["cluster_count"].as_u64().unwrap() >= 1);
    }
}
