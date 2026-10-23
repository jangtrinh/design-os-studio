//! Text styles as pure document functions: create one from text layers and detach it again.
//!
//! `designos_doc::styles_ops` does this for effect and grid styles only. Like those, each function
//! returns a [`Command`] and leaves the document alone, so the caller decides how the change joins
//! the undo stack.

use designos_doc::lint::token_match::same_text_style;
use designos_doc::*;

/// Key of the `style_refs` entry that links a text layer to its text style.
const TEXT_REF: &str = "text";

/// Whether every run of `t` has the base style's values, colour aside: the layer then has one text
/// style to share. A layer whose runs differ has none, and a style made from its base would not
/// match what is drawn.
pub fn has_single_text_style(t: &TextProps) -> bool {
    t.runs.iter().all(|r| same_text_style(&r.style, &t.style))
}

/// The text layers among `nodes`, each once and in the order given. Other nodes are skipped.
fn text_layers<'a>(doc: &'a Document, nodes: &[NodeId]) -> Result<Vec<(&'a Node, &'a TextProps)>, StyleOpError> {
    let mut out: Vec<(&Node, &TextProps)> = Vec::new();
    for id in nodes {
        let n = doc.get(*id).ok_or(DocError::NotFound(*id))?;
        if let (NodeKind::Text(t), false) = (&n.kind, out.iter().any(|(seen, _)| seen.id == *id)) {
            out.push((n, t));
        }
    }
    Ok(out)
}

/// The base style the text layers among `nodes` share: each has one style (see
/// [`has_single_text_style`]) and all are alike, colour aside. `None` when there is no text layer or
/// they differ, so no single style describes them.
pub fn shared_text_style(doc: &Document, nodes: &[NodeId]) -> Option<TextStyle> {
    let layers = text_layers(doc, nodes).ok()?;
    let (_, first) = *layers.first()?;
    layers.iter().all(|(_, t)| has_single_text_style(t) && same_text_style(&t.style, &first.style)).then(|| first.style.clone())
}

/// First `Text style N` that is neither the key nor the display name of a style of `doc`, so a style
/// renamed to a name in the series does not get a twin.
fn next_free_name(doc: &Document) -> String {
    (1..)
        .map(|n| format!("Text style {n}"))
        .find(|name| !doc.styles.contains_key(name) && !doc.styles.keys().any(|k| doc.style_name(k) == name))
        .unwrap_or_else(|| "Text style".to_string())
}

/// Creates the text style `Text style N` from the text layers among `nodes` (their shared base
/// style: family, size, weight, line height, letter spacing and the rest), links every one of them
/// to it and returns the new key with the command. The layers keep their values. Refused when no
/// node is a text layer, or when their type is not one style (see [`shared_text_style`]).
pub fn create_text_style_from_nodes(doc: &Document, nodes: &[NodeId]) -> Result<(String, Command), StyleOpError> {
    let layers = text_layers(doc, nodes)?;
    let Some((first, _)) = layers.first() else {
        return Err(StyleOpError::WrongNodeKind(nodes.first().copied().unwrap_or_default()));
    };
    let style = shared_text_style(doc, nodes).ok_or(StyleOpError::NothingToStyle(first.id))?;
    let name = next_free_name(doc);
    let mut styles = doc.styles.clone();
    let mut style_meta = doc.style_meta.clone();
    styles.insert(name.clone(), Style::Text(style));
    style_meta.insert(
        name.clone(),
        StyleMeta { name: name.clone(), key: None, description: String::new(), property: StyleProperty::Text, bindings: Default::default() },
    );
    let mut cmds = vec![Command::SetStyles { styles, style_meta }];
    for (n, _) in layers {
        let mut linked = n.clone();
        linked.style_refs.insert(TEXT_REF.into(), name.clone());
        cmds.push(Command::Update { node: Box::new(linked) });
    }
    Ok((name, Command::Batch(cmds)))
}

