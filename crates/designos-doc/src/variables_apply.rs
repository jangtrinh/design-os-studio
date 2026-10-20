//! Applies variable bindings to nodes: substitutes the value a bound variable
//! resolves to (in the mode in effect at the node) into the node property the
//! binding names. Pure helpers; the document itself keeps the raw values.
//!
//! Binding keys follow Figma's `boundVariables`: `fills/N`, `strokes/N`,
//! `width`, `height`, `topLeftRadius`..., `itemSpacing`, `counterAxisSpacing`,
//! `paddingLeft`..., `opacity`, `visible`, `characters`, `fontSize`,
//! `fontWeight`, `lineHeight`, `letterSpacing`, `strokeWeight` (and the
//! per-side `strokeTopWeight`...). Opacity variables hold percent (0..=100),
//! as in Figma; line height is in pixels.
//!
//! A paint or effect colour can also bind an opacity variable
//! (`fills/N/opacity`, `strokes/N/opacity`, `effects/N/opacity`, scope
//! `COLOR_OPACITY`); the variable value (percent) becomes that paint's or
//! effect colour's opacity, see [`set_bound_property`].

use crate::*;
use std::borrow::Cow;

/// Kind of variable a binding key accepts, or `None` for unknown keys.
pub fn property_kind(key: &str) -> Option<VariableKind> {
    Some(match key {
        k if paint_opacity_key(k).is_some() => VariableKind::Number,
        k if k.starts_with("fills/") || k.starts_with("strokes/") => VariableKind::Color,
        "width" | "height" | "topLeftRadius" | "topRightRadius" | "bottomLeftRadius" | "bottomRightRadius" | "itemSpacing" | "counterAxisSpacing"
        | "paddingLeft" | "paddingRight" | "paddingTop" | "paddingBottom" | "opacity" | "fontSize" | "fontWeight" | "lineHeight" | "letterSpacing"
        | "strokeWeight" | "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => VariableKind::Number,
        "visible" => VariableKind::Boolean,
        "characters" => VariableKind::String,
        _ => return None,
    })
}

/// Whether a variable with these Figma `scopes` may be bound to `key` on a
/// node of `kind`. An empty list (or `ALL_SCOPES`) allows everything.
pub fn scope_allows(scopes: &[String], key: &str, kind: &NodeKind) -> bool {
    if scopes.is_empty() || scopes.iter().any(|s| s == "ALL_SCOPES") {
        return true;
    }
    let has = |names: &[&str]| scopes.iter().any(|s| names.contains(&s.as_str()));
    match key {
        k if paint_opacity_key(k).is_some() => has(&["COLOR_OPACITY"]),
        k if k.starts_with("fills/") => {
            let specific = match kind {
                NodeKind::Text(_) => "TEXT_FILL",
                NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section => "FRAME_FILL",
                _ => "SHAPE_FILL",
            };
            has(&["ALL_FILLS", specific])
        }
        k if k.starts_with("strokes/") => has(&["STROKE_COLOR"]),
        "width" | "height" => has(&["WIDTH_HEIGHT"]),
        "topLeftRadius" | "topRightRadius" | "bottomLeftRadius" | "bottomRightRadius" => has(&["CORNER_RADIUS"]),
        "itemSpacing" | "counterAxisSpacing" | "paddingLeft" | "paddingRight" | "paddingTop" | "paddingBottom" => has(&["GAP"]),
        "opacity" => has(&["OPACITY"]),
        "strokeWeight" | "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => has(&["STROKE_FLOAT"]),
        "fontSize" => has(&["FONT_SIZE"]),
        "fontWeight" => has(&["FONT_WEIGHT"]),
        "lineHeight" => has(&["LINE_HEIGHT"]),
        "letterSpacing" => has(&["LETTER_SPACING"]),
        "characters" => has(&["TEXT_CONTENT"]),
        "visible" => true,
        _ => false,
    }
}

