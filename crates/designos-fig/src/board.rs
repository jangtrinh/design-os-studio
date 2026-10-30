//! FigJam (board) node types in REST JSON: `STICKY`, `SHAPE_WITH_TEXT`, `CONNECTOR`,
//! `STAMP` and the `editorType: "figjam"` file marker, mapped to and from the board
//! kinds of the document model. `SECTION` needs no special handling.

use crate::export::{n, paints, v2};
use crate::rest_import::{farr, fbool, fnum, fstr, parse_paints, vec2};
use designos_doc::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Whether a file (or the node tree under it) comes from FigJam.
pub(crate) fn is_figjam(root: &Value) -> bool {
    fstr(root, "editorType").is_some_and(|t| t.eq_ignore_ascii_case("figjam"))
}

fn ink() -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x1E))]
}

/// The text of a board node: `characters` (or `text.characters`) and, when present, the type style.
fn board_text_props(v: &Value, base: TextProps) -> TextProps {
    let mut t = base;
    let chars = fstr(v, "characters").or_else(|| v.get("text").and_then(|x| fstr(x, "characters")));
    if let Some(c) = chars {
        t.content = c.to_string();
    }
    if let Some(st) = v.get("style").or_else(|| v.get("text").and_then(|x| x.get("style"))) {
        if let Some(fs) = fnum(st, "fontSize").filter(|f| *f > 0.0) {
            t.style.font_size = fs;
        }
        if let Some(f) = fstr(st, "fontFamily") {
            t.style.font_family = f.to_string();
        }
        if let Some(w) = fnum(st, "fontWeight") {
            t.style.set_font_weight(w as u16);
        }
        match fstr(st, "textAlignHorizontal") {
            Some("CENTER") => t.style.align_h = TextAlignH::Center,
            Some("RIGHT") => t.style.align_h = TextAlignH::Right,
            Some("LEFT") => t.style.align_h = TextAlignH::Left,
            _ => {}
        }
        match fstr(st, "textAlignVertical") {
            Some("CENTER") => t.style.align_v = TextAlignV::Center,
            Some("BOTTOM") => t.style.align_v = TextAlignV::Bottom,
            Some("TOP") => t.style.align_v = TextAlignV::Top,
            _ => {}
        }
    }
    t
}

fn shape_from_name(s: &str) -> ShapeKind {
    match s {
        "ROUNDED_RECTANGLE" | "SPEECH_BUBBLE" | "ENG_FOLDER" | "ENG_FILE" | "DOCUMENT_SINGLE" | "DOCUMENT_MULTIPLE" => ShapeKind::RoundedRect,
        "ELLIPSE" | "ENG_DATABASE" | "ENG_QUEUE" | "SUMMING_JUNCTION" | "OR" => ShapeKind::Ellipse,
        "DIAMOND" | "SHIELD" => ShapeKind::Diamond,
        "TRIANGLE_UP" => ShapeKind::Triangle,
        "TRIANGLE_DOWN" => ShapeKind::TriangleDown,
        "PARALLELOGRAM_RIGHT" | "PARALLELOGRAM_LEFT" | "PARALLELOGRAM" | "TRAPEZOID" | "MANUAL_INPUT" => ShapeKind::Parallelogram,
        "STAR" | "PLUS" => ShapeKind::Star,
        "HEXAGON" | "OCTAGON" | "CHEVRON" | "ARROW_LEFT" | "ARROW_RIGHT" => ShapeKind::Hexagon,
        "PENTAGON" => ShapeKind::Pentagon,
        // SQUARE, RECTANGLE, PREDEFINED_PROCESS, INTERNAL_STORAGE and every unknown shape.
        _ => ShapeKind::Square,
    }
}

