//! Effect and grid styles as pure document functions.
//!
//! A shared style lives in [`Document::styles`] (values) and
//! [`Document::style_meta`] (name, library key, description, property). A node
//! is linked to it through `style_refs` (`"effect"` / `"grid"`) and carries its
//! own copy of the values, so a detached node keeps its look.
//!
//! Like [`crate::components`] and [`crate::library_ops`], every function
//! returns a [`Command`] and leaves the document alone, so the caller decides
//! how the change joins the undo stack.

use crate::*;
use std::collections::BTreeSet;

/// Why an effect / grid style operation was refused.
#[derive(Debug, thiserror::Error)]
pub enum StyleOpError {
    #[error(transparent)]
    Doc(#[from] DocError),
    #[error("style {0:?} does not exist")]
    UnknownStyle(String),
    #[error("{0:?} is not an effect or grid style")]
    NotEffectOrGrid(StyleProperty),
    #[error("node {0:?} cannot carry that kind of style")]
    WrongNodeKind(NodeId),
    #[error("node {0:?} has no values to turn into a style")]
    NothingToStyle(NodeId),
    #[error("a style name cannot be empty")]
    EmptyName,
    #[error("style {0:?} holds a different kind of values than the ones given")]
    KindMismatch(String),
    #[error("style {0:?} belongs to a library; edit it in the library file")]
    LibraryStyle(String),
    #[error("variable {0:?} does not exist")]
    UnknownVariable(String),
    #[error("property {0:?} cannot be bound on this style")]
    InvalidProperty(String),
}

/// Kind of variable a binding key on `style` accepts, or `None` if `property` is invalid for `style`.
pub fn style_property_kind(style: &Style, property: &str) -> Option<VariableKind> {
    let kind = property_kind(property)?;
    let valid = match style {
        Style::Fill(_) => property.starts_with("fills/"),
        Style::Text(_) => matches!(
            property,
            "fontSize" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent" | "fontWeight" | "fontFamily" | "fontStyle"
        ),
        Style::Effect(_) => property.starts_with("effects/"),
        Style::Grid(_) => false,
    };
    valid.then_some(kind)
}

/// Key of the `style_refs` entry that links a node to a style of `prop`.
pub fn style_ref_key(prop: StyleProperty) -> Option<&'static str> {
    match prop {
        StyleProperty::Effect => Some("effect"),
        StyleProperty::Grid => Some("grid"),
        _ => None,
    }
}

fn effect_or_grid_ref(prop: StyleProperty) -> Result<&'static str, StyleOpError> {
    style_ref_key(prop).ok_or(StyleOpError::NotEffectOrGrid(prop))
}

/// First free key `"<base> <n>"` in `doc.styles`.
pub fn unique_style_key(doc: &Document, base: &str) -> String {
    (1..).map(|n| format!("{base} {n}")).find(|k| !doc.styles.contains_key(k)).unwrap_or_else(|| base.to_string())
}

/// Links `n` to style `key` and copies the style's values onto it. Returns
/// false (node untouched) when `n` cannot carry the style (a grid on a node
/// that is not a frame).
fn link(n: &mut Node, key: &str, style: &Style, prop: StyleProperty, ref_key: &str) -> bool {
    if prop == StyleProperty::Grid && n.kind.frame_props().is_none() {
        return false;
    }
    n.style_refs.insert(ref_key.into(), key.into());
    apply_style_values(n, style, key);
    true
}

/// The node's current values as a style of `prop`, or why it has none to share.
fn values_of_node(n: &Node, prop: StyleProperty) -> Result<Style, StyleOpError> {
    match prop {
        StyleProperty::Effect if n.effects.is_empty() => Err(StyleOpError::NothingToStyle(n.id)),
        StyleProperty::Effect => Ok(Style::Effect(n.effects.clone())),
        StyleProperty::Grid => match n.kind.frame_props() {
            Some(f) if !f.grids.is_empty() => Ok(Style::Grid(f.grids.clone())),
            Some(_) => Err(StyleOpError::NothingToStyle(n.id)),
            None => Err(StyleOpError::WrongNodeKind(n.id)),
        },
        other => Err(StyleOpError::NotEffectOrGrid(other)),
    }
}