/// Binding keys that make sense for a node (what the inspector can bind).
pub fn bindable_keys(kind: &NodeKind) -> Vec<&'static str> {
    let mut v = vec!["width", "height", "opacity", "visible", "fills/0", "strokes/0", "strokeWeight"];
    if !matches!(kind, NodeKind::Text(_)) {
        v.extend(["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"]);
    }
    if kind.frame_props().is_some_and(|f| f.auto_layout.is_some()) {
        v.extend(["itemSpacing", "counterAxisSpacing", "paddingLeft", "paddingRight", "paddingTop", "paddingBottom"]);
    }
    if matches!(kind, NodeKind::Text(_)) {
        v.extend(["characters", "fontSize", "fontWeight", "lineHeight", "letterSpacing"]);
    }
    v
}

/// Which list a composed-opacity binding key (`fills/N/opacity`, `strokes/N/opacity`,
/// `effects/N/opacity`) addresses, and the index into it.
enum OpacityTarget {
    Fill(usize),
    Stroke(usize),
    Effect(usize),
}

fn paint_opacity_key(key: &str) -> Option<OpacityTarget> {
    let (list, rest) = key.split_once('/')?;
    let index: usize = rest.strip_suffix("/opacity")?.parse().ok()?;
    match list {
        "fills" => Some(OpacityTarget::Fill(index)),
        "strokes" => Some(OpacityTarget::Stroke(index)),
        "effects" => Some(OpacityTarget::Effect(index)),
        _ => None,
    }
}

/// Sets the opacity of the paint or effect colour a composed-opacity key addresses to
/// `percent` / 100, clamped to 0..=1. A missing target is ignored.
///
/// Replace, not multiply: the bound variable value IS the opacity. This runs on stored nodes
/// (bind, variable edit, detach bake) as well as on render-time copies, so it must be idempotent;
/// a multiply would compound with the value already written on every edit.
fn compose_opacity(n: &mut Node, target: OpacityTarget, percent: f32) {
    let value = (percent / 100.0).clamp(0.0, 1.0);
    match target {
        OpacityTarget::Fill(i) => {
            let paints = match &mut n.kind {
                NodeKind::Text(t) => &mut t.style.fills,
                _ => &mut n.fills,
            };
            if let Some(p) = paints.get_mut(i) {
                p.opacity = value;
            }
        }
        OpacityTarget::Stroke(i) => {
            if let Some(p) = n.stroke.paints.get_mut(i) {
                p.opacity = value;
            }
        }
        OpacityTarget::Effect(i) => {
            if let Some(Effect::DropShadow { color, .. } | Effect::InnerShadow { color, .. }) = n.effects.get_mut(i) {
                color.a = value;
            }
        }
    }
}

fn color_of(v: &VariableValue) -> Option<Color> {
    match v {
        VariableValue::Color(c) => Some(*c),
        _ => None,
    }
}

fn number_of(v: &VariableValue) -> Option<f32> {
    match v {
        VariableValue::Number(n) if n.is_finite() => Some(*n),
        _ => None,
    }
}

fn paint_at<'a>(paints: &'a mut [Paint], key: &str, prefix: &str) -> Option<&'a mut Paint> {
    let i: usize = key.strip_prefix(prefix)?.parse().ok()?;
    paints.get_mut(i)
}