fn shape_name(k: ShapeKind) -> &'static str {
    match k {
        ShapeKind::Square => "SQUARE",
        ShapeKind::RoundedRect => "ROUNDED_RECTANGLE",
        ShapeKind::Ellipse => "ELLIPSE",
        ShapeKind::Diamond => "DIAMOND",
        ShapeKind::Triangle => "TRIANGLE_UP",
        ShapeKind::TriangleDown => "TRIANGLE_DOWN",
        ShapeKind::Parallelogram => "PARALLELOGRAM_RIGHT",
        ShapeKind::Star => "STAR",
        ShapeKind::Hexagon => "HEXAGON",
        ShapeKind::Pentagon => "PENTAGON",
    }
}

fn cap_from_name(s: Option<&str>) -> ConnectorCap {
    match s {
        Some("LINE_ARROW") | Some("ARROW_LINES") => ConnectorCap::Arrow,
        Some("TRIANGLE_ARROW") | Some("TRIANGLE_FILLED") | Some("ARROW_EQUILATERAL") => ConnectorCap::Triangle,
        Some("CIRCLE_FILLED") => ConnectorCap::Circle,
        Some("DIAMOND_FILLED") => ConnectorCap::Diamond,
        _ => ConnectorCap::None,
    }
}

fn cap_name(c: ConnectorCap) -> &'static str {
    match c {
        ConnectorCap::None => "NONE",
        ConnectorCap::Arrow => "LINE_ARROW",
        ConnectorCap::Triangle => "TRIANGLE_FILLED",
        ConnectorCap::Circle => "CIRCLE_FILLED",
        ConnectorCap::Diamond => "DIAMOND_FILLED",
    }
}

fn magnet_from_name(s: Option<&str>) -> Magnet {
    match s {
        Some("TOP") => Magnet::Top,
        Some("RIGHT") => Magnet::Right,
        Some("BOTTOM") => Magnet::Bottom,
        Some("LEFT") => Magnet::Left,
        Some("CENTER") => Magnet::Center,
        _ => Magnet::Auto,
    }
}

fn magnet_name(m: Magnet) -> &'static str {
    match m {
        Magnet::Auto => "AUTO",
        Magnet::Top => "TOP",
        Magnet::Right => "RIGHT",
        Magnet::Bottom => "BOTTOM",
        Magnet::Left => "LEFT",
        Magnet::Center => "CENTER",
    }
}

/// Stamp kind from the REST node: an explicit `stampType` wins, otherwise the layer name.
fn stamp_kind(v: &Value) -> String {
    let hint = fstr(v, "stampType").or_else(|| fstr(v, "stampSubType")).or_else(|| fstr(v, "name")).unwrap_or("").to_ascii_lowercase();
    let known = [
        ("heart", "heart"),
        ("star", "star"),
        ("question", "question"),
        ("thumb", "thumbs_up"),
        ("like", "thumbs_up"),
        ("plus", "plus_one"),
        ("+1", "plus_one"),
    ];
    known.iter().find(|(k, _)| hint.contains(k)).map(|(_, k)| (*k).to_string()).unwrap_or_else(|| "thumbs_up".to_string())
}

