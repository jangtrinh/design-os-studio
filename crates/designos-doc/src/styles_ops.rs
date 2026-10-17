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
    apply_style_values(n, style, prop, key);
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
    style_meta.insert(key.clone(), StyleMeta { name, key: None, description: String::new(), property: prop });
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

/// Replaces the values of style `key` and pushes them to every linked node.
/// `style` must be the same kind (effect / grid) as the stored one.
pub fn update_style(doc: &Document, key: &str, style: Style) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    let prop = style_property(doc, key);
    let ref_key = effect_or_grid_ref(prop)?;
    if std::mem::discriminant(stored) != std::mem::discriminant(&style) {
        return Err(StyleOpError::NotEffectOrGrid(prop));
    }
    let mut styles = doc.styles.clone();
    styles.insert(key.to_string(), style.clone());
    let mut cmds = vec![Command::SetStyles { styles, style_meta: doc.style_meta.clone() }];
    for n in doc.nodes.values().filter(|n| n.style_refs.get(ref_key).is_some_and(|k| k == key)) {
        let mut copy = n.clone();
        if apply_style_values(&mut copy, &style, prop, key) {
            cmds.push(Command::Update { node: Box::new(copy) });
        }
    }
    Ok(Command::Batch(cmds))
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
        LayoutGrid::Columns { count, gutter: 20.0, offset: 0.0, color: Color::rgba(255, 0, 0, 26) }
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
        assert_eq!(d.style_meta[&key], StyleMeta { name: "Effect style 1".into(), key: None, description: String::new(), property: StyleProperty::Effect });
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