/// Writes `value` into the node property named by `key`; mismatched types and
/// unknown keys are ignored.
pub fn set_bound_property(n: &mut Node, key: &str, value: &VariableValue) {
    if let Some(target) = paint_opacity_key(key) {
        if let Some(x) = number_of(value) {
            compose_opacity(n, target, x);
        }
        return;
    }
    if key.starts_with("fills/") {
        let Some(c) = color_of(value) else { return };
        // Text is painted from its runs: the bytes a run covers take the run's fills, not the
        // base's. A run follows the binding while it still has the base's colour; a run the author
        // coloured differently (a highlighted word) keeps its own.
        if let (NodeKind::Text(t), Some(i)) = (&mut n.kind, key.strip_prefix("fills/").and_then(|s| s.parse::<usize>().ok())) {
            if let Some(old) = t.style.fills.get(i).map(|p| p.kind.clone()) {
                for p in t.runs.iter_mut().filter_map(|r| r.style.fills.get_mut(i)).filter(|p| p.kind == old) {
                    p.kind = PaintKind::Solid(c);
                    if c.a < 1.0 {
                        p.opacity = 1.0;
                    }
                }
            }
        }
        let paints = match &mut n.kind {
            NodeKind::Text(t) => &mut t.style.fills,
            _ => &mut n.fills,
        };
        if let Some(p) = paint_at(paints, key, "fills/") {
            p.kind = PaintKind::Solid(c);
            if c.a < 1.0 {
                p.opacity = 1.0;
            }
        }
        return;
    }
    if key.starts_with("strokes/") {
        let Some(c) = color_of(value) else { return };
        if let Some(p) = paint_at(&mut n.stroke.paints, key, "strokes/") {
            p.kind = PaintKind::Solid(c);
            if c.a < 1.0 {
                p.opacity = 1.0;
            }
        }
        return;
    }
    if key == "visible" {
        if let VariableValue::Boolean(b) = value {
            n.visible = *b;
        }
        return;
    }
    if key == "characters" {
        if let (VariableValue::String(s), NodeKind::Text(t)) = (value, &mut n.kind) {
            t.content = s.clone();
            t.runs.clear();
        }
        return;
    }
    let Some(x) = number_of(value) else { return };
    let is_rect_like = matches!(
        n.kind,
        NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section | NodeKind::Rectangle
    );
    match key {
        "width" => {
            n.size.x = x.max(0.0);
            if is_rect_like {
                n.fill_geometry = None;
                n.stroke_geometry = None;
            }
        }
        "height" => {
            n.size.y = x.max(0.0);
            if is_rect_like {
                n.fill_geometry = None;
                n.stroke_geometry = None;
            }
        }
        "opacity" => n.opacity = (x / 100.0).clamp(0.0, 1.0),
        "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" => {
            let i = match key {
                "topLeftRadius" => 0,
                "topRightRadius" => 1,
                "bottomRightRadius" => 2,
                _ => 3,
            };
            n.corner_radii[i] = x.max(0.0);
            if is_rect_like {
                n.fill_geometry = None;
                n.stroke_geometry = None;
            }
        }
        "strokeWeight" => {
            n.stroke.weight = x.max(0.0);
            n.stroke_geometry = None;
        }
        "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => {
            let w = n.stroke.weight;
            let sw = n.stroke.side_weights.get_or_insert([w; 4]);
            let i = match key {
                "strokeTopWeight" => 0,
                "strokeRightWeight" => 1,
                "strokeBottomWeight" => 2,
                _ => 3,
            };
            sw[i] = x.max(0.0);
            n.stroke_geometry = None;
        }
        "itemSpacing" | "counterAxisSpacing" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" => {
            if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                let x = x.max(0.0);
                match key {
                    "itemSpacing" => al.item_spacing = x,
                    "counterAxisSpacing" => al.counter_spacing = x,
                    "paddingTop" => al.padding[0] = x,
                    "paddingRight" => al.padding[1] = x,
                    "paddingBottom" => al.padding[2] = x,
                    _ => al.padding[3] = x,
                }
            }
        }
        "fontSize" | "fontWeight" | "lineHeight" | "letterSpacing" => {
            if let NodeKind::Text(t) = &mut n.kind {
                let apply = |s: &mut TextStyle| match key {
                    "fontSize" => s.font_size = x.max(1.0),
                    "fontWeight" => s.font_weight = x.round().clamp(1.0, 1000.0) as u16,
                    "lineHeight" => s.line_height = LineHeight::Pixels(x.max(0.0)),
                    _ => s.letter_spacing = x,
                };
                apply(&mut t.style);
                for r in &mut t.runs {
                    apply(&mut r.style);
                }
            }
        }
        _ => {}
    }
}