fn get_node(doc: &Document, id: NodeId) -> Result<&Node, StyleOpError> {
    Ok(doc.get(id).ok_or(DocError::NotFound(id))?)
}

fn updated(n: &Node, f: impl FnOnce(&mut Node)) -> Command {
    let mut n = n.clone();
    f(&mut n);
    Command::Update { node: Box::new(n) }
}

/// Creates a style of `prop` (effect or grid) from the values of `node`, links
/// the node to it and returns the new key with the command. `name` defaults to
/// "Effect style N" / "Grid style N"; "/" in it groups styles into folders.
pub fn create_style_from_node(doc: &Document, node: NodeId, prop: StyleProperty, name: Option<&str>) -> Result<(String, Command), StyleOpError> {
    let ref_key = effect_or_grid_ref(prop)?;
    let n = get_node(doc, node)?;
    let style = values_of_node(n, prop)?;
    let name = match name.map(str::trim) {
        Some("") => return Err(StyleOpError::EmptyName),
        Some(given) => given.to_string(),
        None => unique_style_key(doc, if prop == StyleProperty::Effect { "Effect style" } else { "Grid style" }),
    };
    let key = if doc.styles.contains_key(&name) { unique_style_key(doc, &name) } else { name.clone() };
    let mut styles = doc.styles.clone();
    let mut style_meta = doc.style_meta.clone();
    styles.insert(key.clone(), style);
    style_meta.insert(key.clone(), StyleMeta { name, key: None, description: String::new(), property: prop, bindings: Default::default() });
    let link = updated(n, |n| {
        n.style_refs.insert(ref_key.into(), key.clone());
    });
    Ok((key.clone(), Command::Batch(vec![Command::SetStyles { styles, style_meta }, link])))
}

/// Links every node of `nodes` to style `key`, copying its values (grid
/// styles skip nodes that are not frames).
pub fn apply_style(doc: &Document, nodes: &[NodeId], key: &str) -> Result<Command, StyleOpError> {
    let style = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    let prop = style_property(doc, key);
    let ref_key = effect_or_grid_ref(prop)?;
    let mut cmds = Vec::with_capacity(nodes.len());
    for id in nodes {
        let n = get_node(doc, *id)?;
        let mut copy = n.clone();
        if link(&mut copy, key, style, prop, ref_key) {
            cmds.push(Command::Update { node: Box::new(copy) });
        }
    }
    Ok(Command::Batch(cmds))
}

/// The `style_refs` entries through which a node can be linked to a style of this kind. The kind
/// of the values decides, so metadata naming another property (a text style filed under the
/// default `Fill`) cannot hide the linked nodes. A colour style can be linked as a fill, as a
/// stroke, or both.
fn ref_keys(style: &Style) -> &'static [&'static str] {
    match style {
        Style::Fill(_) => &["fill", "stroke"],
        Style::Text(_) => &["text"],
        Style::Effect(_) => &["effect"],
        Style::Grid(_) => &["grid"],
    }
}

/// Nodes of this document (excluding copies on the hidden library page) that are linked to style
/// `key` through a `style_refs` slot matching the style's kind.
pub fn linked_style_nodes(doc: &Document, key: &str) -> Vec<NodeId> {
    let Some(stored) = doc.styles.get(key) else {
        return Vec::new();
    };
    let refs = ref_keys(stored);
    doc.nodes.values().filter(|n| refs.iter().any(|r| n.style_refs.get(*r).is_some_and(|k| k == key)) && !is_library_node(doc, n.id)).map(|n| n.id).collect()
}