/// Drops the text style link of every node of `nodes` that has one. The nodes keep their values and
/// the style itself stays in the document.
pub fn detach_text_style(doc: &Document, nodes: &[NodeId]) -> Result<Command, StyleOpError> {
    let mut cmds = Vec::new();
    for id in nodes.iter().collect::<std::collections::BTreeSet<_>>() {
        let n = doc.get(*id).ok_or(DocError::NotFound(*id))?;
        if n.style_refs.contains_key(TEXT_REF) {
            let mut unlinked = n.clone();
            unlinked.style_refs.remove(TEXT_REF);
            cmds.push(Command::Update { node: Box::new(unlinked) });
        }
    }
    Ok(Command::Batch(cmds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page with a text layer `t` and a rectangle `r`.
    fn fixture() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let rid = d.alloc_id();
        let r = d.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let tid = d.alloc_id();
        let t = d.insert(page, Node::text(tid, Rect::new(0.0, 20.0, 100.0, 20.0), "Hello", TextStyle { font_size: 21.0, ..Default::default() })).unwrap();
        (d, t, r)
    }

    /// A second text layer with the type of `like`.
    fn add_text_like(d: &mut Document, like: NodeId, content: &str) -> NodeId {
        let style = match &d.get(like).unwrap().kind {
            NodeKind::Text(t) => t.style.clone(),
            other => panic!("{other:?}"),
        };
        let id = d.alloc_id();
        let page = d.pages[0];
        d.insert(page, Node::text(id, Rect::new(0.0, 60.0, 100.0, 20.0), content, style)).unwrap()
    }

    fn apply(d: &mut Document, cmd: Command) {
        History::default().exec(d, cmd).expect("the command applies");
    }

    #[test]
    fn creating_copies_the_layer_style_and_links_the_layer() {
        let (mut d, t, _) = fixture();
        let (key, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        assert_eq!(key, "Text style 1");
        apply(&mut d, cmd);
        let NodeKind::Text(tp) = &d.get(t).unwrap().kind else { panic!("text layer") };
        assert!(matches!(&d.styles[&key], Style::Text(ts) if ts == &tp.style));
        assert_eq!(d.style_meta[&key].property, StyleProperty::Text);
        assert_eq!(d.get(t).unwrap().style_refs.get("text"), Some(&key));
    }

    #[test]
    fn names_skip_keys_and_display_names_in_use() {
        let (mut d, t, _) = fixture();
        let (_, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        // Key "Text style 1" now shows as "Text style 2", and a library style is keyed "Text style 3".
        d.style_meta.get_mut("Text style 1").unwrap().name = "Text style 2".into();
        d.styles.insert("Text style 3".into(), Style::Text(TextStyle::default()));
        let (key, _) = create_text_style_from_nodes(&d, &[t]).unwrap();
        assert_eq!(key, "Text style 4");
    }

    #[test]
    fn only_a_text_layer_with_one_style_can_be_turned_into_a_style() {
        let (d, t, r) = fixture();
        assert!(matches!(create_text_style_from_nodes(&d, &[r]), Err(StyleOpError::WrongNodeKind(id)) if id == r));
        assert!(matches!(create_text_style_from_nodes(&d, &[NodeId(9999)]), Err(StyleOpError::Doc(_))));
        assert!(matches!(create_text_style_from_nodes(&d, &[]), Err(StyleOpError::WrongNodeKind(_))));
        let mut mixed = d.clone();
        if let NodeKind::Text(tp) = &mut mixed.get_mut(t).unwrap().kind {
            tp.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_weight: 700, ..tp.style.clone() } });
        }
        assert!(matches!(create_text_style_from_nodes(&mixed, &[t]), Err(StyleOpError::NothingToStyle(_))));
        // Colour is not part of a text style.
        if let NodeKind::Text(tp) = &mut mixed.get_mut(t).unwrap().kind {
            tp.runs[0].style = TextStyle { fills: vec![Paint::solid(Color::rgb(255, 0, 0))], ..tp.style.clone() };
        }
        assert!(create_text_style_from_nodes(&mixed, &[t]).is_ok());
    }

    #[test]
    fn several_layers_with_the_same_type_get_one_style_and_all_link_to_it() {
        let (mut d, t, r) = fixture();
        let u = add_text_like(&mut d, t, "World");
        // A rectangle in the selection is skipped, and a node given twice is linked once.
        let (key, cmd) = create_text_style_from_nodes(&d, &[t, r, u, t]).unwrap();
        apply(&mut d, cmd);
        assert_eq!(d.styles.len(), 1);
        for id in [t, u] {
            assert_eq!(d.get(id).unwrap().style_refs.get("text"), Some(&key));
        }
        assert!(d.get(r).unwrap().style_refs.is_empty());
    }

    #[test]
    fn several_layers_with_different_type_have_no_single_style() {
        let (mut d, t, _) = fixture();
        let u = add_text_like(&mut d, t, "World");
        if let NodeKind::Text(tp) = &mut d.get_mut(u).unwrap().kind {
            tp.style.font_size += 1.0;
        }
        assert!(shared_text_style(&d, &[t, u]).is_none());
        assert!(matches!(create_text_style_from_nodes(&d, &[t, u]), Err(StyleOpError::NothingToStyle(_))));
        assert!(shared_text_style(&d, &[t]).is_some());
    }

    #[test]
    fn detaching_drops_the_link_and_keeps_the_style_and_the_values() {
        let (mut d, t, r) = fixture();
        let (key, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        let before = d.get(t).unwrap().kind.clone();
        let cmd = detach_text_style(&d, &[t, r, t]).unwrap();
        apply(&mut d, cmd);
        assert!(d.get(t).unwrap().style_refs.is_empty());
        assert!(d.styles.contains_key(&key));
        assert_eq!(d.get(t).unwrap().kind, before);
    }
}