/// Board node kind for a REST `type`, or `None` for every other type. `origin` is the
/// page-space position of the parent, to turn absolute connector points into parent space.
pub(crate) fn kind_from_json(ty: &str, v: &Value, origin: Vec2) -> Option<NodeKind> {
    Some(match ty {
        "STICKY" => {
            let mut p = StickyProps::default();
            if let Some(c) = parse_paints(v.get("fills")).iter().find_map(|f| match &f.kind {
                PaintKind::Solid(c) if f.visible => Some(*c),
                _ => None,
            }) {
                p.color = c.with_alpha(1.0);
            }
            p.text = board_text_props(v, p.text);
            p.show_author = fbool(v, "authorVisible").unwrap_or(false);
            p.author = fstr(v, "authorName").map(String::from).filter(|s| !s.is_empty());
            NodeKind::Sticky(p)
        }
        "SHAPE_WITH_TEXT" => {
            let shape_str = fstr(v, "shapeType").or_else(|| fstr(v, "shapeWithTextType"));
            let mut p = ShapeWithTextProps { shape: shape_str.map(shape_from_name).unwrap_or_default(), ..Default::default() };
            p.text = board_text_props(v, p.text);
            NodeKind::ShapeWithText(p)
        }
        "STAMP" => NodeKind::Stamp { kind: stamp_kind(v) },
        "CONNECTOR" => {
            let end = |key: &str| -> ConnectorEnd {
                let e = v.get(key);
                let pos = e.and_then(|e| e.get("position")).and_then(vec2).unwrap_or(Vec2::ZERO);
                ConnectorEnd {
                    node: None,
                    magnet: magnet_from_name(e.and_then(|e| fstr(e, "magnet"))),
                    // free ends are absolute; the link pass replaces attached ones
                    point: Vec2::new(pos.x - origin.x, pos.y - origin.y),
                }
            };
            let mut p = ConnectorProps {
                start: end("connectorStart"),
                end: end("connectorEnd"),
                line: match fstr(v, "connectorLineType") {
                    Some("STRAIGHT") => ConnectorLine::Straight,
                    Some("CURVED") => ConnectorLine::Curved,
                    _ => ConnectorLine::Elbowed,
                },
                start_cap: cap_from_name(fstr(v, "connectorStartStrokeCap")),
                end_cap: cap_from_name(fstr(v, "connectorEndStrokeCap")),
                label: None,
            };
            let label = board_text_props(v, board_text(14.0, TextAlignH::Center, TextAlignV::Center));
            if !label.content.is_empty() {
                p.label = Some(label);
            }
            NodeKind::Connector(p)
        }
        _ => return None,
    })
}

/// Fix-ups after the generic node fields were parsed.
pub(crate) fn finish_node(n: &mut Node) {
    match &mut n.kind {
        NodeKind::Sticky(_) => {
            // the sticky color lives in its props
            n.fills.clear();
            n.stroke.paints.clear();
            n.stroke.weight = 0.0;
        }
        NodeKind::Connector(_) => {
            if n.stroke.paints.is_empty() {
                n.stroke.paints = ink();
            }
            if n.stroke.weight <= 0.0 {
                n.stroke.weight = 2.0;
            }
            n.stroke.align = StrokeAlign::Center;
            n.stroke.join = StrokeJoin::Round;
            n.stroke.cap = StrokeCap::Round;
        }
        NodeKind::ShapeWithText(_) => {
            if n.stroke.paints.is_empty() {
                n.stroke.paints = ink();
                n.stroke.weight = n.stroke.weight.max(2.0);
                n.stroke.align = StrokeAlign::Center;
            }
        }
        _ => {}
    }
}

pub(crate) fn link_connector_list(doc: &mut Document, ids: &BTreeMap<String, NodeId>, connectors: &[(NodeId, Option<String>, Option<String>)]) {
    for (cid, start_ep, end_ep) in connectors {
        let a = start_ep.as_deref().and_then(|s| ids.get(s)).copied();
        let b = end_ep.as_deref().and_then(|s| ids.get(s)).copied();
        if let Some(NodeKind::Connector(c)) = doc.get_mut(*cid).map(|n| &mut n.kind) {
            c.start.node = a;
            c.end.node = b;
        }
        sync_connector(doc, *cid);
    }
}