/// Replaces the values of style `key` and pushes them to every linked node, as one command (one
/// undo step). Works for fill, stroke, text, effect and grid styles; `style` must be the same
/// kind as the stored one. A style published by a library is refused: it is edited in its library
/// file and reaches this one through a library update.
pub fn update_style(doc: &Document, key: &str, style: Style) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    if std::mem::discriminant(stored) != std::mem::discriminant(&style) {
        return Err(StyleOpError::KindMismatch(key.into()));
    }
    let mut styles = doc.styles.clone();
    styles.insert(key.to_string(), style.clone());
    let mut cmds = vec![Command::SetStyles { styles, style_meta: doc.style_meta.clone() }];
    for id in linked_style_nodes(doc, key) {
        if let Some(n) = doc.get(id) {
            let mut copy = n.clone();
            if apply_style_values(&mut copy, &style, key) {
                cmds.push(Command::Update { node: Box::new(copy) });
            }
        }
    }
    Ok(Command::Batch(cmds))
}

/// Binds `property` of style `key` to `variable`. Validates that the style
/// exists, the variable exists, and the variable's kind matches the property
/// (colour variable for `fills/0`, number variable for `fontSize`, etc.).
pub fn bind_style_property(doc: &Document, key: &str, property: &str, variable: &str) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let expected_kind = style_property_kind(stored, property).ok_or_else(|| StyleOpError::InvalidProperty(property.into()))?;
    let var = doc.variables.get(variable).ok_or_else(|| StyleOpError::UnknownVariable(variable.into()))?;
    if var.kind != expected_kind {
        return Err(StyleOpError::KindMismatch(key.into()));
    }
    let mut styles = doc.styles.clone();
    let mut style = stored.clone();
    if let Some(val) = doc.resolve_variable_default(variable) {
        set_bound_style_property(&mut style, property, &val);
        styles.insert(key.to_string(), style.clone());
    }
    let prop_kind = style_property(doc, key);
    let mut style_meta = doc.style_meta.clone();
    let meta = style_meta.entry(key.to_string()).or_insert_with(|| StyleMeta {
        name: key.to_string(),
        key: None,
        description: String::new(),
        property: prop_kind,
        bindings: Default::default(),
    });
    meta.bindings.insert(property.to_string(), variable.to_string());
    let mut cmds = vec![Command::SetStyles { styles, style_meta }];
    for id in linked_style_nodes(doc, key) {
        let Some(n) = doc.get(id) else { continue };
        let mut copy = n.clone();
        if let Some(val) = doc.resolve_variable(id, variable) {
            if n.style_refs.get("stroke").is_some_and(|k| k == key) {
                if let Some(rest) = property.strip_prefix("fills/") {
                    let s_key = format!("strokes/{rest}");
                    if !copy.variables.bindings.contains_key(&s_key) {
                        set_bound_property(&mut copy, &s_key, &val);
                    }
                }
            }
            if n.style_refs.get("fill").is_some_and(|k| k == key)
                || n.style_refs.get("text").is_some_and(|k| k == key)
                || n.style_refs.get("effect").is_some_and(|k| k == key)
            {
                if !copy.variables.bindings.contains_key(property) {
                    set_bound_property(&mut copy, property, &val);
                }
            }
        }
        if copy != *n {
            cmds.push(Command::Update { node: Box::new(copy) });
        }
    }
    Ok(if cmds.len() == 1 { cmds.pop().unwrap() } else { Command::Batch(cmds) })
}

/// Removes the variable binding on `property` of style `key`.
pub fn unbind_style_property(doc: &Document, key: &str, property: &str) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    if style_property_kind(stored, property).is_none() {
        return Err(StyleOpError::InvalidProperty(property.into()));
    }
    let mut style_meta = doc.style_meta.clone();
    if let Some(meta) = style_meta.get_mut(key) {
        meta.bindings.remove(property);
    }
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