/// The node with its variable bindings applied. Borrowed when it has none.
pub fn resolve_node<'a>(doc: &Document, node: &'a Node) -> Cow<'a, Node> {
    if node.variables.bindings.is_empty() || doc.variables.is_empty() {
        return Cow::Borrowed(node);
    }
    let mut out: Option<Node> = None;
    for (key, var) in &node.variables.bindings {
        if let Some(v) = doc.resolve_variable(node.id, var) {
            set_bound_property(out.get_or_insert_with(|| node.clone()), key, &v);
        }
    }
    match out {
        Some(n) => Cow::Owned(n),
        None => Cow::Borrowed(node),
    }
}

/// [`resolve_node`] by id.
pub fn resolved_node(doc: &Document, id: NodeId) -> Option<Cow<'_, Node>> {
    doc.get(id).map(|n| resolve_node(doc, n))
}

/// A copy of the document with every binding substituted (for export and
/// tools that read raw node properties).
pub fn apply_bound_variables(doc: &Document) -> Document {
    let mut out = doc.clone();
    for (id, n) in &doc.nodes {
        if !n.variables.bindings.is_empty() {
            if let Cow::Owned(r) = resolve_node(doc, n) {
                out.nodes.insert(*id, r);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
        Variable {
            id: id.into(),
            name: id.into(),
            collection: "c".into(),
            kind,
            values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        }
    }

    fn setup() -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
        f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout::default()), ..Default::default() });
        let frame = d.insert(page, f).unwrap();
        let tid = d.alloc_id();
        let text = d.insert(frame, Node::text(tid, Rect::new(0.0, 0.0, 50.0, 20.0), "hi", TextStyle::default())).unwrap();
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "C".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec![],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let n = |x: f32| VariableValue::Number(x);
        let vars = [
            var("bg", VariableKind::Color, VariableValue::Color(Color::WHITE), VariableValue::Color(Color::BLACK)),
            var("gap", VariableKind::Number, n(8.0), n(16.0)),
            var("w", VariableKind::Number, n(120.0), n(240.0)),
            var("op", VariableKind::Number, n(50.0), n(25.0)),
            var("size", VariableKind::Number, n(14.0), n(20.0)),
            var("txt", VariableKind::String, VariableValue::String("Light".into()), VariableValue::String("Dark".into())),
            var("show", VariableKind::Boolean, VariableValue::Boolean(true), VariableValue::Boolean(false)),
        ];
        for v in vars {
            d.variables.insert(v.id.clone(), v);
        }
        let fr = d.get_mut(frame).unwrap();
        for (k, v) in [
            ("fills/0", "bg"),
            ("itemSpacing", "gap"),
            ("paddingLeft", "gap"),
            ("width", "w"),
            ("opacity", "op"),
            ("topLeftRadius", "gap"),
            ("strokeWeight", "gap"),
        ] {
            fr.variables.bindings.insert(k.into(), v.into());
        }
        let tx = d.get_mut(text).unwrap();
        for (k, v) in [("fontSize", "size"), ("characters", "txt"), ("visible", "show"), ("fills/0", "bg"), ("letterSpacing", "gap"), ("fontWeight", "w")] {
            tx.variables.bindings.insert(k.into(), v.into());
        }
        (d, page, frame, text)
    }

    #[test]
    fn substitutes_values_in_default_mode() {
        let (d, _, frame, text) = setup();
        let f = resolved_node(&d, frame).unwrap();
        assert_eq!(f.fills[0].kind, PaintKind::Solid(Color::WHITE));
        assert_eq!(f.size.x, 120.0);
        assert_eq!(f.opacity, 0.5);
        assert_eq!(f.corner_radii[0], 8.0);
        assert_eq!(f.stroke.weight, 8.0);
        let al = f.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!((al.item_spacing, al.padding[3]), (8.0, 8.0));
        let t = resolved_node(&d, text).unwrap();
        let NodeKind::Text(tp) = &t.kind else { panic!() };
        assert_eq!(tp.style.font_size, 14.0);
        assert_eq!(tp.content, "Light");
        assert_eq!(tp.style.letter_spacing, 8.0);
        assert_eq!(tp.style.font_weight, 120);
        assert_eq!(tp.style.fills[0].kind, PaintKind::Solid(Color::WHITE));
        assert!(t.visible);
        // raw node is untouched
        assert_eq!(d.get(frame).unwrap().size.x, 100.0);
    }

    #[test]
    fn mode_on_ancestor_switches_values() {
        let (mut d, _, frame, text) = setup();
        d.get_mut(frame).unwrap().variables.modes.insert("c".into(), "dark".into());
        let f = resolved_node(&d, frame).unwrap();
        assert_eq!(f.fills[0].kind, PaintKind::Solid(Color::BLACK));
        assert_eq!(f.size.x, 240.0);
        let t = resolved_node(&d, text).unwrap();
        let NodeKind::Text(tp) = &t.kind else { panic!() };
        assert_eq!(tp.content, "Dark");
        assert!(!t.visible);
    }

    #[test]
    fn unbound_nodes_are_borrowed_and_bad_bindings_ignored() {
        let (mut d, page, frame, _) = setup();
        assert!(matches!(resolved_node(&d, page).unwrap(), Cow::Borrowed(_)));
        d.get_mut(frame).unwrap().variables.bindings.insert("fills/5".into(), "bg".into());
        d.get_mut(frame).unwrap().variables.bindings.insert("width".into(), "bg".into()); // colour into a number slot
        d.get_mut(frame).unwrap().variables.bindings.insert("height".into(), "missing".into());
        let f = resolved_node(&d, frame).unwrap();
        assert_eq!(f.size.y, 100.0);
        assert_eq!(f.size.x, 100.0);
    }

    #[test]
    fn apply_bound_variables_copies_document() {
        let (mut d, _, frame, _) = setup();
        d.get_mut(frame).unwrap().variables.modes.insert("c".into(), "dark".into());
        let r = apply_bound_variables(&d);
        assert_eq!(r.get(frame).unwrap().size.x, 240.0);
        assert_eq!(d.get(frame).unwrap().size.x, 100.0);
    }

    fn composed_setup() -> (Document, NodeId) {
        let (mut d, _, frame, _) = setup();
        let teal = Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 };
        d.variables.insert("brand".into(), var("brand", VariableKind::Color, VariableValue::Color(teal), VariableValue::Color(teal)));
        let mut fade = var("fade", VariableKind::Number, VariableValue::Number(50.0), VariableValue::Number(25.0));
        fade.scopes = vec!["COLOR_OPACITY".into()];
        d.variables.insert("fade".into(), fade);
        let n = d.get_mut(frame).unwrap();
        n.variables.bindings.clear();
        n.fills = vec![Paint { opacity: 0.8, ..Paint::solid(Color::WHITE) }];
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        }];
        for (k, v) in [("fills/0", "brand"), ("fills/0/opacity", "fade"), ("strokes/0/opacity", "fade"), ("effects/0/opacity", "fade")] {
            n.variables.bindings.insert(k.into(), v.into());
        }
        (d, frame)
    }

    #[test]
    fn color_and_opacity_variables_compose_into_one_rgba() {
        let (d, frame) = composed_setup();
        let f = resolved_node(&d, frame).unwrap();
        // colour from the colour variable, paint opacity = the variable (50 %), not 0.8 x 50 %
        assert_eq!(f.fills[0].kind, PaintKind::Solid(Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 }));
        assert!((f.fills[0].opacity - 0.5).abs() < 1e-6, "fill opacity {}", f.fills[0].opacity);
        // an opacity variable on an unbound colour sets its opacity
        assert_eq!(f.stroke.paints[0].kind, PaintKind::Solid(Color::BLACK));
        assert!((f.stroke.paints[0].opacity - 0.5).abs() < 1e-6);
        // effect colour alpha = the variable (50 %), not 0.5 x 50 %
        let Effect::DropShadow { color, .. } = &f.effects[0] else { panic!() };
        assert!((color.a - 0.5).abs() < 1e-6, "shadow alpha {}", color.a);
        assert_eq!((color.r, color.g, color.b), (0.0, 0.0, 0.0));
        // the raw node is untouched
        assert_eq!(d.get(frame).unwrap().fills[0].opacity, 0.8);
    }

    #[test]
    fn opacity_variable_follows_the_mode_and_clamps() {
        let (mut d, frame) = composed_setup();
        d.get_mut(frame).unwrap().variables.modes.insert("c".into(), "dark".into());
        let f = resolved_node(&d, frame).unwrap();
        assert!((f.fills[0].opacity - 0.25).abs() < 1e-6, "dark mode 25 %");
        d.variables.get_mut("fade").unwrap().values.insert("dark".into(), VariableValue::Number(900.0));
        assert_eq!(resolved_node(&d, frame).unwrap().fills[0].opacity, 1.0);
    }

    #[test]
    fn applying_a_resolved_opacity_twice_equals_applying_it_once() {
        let (d, frame) = composed_setup();
        let once = resolved_node(&d, frame).unwrap().into_owned();
        // re-applying the same bindings on already-resolved nodes must not compound
        let mut twice = once.clone();
        let fade = VariableValue::Number(50.0);
        for key in ["fills/0/opacity", "strokes/0/opacity", "effects/0/opacity"] {
            set_bound_property(&mut twice, key, &fade);
        }
        assert_eq!(twice.fills[0].opacity, once.fills[0].opacity);
        assert_eq!(twice.stroke.paints[0].opacity, once.stroke.paints[0].opacity);
        assert_eq!(twice.effects, once.effects);
        // and the document-level copy is a fixed point
        let doc_once = apply_bound_variables(&d);
        let doc_twice = apply_bound_variables(&doc_once);
        assert_eq!(doc_twice.get(frame).unwrap().fills, doc_once.get(frame).unwrap().fills);
        assert_eq!(doc_twice.get(frame).unwrap().effects, doc_once.get(frame).unwrap().effects);
    }

    #[test]
    fn opacity_binding_needs_a_number_and_a_paint_that_exists() {
        let (mut d, frame) = composed_setup();
        let n = d.get_mut(frame).unwrap();
        n.variables.bindings.insert("fills/0/opacity".into(), "brand".into()); // colour into a number slot
        n.variables.bindings.insert("fills/7/opacity".into(), "fade".into()); // no such paint
        let f = resolved_node(&d, frame).unwrap();
        assert_eq!(f.fills[0].opacity, 0.8);
        assert_eq!(property_kind("fills/0/opacity"), Some(VariableKind::Number));
        assert_eq!(property_kind("effects/1/opacity"), Some(VariableKind::Number));
    }

    #[test]
    fn color_opacity_scope_gates_paint_opacity_bindings() {
        let frame = NodeKind::Frame(FrameProps::default());
        let only = |s: &str| vec![s.to_string()];
        assert!(scope_allows(&only("COLOR_OPACITY"), "fills/0/opacity", &frame));
        assert!(scope_allows(&only("COLOR_OPACITY"), "strokes/0/opacity", &frame));
        assert!(scope_allows(&only("COLOR_OPACITY"), "effects/0/opacity", &frame));
        assert!(!scope_allows(&only("OPACITY"), "fills/0/opacity", &frame));
        assert!(!scope_allows(&only("COLOR_OPACITY"), "opacity", &frame));
        assert!(scope_allows(&[], "fills/0/opacity", &frame));
    }

    #[test]
    fn kinds_and_scopes() {
        assert_eq!(property_kind("fills/2"), Some(VariableKind::Color));
        assert_eq!(property_kind("paddingTop"), Some(VariableKind::Number));
        assert_eq!(property_kind("nope"), None);
        let frame = NodeKind::Frame(FrameProps::default());
        let text = NodeKind::Text(TextProps {
            content: String::new(),
            style: TextStyle::default(),
            runs: vec![],
            auto_resize: TextAutoResize::None,
            max_lines: None,
            ..Default::default()
        });
        assert!(scope_allows(&[], "width", &frame));
        assert!(scope_allows(&["GAP".into()], "itemSpacing", &frame));
        assert!(!scope_allows(&["GAP".into()], "width", &frame));
        assert!(scope_allows(&["FRAME_FILL".into()], "fills/0", &frame));
        assert!(!scope_allows(&["FRAME_FILL".into()], "fills/0", &text));
        assert!(scope_allows(&["TEXT_FILL".into()], "fills/0", &text));
        assert!(scope_allows(&["ALL_FILLS".into()], "fills/0", &text));
    }

    #[test]
    fn color_variable_with_alpha_does_not_multiply_stored_paint_opacity() {
        let (mut d, _, frame, _) = setup();
        let alpha_white_20 = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 };
        d.variables.insert(
            "alpha-white-20".into(),
            var("alpha-white-20", VariableKind::Color, VariableValue::Color(alpha_white_20), VariableValue::Color(alpha_white_20)),
        );
        let n = d.get_mut(frame).unwrap();
        n.variables.bindings.clear();
        n.fills = vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }];
        n.stroke.paints = vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }];
        n.variables.bindings.insert("fills/0".into(), "alpha-white-20".into());
        n.variables.bindings.insert("strokes/0".into(), "alpha-white-20".into());

        let r = resolved_node(&d, frame).unwrap();
        let PaintKind::Solid(fc) = r.fills[0].kind else { panic!() };
        assert!((fc.a * r.fills[0].opacity - 0.2).abs() < 1e-6, "fill effective alpha was {}", fc.a * r.fills[0].opacity);
        let PaintKind::Solid(sc) = r.stroke.paints[0].kind else { panic!() };
        assert!((sc.a * r.stroke.paints[0].opacity - 0.2).abs() < 1e-6, "stroke effective alpha was {}", sc.a * r.stroke.paints[0].opacity);
    }

    fn text_with_runs(base: Color, runs: &[Color]) -> Node {
        let style = |c| TextStyle { fills: vec![Paint::solid(c)], ..TextStyle::default() };
        let mut n = Node::text(NodeId(1), Rect::new(0.0, 0.0, 10.0, 10.0), "abc", style(base));
        if let NodeKind::Text(t) = &mut n.kind {
            t.runs = runs.iter().enumerate().map(|(i, c)| TextRun { start: i, end: i + 1, style: style(*c) }).collect();
        }
        n
    }

    /// The base fill, then the fill of each run, as the renderer would read them.
    fn fills(n: &Node) -> Vec<Color> {
        let NodeKind::Text(t) = &n.kind else { return vec![] };
        let solid = |ps: &[Paint]| match ps.first().map(|p| &p.kind) {
            Some(PaintKind::Solid(c)) => *c,
            _ => Color::BLACK,
        };
        std::iter::once(solid(&t.style.fills)).chain(t.runs.iter().map(|r| solid(&r.style.fills))).collect()
    }

    #[test]
    fn text_fill_binding_reaches_the_runs_that_match_the_base_and_keeps_the_others() {
        let (blue, red) = (Color::rgb(26, 115, 232), Color::rgb(229, 72, 77));
        let mut n = text_with_runs(blue, &[blue, red, blue]);
        set_bound_property(&mut n, "fills/0", &VariableValue::Color(Color::WHITE));
        assert_eq!(fills(&n), vec![Color::WHITE, Color::WHITE, red, Color::WHITE], "the highlighted run keeps its colour");
        // A paint index the base lacks changes nothing.
        let mut n = text_with_runs(blue, &[blue, red]);
        set_bound_property(&mut n, "fills/1", &VariableValue::Color(Color::WHITE));
        assert_eq!(fills(&n), vec![blue, blue, red]);
    }
}