/// Attaches connector ends to the nodes they name (`endpointNodeId` is a REST id) and
/// places every connector on its route. Walks the REST tree below `root`.
pub(crate) fn link_connectors(doc: &mut Document, ids: &BTreeMap<String, NodeId>, root: &Value) {
    fn rec(doc: &mut Document, ids: &BTreeMap<String, NodeId>, v: &Value, out: &mut Vec<NodeId>) {
        if fstr(v, "type") == Some("CONNECTOR") {
            if let Some(&cid) = fstr(v, "id").and_then(|s| ids.get(s)) {
                let target = |key: &str| v.get(key).and_then(|e| fstr(e, "endpointNodeId")).and_then(|s| ids.get(s)).copied();
                let (a, b) = (target("connectorStart"), target("connectorEnd"));
                if let Some(NodeKind::Connector(c)) = doc.get_mut(cid).map(|n| &mut n.kind) {
                    c.start.node = a;
                    c.end.node = b;
                }
                out.push(cid);
            }
        }
        for c in farr(v, "children") {
            rec(doc, ids, c, out);
        }
    }
    let mut connectors = Vec::new();
    rec(doc, ids, root, &mut connectors);
    for c in connectors {
        sync_connector(doc, c);
    }
}

/// REST `type` of a board kind.
pub(crate) fn type_name(k: &NodeKind) -> Option<&'static str> {
    match k {
        NodeKind::Sticky(_) => Some("STICKY"),
        NodeKind::Connector(_) => Some("CONNECTOR"),
        NodeKind::ShapeWithText(_) => Some("SHAPE_WITH_TEXT"),
        NodeKind::Stamp { .. } => Some("STAMP"),
        _ => None,
    }
}

fn text_fields(m: &mut Map<String, Value>, t: &TextProps) {
    m.insert("characters".into(), t.content.clone().into());
    let s = &t.style;
    m.insert(
        "style".into(),
        json!({
            "fontFamily": s.font_family,
            "fontWeight": s.font_weight,
            "fontSize": n(s.font_size),
            "textAlignHorizontal": match s.align_h { TextAlignH::Left => "LEFT", TextAlignH::Center => "CENTER", TextAlignH::Right => "RIGHT", TextAlignH::Justified => "JUSTIFIED" },
            "textAlignVertical": match s.align_v { TextAlignV::Top => "TOP", TextAlignV::Center => "CENTER", TextAlignV::Bottom => "BOTTOM" },
        }),
    );
}

/// Board-specific fields of an exported node. `origin` is the page-space position of the
/// parent (connector points are written in page space); `id_of` maps a node to its REST id.
pub(crate) fn export_fields(node: &Node, origin: Vec2, m: &mut Map<String, Value>, id_of: &dyn Fn(NodeId) -> String) {
    match &node.kind {
        NodeKind::Sticky(s) => {
            m.insert("fills".into(), paints(&[Paint::solid(s.color)]));
            m.insert("authorVisible".into(), s.show_author.into());
            if let Some(a) = &s.author {
                m.insert("authorName".into(), a.clone().into());
            }
            text_fields(m, &s.text);
        }
        NodeKind::ShapeWithText(s) => {
            m.insert("shapeType".into(), shape_name(s.shape).into());
            text_fields(m, &s.text);
        }
        NodeKind::Stamp { kind } => {
            m.insert("stampType".into(), kind.clone().into());
        }
        NodeKind::Connector(c) => {
            let end = |e: &ConnectorEnd| {
                let mut o = Map::new();
                if let Some(t) = e.node {
                    o.insert("endpointNodeId".into(), id_of(t).into());
                }
                o.insert("position".into(), v2(Vec2::new(e.point.x + origin.x, e.point.y + origin.y)));
                o.insert("magnet".into(), magnet_name(e.magnet).into());
                Value::Object(o)
            };
            m.insert("connectorStart".into(), end(&c.start));
            m.insert("connectorEnd".into(), end(&c.end));
            m.insert(
                "connectorLineType".into(),
                match c.line {
                    ConnectorLine::Elbowed => "ELBOWED",
                    ConnectorLine::Straight => "STRAIGHT",
                    ConnectorLine::Curved => "CURVED",
                }
                .into(),
            );
            m.insert("connectorStartStrokeCap".into(), cap_name(c.start_cap).into());
            m.insert("connectorEndStrokeCap".into(), cap_name(c.end_cap).into());
            if let Some(l) = &c.label {
                text_fields(m, l);
            }
        }
        _ => {}
    }
}