/// Drops the `prop` link of every node of `nodes` that has one. The nodes keep
/// their values and the style itself stays in the document.
pub fn detach_style(doc: &Document, nodes: &[NodeId], prop: StyleProperty) -> Result<Command, StyleOpError> {
    let ref_key = effect_or_grid_ref(prop)?;
    let mut cmds = Vec::new();
    for id in nodes.iter().collect::<BTreeSet<_>>() {
        let n = get_node(doc, *id)?;
        if n.style_refs.contains_key(ref_key) {
            cmds.push(updated(n, |n| {
                n.style_refs.remove(ref_key);
            }));
        }
    }
    Ok(Command::Batch(cmds))
}

/// Renames style `key` (display name only; the key and every link stay).
pub fn rename_style(doc: &Document, key: &str, name: &str) -> Result<Command, StyleOpError> {
    if !doc.styles.contains_key(key) {
        return Err(StyleOpError::UnknownStyle(key.into()));
    }
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let property = style_property(doc, key);
    let name = name.trim();
    if name.is_empty() {
        return Err(StyleOpError::EmptyName);
    }
    let mut style_meta = doc.style_meta.clone();
    style_meta.entry(key.to_string()).and_modify(|m| m.name = name.to_string()).or_insert_with(|| StyleMeta {
        name: name.to_string(),
        key: None,
        description: String::new(),
        property,
        bindings: Default::default(),
    });
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shadow(radius: f32) -> Effect {
        Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: Vec2::new(0.0, 4.0), radius, spread: 0.0, visible: true, show_behind: false }
    }

    fn cols(count: u32) -> LayoutGrid {
        LayoutGrid::columns(count, 20.0, 0.0, Color::rgba(255, 0, 0, 26))
    }

    /// A page with a shadowed rectangle `a`, a plain rectangle `b`, a frame `f` with a 12 column grid and a plain frame `g`.
    fn fixture() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for (i, (frame, name)) in [(false, "a"), (false, "b"), (true, "f"), (true, "g")].into_iter().enumerate() {
            let id = d.alloc_id();
            let r = Rect::new(10.0 * i as f32, 0.0, 50.0, 50.0);
            let mut n = if frame { Node::frame(id, r) } else { Node::rectangle(id, r) };
            n.name = name.into();
            ids.push(d.insert(page, n).unwrap());
        }
        let (a, b, f, g) = (ids[0], ids[1], ids[2], ids[3]);
        d.get_mut(a).unwrap().effects = vec![shadow(8.0)];
        d.get_mut(f).unwrap().kind.frame_props_mut().unwrap().grids = vec![cols(12)];
        (d, a, b, f, g)
    }

    fn run(d: &mut Document, h: &mut History, c: Result<Command, StyleOpError>) {
        h.exec(d, c.expect("operation accepted")).unwrap();
    }

    fn grids(d: &Document, id: NodeId) -> Vec<LayoutGrid> {
        d.get(id).unwrap().kind.frame_props().unwrap().grids.clone()
    }

    #[test]
    fn effect_style_is_created_from_a_node_and_undone_in_one_step() {
        let (mut d, a, ..) = fixture();
        let mut h = History::default();
        let (key, cmd) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        assert_eq!(key, "Effect style 1");
        h.exec(&mut d, cmd).unwrap();
        match &d.styles[&key] {
            Style::Effect(fx) => assert_eq!(fx, &vec![shadow(8.0)]),
            other => panic!("expected an effect style, got {other:?}"),
        }
        assert_eq!(d.style_meta[&key], StyleMeta { name: "Effect style 1".into(), key: None, description: String::new(), property: StyleProperty::Effect, bindings: Default::default() });
        assert_eq!(d.get(a).unwrap().style_refs.get("effect"), Some(&key));
        h.undo(&mut d).unwrap();
        assert!(d.styles.is_empty() && d.style_meta.is_empty());
        assert!(d.get(a).unwrap().style_refs.is_empty());
    }

    #[test]
    fn grid_style_is_created_from_a_frame_with_a_chosen_name() {
        let (mut d, _a, _b, f, _g) = fixture();
        let mut h = History::default();
        let (key, cmd) = create_style_from_node(&d, f, StyleProperty::Grid, Some(" Layout/12 col ")).unwrap();
        h.exec(&mut d, cmd).unwrap();
        assert_eq!(key, "Layout/12 col");
        assert_eq!(d.style_name(&key), "Layout/12 col");
        assert!(matches!(&d.styles[&key], Style::Grid(g) if g == &vec![cols(12)]));
        assert_eq!(d.get(f).unwrap().style_refs.get("grid"), Some(&key));
        // A taken name gets a numeric suffix instead of overwriting.
        let (second, _) = create_style_from_node(&d, f, StyleProperty::Grid, Some("Layout/12 col")).unwrap();
        assert_eq!(second, "Layout/12 col 1");
    }

    #[test]
    fn creation_is_refused_without_values_or_on_the_wrong_node() {
        let (d, a, b, f, g) = fixture();
        assert!(matches!(create_style_from_node(&d, b, StyleProperty::Effect, None).unwrap_err(), StyleOpError::NothingToStyle(x) if x == b));
        assert!(matches!(create_style_from_node(&d, g, StyleProperty::Grid, None).unwrap_err(), StyleOpError::NothingToStyle(x) if x == g));
        assert!(matches!(create_style_from_node(&d, a, StyleProperty::Grid, None).unwrap_err(), StyleOpError::WrongNodeKind(x) if x == a));
        assert!(matches!(create_style_from_node(&d, f, StyleProperty::Fill, None).unwrap_err(), StyleOpError::NotEffectOrGrid(StyleProperty::Fill)));
        assert!(matches!(create_style_from_node(&d, a, StyleProperty::Effect, Some("  ")).unwrap_err(), StyleOpError::EmptyName));
        assert!(matches!(create_style_from_node(&d, NodeId(9999), StyleProperty::Effect, None), Err(StyleOpError::Doc(DocError::NotFound(_)))));
    }

    #[test]
    fn applying_links_nodes_and_copies_values() {
        let (mut d, a, b, f, g) = fixture();
        let mut h = History::default();
        let (fx_key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let (grid_key, c) = create_style_from_node(&d, f, StyleProperty::Grid, None).unwrap();
        h.exec(&mut d, c).unwrap();
        // The effect lands on a frame and a rectangle; the grid skips the rectangle.
        let c = apply_style(&d, &[b, g], &fx_key);
        run(&mut d, &mut h, c);
        let c = apply_style(&d, &[b, g], &grid_key);
        run(&mut d, &mut h, c);
        assert_eq!(d.get(b).unwrap().effects, vec![shadow(8.0)]);
        assert_eq!(d.get(b).unwrap().style_refs.get("effect"), Some(&fx_key));
        assert!(!d.get(b).unwrap().style_refs.contains_key("grid"), "a rectangle has no grids");
        assert_eq!(d.get(g).unwrap().style_refs.get("grid"), Some(&grid_key));
        assert_eq!(grids(&d, g), vec![cols(12)]);
        h.undo(&mut d).unwrap();
        assert!(!d.get(g).unwrap().style_refs.contains_key("grid"));
        assert!(grids(&d, g).is_empty());
        assert!(matches!(apply_style(&d, &[b], "nope").unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
    }

    #[test]
    fn editing_a_style_updates_every_linked_node_and_only_those() {
        let (mut d, a, b, f, g) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = apply_style(&d, &[f], &key);
        run(&mut d, &mut h, c);
        let c = update_style(&d, &key, Style::Effect(vec![shadow(24.0), Effect::LayerBlur { radius: 2.0, visible: true }]));
        run(&mut d, &mut h, c);
        let expected = vec![shadow(24.0), Effect::LayerBlur { radius: 2.0, visible: true }];
        assert_eq!(d.get(a).unwrap().effects, expected);
        assert_eq!(d.get(f).unwrap().effects, expected);
        assert!(d.get(b).unwrap().effects.is_empty() && d.get(g).unwrap().effects.is_empty(), "unlinked nodes are untouched");
        // A grid style cannot be overwritten by effect values.
        let (gkey, c) = create_style_from_node(&d, f, StyleProperty::Grid, None).unwrap();
        h.exec(&mut d, c).unwrap();
        assert!(update_style(&d, &gkey, Style::Effect(vec![shadow(1.0)])).is_err());
        let c = update_style(&d, &gkey, Style::Grid(vec![cols(4)]));
        run(&mut d, &mut h, c);
        assert_eq!(grids(&d, f), vec![cols(4)]);
        h.undo(&mut d).unwrap();
        assert_eq!(grids(&d, f), vec![cols(12)]);
    }

    /// Page with a linked rectangle per fill / stroke style, a text layer linked to a text and a
    /// fill style, and an unlinked rectangle. Returns the document and the ids in that order.
    fn paint_and_text_fixture() -> (Document, [NodeId; 4]) {
        let red = Paint::solid(Color::rgb(255, 0, 0));
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for i in 0..4 {
            let id = d.alloc_id();
            let r = Rect::new(60.0 * i as f32, 0.0, 50.0, 50.0);
            let n = if i == 2 {
                Node::text(id, r, "Hi", TextStyle { font_size: 12.0, fills: vec![red.clone()], ..Default::default() })
            } else {
                Node::rectangle(id, r)
            };
            ids.push(d.insert(page, n).unwrap());
        }
        d.styles.insert("Brand".into(), Style::Fill(vec![red.clone()]));
        d.styles.insert("Line".into(), Style::Fill(vec![Paint::solid(Color::rgb(0, 255, 0))]));
        d.style_meta.insert("Line".into(), StyleMeta { name: "Line".into(), key: None, description: String::new(), property: StyleProperty::Stroke, bindings: Default::default() });
        d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 12.0, ..Default::default() }));
        // Metadata that never named the property (it defaults to Fill) must not hide the text layers.
        d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), ..Default::default() });
        for (id, refs) in [(ids[0], vec![("fill", "Brand")]), (ids[1], vec![("stroke", "Line")]), (ids[2], vec![("text", "Body"), ("fill", "Brand")])] {
            let n = d.get_mut(id).unwrap();
            for (k, v) in refs {
                n.style_refs.insert(k.into(), v.into());
            }
        }
        (d, [ids[0], ids[1], ids[2], ids[3]])
    }

    #[test]
    fn editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step() {
        let (mut d, [fill_node, stroke_node, text_node, loose]) = paint_and_text_fixture();
        let mut h = History::default();
        let loose_fills = d.get(loose).unwrap().fills.clone();
        let stroke_before = d.get(stroke_node).unwrap().stroke.paints.clone();
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        assert!(matches!(&d.styles["Brand"], Style::Fill(p) if p == &vec![blue.clone()]));
        assert_eq!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.fills, vec![blue.clone()], "a linked text layer takes the colour"),
            other => panic!("{other:?}"),
        }
        assert_eq!(d.get(loose).unwrap().fills, loose_fills, "unlinked nodes are untouched");

        let white = Paint::solid(Color::rgb(255, 255, 255));
        let c = update_style(&d, "Line", Style::Fill(vec![white.clone()]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, vec![white]);
        assert!(d.get(fill_node).unwrap().fills != d.get(stroke_node).unwrap().stroke.paints);

        let c = update_style(&d, "Body", Style::Text(TextStyle { font_size: 20.0, font_weight: 700, ..Default::default() }));
        run(&mut d, &mut h, c);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => {
                assert_eq!((t.style.font_size, t.style.font_weight), (20.0, 700));
                assert_eq!(t.style.fills, vec![blue.clone()], "a text style carries no colour");
            }
            other => panic!("{other:?}"),
        }
        // One undo step per edit, each taking the style and all of its layers back together.
        h.undo(&mut d).unwrap();
        assert!(matches!(&d.styles["Body"], Style::Text(t) if t.font_size == 12.0));
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.font_size, 12.0),
            other => panic!("{other:?}"),
        }
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, stroke_before);
    }

    #[test]
    fn a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides() {
        let (mut d, [fill_node, stroke_node, text_node, _loose]) = paint_and_text_fixture();
        let mut h = History::default();
        // The shape imports give one style that is used as a fill and as a stroke the property `Fill`.
        d.style_meta.remove("Brand");
        d.get_mut(stroke_node).unwrap().style_refs.clear();
        d.get_mut(stroke_node).unwrap().style_refs.insert("stroke".into(), "Brand".into());
        let both = d.alloc_id();
        let page = d.pages[0];
        let mut n = Node::rectangle(both, Rect::new(0.0, 100.0, 50.0, 50.0));
        n.style_refs.insert("fill".into(), "Brand".into());
        n.style_refs.insert("stroke".into(), "Brand".into());
        d.insert(page, n).unwrap();
        // A copy on the hidden library page is not the file's own layer: it keeps its values.
        let lp = crate::library_ops::ensure_library_page(&mut d);
        let hidden = d.alloc_id();
        let mut n = Node::rectangle(hidden, Rect::new(0.0, 0.0, 5.0, 5.0));
        n.style_refs.insert("fill".into(), "Brand".into());
        d.insert(lp, n).unwrap();
        let hidden_before = d.get(hidden).unwrap().clone();
        let stroke_before = d.get(stroke_node).unwrap().stroke.paints.clone();
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(hidden).unwrap(), &hidden_before);
        assert_eq!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, vec![blue.clone()], "a stroke-only link is updated");
        assert_eq!(d.get(both).unwrap().fills, vec![blue.clone()]);
        assert_eq!(d.get(both).unwrap().stroke.paints, vec![blue.clone()], "one node linked on both sides gets both");
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.fills, vec![blue]),
            other => panic!("{other:?}"),
        }
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, stroke_before);
    }

    #[test]
    fn a_colour_style_edit_recolours_every_run_of_a_rich_text_layer() {
        let (mut d, [_, _, text_node, _]) = paint_and_text_fixture();
        let mut h = History::default();
        let red = Paint::solid(Color::rgb(255, 0, 0));
        let green = Paint::solid(Color::rgb(0, 255, 0));
        if let NodeKind::Text(t) = &mut d.get_mut(text_node).unwrap().kind {
            t.runs = vec![
                TextRun { start: 0, end: 1, style: TextStyle { font_size: 12.0, fills: vec![red.clone()], ..Default::default() } },
                TextRun { start: 1, end: 2, style: TextStyle { font_size: 12.0, fills: vec![green], ..Default::default() } },
            ];
        }
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => {
                assert_eq!(t.style.fills, vec![blue.clone()]);
                assert_eq!(t.runs.len(), 2);
                for r in &t.runs {
                    assert_eq!(r.style.fills, vec![blue.clone()], "run {}..{} keeps its old colour", r.start, r.end);
                    assert_eq!(r.style.font_size, 12.0, "only the colour changes");
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file() {
        let (mut d, [fill_node, ..]) = paint_and_text_fixture();
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        assert!(is_library_style(&d, "Brand"));
        assert!(!is_library_style(&d, "Line"));
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        assert!(matches!(update_style(&d, "Brand", Style::Fill(vec![blue.clone()])).unwrap_err(), StyleOpError::LibraryStyle(k) if k == "Brand"));
        assert_ne!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        // A file's own styles stay editable.
        assert!(update_style(&d, "Line", Style::Fill(vec![blue])).is_ok());
    }

    #[test]
    fn a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file() {
        let (mut d, _) = paint_and_text_fixture();
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        assert!(matches!(rename_style(&d, "Brand", "Brand/Renamed").unwrap_err(), StyleOpError::LibraryStyle(k) if k == "Brand"));
        assert_eq!(d.style_name("Brand"), "Brand");
        // A local style can still be renamed.
        let mut h = History::default();
        let c = rename_style(&d, "Line", "Line/Thin");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_name("Line"), "Line/Thin");
    }

    #[test]
    fn editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused() {
        let (d, _) = paint_and_text_fixture();
        assert!(matches!(update_style(&d, "Brand", Style::Text(TextStyle::default())).unwrap_err(), StyleOpError::KindMismatch(k) if k == "Brand"));
        assert!(matches!(update_style(&d, "Body", Style::Fill(vec![])).unwrap_err(), StyleOpError::KindMismatch(k) if k == "Body"));
        assert!(matches!(update_style(&d, "nope", Style::Fill(vec![])).unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
    }

    #[test]
    fn detaching_drops_the_link_but_keeps_the_values() {
        let (mut d, a, b, ..) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = apply_style(&d, &[b], &key);
        run(&mut d, &mut h, c);
        let c = detach_style(&d, &[a, b, b], StyleProperty::Effect);
        run(&mut d, &mut h, c);
        for id in [a, b] {
            assert!(d.get(id).unwrap().style_refs.is_empty());
            assert_eq!(d.get(id).unwrap().effects, vec![shadow(8.0)]);
        }
        assert!(d.styles.contains_key(&key), "the style itself stays");
        // Editing the style no longer reaches detached nodes.
        let c = update_style(&d, &key, Style::Effect(vec![shadow(1.0)]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(a).unwrap().effects, vec![shadow(8.0)]);
        assert!(detach_style(&d, &[a], StyleProperty::Text).is_err());
    }

    #[test]
    fn renaming_changes_the_display_name_only() {
        let (mut d, a, ..) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = rename_style(&d, &key, " Shadow/Card ");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_name(&key), "Shadow/Card");
        assert_eq!(d.get(a).unwrap().style_refs.get("effect"), Some(&key));
        assert!(matches!(rename_style(&d, &key, "").unwrap_err(), StyleOpError::EmptyName));
        assert!(matches!(rename_style(&d, "nope", "x").unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
        h.undo(&mut d).unwrap();
        assert_eq!(d.style_name(&key), "Effect style 1");
        // A style without metadata gets some when it is renamed.
        d.style_meta.remove(&key);
        let c = rename_style(&d, &key, "Plain");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_meta[&key].property, StyleProperty::Effect);
    }

    #[test]
    fn effect_and_grid_styles_survive_a_native_save_and_load() {
        let (mut d, a, _b, f, _g) = fixture();
        let mut h = History::default();
        let (fx, c) = create_style_from_node(&d, a, StyleProperty::Effect, Some("Shadow/Card")).unwrap();
        h.exec(&mut d, c).unwrap();
        let (gr, c) = create_style_from_node(&d, f, StyleProperty::Grid, Some("Layout/12 col")).unwrap();
        h.exec(&mut d, c).unwrap();
        let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert!(matches!(&loaded.styles[&fx], Style::Effect(v) if v == &vec![shadow(8.0)]));
        assert!(matches!(&loaded.styles[&gr], Style::Grid(v) if v == &vec![cols(12)]));
        assert_eq!(loaded.style_meta[&fx].property, StyleProperty::Effect);
        assert_eq!(loaded.style_meta[&gr].property, StyleProperty::Grid);
        assert_eq!(loaded.style_name(&fx), "Shadow/Card");
        assert_eq!(loaded.get(a).unwrap().style_refs.get("effect"), Some(&fx));
        assert_eq!(loaded.get(f).unwrap().style_refs.get("grid"), Some(&gr));
        // The loaded copy keeps working: editing the style reaches the linked node.
        let c = update_style(&loaded, &fx, Style::Effect(vec![shadow(2.0)])).unwrap();
        let mut loaded = loaded;
        h.exec(&mut loaded, c).unwrap();
        assert_eq!(loaded.get(a).unwrap().effects, vec![shadow(2.0)]);
    }
}
